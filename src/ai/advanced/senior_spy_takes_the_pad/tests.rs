use std::collections::BTreeSet;

use super::super::test_support::opt_in_off_in_both_controllers;
use super::super::{AdvancedAi, VictoryTarget};
use crate::game::{Game, Spy};

#[test]
fn senior_spy_takes_the_pad_is_a_native_opt_in_off_in_both_controllers() {
    opt_in_off_in_both_controllers("senior-spy-takes-the-pad", |ai| ai.senior_spy_takes_the_pad);
}

fn spy(g: &mut Game, owner: usize, level: i64, city: Option<u32>) -> u32 {
    let id = g.next_id;
    g.next_id += 1;
    g.spies.insert(
        id,
        Spy {
            id,
            owner,
            level,
            promotions: BTreeSet::new(),
            city,
            ready_turn: g.turn,
            mission: None,
            sources_city: None,
            sources_until: 0,
            captured_by: None,
        },
    );
    id
}

/// Seat 1, the space racer, has launched its Earth Satellite (one stage) and
/// holds a Spaceport by its capital that seat 0 has seen, and a second city
/// with a seen Campus. Seat 0's level-0 Spy holds the capital; a level-2 Spy
/// stands at home.
fn racer_with_a_pad() -> (Game, u32, u32, u32, u32) {
    let mut g = Game::new_full(2, 30, 20, 41_830, 250, 0, false);
    for pid in 0..2 {
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
    g.current = 0;
    g.turn = 150;
    g.at_war.clear();
    g.record_contact(0, 1);
    let capital = g.player_city_ids(1)[0];
    let centre = g.cities[&capital].pos;
    g.players[0].explored.insert(centre);
    g.players[1].techs.insert(crate::name!("rocketry"));
    g.players[1]
        .science_projects
        .insert("launch_earth_satellite".to_string());
    let free_land = |g: &Game, around: (i32, i32), radius: i32| {
        g.wdisk(around, radius).into_iter().find(|pos| {
            *pos != around
                && g.map.get(*pos).is_some_and(|tile| {
                    tile.district.is_none() && tile.owner_city.is_none() && !g.rules.is_water(tile)
                })
        })
    };
    let pad = free_land(&g, centre, 1).expect("the fixture capital has a free land neighbour");
    let tile = g.map.tiles.get_mut(&pad).unwrap();
    tile.owner_city = Some(capital);
    tile.district = Some(crate::name!("spaceport"));
    tile.pillaged = false;
    tile.improvement = None;
    g.cities
        .get_mut(&capital)
        .unwrap()
        .districts
        .insert(crate::name!("spaceport"), pad);
    g.players[0].explored.insert(pad);
    let second_site = g
        .wdisk(centre, 6)
        .into_iter()
        .filter(|pos| g.wdist(centre, *pos) >= 4)
        .find(|pos| {
            g.map
                .get(*pos)
                .is_some_and(|tile| tile.owner_city.is_none() && !g.rules.is_water(tile))
        })
        .expect("the fixture has room for a second racer city");
    let second = g.found_city_for(1, second_site, None);
    let second_pos = g.cities[&second].pos;
    g.players[0].explored.insert(second_pos);
    let home = g.player_city_ids(0)[0];
    let junior = spy(&mut g, 0, 0, Some(capital));
    let senior = spy(&mut g, 0, 2, Some(home));
    (g, capital, second, junior, senior)
}

fn on() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_senior_spy_takes_the_pad();
    ai
}

/// The pad, pillaged by our last Disrupt: the host offers none until the
/// racer repairs it, so the swap costs no attempt.
fn pillage_the_pad(g: &mut Game, capital: u32) {
    let pad = g.cities[&capital].districts[&crate::name!("spaceport")];
    g.map.tiles.get_mut(&pad).unwrap().pillaged = true;
}

#[test]
fn the_senior_takes_the_pad_and_the_junior_moves_on() {
    let (mut g, capital, second, junior, senior) = racer_with_a_pad();
    pillage_the_pad(&mut g, capital);
    let ai = on();
    let ids = [junior, senior];
    assert_eq!(
        ai.senior_spy_swap(&g, 0, &[1], &ids),
        Some((junior, senior, capital, second))
    );
    let mut ai = ai;
    let mut swapped = g.clone();
    assert!(ai.senior_spy_swap_pass(&mut swapped, 0, Some(1), &ids));
    assert!(ai.spy_orders_until.contains_key(&junior));
    assert!(ai.spy_orders_until.contains_key(&senior));
    // No named leader (the live seat, `racer-keeps-the-spies` off): every
    // major rival is read, and the swap is the same.
    let mut unnamed = on();
    let mut swapped = g.clone();
    assert!(unnamed.senior_spy_swap_pass(&mut swapped, 0, None, &ids));
}

#[test]
fn no_swap_off_while_the_pad_stands_or_without_a_free_senior() {
    let (standing, capital, _, junior, senior) = racer_with_a_pad();
    let ids = [junior, senior];
    // The pad stands: the holder is offered Disrupt Rocketry this turn and
    // runs it, so no swap.
    assert!(AdvancedAi::racer_spy_disrupt_offered(
        &standing,
        junior,
        &standing.legal_spy_actions(0, junior)
    ));
    assert_eq!(on().senior_spy_swap(&standing, 0, &[1], &ids), None);
    let mut g = standing.clone();
    pillage_the_pad(&mut g, capital);
    // Off.
    let mut off = AdvancedAi::targeting(VictoryTarget::Science);
    assert!(!off.senior_spy_swap_pass(&mut g.clone(), 0, Some(1), &ids));
    // The holder is the senior one.
    let mut even = g.clone();
    even.spies.get_mut(&senior).unwrap().level = 0;
    assert_eq!(on().senior_spy_swap(&even, 0, &[1], &ids), None);
    // The holder's last order is still running.
    let mut busy = on();
    busy.spy_orders_until.insert(junior, g.turn + 3);
    assert_eq!(busy.senior_spy_swap(&g, 0, &[1], &ids), None);
}
