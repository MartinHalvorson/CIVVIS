use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

const HOME: Pos = (6, 8);
const OURS: Pos = (7, 9);
const RIVAL_APOSTLE: Pos = (8, 10);
const FAR_TARGET: Pos = (30, 8);

/// Our founder's lone Missionary at home, a rival Apostle beside it and a
/// rival city far off.
fn board(target: VictoryTarget) -> (Game, AdvancedAi, u32) {
    let mut g = Game::new_full(2, 44, 24, 936_315, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    g.found_city_for(0, HOME, None);
    g.found_city_for(1, FAR_TARGET, None);
    g.players[0].religion = Some("Buddhism".into());
    g.players[1].religion = Some("Orthodoxy".into());
    let ours = spawn(&mut g, "missionary", 0, OURS, "Buddhism");
    spawn(&mut g, "apostle", 1, RIVAL_APOSTLE, "Orthodoxy");
    g.current = 0;
    g.turn = 90;
    let mut ai = AdvancedAi::targeting(target);
    ai.enable_spreader_steps_out_of_the_apostle();
    (g, ai, ours)
}

fn spawn(g: &mut Game, kind: &str, owner: usize, pos: Pos, faith: &str) -> u32 {
    let uid = g.spawn_unit(kind, owner, pos);
    let unit = g.units.get_mut(&uid).unwrap();
    unit.religion = Some(faith.into());
    unit.charges = 3;
    uid
}

#[test]
fn the_gene_is_opt_in() {
    opt_in_off_in_both_controllers("religion-seat-marches-in-a-pack", |ai| {
        ai.religion_seat_marches_in_a_pack
    });
}

/// A lone spreader at home waits with a rival Apostle in sight, and only then.
#[test]
fn a_lone_spreader_waits_for_its_pack() {
    let (g, ai, ours) = board(VictoryTarget::Religion);
    assert!(
        !ai.religion_seat_holds_for_the_pack(&g, 0, ours, FAR_TARGET),
        "gene off"
    );

    let (g, mut ai, ours) = board(VictoryTarget::Religion);
    ai.enable_religion_seat_marches_in_a_pack();
    assert!(
        !ai.theological_threats(&g, 0, ours).is_empty(),
        "fixture: the rival Apostle is in sight"
    );
    assert!(ai.religion_seat_holds_for_the_pack(&g, 0, ours, FAR_TARGET));

    let (g, mut ai, ours) = board(VictoryTarget::Culture);
    ai.enable_religion_seat_marches_in_a_pack();
    assert!(
        !ai.religion_seat_holds_for_the_pack(&g, 0, ours, FAR_TARGET),
        "a Culture seat"
    );
}

/// A short walk, a home target, or a unit already on the road do not wait.
#[test]
fn only_a_long_walk_from_home_waits() {
    let (mut g, mut ai, ours) = board(VictoryTarget::Religion);
    ai.enable_religion_seat_marches_in_a_pack();
    assert!(
        !ai.religion_seat_holds_for_the_pack(&g, 0, ours, HOME),
        "our own city"
    );
    g.found_city_for(1, (10, 8), None);
    assert!(
        !ai.religion_seat_holds_for_the_pack(&g, 0, ours, (10, 8)),
        "a short walk"
    );
    g.units.get_mut(&ours).unwrap().pos = (18, 8);
    assert!(
        !ai.religion_seat_holds_for_the_pack(&g, 0, ours, FAR_TARGET),
        "already on the road"
    );
}

/// A full pack with an Apostle leaves; without the Apostle it keeps waiting.
#[test]
fn a_full_pack_needs_an_apostle() {
    let (mut g, mut ai, ours) = board(VictoryTarget::Religion);
    ai.enable_religion_seat_marches_in_a_pack();
    spawn(&mut g, "missionary", 0, (7, 8), "Buddhism");
    spawn(&mut g, "missionary", 0, (6, 9), "Buddhism");
    assert!(
        ai.religion_seat_holds_for_the_pack(&g, 0, ours, FAR_TARGET),
        "three Missionaries, no Apostle"
    );

    let (mut g, mut ai, ours) = board(VictoryTarget::Religion);
    ai.enable_religion_seat_marches_in_a_pack();
    spawn(&mut g, "missionary", 0, (7, 8), "Buddhism");
    spawn(&mut g, "apostle", 0, (6, 9), "Buddhism");
    assert!(!ai.religion_seat_holds_for_the_pack(&g, 0, ours, FAR_TARGET));
}

/// The wait has a limit, and a unit that has used it does not wait again.
#[test]
fn the_wait_runs_out() {
    let (mut g, mut ai, ours) = board(VictoryTarget::Religion);
    ai.enable_religion_seat_marches_in_a_pack();
    assert!(ai.religion_seat_holds_for_the_pack(&g, 0, ours, FAR_TARGET));
    g.turn += PACK_WAIT_TURNS - 1;
    assert!(ai.religion_seat_holds_for_the_pack(&g, 0, ours, FAR_TARGET));
    g.turn += 1;
    assert!(!ai.religion_seat_holds_for_the_pack(&g, 0, ours, FAR_TARGET));
    g.turn += 10;
    assert!(!ai.religion_seat_holds_for_the_pack(&g, 0, ours, FAR_TARGET));
}
