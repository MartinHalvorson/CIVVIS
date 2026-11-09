//! The model has to play the engine's game. These hold it to the engine on
//! real positions: the same strengths, the same reach, the same legal blows.

use super::model::{Line, Objective, Sim, State, Weights, OURS, THEIRS};
use super::{extract, search, solve};
use crate::ai::advanced::{AdvancedAi, GrandStrategy, StrategicPlan};
use crate::game::Game;
use crate::hex;
use crate::Pos;

fn at(col: i32, row: i32) -> Pos {
    hex::offset_to_axial(col, row)
}

fn open_field() -> Game {
    let mut g = crate::doctrine::build(
        crate::doctrine::position("the_reserve").expect("fixture"),
        3,
    )
    .expect("buildable");
    for pid in 0..2 {
        for uid in g.player_unit_ids(pid) {
            g.remove_unit(uid);
        }
    }
    g
}

fn no_plan(g: &Game) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    }
}

fn snapshot_of(g: &Game) -> extract::Snapshot {
    let ai = AdvancedAi::new();
    let theaters = extract::theaters(&ai, g, 0);
    assert_eq!(theaters.len(), 1, "one fight on this board");
    let plan = no_plan(g);
    extract::snapshot(&ai, g, 0, &theaters[0], &plan).expect("a snapshot")
}

fn sim_of<'a>(snap: &'a extract::Snapshot) -> Sim<'a> {
    Sim::new(
        &snap.board,
        &snap.state,
        Weights::default(),
        Objective::Field,
    )
}

#[test]
fn the_model_reads_the_engines_strengths_on_a_real_position() {
    let mut g = open_field();
    let ours = [
        g.spawn_unit("warrior", 0, at(9, 6)),
        g.spawn_unit("archer", 0, at(8, 7)),
        g.spawn_unit("spearman", 0, at(9, 8)),
    ];
    let theirs = [
        g.spawn_unit("horseman", 1, at(10, 6)),
        g.spawn_unit("warrior", 1, at(10, 7)),
    ];
    g.units.get_mut(&ours[0]).unwrap().hp = 62;
    g.units.get_mut(&theirs[1]).unwrap().hp = 41;
    g.units.get_mut(&theirs[1]).unwrap().fortify_turns = 2;
    let snap = snapshot_of(&g);
    let sim = sim_of(&snap);
    let index = |uid: u32| snap.unit_ids.iter().position(|id| *id == uid).unwrap();
    for a in ours {
        for d in theirs {
            let (i, j) = (index(a), index(d));
            let spec = &g.rules.units[g.units[&a].kind];
            if spec.has_ranged_attack() {
                let pos = g.units[&d].pos;
                let (ea, ed) = g.ranged_strike_strengths(a, d, pos).unwrap();
                let (ma, md) = sim.ranged_strengths(i, j);
                assert!((ea as f32 - ma).abs() < 0.01, "shot {a}->{d}: {ea} {ma}");
                assert!((ed as f32 - md).abs() < 0.01, "shot {a}->{d}: {ed} {md}");
            } else {
                let (ea, ed) = g.melee_exchange_strengths(a, d).unwrap();
                let (ma, md) = sim.melee_strengths(i, j);
                assert!((ea as f32 - ma).abs() < 0.01, "blow {a}->{d}: {ea} {ma}");
                assert!((ed as f32 - md).abs() < 0.01, "blow {a}->{d}: {ed} {md}");
            }
        }
    }
    // And their answer, which the rollouts lean on.
    for a in theirs {
        for d in ours {
            let (i, j) = (index(a), index(d));
            let (ea, ed) = g.melee_exchange_strengths(a, d).unwrap();
            let (ma, md) = sim.melee_strengths(i, j);
            assert!((ea as f32 - ma).abs() < 0.01, "blow {a}->{d}: {ea} {ma}");
            assert!((ed as f32 - md).abs() < 0.01, "blow {a}->{d}: {ed} {md}");
        }
    }
}

#[test]
fn a_flood_on_the_model_matches_the_engines_reach() {
    let mut g = open_field();
    let ours = g.spawn_unit("horseman", 0, at(9, 6));
    g.spawn_unit("warrior", 1, at(13, 6));
    let snap = snapshot_of(&g);
    let mut sim = sim_of(&snap);
    let i = snap.unit_ids.iter().position(|id| *id == ours).unwrap();
    sim.flood_public(i);
    let mut model: Vec<Pos> = sim
        .reach_public()
        .iter()
        .map(|(t, _)| snap.tiles[*t as usize])
        .filter(|p| *p != g.units[&ours].pos)
        .collect();
    let mut engine: Vec<Pos> = g
        .approach_arrivals(ours)
        .into_iter()
        .map(|(p, _)| p)
        .filter(|p| snap.tiles.contains(p))
        .collect();
    model.sort_unstable();
    engine.sort_unstable();
    assert_eq!(model, engine);
}

#[test]
fn every_planned_line_is_one_the_engine_accepts() {
    let mut g = open_field();
    g.spawn_unit("warrior", 0, at(9, 6));
    g.spawn_unit("archer", 0, at(8, 7));
    g.spawn_unit("spearman", 0, at(9, 8));
    g.spawn_unit("warrior", 1, at(11, 6));
    g.spawn_unit("archer", 1, at(12, 7));
    let snap = snapshot_of(&g);
    let solution = solve(
        &snap.board,
        &snap.state,
        snap.objective,
        search::Budget::DEFAULT,
        Weights::default(),
        9,
    );
    assert!(!solution.plan.is_empty());
    let mut ai = AdvancedAi::new();
    ai.enable_battle_planner_2();
    ai.enable_battle_solver();
    let applied = ai.apply_solution(&mut g, 0, &snap, &solution);
    assert_eq!(applied.dropped, 0, "{:?}", solution.plan);
    assert!(applied.lines > 0);
    let _: (Vec<Line>, u8, u8) = (Vec::new(), OURS, THEIRS);
    let _: Option<State> = None;
}

#[test]
fn the_gene_ships_off_and_is_registered() {
    let ai = AdvancedAi::new();
    assert!(!ai.battle_solver, "an opt-in ships off");
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.opt_in() && gene.field == "battle_solver"));
}
