use std::collections::BTreeSet;
use std::sync::Arc;

use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::{StrategicPlan, VictoryTarget};

/// India, Persia and Scythia of live 212021Z.
const INDIA: usize = 1;
const PERSIA: usize = 2;
const SCYTHIA: usize = 3;

/// Four majors at peace and mutually known, seat 0 ours. Seats 1-3 hold
/// Rocketry and two cities each, both seen by us; no pad is seen.
fn board() -> Game {
    let mut g = Game::new_full(4, 44, 28, 51_115, 250, 0, false);
    for pid in 0..4 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .expect("each fixture major begins with a Settler");
        let position = g.units[&settler].pos;
        g.found_city_for(pid, position, None);
        for uid in g.player_unit_ids(pid) {
            g.remove_unit(uid);
        }
    }
    for pid in [INDIA, PERSIA, SCYTHIA] {
        let centre = g.cities[&g.player_city_ids(pid)[0]].pos;
        let site = g
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| {
                let tile = &g.map.tiles[pos];
                tile.owner_city.is_none()
                    && !g.rules.is_water(tile)
                    && g.cities.values().all(|city| g.wdist(city.pos, *pos) >= 5)
            })
            .min_by_key(|pos| (g.wdist(centre, *pos), *pos))
            .expect("the fixture map has room for a second city");
        g.found_city_for(pid, site, None);
        g.players[pid].techs.insert(crate::name!("rocketry"));
    }
    g.current = 0;
    g.turn = 180;
    g.at_war.clear();
    for pid in [INDIA, PERSIA, SCYTHIA] {
        g.record_contact(0, pid);
    }
    for cid in g.cities.keys().copied().collect::<Vec<_>>() {
        let centre = g.cities[&cid].pos;
        g.players[0].explored.insert(centre);
    }
    g
}

/// A level-0 Spy of seat 0 standing in `city`, free to act, its sources
/// there already gained.
fn idle_spy(g: &mut Game, city: u32) -> u32 {
    let id = g.next_id;
    g.next_id += 1;
    g.spies.insert(
        id,
        crate::game::Spy {
            id,
            owner: 0,
            level: 0,
            promotions: BTreeSet::new(),
            city: Some(city),
            ready_turn: g.turn,
            mission: None,
            sources_city: Some(city),
            sources_until: g.turn + 20,
            captured_by: None,
        },
    );
    id
}

fn land(g: &mut Game, seat: usize, projects: usize) {
    g.players[seat].science_projects.clear();
    for project in [
        "launch_earth_satellite",
        "launch_moon_landing",
        "launch_mars_colony",
        "exoplanet_expedition",
    ]
    .iter()
    .take(projects)
    {
        g.players[seat].science_projects.insert(project.to_string());
    }
}

/// The host's Science a turn for `seat`.
fn set_science(g: &mut Game, seat: usize, science: f64) {
    let derived = AdvancedAi::seat_science_per_turn(g, seat)
        - g.observed_yield_adjustments
            .get(&seat)
            .map_or(0.0, |adjustment| adjustment.science);
    Arc::make_mut(&mut g.observed_yield_adjustments)
        .entry(seat)
        .or_default()
        .science = science - derived;
}

/// Lifts `seat`'s Production a turn far above every other racer's.
fn outbuilds(g: &mut Game, seat: usize) {
    Arc::make_mut(&mut g.observed_yield_adjustments)
        .entry(seat)
        .or_default()
        .production = 5_000.0;
}

/// Projects landed and Science a turn for India, Persia and Scythia.
fn race(g: &mut Game, rows: [(usize, usize, f64); 3]) {
    for (seat, projects, science) in rows {
        land(g, seat, projects);
        set_science(g, seat, science);
    }
}

fn plan() -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Culture,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: 0,
        rush: false,
    }
}

/// The live arming the gene joins: the Rocketry-trigger network with
/// `racer-keeps-the-spies`.
fn keeps() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_space_denial_spies_from_rocketry();
    ai.enable_racer_keeps_the_spies();
    ai
}

fn covers() -> AdvancedAi {
    let mut ai = keeps();
    ai.enable_denial_spies_cover_every_racer();
    ai
}

/// `count` idle Spies at our capital, one pass of the Spy turn, and the
/// owner of the city each Spy then stands in.
fn posts(mut ai: AdvancedAi, g: &mut Game, count: usize) -> Vec<Option<usize>> {
    let home = g.player_city_ids(0)[0];
    let spies: Vec<u32> = (0..count).map(|_| idle_spy(g, home)).collect();
    ai.advanced_spies(g, 0, &plan());
    spies
        .iter()
        .map(|spy| {
            g.spies[spy]
                .city
                .and_then(|city| g.cities.get(&city))
                .map(|city| city.owner)
        })
        .collect()
}

fn racers() -> BTreeSet<usize> {
    BTreeSet::from([INDIA, PERSIA, SCYTHIA])
}

#[test]
fn denial_spies_cover_every_racer_is_opt_in() {
    opt_in_off_in_both_controllers("denial-spies-cover-every-racer", |ai| {
        ai.denial_spies_cover_every_racer
    });
}

#[test]
fn off_the_co_leaders_are_empty_and_the_target_is_the_stock_leader() {
    let mut g = board();
    race(
        &mut g,
        [(INDIA, 3, 344.0), (PERSIA, 2, 532.0), (SCYTHIA, 2, 435.0)],
    );
    assert!(keeps().racer_cover_co_leaders(&g, &racers()).is_empty());
    let home = g.player_city_ids(0)[0];
    let spy = idle_spy(&mut g, home);
    assert_eq!(
        AdvancedAi::racer_cover_target(&g, spy, Some(PERSIA), &[], &[], &BTreeSet::new()),
        Some(PERSIA)
    );
}

#[test]
fn level_racers_with_one_far_ahead_on_science_get_a_spy_each() {
    // 212021Z from turn 190: all three on three projects, Persia making
    // about half as much Science again as India.
    let run = |ai: AdvancedAi| {
        let mut g = board();
        race(
            &mut g,
            [(INDIA, 3, 350.0), (PERSIA, 3, 600.0), (SCYTHIA, 3, 300.0)],
        );
        outbuilds(&mut g, PERSIA);
        posts(ai, &mut g, 3)
    };
    let off = run(keeps());
    assert_eq!(
        off.iter().filter(|owner| **owner == Some(PERSIA)).count(),
        2,
        "off: Persia's two cities take two Spies: {off:?}"
    );
    assert!(
        !off.contains(&Some(INDIA)) && !off.contains(&Some(SCYTHIA)),
        "off: no Spy covers India or Scythia: {off:?}"
    );
    let on: BTreeSet<Option<usize>> = run(covers()).into_iter().collect();
    assert_eq!(
        on,
        BTreeSet::from([Some(INDIA), Some(PERSIA), Some(SCYTHIA)]),
        "on: one Spy each"
    );
}

#[test]
fn the_live_frame_at_turn_180_spreads_from_persia() {
    // 212021Z at turn 180: India three projects at 344 Science, Persia two
    // at 532, Scythia two at 435. The Science edge lifts both behind India
    // level with it and the old pick names Persia; by progress India leads.
    let mut g = board();
    race(
        &mut g,
        [(INDIA, 3, 344.0), (PERSIA, 2, 532.0), (SCYTHIA, 2, 435.0)],
    );
    outbuilds(&mut g, PERSIA);
    assert_eq!(AdvancedAi::racer_spy_leader(&g, &racers()), Some(PERSIA));
    assert_eq!(
        covers().racer_cover_co_leaders(&g, &racers()),
        vec![INDIA, PERSIA, SCYTHIA]
    );
    let mut off = board();
    race(
        &mut off,
        [(INDIA, 3, 344.0), (PERSIA, 2, 532.0), (SCYTHIA, 2, 435.0)],
    );
    outbuilds(&mut off, PERSIA);
    assert_eq!(
        posts(keeps(), &mut off, 2),
        vec![Some(PERSIA), Some(PERSIA)]
    );
    assert_eq!(posts(covers(), &mut g, 2), vec![Some(INDIA), Some(PERSIA)]);
}

#[test]
fn a_racer_two_projects_ahead_gets_every_spy() {
    let run = |ai: AdvancedAi| {
        let mut g = board();
        race(
            &mut g,
            [(INDIA, 3, 300.0), (PERSIA, 1, 600.0), (SCYTHIA, 1, 500.0)],
        );
        let co = ai.racer_cover_co_leaders(&g, &racers());
        (co, posts(ai, &mut g, 2))
    };
    let (co, on) = run(covers());
    assert_eq!(co, vec![INDIA], "no racer within a project of India");
    assert_eq!(on, vec![Some(INDIA), Some(INDIA)], "no spread");
    let (_, off) = run(keeps());
    assert_eq!(
        off, on,
        "the stock pick agrees: two behind never counts level"
    );
}

#[test]
fn level_progress_is_told_apart_by_science() {
    // India and Persia on two projects; Persia makes more Science, India
    // out-builds it. The stock pick follows Production, the gene Science.
    let mut g = board();
    race(
        &mut g,
        [(INDIA, 2, 300.0), (PERSIA, 2, 450.0), (SCYTHIA, 0, 900.0)],
    );
    outbuilds(&mut g, INDIA);
    assert_eq!(
        AdvancedAi::racer_cover_ranked(&g, &racers()),
        vec![PERSIA, INDIA, SCYTHIA]
    );
    assert_eq!(AdvancedAi::racer_spy_leader(&g, &racers()), Some(INDIA));
    assert_eq!(
        covers().racer_cover_co_leaders(&g, &racers()),
        vec![PERSIA, INDIA],
        "Scythia, two projects behind, is no co-leader for all its Science"
    );
    let mut on = board();
    race(
        &mut on,
        [(INDIA, 2, 300.0), (PERSIA, 2, 450.0), (SCYTHIA, 0, 900.0)],
    );
    outbuilds(&mut on, INDIA);
    assert_eq!(posts(covers(), &mut on, 1), vec![Some(PERSIA)]);
}

#[test]
fn the_trip_counts_once_every_expedition_is_under_way() {
    // 212021Z at turn 210: India 19 of 25 points, Persia 13, Scythia 12, all
    // launched; a fourth racer on three projects would trail by more than
    // a step.
    let mut g = board();
    race(
        &mut g,
        [(INDIA, 4, 472.0), (PERSIA, 4, 454.0), (SCYTHIA, 4, 406.0)],
    );
    for (seat, points) in [(INDIA, 19.0), (PERSIA, 13.0), (SCYTHIA, 12.0)] {
        let stats = Arc::make_mut(&mut g.observed_public_empire_stats)
            .entry(seat)
            .or_default();
        stats.science_victory_points = Some(points);
        stats.science_victory_points_needed = Some(25.0);
    }
    assert!((AdvancedAi::racer_space_progress(&g, INDIA) - 4.76).abs() < 1e-9);
    assert_eq!(
        covers().racer_cover_co_leaders(&g, &racers()),
        vec![INDIA, PERSIA, SCYTHIA]
    );
    land(&mut g, SCYTHIA, 3);
    assert_eq!(
        covers().racer_cover_co_leaders(&g, &racers()),
        vec![INDIA, PERSIA],
        "three projects against India's 4.76 is more than a step behind"
    );
}

#[test]
fn a_spy_sharing_a_co_leader_moves_to_the_uncovered_one() {
    // Two Spies stand in Persia's two cities (posted while Persia led
    // alone); India is now level and no Spy of ours covers it.
    let run = |ai: AdvancedAi| {
        let mut g = board();
        race(
            &mut g,
            [(INDIA, 2, 300.0), (PERSIA, 2, 500.0), (SCYTHIA, 0, 200.0)],
        );
        outbuilds(&mut g, PERSIA);
        let persia = g.player_city_ids(PERSIA);
        let first = idle_spy(&mut g, persia[0]);
        let second = idle_spy(&mut g, persia[1]);
        let mut ai = ai;
        ai.advanced_spies(&mut g, 0, &plan());
        let owner = |spy: u32| {
            g.spies[&spy]
                .city
                .and_then(|city| g.cities.get(&city))
                .map(|city| city.owner)
        };
        (owner(first), owner(second))
    };
    assert_eq!(run(keeps()), (Some(PERSIA), Some(PERSIA)), "off: both stay");
    assert_eq!(
        run(covers()),
        (Some(INDIA), Some(PERSIA)),
        "on: the first moves to India, the second, now alone, stays"
    );
}

#[test]
fn the_reason_names_the_tag_the_racers_and_their_projects() {
    let mut g = board();
    race(
        &mut g,
        [(INDIA, 3, 344.0), (PERSIA, 2, 532.0), (SCYTHIA, 1, 435.0)],
    );
    let home = g.player_city_ids(0)[0];
    let spy = idle_spy(&mut g, home);
    let co = covers().racer_cover_co_leaders(&g, &racers());
    let reason = AdvancedAi::racer_cover_reason(&g, spy, INDIA, &racers(), &co);
    assert!(
        reason.starts_with("denial-spies-cover-every-racer: "),
        "{reason}"
    );
    let india = g.players[INDIA].civ.to_string();
    let persia = g.players[PERSIA].civ.to_string();
    let scythia = g.players[SCYTHIA].civ.to_string();
    assert!(reason.contains(&format!("{india} 3 projects")), "{reason}");
    assert!(reason.contains(&format!("{persia} 2 projects")), "{reason}");
    assert!(
        reason.contains(&format!("further back: {scythia} 1 project")),
        "{reason}"
    );
}
