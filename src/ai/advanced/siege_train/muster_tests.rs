//! `stage-musters-out-of-reach`: while a train cannot yet close, its melee and
//! shooters muster out of the defenders' reach instead of waiting on the
//! staging ring. Over the 10-04/05 control runs 59% of the land soldiers we
//! lost died while their siege read Stage, 772 of them on the ring itself.

use super::tests::{plan_against, walled_city};
use super::*;

/// `walled_city` on open grassland, at war, with a hostile Swordsman two
/// tiles out on the west road and our Warrior two tiles beyond it, on the
/// staging ring and inside the Swordsman's reach.
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
    let raider_at = (city.0 - 2, city.1);
    let ring = (city.0 - 4, city.1);
    assert_eq!(g.wdist(ring, city), 4, "fixture: the ring tile");
    g.spawn_unit("swordsman", 1, raider_at);
    let warrior = g.spawn_unit("warrior", 0, ring);
    (g, cid, warrior, ring)
}

fn train(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_shared_danger();
    if gene {
        ai.enable_stage_musters_out_of_reach();
    }
    ai
}

fn danger(g: &Game, uid: u32, at: Pos) -> f64 {
    let mut field = super::super::battle_planner::DangerField::with_reach(g, 0, true);
    field.share(g);
    field.rotation_danger(at, uid)
}

#[test]
fn a_ring_member_under_reach_falls_back_only_under_the_gene() {
    for gene in [false, true] {
        let (mut g, cid, warrior, ring) = ring_under_a_raider();
        let limit = f64::from(g.units[&warrior].hp) * MUSTER_DANGER_SHARE;
        assert!(
            danger(&g, warrior, ring) > limit,
            "fixture: the ring reads {:.1} against {limit:.1}",
            danger(&g, warrior, ring)
        );
        let city = CityView::of(&g, cid).unwrap();
        let plan = plan_against(&g, cid);
        let mut ai = train(gene);
        ai.stage_muster_ready.insert(cid, false);
        ai.siege_stage_step(&mut g, 0, warrior, &city, &plan);
        let at = g.units[&warrior].pos;
        if gene {
            assert_ne!(at, ring, "the member leaves the reach");
            assert!(danger(&g, warrior, at) <= limit, "it stands under the line at {at:?}");
            assert!(g.wdist(at, city.pos) > CITY_STRIKE_RANGE);
        } else {
            assert_eq!(at, ring, "today the member holds the ring");
        }
    }
}

#[test]
fn a_ready_train_keeps_the_ordinary_stage_step() {
    let (mut g, cid, warrior, ring) = ring_under_a_raider();
    let city = CityView::of(&g, cid).unwrap();
    let plan = plan_against(&g, cid);
    let mut ai = train(true);
    ai.stage_muster_ready.insert(cid, true);
    ai.siege_stage_step(&mut g, 0, warrior, &city, &plan);
    assert_eq!(g.units[&warrior].pos, ring, "a train that can close holds its ring");
}
