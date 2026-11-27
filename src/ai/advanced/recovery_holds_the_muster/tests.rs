//! `recovery-holds-the-muster`.
use super::*;
use crate::ai::advanced::objective_board::ObjectiveKey;
use crate::ai::advanced::siege_train::Siege;
use crate::name;
use crate::Pos;
use std::collections::BTreeMap;

fn at(col: i32, row: i32) -> Pos {
    crate::hex::offset_to_axial(col, row)
}

/// Two empires on open grassland, capitals at (6, 8) and (20, 8), at war,
/// every tile explored, turn 100, with an army of ours at home.
fn board() -> (Game, u32) {
    let capitals = [at(6, 8), at(20, 8)];
    let mut g = Game::new_full(2, 36, 22, 4394, 1_000, 0, false);
    for unit in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(unit);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    for (pid, pos) in capitals.iter().enumerate() {
        g.found_city_for(pid, *pos, None);
    }
    for pid in 0..2 {
        g.players[pid].met.insert(1 - pid);
        g.players[pid].explored.extend(g.map.tiles.keys().copied());
    }
    g.at_war.clear();
    g.at_war.insert((0, 1));
    g.turn = 100;
    g.current = 0;
    for (kind, pos) in [
        ("swordsman", at(8, 8)),
        ("swordsman", at(8, 9)),
        ("archer", at(7, 8)),
        ("archer", at(9, 8)),
        ("spearman", at(9, 9)),
        ("catapult", at(7, 9)),
    ] {
        let uid = g.spawn_test_unit(kind, 0, pos);
        let moves = g.unit_max_moves(uid);
        let unit = g.units.get_mut(&uid).unwrap();
        unit.moves_left = moves;
        unit.attacks_left = 1;
    }
    let target = g.city_at(at(20, 8)).expect("their capital");
    (g, target)
}

fn plan(g: &Game, target: u32, strategy: GrandStrategy) -> StrategicPlan {
    StrategicPlan {
        strategy,
        target_player: Some(g.cities[&target].owner),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    }
}

fn ai(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_objective_board();
    if gene {
        ai.enable_recovery_holds_the_muster();
    }
    ai
}

/// Whether the board raised a Siege row on `target` under `strategy`.
fn siege_row(g: &Game, ai: &mut AdvancedAi, target: u32, strategy: GrandStrategy) -> bool {
    ai.rebuild_force_groups(g, 0, &plan(g, target, strategy));
    ai.objective_board()
        .rows
        .iter()
        .any(|row| row.key == ObjectiveKey::Siege(target))
}

fn siege_at(stage: SiegeStage, turn: u32) -> Siege {
    Siege {
        stage,
        taker: None,
        entered: turn,
        assessed: turn,
        posts: BTreeMap::new(),
        short_since: None,
    }
}

#[test]
fn the_gene_ships_off_and_is_registered() {
    crate::ai::advanced::test_support::opt_in_off_in_both_controllers(
        "recovery-holds-the-muster",
        |ai| ai.recovery_holds_the_muster,
    );
}

/// Live `civvis-t8-20261126T122815Z`: "will muster for Siege Brisbane"
/// under "recovery | at war and losing ground at home". With the gene off
/// the Recovery plan still raises the Siege row; with it on the row stands
/// down, and a Conquest plan keeps it.
#[test]
fn recovery_stands_the_uninvested_siege_down() {
    let (g, target) = board();
    assert!(siege_row(
        &g,
        &mut ai(false),
        target,
        GrandStrategy::Recovery
    ));
    assert!(!siege_row(
        &g,
        &mut ai(true),
        target,
        GrandStrategy::Recovery
    ));
    assert!(siege_row(
        &g,
        &mut ai(true),
        target,
        GrandStrategy::Conquest
    ));

    let mut on = ai(true);
    assert!(!siege_row(&g, &mut on, target, GrandStrategy::Recovery));
    assert!(on.recovery_holds_the_siege(&g, target));
    assert!(!ai(false).recovery_holds_the_siege(&g, target));
}

/// The plan flips between Recovery and its lane while the war lasts: a
/// Recovery reading holds the muster for `HOLD_TURNS` turns, then lifts.
#[test]
fn the_hold_outlasts_a_flip_then_lifts() {
    let (mut g, target) = board();
    let mut on = ai(true);
    assert!(!siege_row(&g, &mut on, target, GrandStrategy::Recovery));
    let start = g.turn;
    let mut rows = Vec::new();
    for step in 1..=HOLD_TURNS {
        g.turn = start + step;
        rows.push(siege_row(&g, &mut on, target, GrandStrategy::Science));
    }
    let held = (HOLD_TURNS - 1) as usize;
    assert!(rows[..held].iter().all(|row| !row), "{rows:?}");
    assert!(rows[held], "{rows:?}");
}

/// An invested siege, or a falling city, keeps its row under Recovery.
#[test]
fn an_invested_siege_or_a_falling_city_keeps_the_row() {
    let (g, target) = board();
    let mut on = ai(true);
    on.sieges
        .insert(target, siege_at(SiegeStage::Invest, g.turn));
    assert!(!on.recovery_holds_the_siege(&g, target));
    on.sieges
        .insert(target, siege_at(SiegeStage::Stage, g.turn));
    assert!(!siege_row(&g, &mut on, target, GrandStrategy::Recovery));

    let (mut g, target) = board();
    {
        let city = g.cities.get_mut(&target).unwrap();
        city.wall_hp = 0;
        city.hp = 80;
    }
    let mut on = ai(true);
    assert!(siege_row(&g, &mut on, target, GrandStrategy::Recovery));
    assert!(!on.recovery_holds_the_siege(&g, target));
}
