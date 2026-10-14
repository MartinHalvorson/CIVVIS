use std::collections::BTreeSet;
use std::sync::Arc;

use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::{StrategicPlan, VictoryTarget};
use crate::game::HostUnitFacts;

/// Three majors at peace and mutually known, seat 0 the denier. Seat 1 gets a
/// second city, so it can stand two pads.
fn board() -> (Game, u32, u32) {
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
    let first = g.player_city_ids(1)[0];
    let centre = g.cities[&first].pos;
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
    g.turn = 180;
    g.at_war.clear();
    g.record_contact(0, 1);
    g.record_contact(0, 2);
    for cid in g.cities.keys().copied().collect::<Vec<_>>() {
        let centre = g.cities[&cid].pos;
        g.players[0].explored.insert(centre);
    }
    // The Moon landed: two space projects.
    for project in ["launch_earth_satellite", "launch_moon_landing"] {
        g.players[1].science_projects.insert(project.to_string());
    }
    (g, first, second)
}

/// A Spaceport on a free owned land tile of `cid`, filed under `filed_under`
/// (normally `cid` itself), and seen by seat 0.
fn give_pad(g: &mut Game, cid: u32, filed_under: u32) -> Pos {
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
    tile.owner_city = Some(filed_under);
    tile.district = Some(crate::name!("spaceport"));
    tile.pillaged = false;
    tile.improvement = None;
    g.cities
        .get_mut(&filed_under)
        .unwrap()
        .districts
        .insert(crate::name!("spaceport"), pad);
    g.players[0].explored.insert(pad);
    pad
}

/// A level-0 spy of seat 0 standing in `city`, free to act this turn, its
/// sources there already gained (so neither a promotion nor Gain Sources is
/// the order the pass gives it).
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

/// The host's own operation menu for `spy`.
fn host_menu(g: &mut Game, spy: u32, missions: &[&str]) {
    Arc::make_mut(&mut g.host_unit_facts).insert(
        spy,
        HostUnitFacts {
            spy_missions: Some(missions.iter().map(|mission| mission.to_string()).collect()),
            ..Default::default()
        },
    );
}

/// The war plan's Conquest target, as G398 played it.
fn conquest_on(target: usize) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(target),
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: 0,
        rush: false,
    }
}

fn stock() -> AdvancedAi {
    AdvancedAi::targeting(VictoryTarget::Domination)
}

fn every_pad() -> AdvancedAi {
    let mut ai = stock();
    ai.enable_science_denial_every_pad();
    ai
}

fn mission(g: &Game, spy: u32) -> Option<String> {
    g.spies[&spy]
        .mission
        .as_ref()
        .map(|mission| mission.kind.clone())
}

#[test]
fn every_pad_is_opt_in() {
    opt_in_off_in_both_controllers("science-denial-every-pad", |ai| ai.science_denial_every_pad);
}

#[test]
fn the_rival_qualifies_at_the_moon_landing() {
    let (mut g, _, _) = board();
    assert!(stock().every_pad_threats(&g, 0).is_empty(), "off: nothing");
    assert_eq!(every_pad().every_pad_threats(&g, 0), BTreeSet::from([1]));
    g.players[1].science_projects.remove("launch_moon_landing");
    assert!(
        every_pad().every_pad_threats(&g, 0).is_empty(),
        "one project landed is not yet the decisive race"
    );
}

#[test]
fn the_disruption_outranks_the_unrest() {
    // G398's shape: Conquest on the racer, a spy with sources in its pad
    // city. Value times chance prefers Foment Unrest (320 x 0.76 against
    // 190 x 0.40); the gene runs the disruption.
    let run = |mut ai: AdvancedAi| {
        let (mut g, first, _) = board();
        give_pad(&mut g, first, first);
        let spy = idle_spy(&mut g, first);
        ai.advanced_spies(&mut g, 0, &conquest_on(1));
        mission(&g, spy)
    };
    let off = run(stock());
    assert!(
        off.as_deref() != Some("disrupt_rocketry"),
        "stock: the table picks {off:?}"
    );
    assert_eq!(run(every_pad()).as_deref(), Some("disrupt_rocketry"));
}

#[test]
fn the_host_menu_files_a_tied_pad_under_the_spys_city() {
    // The board filed the pad under the sibling city; the host offers the
    // disruption from the city the spy stands in.
    let run = |mut ai: AdvancedAi| {
        let (mut g, first, second) = board();
        let pad = give_pad(&mut g, first, second);
        let spy = idle_spy(&mut g, first);
        host_menu(
            &mut g,
            spy,
            &["foment_unrest", "listening_post", "disrupt_rocketry"],
        );
        ai.advanced_spies(&mut g, 0, &conquest_on(1));
        (
            mission(&g, spy),
            g.spies[&spy].mission.as_ref().map(|m| m.target),
            pad,
        )
    };
    let (off, _, _) = run(stock());
    assert_eq!(
        off.as_deref(),
        Some("foment_unrest"),
        "stock: the board offers no disruption here"
    );
    let (on, target, pad) = run(every_pad());
    assert_eq!(on.as_deref(), Some("disrupt_rocketry"));
    assert_eq!(
        target,
        Some(pad),
        "the disruption is aimed at the pad itself"
    );
}

#[test]
fn two_spies_in_one_pad_city_spread_to_the_second_pad() {
    let run = |mut ai: AdvancedAi| {
        let (mut g, first, second) = board();
        give_pad(&mut g, first, first);
        give_pad(&mut g, second, second);
        let holder = idle_spy(&mut g, first);
        let extra = idle_spy(&mut g, first);
        // The holder is already running something; the extra spy is idle.
        g.spies.get_mut(&holder).unwrap().mission = Some(crate::game::SpyMission {
            kind: "disrupt_rocketry".to_string(),
            city: first,
            target: g.cities[&first].pos,
            started: g.turn,
            ends: g.turn + 4,
        });
        ai.advanced_spies(&mut g, 0, &conquest_on(1));
        (g.spies[&extra].city, first, second)
    };
    let (city, first, _) = run(stock());
    assert_eq!(
        city,
        Some(first),
        "stock: the second spy stays in the held city"
    );
    let (city, _, second) = run(every_pad());
    assert_eq!(city, Some(second), "the gene: it leaves for the free pad");
}

#[test]
fn spies_at_home_post_one_to_each_pad() {
    let (mut g, first, second) = board();
    give_pad(&mut g, first, first);
    give_pad(&mut g, second, second);
    let home = g.player_city_ids(0)[0];
    let a = idle_spy(&mut g, home);
    let b = idle_spy(&mut g, home);
    every_pad().advanced_spies(&mut g, 0, &conquest_on(1));
    let posted: BTreeSet<Option<u32>> = [a, b].iter().map(|spy| g.spies[spy].city).collect();
    assert_eq!(posted, BTreeSet::from([Some(first), Some(second)]));
}

#[test]
fn a_pad_city_the_host_refuses_is_left_for_the_free_one() {
    // G408's shape: the spy holds a pad city whose host menu lists no
    // Disrupt Rocketry with the pad standing — the board filed a tied pad
    // under the wrong city — while the rival's other pad city is free.
    let run = |mut ai: AdvancedAi| {
        let (mut g, first, second) = board();
        give_pad(&mut g, first, first);
        give_pad(&mut g, second, second);
        let spy = idle_spy(&mut g, first);
        host_menu(&mut g, spy, &["foment_unrest", "listening_post"]);
        ai.advanced_spies(&mut g, 0, &conquest_on(1));
        let refused = ai.science_denial_refused_pads.contains_key(&first);
        (g.spies[&spy].city, refused, first, second)
    };
    let (city, refused, first, _) = run(stock());
    assert_eq!(city, Some(first), "stock: the spy stays");
    assert!(!refused, "stock: nothing is remembered");
    let (city, refused, _, second) = run(every_pad());
    assert_eq!(city, Some(second), "the gene: it leaves for the free pad");
    assert!(refused, "and the refusing city is remembered");
}

#[test]
fn a_running_operation_is_no_refusal() {
    let (mut g, first, second) = board();
    give_pad(&mut g, first, first);
    give_pad(&mut g, second, second);
    let worker = idle_spy(&mut g, first);
    let spare = idle_spy(&mut g, first);
    host_menu(&mut g, spare, &["foment_unrest", "listening_post"]);
    Arc::make_mut(&mut g.host_unit_facts)
        .entry(worker)
        .or_default()
        .spy_operation = Some("disrupt_rocketry".to_string());
    let mut ai = every_pad();
    let mut pads = BTreeSet::from([first, second]);
    ai.every_pad_drop_refused(&g, 0, &mut pads);
    assert_eq!(
        pads,
        BTreeSet::from([first, second]),
        "the disruption already running there is why"
    );
}
