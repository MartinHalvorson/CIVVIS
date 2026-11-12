//! Focused tests for the capture forecast (`advanced/assault_forecast.rs`)
//! and the two genes that read it.
//!
//! The board is open grassland: our city in the west at (2, 8); player 1, at
//! war with us, holds the city at [`TARGET`] with the walls and the defence
//! each test poses.

use super::*;
use crate::ai::advanced::siege_train::SiegeStage;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::{GrandStrategy, StrategicPlan};
use crate::ai::Ai;
use crate::game::Action;
use crate::reasoning::Journal;
use std::sync::Arc;

const TARGET: Pos = (18, 8);

/// Our city at (2, 8) and player 1's at [`TARGET`], at war, turn 80. The
/// target's walls are `walls` (also its maximum) and its defence `defence`.
fn front(walls: i32, defence: f64) -> (Game, u32) {
    let mut g = Game::new_full(2, 36, 22, 4_175, 300, 0, false);
    let ids: Vec<_> = g.units.keys().copied().collect();
    for id in ids {
        g.remove_unit(id);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    g.found_city_for(0, (2, 8), None);
    let target = g.found_city_for(1, TARGET, None);
    {
        let city = g.cities.get_mut(&target).unwrap();
        if walls > 0 {
            city.buildings.push(crate::name!("walls"));
        }
        city.wall_hp = walls;
    }
    Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(target, walls);
    Arc::make_mut(&mut g.observed_city_strength).insert(target, defence);
    for pid in 0..2 {
        g.players[pid].met.insert(1 - pid);
    }
    g.record_contact(0, 1);
    g.at_war.insert((0, 1));
    g.current = 0;
    g.turn = 80;
    (g, target)
}

/// Land tiles exactly `distance` from the target, west side first.
fn ring(g: &Game, distance: i32) -> Vec<Pos> {
    let mut tiles: Vec<Pos> = g
        .wring(TARGET, distance)
        .into_iter()
        .filter(|pos| g.map.get(*pos).is_some() && g.city_at(*pos).is_none())
        .collect();
    tiles.sort_by_key(|pos| (pos.0, pos.1));
    tiles
}

/// `kinds` spawned for player 0 on free tiles `distance` from the target.
fn army(g: &mut Game, kinds: &[&str], distance: i32) -> Vec<u32> {
    let free: Vec<Pos> = ring(g, distance)
        .into_iter()
        .filter(|pos| g.unit_ids_at(*pos).is_empty())
        .collect();
    kinds
        .iter()
        .zip(free)
        .map(|(kind, pos)| g.spawn_test_unit(kind, 0, pos))
        .collect()
}

fn forecast(g: &Game, cid: u32, force: &[u32]) -> AssaultForecast {
    AdvancedAi::new()
        .assault_forecast(g, 0, cid, force)
        .expect("a city at war reads")
}

fn plan_against(g: &Game, cid: u32) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(cid),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    }
}

/// Every unit of `pid` through the siege doctrine, as the ladder loops a
/// unit while it acts (`siege_train`'s own test driver).
fn play(ai: &mut AdvancedAi, g: &mut Game, pid: usize, plan: &StrategicPlan) {
    ai.rebuild_force_groups(g, pid, plan);
    let mut ids = g.player_unit_ids(pid);
    ids.sort_unstable();
    for uid in ids {
        for _ in 0..8 {
            if !g.units.contains_key(&uid) || g.units[&uid].moves_left <= 0.0 {
                break;
            }
            if ai.force_groups_dirty {
                ai.rebuild_force_groups(g, pid, plan);
                ai.force_groups_dirty = false;
            }
            match ai.siege_doctrine_step(g, pid, uid, plan) {
                Some(true) => {}
                _ => break,
            }
        }
    }
}

fn end_round(g: &mut Game) {
    assert!(g.apply(0, &Action::EndTurn).is_ok());
    assert!(g.apply(1, &Action::EndTurn).is_ok());
    assert_eq!(g.current, 0);
}

fn siege_seat() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_siege_needs_a_breaker();
    ai
}

#[test]
fn both_genes_ship_off() {
    opt_in_off_in_both_controllers("assault-forecast-is-read", |ai| ai.assault_forecast_is_read);
    opt_in_off_in_both_controllers("siege-goes-on-the-forecast", |ai| {
        ai.siege_goes_on_the_forecast
    });
    let mut ai = AdvancedAi::new();
    assert!(!ai.assault_forecast_read());
    ai.enable_siege_goes_on_the_forecast();
    assert!(
        ai.assault_forecast_read(),
        "the go gene implies the reading"
    );
}

#[test]
fn an_open_city_before_enough_melee_reads_high() {
    let (mut g, cid) = front(0, 20.0);
    let force = army(&mut g, &["swordsman"; 4], 2);
    let read = forecast(&g, cid, &force);
    assert_eq!((read.present, read.arriving), (4, 0));
    assert!(
        read.p() >= 0.8,
        "an open city before four Swordsmen: {read:?}"
    );
    assert!(read.within(4) >= 0.5, "and soon: {read:?}");
    for k in 1..HORIZON {
        assert!(read.within(k) <= read.within(k + 1), "a curve: {read:?}");
    }
}

#[test]
fn four_hundred_walls_and_no_breaker_read_near_zero() {
    let (mut g, cid) = front(400, 40.0);
    g.spawn_test_unit("spearman", 1, TARGET);
    let mut force = army(&mut g, &["swordsman"; 6], 2);
    force.extend(army(&mut g, &["archer", "archer"], 3));
    let read = forecast(&g, cid, &force);
    assert!(
        read.p() <= 0.05,
        "400 walls, nothing to open them: {read:?}"
    );
}

/// Live t100 (the -c2 brief): 771 military against 197 sat in `stage`
/// before a 100-wall city of defence 67 while our best unit was 35-36
/// strength. A defence over our best unit reads near zero however many
/// bodies stand there.
#[test]
fn a_defence_over_our_best_unit_reads_near_zero_whatever_the_bodies() {
    let (mut g, cid) = front(100, 67.0);
    g.spawn_test_unit("line_infantry", 1, TARGET);
    let mut force = army(&mut g, &["swordsman"; 6], 2);
    force.extend(army(&mut g, &["horseman"; 4], 3));
    force.extend(army(&mut g, &["catapult", "catapult"], 3));
    let read = forecast(&g, cid, &force);
    assert!(read.p() <= 0.05, "defence 67 against 35-36: {read:?}");
}

#[test]
fn the_forecast_grows_with_the_force() {
    let mut last = -1.0f32;
    let mut first = None;
    for size in 1..=5 {
        let (mut g, cid) = front(0, 30.0);
        let force = army(&mut g, &vec!["swordsman"; size], 2);
        let p = forecast(&g, cid, &force).p();
        assert!(
            p + 0.03 >= last,
            "{size} Swordsmen read {p}, fewer read {last}"
        );
        last = last.max(p);
        first.get_or_insert(p);
    }
    assert!(
        last - first.unwrap() >= 0.3,
        "five Swordsmen read well over one: {first:?} -> {last}"
    );
}

#[test]
fn the_forecast_falls_with_the_walls() {
    let mut reads = Vec::new();
    for walls in [0, 100, 200, 400] {
        let (mut g, cid) = front(walls, 28.0);
        let mut force = army(&mut g, &["swordsman"; 4], 2);
        force.extend(army(&mut g, &["catapult", "catapult"], 3));
        reads.push((walls, forecast(&g, cid, &force).p()));
    }
    for pair in reads.windows(2) {
        assert!(
            pair[1].1 <= pair[0].1 + 0.03,
            "more walls never read higher: {reads:?}"
        );
    }
    assert!(reads[0].1 >= reads[3].1 + 0.3, "{reads:?}");
}

/// A member off the board walks in on its route instead of being dropped.
#[test]
fn a_far_member_walks_in_and_is_not_dropped() {
    let (mut g, cid) = front(0, 20.0);
    let force = army(&mut g, &["swordsman"; 4], 12);
    let read = forecast(&g, cid, &force);
    assert_eq!((read.present, read.arriving), (0, 4), "{read:?}");
    assert_eq!(read.within(1), 0.0, "nobody is there yet: {read:?}");
    assert!(read.p() >= 0.3, "they arrive and take it: {read:?}");
    assert!(!read.gathered(), "four turns out is not gathered: {read:?}");
}

#[test]
fn walls_before_melee_alone_are_short_of_a_breaker() {
    let (mut g, cid) = front(200, 30.0);
    // A gun of ours at home: the kind is fielded, so it can be named.
    g.spawn_test_unit("catapult", 0, (3, 8));
    let force = army(&mut g, &["swordsman"; 6], 2);
    let read = forecast(&g, cid, &force);
    assert_eq!(read.short_of, Some(ShortOf::Breaker), "{read:?}");
}

#[test]
fn an_open_city_before_shooters_alone_is_short_of_a_taker() {
    let (mut g, cid) = front(0, 25.0);
    g.spawn_test_unit("swordsman", 0, (3, 8));
    let force = army(&mut g, &["archer"; 4], 2);
    let read = forecast(&g, cid, &force);
    assert_eq!(read.p(), 0.0, "shooters cannot take a city: {read:?}");
    assert_eq!(read.short_of, Some(ShortOf::Taker), "{read:?}");
}

#[test]
fn the_turn_pays_once() {
    let (mut g, cid) = front(0, 20.0);
    let force = army(&mut g, &["swordsman"; 3], 2);
    let ai = AdvancedAi::new();
    let first = ai.assault_forecast(&g, 0, cid, &force);
    let again = ai.assault_forecast(&g, 0, cid, &force);
    let curve = ai.assault_capture_curve(&g, 0, cid, &force);
    assert_eq!(first, again);
    assert_eq!(
        curve.map(|read| read.by_turn),
        first.as_ref().map(|read| read.by_turn)
    );
    assert_eq!(ai.assault_forecasts_computed(&g), 1);
    let fewer = &force[..2];
    ai.assault_capture_curve(&g, 0, cid, fewer);
    assert_eq!(ai.assault_forecasts_computed(&g), 2);
}

/// The reading changes nothing: four turns of a posed siege play out the
/// same, unit for unit and record for record, with the gene on, and the
/// journal has its lines.
#[test]
fn the_reading_changes_no_decision() {
    let run = |read: bool| {
        let (mut g, cid) = front(100, 25.0);
        g.spawn_test_unit("spearman", 1, TARGET);
        army(&mut g, &["swordsman"; 4], 3);
        army(&mut g, &["catapult", "catapult"], 4);
        let journal = Journal::recording();
        let mut ai = siege_seat();
        ai.attach_journal(journal.handle());
        if read {
            ai.enable_assault_forecast_is_read();
        }
        let plan = plan_against(&g, cid);
        let mut trace = Vec::new();
        for _ in 0..4 {
            play(&mut ai, &mut g, 0, &plan);
            let units: Vec<(u32, Pos, i32)> = g
                .units
                .values()
                .map(|unit| (unit.id, unit.pos, unit.hp))
                .collect();
            let city = &g.cities[&cid];
            trace.push((
                units,
                city.hp,
                city.wall_hp,
                format!("{:?}", ai.sieges.get(&cid)),
            ));
            end_round(&mut g);
        }
        let lines = journal
            .since(0)
            .thoughts
            .iter()
            .filter(|thought| thought.headline.starts_with("Assault forecast for"))
            .count();
        (trace, lines)
    };
    let (off, off_lines) = run(false);
    let (on, on_lines) = run(true);
    assert_eq!(off, on, "the reading changes no decision");
    assert_eq!(off_lines, 0);
    assert!(on_lines >= 3, "a line a turn: {on_lines}");
}

/// Walls 100 before six Swordsmen and no gun: `siege-needs-a-breaker`
/// holds the stock train in Stage, and the forecast reads the melee taking
/// it, so the gene goes.
#[test]
fn the_forecast_goes_where_the_breaker_hold_stages() {
    let stage_after = |gene: bool| {
        let (mut g, cid) = front(100, 20.0);
        army(&mut g, &["swordsman"; 6], 2);
        let mut ai = siege_seat();
        if gene {
            ai.enable_siege_goes_on_the_forecast();
        }
        let plan = plan_against(&g, cid);
        play(&mut ai, &mut g, 0, &plan);
        ai.sieges.get(&cid).map(|siege| siege.stage)
    };
    assert_eq!(stage_after(false), Some(SiegeStage::Stage));
    let on = stage_after(true);
    assert!(
        matches!(
            on,
            Some(SiegeStage::Invest | SiegeStage::Reduce | SiegeStage::Take)
        ),
        "the forecast goes: {on:?}"
    );
}

/// Three gathered hopeless readings stand the siege down: the train stops
/// mustering for it and the stand-down is kept by the city's tile.
#[test]
fn hopeless_readings_stand_the_siege_down() {
    let (mut g, cid) = front(400, 60.0);
    g.spawn_test_unit("musketman", 1, TARGET);
    army(&mut g, &["warrior"; 4], 3);
    let journal = Journal::recording();
    let mut ai = siege_seat();
    ai.attach_journal(journal.handle());
    ai.enable_siege_goes_on_the_forecast();
    let plan = plan_against(&g, cid);
    let mut down_at = None;
    for round in 0..5 {
        play(&mut ai, &mut g, 0, &plan);
        if ai.forecast_stood_down_at(&g, TARGET) {
            down_at = Some(round);
            break;
        }
        end_round(&mut g);
    }
    assert_eq!(
        down_at,
        Some(2),
        "the third hopeless reading stands it down"
    );
    assert!(journal
        .since(0)
        .thoughts
        .iter()
        .any(|thought| thought.headline.contains("the forecast reads it hopeless")));
    end_round(&mut g);
    ai.rebuild_force_groups(&g, 0, &plan);
    let groups = ai.force_groups.clone();
    assert!(
        groups
            .iter()
            .all(|group| ai.siege_city_of(&g, 0, &plan, group).is_none()),
        "no train musters for a stood-down city"
    );
    // Off, the same readings stand nothing down.
    let mut off = siege_seat();
    off.siege_forecast_hopeless.insert(
        TARGET,
        HopelessRun {
            last: g.turn,
            down_until: g.turn + 5,
            ..HopelessRun::default()
        },
    );
    assert!(!off.forecast_stood_down_at(&g, TARGET));
}

/// The Objective Board's Siege row is worth value x P / expected turns
/// with the gene on, its raw value with it off.
#[test]
fn the_siege_row_ranks_on_the_forecast() {
    let row_value = |gene: bool| {
        let (mut g, cid) = front(0, 20.0);
        army(&mut g, &["swordsman"; 4], 3);
        let mut ai = AdvancedAi::new();
        ai.enable_objective_board();
        ai.enable_siege_train();
        if gene {
            ai.enable_siege_goes_on_the_forecast();
        }
        let plan = plan_against(&g, cid);
        ai.assess_board_if_stale(&g, 0, &plan);
        let row = ai
            .objective_board()
            .rows
            .iter()
            .find(|row| row.key == crate::ai::advanced::objective_board::ObjectiveKey::Siege(cid))
            .map(|row| row.value);
        let pool = ai.board_pool(&g, 0);
        let rate = ai.forecast_siege_rate(&g, 0, cid, &pool);
        (row, rate)
    };
    let (off, _) = row_value(false);
    let (on, rate) = row_value(true);
    let (off, on, rate) = (off.unwrap(), on.unwrap(), rate.unwrap());
    assert!(rate > 0.0, "four Swordsmen take an open city");
    let want = off * rate * crate::ai::advanced::objective_board::NO_DEADLINE_HORIZON;
    assert!((on - want).abs() < 1e-6, "on {on}, off {off}, rate {rate}");
}
