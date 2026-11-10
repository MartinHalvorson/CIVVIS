//! The final war's sieges: a train that has arrived commits instead of
//! waiting on walls it cannot answer, and a rival's army at the same city is
//! not billed as that city's defence.

use std::sync::Arc;

use super::super::ForcePosture;
use super::tests::{at_distance, plan_against, walled_city};
use super::*;
use crate::game::PlayOnMode;

/// `walled_city` behind Medieval Walls at full strength, on open ground, at
/// war with seat 0.
fn medieval_city() -> (Game, u32) {
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
    let walled = g.cities.get_mut(&cid).unwrap();
    walled.buildings = vec![crate::name!("walls"), crate::name!("medieval_walls")];
    Arc::make_mut(&mut g.observed_city_max_wall_hp).remove(&cid);
    let max = g.city_max_wall_hp(&g.cities[&cid]);
    g.cities.get_mut(&cid).unwrap().wall_hp = max;
    (g, cid)
}

/// "One more turn" on a decided world: only the survivor can win now.
fn play_to_the_last_civ(g: &mut Game) {
    g.winner = Some(1);
    g.victory_type = Some("score".to_string());
    assert!(g.play_on(PlayOnMode::LastCivStanding));
    assert!(g.plays_to_the_last_civ());
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

fn train() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_siege_positive_damage_budget();
    ai.enable_siege_needs_a_breaker();
    ai
}

/// Three Men-at-Arms on the staging ring of walls no unit of theirs can
/// open: an ordinary war holds them in Stage for a breaker that is not
/// coming, the final war sends them in.
#[test]
fn a_staged_train_commits_in_the_final_war_and_waits_in_an_ordinary_one() {
    for last_civ in [false, true] {
        let (mut g, cid) = medieval_city();
        if last_civ {
            play_to_the_last_civ(&mut g);
        }
        let units: Vec<u32> = at_distance(&g, cid, 4)
            .into_iter()
            .take(3)
            .map(|pos| g.spawn_unit("man_at_arms", 0, pos))
            .collect();
        let group = group_on(&g, cid, &units);
        let plan = plan_against(&g, cid);
        let mut ai = train();
        ai.force_groups.push(group.clone());
        ai.assess_siege(&g, 0, cid, &plan, &group);
        let stage = ai.sieges[&cid].stage;
        if last_civ {
            assert_ne!(stage, SiegeStage::Stage, "the staged train goes in");
        } else {
            assert_eq!(stage, SiegeStage::Stage, "no breaker, no Invest");
        }
    }
}

/// The same train with only one of its three on the ring keeps staging: the
/// commitment waits for half the train's strength to arrive.
#[test]
fn a_train_still_on_the_road_keeps_staging_in_the_final_war() {
    let (mut g, cid) = medieval_city();
    play_to_the_last_civ(&mut g);
    let near = at_distance(&g, cid, 4)[0];
    let far = at_distance(&g, cid, 8);
    let mut units = vec![g.spawn_unit("man_at_arms", 0, near)];
    units.extend(
        far.into_iter()
            .take(2)
            .map(|pos| g.spawn_unit("man_at_arms", 0, pos)),
    );
    let group = group_on(&g, cid, &units);
    let plan = plan_against(&g, cid);
    let mut ai = train();
    ai.force_groups.push(group.clone());
    ai.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(ai.sieges[&cid].stage, SiegeStage::Stage);
}

/// A third civilization's soldier beside the city is our enemy too, so an
/// ordinary war bills it with the defence. In the final war every army at
/// that city is somebody's besieger: billing them would hold three trains
/// outside one city, each waiting on the other two.
#[test]
fn a_rival_besieger_is_not_billed_as_the_citys_defence() {
    let mut g = Game::new_full(3, 40, 24, 7_221, 300, 0, false);
    for pid in 0..3 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| g.units[unit].kind == "settler")
            .expect("each major starts with a settler");
        g.found_city_for(pid, g.units[&settler].pos, None);
    }
    for unit in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(unit);
    }
    g.current = 0;
    g.turn = 200;
    g.at_war.insert((0, 1));
    g.at_war.insert((0, 2));
    let cid = g.player_city_ids(1)[0];
    let city = g.cities[&cid].pos;
    let rival_at = g
        .wdisk(city, 3)
        .into_iter()
        .find(|pos| g.wdist(*pos, city) == 3 && g.city_at(*pos).is_none())
        .expect("a tile three from the city");
    let watch_at = g
        .nbrs(rival_at)
        .into_iter()
        .find(|pos| g.city_at(*pos).is_none() && g.wdist(*pos, city) >= 3)
        .expect("a tile beside it");
    for pos in [rival_at, watch_at] {
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    g.spawn_test_unit("scout", 0, watch_at);
    let rival = g.spawn_test_unit("modern_armor", 2, rival_at);
    assert!(g.unit_visible_to(rival, 0), "fixture: we see the rival");

    let view = CityView::of(&g, cid).unwrap();
    let ordinary = siege_bill(&g, 0, &view);
    play_to_the_last_civ(&mut g);
    let view = CityView::of(&g, cid).unwrap();
    let last_civ = siege_bill(&g, 0, &view);
    g.remove_unit(rival);
    let bare = siege_bill(&g, 0, &view);
    assert!(
        ordinary > bare,
        "an ordinary war bills every hostile soldier near the city: {ordinary:.1} against {bare:.1}"
    );
    assert!(
        (last_civ - bare).abs() < 1e-9,
        "the final war bills the city's own defence: {last_civ:.1} against {bare:.1}"
    );
}
