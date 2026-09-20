use super::super::siege_train::Siege;
use super::tests::{at, conquest, flat_board, on, war};
use super::*;

#[test]
fn unopposed_siege_stage_rallies_near_the_city() {
    let objective = at(24, 10);
    let approach = at(12, 10);
    let mut g = flat_board(91_628, &[at(6, 10), objective], false);
    war(&mut g, 0, 1);
    let target = g.city_at(objective).unwrap();
    g.cities.get_mut(&target).unwrap().pop = 20;
    for _ in 0..12 {
        g.spawn_test_unit("warrior", 0, approach);
    }
    let mut ai = on();
    ai.battlefront_observation = false;
    ai.siege_train = true;
    ai.sieges.insert(
        target,
        Siege {
            stage: SiegeStage::Stage,
            taker: None,
            entered: g.turn,
            assessed: g.turn,
            posts: BTreeMap::new(),
        },
    );
    ai.rebuild_force_groups(&g, 0, &conquest(&g, Some(target)));
    let group = ai
        .force_groups
        .iter()
        .find(|group| group.objective == objective)
        .expect("projected siege group");
    assert_eq!(group.posture, ForcePosture::Muster);
    assert!(
        (2..=3).contains(&g.wdist(group.anchor, objective)),
        "staging at {:?} cannot keep the army at {:?} away from {:?}",
        group.anchor,
        approach,
        objective
    );
    assert!(g.wdist(approach, group.anchor) < g.wdist(approach, objective));
}

#[test]
fn unopposed_defensive_rally_keeps_its_existing_anchor() {
    let objective = at(24, 10);
    let approach = at(12, 10);
    let g = flat_board(91_629, &[objective, at(6, 10)], false);
    let ai = on();
    let visible = ai.battlefront_visibility(&g, 0);
    assert_eq!(
        ai.far_side(&g, 0, objective, approach, &visible, false),
        approach
    );
}

#[test]
fn unopposed_siege_without_passable_land_retains_the_force_center() {
    let objective = at(24, 10);
    let approach = at(12, 10);
    let mut g = flat_board(91_630, &[at(6, 10), objective], false);
    for pos in g.wdisk(objective, 3) {
        if g.wdist(pos, objective) >= 2 {
            g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("coast");
        }
    }
    let ai = on();
    let visible = ai.battlefront_visibility(&g, 0);
    assert_eq!(
        ai.far_side(&g, 0, objective, approach, &visible, true),
        approach
    );
}
