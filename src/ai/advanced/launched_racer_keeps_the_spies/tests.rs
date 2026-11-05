use std::collections::BTreeSet;
use std::sync::Arc;

use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

/// Three majors with Science read from the host, seat 0 ours.
fn board(flyer_science: f64, chaser_science: f64) -> Game {
    let mut g = Game::new_full(3, 30, 20, 79_208, 250, 0, false);
    for pid in 0..3 {
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
    for (seat, science) in [(1, flyer_science), (2, chaser_science)] {
        let derived = AdvancedAi::seat_science_per_turn(&g, seat)
            - g.observed_yield_adjustments
                .get(&seat)
                .map_or(0.0, |adjustment| adjustment.science);
        Arc::make_mut(&mut g.observed_yield_adjustments)
            .entry(seat)
            .or_default()
            .science = science - derived;
    }
    g
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

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_launched_racer_keeps_the_spies();
    ai
}

#[test]
fn the_flyer_leads_a_chaser_with_the_science_edge() {
    // 154934Z at turn 190: America (seat 1) launched at 318 Science,
    // Byzantium (seat 2) on three projects at 500.
    let mut g = board(318.0, 500.0);
    land(&mut g, 1, 4);
    land(&mut g, 2, 3);
    let racers = BTreeSet::from([1, 2]);
    assert_eq!(
        AdvancedAi::racer_spy_leader(&g, &racers),
        Some(2),
        "the bug: the edge reads the chaser level and the finish is skipped"
    );
    assert_eq!(armed().racer_spy_leader_of(&g, &racers), Some(1));
    assert_eq!(
        AdvancedAi::new().racer_spy_leader_of(&g, &racers),
        Some(2),
        "off, nothing changes"
    );
}

#[test]
fn before_any_launch_the_old_order_stands() {
    let mut g = board(318.0, 500.0);
    land(&mut g, 1, 3);
    land(&mut g, 2, 2);
    let racers = BTreeSet::from([1, 2]);
    assert_eq!(
        armed().racer_spy_leader_of(&g, &racers),
        AdvancedAi::racer_spy_leader(&g, &racers)
    );
}

#[test]
fn two_flyers_keep_the_old_order_between_them() {
    let mut g = board(318.0, 500.0);
    land(&mut g, 1, 4);
    land(&mut g, 2, 4);
    let racers = BTreeSet::from([1, 2]);
    assert_eq!(
        armed().racer_spy_leader_of(&g, &racers),
        AdvancedAi::racer_spy_leader(&g, &racers)
    );
}

#[test]
fn no_racer_names_nobody() {
    let g = board(318.0, 500.0);
    assert_eq!(armed().racer_spy_leader_of(&g, &BTreeSet::new()), None);
}

#[test]
fn the_gene_is_opt_in() {
    opt_in_off_in_both_controllers("launched-racer-keeps-the-spies", |ai| {
        ai.launched_racer_keeps_the_spies
    });
}
