use std::collections::BTreeSet;

use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::{StrategicPlan, VictoryTarget};

/// Three majors at peace and mutually known, seat 0 ours. Seat 1 leads the
/// space race (Rocketry, the Satellite and the Moon) from its capital and a
/// second city; seat 2 holds Rocketry and one city. Returns the board, seat
/// 1's capital and second city, and seat 2's city.
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
    g.turn = 150;
    g.at_war.clear();
    g.record_contact(0, 1);
    g.record_contact(0, 2);
    for cid in g.cities.keys().copied().collect::<Vec<_>>() {
        let centre = g.cities[&cid].pos;
        g.players[0].explored.insert(centre);
    }
    for seat in [1, 2] {
        g.players[seat].techs.insert(crate::name!("rocketry"));
    }
    for project in ["launch_earth_satellite", "launch_moon_landing"] {
        g.players[1].science_projects.insert(project.to_string());
    }
    let rival = g.player_city_ids(2)[0];
    (g, capital, second, rival)
}

/// A Spaceport on a free owned land tile of `cid`, seen by seat 0.
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

fn culture() -> StrategicPlan {
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

/// The live arming the gene joins on the culture lane.
fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    ai.enable_space_denial_spies_from_rocketry();
    ai.enable_racer_keeps_the_spies();
    ai.enable_racer_spies_search_for_the_pad();
    if gene {
        ai.enable_spare_spies_take_the_next_pad();
    }
    ai
}

#[test]
fn spare_spies_take_the_next_pad_is_opt_in() {
    opt_in_off_in_both_controllers("spare-spies-take-the-next-pad", |ai| {
        ai.spare_spies_take_the_next_pad
    });
}

#[test]
fn the_fixture_leader_is_the_racer_with_the_projects() {
    let (g, ..) = board();
    let racers = seat(VictoryTarget::Culture, true).racer_spy_racers(&g, 0);
    assert_eq!(racers, BTreeSet::from([1, 2]));
    assert_eq!(AdvancedAi::racer_spy_leader(&g, &racers), Some(1));
}

#[test]
fn a_spare_spy_takes_the_next_racers_pad() {
    let run = |gene: bool| {
        let (mut g, capital, second, rival) = board();
        give_pad(&mut g, capital);
        give_pad(&mut g, rival);
        let keeper = idle_spy(&mut g, capital);
        let home = g.player_city_ids(0)[0];
        let spare = idle_spy(&mut g, home);
        seat(VictoryTarget::Culture, gene).advanced_spies(&mut g, 0, &culture());
        (
            g.spies[&keeper].city,
            g.spies[&spare].city,
            capital,
            second,
            rival,
        )
    };
    let (keeper, spare, capital, second, _) = run(false);
    assert_eq!(keeper, Some(capital), "off: the keeper holds the pad");
    assert_eq!(
        spare,
        Some(second),
        "off: the second Spy takes the leader's other city"
    );
    let (keeper, spare, capital, _, rival) = run(true);
    assert_eq!(keeper, Some(capital), "on: the keeper still holds the pad");
    assert_eq!(
        spare,
        Some(rival),
        "on: the spare takes the next racer's pad"
    );
}

#[test]
fn a_free_pad_of_the_leader_comes_first() {
    let (mut g, capital, second, rival) = board();
    give_pad(&mut g, capital);
    give_pad(&mut g, second);
    give_pad(&mut g, rival);
    idle_spy(&mut g, capital);
    let home = g.player_city_ids(0)[0];
    let spy = idle_spy(&mut g, home);
    let ai = seat(VictoryTarget::Culture, true);
    let racers = ai.racer_spy_racers(&g, 0);
    let legal = g.legal_spy_actions(0, spy);
    assert_eq!(
        ai.spare_spy_post(&g, 0, spy, Some(1), &racers, &legal),
        None,
        "the leader's second pad is free: the ordinary post"
    );
    seat(VictoryTarget::Culture, true).advanced_spies(&mut g, 0, &culture());
    assert_eq!(g.spies[&spy].city, Some(second));
}

#[test]
fn no_known_pad_of_the_leader_leaves_the_search_alone() {
    let (mut g, capital, _, rival) = board();
    give_pad(&mut g, rival);
    idle_spy(&mut g, capital);
    let home = g.player_city_ids(0)[0];
    let spy = idle_spy(&mut g, home);
    let ai = seat(VictoryTarget::Culture, true);
    let racers = ai.racer_spy_racers(&g, 0);
    let legal = g.legal_spy_actions(0, spy);
    assert_eq!(
        ai.spare_spy_post(&g, 0, spy, Some(1), &racers, &legal),
        None
    );
}

#[test]
fn a_spare_spy_stays_at_the_next_pad_while_it_lies_pillaged() {
    let run = |gene: bool| {
        let (mut g, capital, second, rival) = board();
        give_pad(&mut g, capital);
        let pad = give_pad(&mut g, rival);
        g.map.tiles.get_mut(&pad).unwrap().pillaged = true;
        idle_spy(&mut g, capital);
        let spare = idle_spy(&mut g, rival);
        let mut ai = seat(VictoryTarget::Culture, gene);
        // Disrupt Rocketry was offered there before: a found pad.
        ai.racer_spy_found_pads.insert(rival);
        let racers = ai.racer_spy_racers(&g, 0);
        let keeps = ai.spare_spy_keeps(&g, 0, spare, Some(1), &racers);
        ai.advanced_spies(&mut g, 0, &culture());
        (keeps, g.spies[&spare].city, second, rival)
    };
    let (keeps, city, second, _) = run(false);
    assert!(!keeps);
    assert_eq!(city, Some(second), "off: back to the leader's free city");
    let (keeps, city, _, rival) = run(true);
    assert!(keeps);
    assert_eq!(city, Some(rival), "on: it waits for the repair");
}

#[test]
fn the_second_of_two_in_the_leaders_pad_city_moves_on() {
    let (mut g, capital, _, rival) = board();
    give_pad(&mut g, capital);
    give_pad(&mut g, rival);
    let first = idle_spy(&mut g, capital);
    let second = idle_spy(&mut g, capital);
    let ai = seat(VictoryTarget::Culture, true);
    let racers = ai.racer_spy_racers(&g, 0);
    let legal_first = g.legal_spy_actions(0, first);
    let legal_second = g.legal_spy_actions(0, second);
    assert_eq!(
        ai.spare_spy_post(&g, 0, first, Some(1), &racers, &legal_first),
        None,
        "the lower id keeps the pad"
    );
    assert_eq!(
        ai.spare_spy_post(&g, 0, second, Some(1), &racers, &legal_second),
        Some(SparePost::Go(rival))
    );
}

#[test]
fn a_science_seat_is_untouched() {
    let (mut g, capital, _, rival) = board();
    give_pad(&mut g, capital);
    give_pad(&mut g, rival);
    idle_spy(&mut g, capital);
    let home = g.player_city_ids(0)[0];
    let spy = idle_spy(&mut g, home);
    let ai = seat(VictoryTarget::Science, true);
    assert!(!ai.spare_spies_on(&g));
    let racers = ai.racer_spy_racers(&g, 0);
    let legal = g.legal_spy_actions(0, spy);
    assert_eq!(
        ai.spare_spy_post(&g, 0, spy, Some(1), &racers, &legal),
        None
    );
}
