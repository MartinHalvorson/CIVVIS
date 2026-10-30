use std::collections::BTreeSet;

use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::VictoryTarget;

/// Seat 0 and three met rivals: seat 1 holds Rocketry only, seat 2 has landed
/// the Moon, seat 3 has launched its Exoplanet Expedition.
fn board() -> Game {
    let mut g = Game::new_full(4, 40, 26, 30_301, 250, 0, false);
    g.current = 0;
    g.turn = 190;
    g.at_war.clear();
    for rival in 1..4 {
        g.record_contact(0, rival);
    }
    g.players[1].techs.insert(crate::name!("rocketry"));
    for rival in 2..4 {
        g.players[rival].techs.insert(crate::name!("rocketry"));
        for project in ["launch_earth_satellite", "launch_moon_landing"] {
            g.players[rival].science_projects.insert(project.to_string());
        }
    }
    for project in ["launch_mars_colony", "exoplanet_expedition"] {
        g.players[3].science_projects.insert(project.to_string());
    }
    g
}

fn network() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_science_denial_every_pad();
    ai.enable_racer_keeps_the_spies();
    ai.enable_space_denial_spies_from_rocketry();
    ai
}

#[test]
fn launched_racer_draws_every_spy_is_opt_in() {
    opt_in_off_in_both_controllers("launched-racer-draws-every-spy", |ai| {
        ai.launched_racer_draws_every_spy
    });
}

/// Stock, the network answers every Rocketry holder; the gene keeps only the
/// launched one, for every consumer of the set.
#[test]
fn a_launched_racer_takes_the_whole_network() {
    let g = board();
    let stock = network();
    assert_eq!(stock.denial_spy_racers(&g, 0), BTreeSet::from([1, 2, 3]));
    assert_eq!(stock.racer_spy_racers(&g, 0), BTreeSet::from([1, 2, 3]));

    let mut on = network();
    on.enable_launched_racer_draws_every_spy();
    assert_eq!(on.denial_spy_racers(&g, 0), BTreeSet::from([3]));
    assert_eq!(on.every_pad_threats(&g, 0), BTreeSet::from([3]));
    assert_eq!(on.racer_spy_racers(&g, 0), BTreeSet::from([3]));
    assert_eq!(on.decisive_space_racers(&g, 0), BTreeSet::from([2, 3]), "wars read the decisive set unchanged");
}

/// Before anyone launches the gene changes nothing.
#[test]
fn no_launch_leaves_the_network_alone() {
    let mut g = board();
    g.players[3].science_projects.remove("exoplanet_expedition");
    let mut on = network();
    on.enable_launched_racer_draws_every_spy();
    assert_eq!(on.denial_spy_racers(&g, 0), network().denial_spy_racers(&g, 0));
    assert_eq!(on.denial_spy_racers(&g, 0), BTreeSet::from([1, 2, 3]));
}

/// Without `space-denial-spies-from-rocketry` the set is the decisive racers,
/// and a launch narrows that too.
#[test]
fn the_decisive_only_network_narrows_as_well() {
    let g = board();
    let mut on = AdvancedAi::targeting(VictoryTarget::Culture);
    on.enable_science_denial_every_pad();
    on.enable_launched_racer_draws_every_spy();
    assert_eq!(on.decisive_space_racers(&g, 0), BTreeSet::from([2, 3]));
    assert_eq!(on.denial_spy_racers(&g, 0), BTreeSet::from([3]));
}

/// A launched rival we have not met is no racer of ours, so it narrows
/// nothing.
#[test]
fn an_unmet_launcher_narrows_nothing() {
    let mut g = Game::new_full(4, 40, 26, 30_301, 250, 0, false);
    g.current = 0;
    g.record_contact(0, 1);
    g.players[1].techs.insert(crate::name!("rocketry"));
    for project in ["launch_earth_satellite", "launch_moon_landing", "launch_mars_colony", "exoplanet_expedition"] {
        g.players[3].science_projects.insert(project.to_string());
    }
    let mut on = network();
    on.enable_launched_racer_draws_every_spy();
    assert_eq!(on.denial_spy_racers(&g, 0), BTreeSet::from([1]));
}
