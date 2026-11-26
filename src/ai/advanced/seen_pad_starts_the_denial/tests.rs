use std::collections::BTreeSet;
use std::sync::Arc;

use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::{GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::HostUnitFacts;
use crate::Pos;

/// Three majors at peace and mutually known, seat 0 ours. Seat 1, the space
/// racer, holds Rocketry and a second city; seat 2 is another rival. Every
/// city centre is explored.
fn board() -> (Game, u32, u32, u32) {
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
    let capital = g.player_city_ids(1)[0];
    let centre = g.cities[&capital].pos;
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
    let second = g.found_city_for(1, site, None);
    g.current = 0;
    g.turn = 140;
    g.at_war.clear();
    g.record_contact(0, 1);
    g.record_contact(0, 2);
    for cid in g.cities.keys().copied().collect::<Vec<_>>() {
        let centre = g.cities[&cid].pos;
        g.players[0].explored.insert(centre);
    }
    g.players[1].techs.insert(crate::name!("rocketry"));
    let rival = g.player_city_ids(2)[0];
    // The racer out-researches us, so we do not lead its race.
    set_science(&mut g, 0, 120.0);
    set_science(&mut g, 1, 340.0);
    set_science(&mut g, 2, 150.0);
    (g, capital, second, rival)
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

/// A completed Spaceport on a free owned land tile of `cid`, seen by seat 0.
fn give_pad(g: &mut Game, cid: u32) -> Pos {
    let centre = g.cities[&cid].pos;
    let pad = g
        .wdisk(centre, 1)
        .into_iter()
        .find(|pos| {
            *pos != centre
                && g.map
                    .get(*pos)
                    .is_some_and(|tile| tile.district.is_none() && !g.rules.is_water(tile))
        })
        .expect("the fixture city has a free land neighbour");
    let tile = g.map.tiles.get_mut(&pad).unwrap();
    tile.owner_city = Some(cid);
    tile.district = Some(crate::name!("spaceport"));
    tile.pillaged = false;
    tile.improvement = None;
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("spaceport"), pad);
    g.players[0].explored.insert(pad);
    pad
}

/// A level-0 Spy of seat 0 standing in `city`, free to act (a levelled Spy
/// owes a promotion first). With `sourced` its sources there are already
/// gained; without, the board offers it the Gain Sources training.
fn spy(g: &mut Game, city: u32, sourced: bool) -> u32 {
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
            sources_city: sourced.then_some(city),
            sources_until: if sourced { g.turn + 20 } else { 0 },
            captured_by: None,
        },
    );
    id
}

fn land(g: &mut Game, seat: usize, projects: usize) {
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

/// A Religion seat: the gene reads the same in every lane.
fn religion_plan() -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Religion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: 0,
        rush: false,
    }
}

fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Religion);
    if gene {
        ai.enable_seen_pad_starts_the_denial();
    }
    ai
}

fn mission(g: &Game, spy: u32) -> Option<String> {
    g.spies[&spy]
        .mission
        .as_ref()
        .map(|mission| mission.kind.clone())
}

fn racers(ai: &AdvancedAi, g: &Game) -> Vec<usize> {
    ai.seen_pad_racers(g, 0)
        .into_iter()
        .map(|entry| entry.racer)
        .collect()
}

#[test]
fn seen_pad_starts_the_denial_is_opt_in() {
    opt_in_off_in_both_controllers("seen-pad-starts-the-denial", |ai| {
        ai.seen_pad_starts_the_denial
    });
}

#[test]
fn a_spy_goes_to_the_racers_seen_pad_before_its_first_launch() {
    // Germany 095636Z: a Spy ran Listening Posts in Ulm, no pad, while
    // Dortmund's pad stood; here the Spy stands in another rival's city.
    let run = |gene: bool| {
        let (mut g, _, second, rival) = board();
        give_pad(&mut g, second);
        let agent = spy(&mut g, rival, true);
        let mut ai = seat(gene);
        ai.advanced_spies(&mut g, 0, &religion_plan());
        (g.spies[&agent].city, second, g.players[0].counters.clone())
    };
    let (city, second, _) = run(false);
    assert_ne!(city, Some(second), "gene off: no post to the pad");
    let (city, second, counters) = run(true);
    assert_eq!(city, Some(second), "the Spy goes to the seen pad");
    assert_eq!(counters.get("seen_pad_posts"), Some(&1));
}

#[test]
fn a_green_spy_at_the_seen_pad_disrupts_before_it_trains() {
    let run = |gene: bool| {
        let (mut g, _, second, _) = board();
        give_pad(&mut g, second);
        let agent = spy(&mut g, second, false);
        let mut ai = seat(gene);
        ai.advanced_spies(&mut g, 0, &religion_plan());
        mission(&g, agent)
    };
    assert_ne!(
        run(false).as_deref(),
        Some("disrupt_rocketry"),
        "gene off: a green Spy trains before the first launch"
    );
    assert_eq!(run(true).as_deref(), Some("disrupt_rocketry"));
}

#[test]
fn nothing_when_the_science_victory_is_off() {
    let (mut g, _, second, rival) = board();
    give_pad(&mut g, second);
    g.victory_conditions.science = false;
    let agent = spy(&mut g, rival, true);
    let mut ai = seat(true);
    assert!(ai.seen_pad_racers(&g, 0).is_empty());
    ai.advanced_spies(&mut g, 0, &religion_plan());
    assert_ne!(g.spies[&agent].city, Some(second));
}

#[test]
fn nothing_against_a_racer_we_lead() {
    let ai = seat(true);
    let (mut g, _, second, _) = board();
    give_pad(&mut g, second);
    assert_eq!(racers(&ai, &g), vec![1], "they out-research us: answered");
    // We have landed a project and they have not.
    let (mut ahead, _, second, _) = board();
    give_pad(&mut ahead, second);
    land(&mut ahead, 0, 1);
    assert!(racers(&ai, &ahead).is_empty(), "a project ahead: we lead");
    // Level on projects, and our Science a turn is at least theirs.
    let (mut level, _, second, _) = board();
    give_pad(&mut level, second);
    set_science(&mut level, 0, 340.0);
    assert!(
        racers(&ai, &level).is_empty(),
        "level, as much Science: we lead"
    );
    // A project behind them, whatever our Science.
    let (mut behind, _, second, _) = board();
    give_pad(&mut behind, second);
    land(&mut behind, 1, 1);
    set_science(&mut behind, 0, 900.0);
    assert_eq!(racers(&ai, &behind), vec![1]);
}

#[test]
fn a_racer_with_no_seen_pad_takes_no_spy() {
    let ai = seat(true);
    let (mut g, _, second, _) = board();
    land(&mut g, 1, 2);
    assert!(racers(&ai, &g).is_empty(), "no pad seen");
    let pad = give_pad(&mut g, second);
    assert_eq!(racers(&ai, &g), vec![1]);
    g.map.tiles.get_mut(&pad).unwrap().pillaged = true;
    assert!(racers(&ai, &g).is_empty(), "a pillaged pad is not standing");
    g.map.tiles.get_mut(&pad).unwrap().pillaged = false;
    g.players[0].explored.remove(&pad);
    assert!(racers(&ai, &g).is_empty(), "a pad we have not seen");
}

#[test]
fn a_launched_racer_yields_its_pads_to_the_next() {
    // Kongo 065313Z: five of our Disrupt Rocketry runs landed on Kongo's pads
    // after its Exoplanet Expedition had launched.
    let ai = seat(true);
    let (mut g, _, second, rival) = board();
    give_pad(&mut g, second);
    give_pad(&mut g, rival);
    g.players[2].techs.insert(crate::name!("rocketry"));
    land(&mut g, 1, 4);
    land(&mut g, 2, 3);
    assert_eq!(
        racers(&ai, &g),
        vec![2],
        "the launched racer is passed over"
    );
}

#[test]
fn the_racer_ahead_comes_first_and_one_two_behind_not_at_all() {
    let ai = seat(true);
    let (mut g, _, second, rival) = board();
    give_pad(&mut g, second);
    give_pad(&mut g, rival);
    g.players[2].techs.insert(crate::name!("rocketry"));
    land(&mut g, 2, 1);
    // 340 Science a turn is short of 1.25 times 300: no edge.
    set_science(&mut g, 2, 300.0);
    assert_eq!(racers(&ai, &g), vec![2, 1], "projects first");
    land(&mut g, 2, 2);
    assert_eq!(
        racers(&ai, &g),
        vec![2],
        "two projects behind: not answered"
    );
    // An idle Spy at home goes to the racer ahead.
    let home = g.player_city_ids(0)[0];
    let agent = spy(&mut g, home, true);
    let racers = ai.seen_pad_racers(&g, 0);
    let legal = g.legal_spy_actions(0, agent);
    assert_eq!(
        AdvancedAi::seen_pad_post(&g, 0, agent, Some(home), &racers, &BTreeSet::new(), &legal),
        Some((rival, 2))
    );
}

#[test]
fn a_listening_post_gives_way_to_a_free_seen_pad() {
    let run = |gene: bool| {
        let (mut g, _, second, rival) = board();
        give_pad(&mut g, second);
        let agent = spy(&mut g, rival, true);
        Arc::make_mut(&mut g.host_unit_facts).insert(
            agent,
            HostUnitFacts {
                spy_operation: Some("listening_post".to_string()),
                ..Default::default()
            },
        );
        let mut ai = seat(gene);
        ai.enable_spy_mission_patience();
        ai.spy_orders_until.insert(agent, g.turn + 5);
        ai.advanced_spies(&mut g, 0, &religion_plan());
        (g.spies[&agent].city, rival, second)
    };
    let (city, rival, _) = run(false);
    assert_eq!(city, Some(rival), "gene off: the Listening Post holds it");
    let (city, _, second) = run(true);
    assert_eq!(city, Some(second), "the seen pad takes it");
}

#[test]
fn a_held_pad_and_a_culture_racers_city_keep_their_spies() {
    let ai = seat(true);
    let (mut g, _, second, rival) = board();
    give_pad(&mut g, second);
    let first = spy(&mut g, second, true);
    let other = spy(&mut g, rival, true);
    let racers = ai.seen_pad_racers(&g, 0);
    let none = BTreeSet::new();
    let legal = g.legal_spy_actions(0, first);
    assert_eq!(
        AdvancedAi::seen_pad_post(&g, 0, first, Some(second), &racers, &none, &legal),
        None,
        "a Spy at the pad stays to disrupt it"
    );
    let legal = g.legal_spy_actions(0, other);
    assert_eq!(
        AdvancedAi::seen_pad_post(&g, 0, other, Some(rival), &racers, &none, &legal),
        None,
        "the pad is held: no second Spy"
    );
    g.spies.remove(&first);
    assert_eq!(
        AdvancedAi::seen_pad_post(&g, 0, other, Some(rival), &racers, &none, &legal),
        Some((second, 1))
    );
    assert_eq!(
        AdvancedAi::seen_pad_post(
            &g,
            0,
            other,
            Some(rival),
            &racers,
            &BTreeSet::from([2]),
            &legal
        ),
        None,
        "a culture racer's city keeps its Spy for the heist"
    );
}

#[test]
fn a_racer_a_project_behind_with_the_science_edge_comes_first() {
    // Russia 122122Z at t150: its Satellite landed, a project behind Vietnam,
    // at 377 Science a turn against Vietnam's 276. Russia won.
    let ai = seat(true);
    let (mut g, _, second, rival) = board();
    give_pad(&mut g, second);
    give_pad(&mut g, rival);
    g.players[2].techs.insert(crate::name!("rocketry"));
    land(&mut g, 2, 1);
    set_science(&mut g, 2, 270.0);
    assert_eq!(racers(&ai, &g), vec![1, 2], "340 is 1.25 times 270");
    // Level on projects, Science decides.
    land(&mut g, 1, 1);
    assert_eq!(racers(&ai, &g), vec![1, 2]);
}
