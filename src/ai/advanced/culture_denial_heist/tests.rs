use std::collections::BTreeSet;
use std::sync::Arc;

use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::{GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::{HostUnitFacts, ObservedPublicEmpireStats};
use crate::Pos;

/// Three majors at peace and mutually known, seat 0 the denier. Seat 1 gets a
/// second city, so it can stand two Theater Squares.
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
    g.turn = 160;
    g.at_war.clear();
    g.record_contact(0, 1);
    g.record_contact(0, 2);
    for cid in g.cities.keys().copied().collect::<Vec<_>>() {
        let centre = g.cities[&cid].pos;
        g.players[0].explored.insert(centre);
    }
    (g, first, second)
}

/// Seat 1 one visitor short of the largest other staycation: the culture
/// race the Domination counter answers.
fn culture_race(g: &mut Game) {
    let stats = Arc::make_mut(&mut g.observed_public_empire_stats);
    for (pid, foreign) in [(0, 0), (1, 19), (2, 0)] {
        stats.insert(
            pid,
            ObservedPublicEmpireStats {
                domestic_tourists: Some(20),
                foreign_tourists: Some(foreign),
                ..Default::default()
            },
        );
    }
}

/// A Theater Square on a free owned land tile of `cid`, seen by seat 0.
fn give_theater(g: &mut Game, cid: u32) -> Pos {
    let centre = g.cities[&cid].pos;
    let site = g
        .wdisk(centre, 1)
        .into_iter()
        .find(|pos| {
            *pos != centre
                && g.map
                    .get(*pos)
                    .is_some_and(|tile| tile.district.is_none() && !g.rules.is_water(tile))
        })
        .expect("the fixture city has a free land neighbour");
    let tile = g.map.tiles.get_mut(&site).unwrap();
    tile.owner_city = Some(cid);
    tile.district = Some(crate::name!("theater_square"));
    tile.pillaged = false;
    tile.improvement = None;
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("theater_square"), site);
    g.players[0].explored.insert(site);
    site
}

/// A level-0 spy of seat 0 standing in `city`, free to act this turn, its
/// sources there already gained.
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

/// A Conquest plan on `target`, the shape the culture losses played.
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
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.deny_leaders = true;
    ai
}

fn heist() -> AdvancedAi {
    let mut ai = stock();
    ai.enable_culture_denial_heist();
    ai
}

fn mission(g: &Game, spy: u32) -> Option<String> {
    g.spies[&spy]
        .mission
        .as_ref()
        .map(|mission| mission.kind.clone())
}

#[test]
fn culture_denial_heist_is_opt_in() {
    opt_in_off_in_both_controllers("culture-denial-heist", |ai| ai.culture_denial_heist);
}

#[test]
fn the_culture_racer_is_the_target() {
    let (mut g, _, _) = board();
    assert!(
        heist().culture_heist_targets(&g, 0).is_empty(),
        "no culture race, no target"
    );
    culture_race(&mut g);
    assert!(
        stock().culture_heist_targets(&g, 0).is_empty(),
        "off: nothing"
    );
    assert_eq!(heist().culture_heist_targets(&g, 0), BTreeSet::from([1]));
}

#[test]
fn the_host_menu_makes_the_heist_legal_and_it_is_run() {
    // T062856Z's shape: a spy in the winner's city, the host listing the
    // heist, the board blind to the works there.
    let run = |mut ai: AdvancedAi| {
        let (mut g, first, _) = board();
        culture_race(&mut g);
        give_theater(&mut g, first);
        let spy = idle_spy(&mut g, first);
        host_menu(
            &mut g,
            spy,
            &["foment_unrest", "listening_post", "great_work_heist"],
        );
        ai.advanced_spies(&mut g, 0, &conquest_on(1));
        mission(&g, spy)
    };
    let off = run(stock());
    assert!(
        off.as_deref() != Some("great_work_heist"),
        "stock: the board offers no heist and the table picks {off:?}"
    );
    assert_eq!(run(heist()).as_deref(), Some("great_work_heist"));
}

#[test]
fn no_heist_without_the_host_menu() {
    // Without the host's word the board still cannot see the works.
    let (mut g, first, _) = board();
    culture_race(&mut g);
    give_theater(&mut g, first);
    let spy = idle_spy(&mut g, first);
    heist().advanced_spies(&mut g, 0, &conquest_on(1));
    assert!(mission(&g, spy).as_deref() != Some("great_work_heist"));
}

#[test]
fn a_spy_elsewhere_leaves_for_the_racers_theater_city() {
    let run = |mut ai: AdvancedAi| {
        let (mut g, first, _) = board();
        culture_race(&mut g);
        give_theater(&mut g, first);
        let elsewhere = g.player_city_ids(2)[0];
        let spy = idle_spy(&mut g, elsewhere);
        host_menu(&mut g, spy, &["foment_unrest", "listening_post"]);
        ai.advanced_spies(&mut g, 0, &conquest_on(2));
        (g.spies[&spy].city, first, elsewhere)
    };
    let (city, _, elsewhere) = run(stock());
    assert_eq!(city, Some(elsewhere), "stock: it stays");
    let (city, first, _) = run(heist());
    assert_eq!(
        city,
        Some(first),
        "the gene: it leaves for the racer's city"
    );
}

#[test]
fn a_spy_offered_the_heist_where_it_stands_stays() {
    let (mut g, first, _) = board();
    culture_race(&mut g);
    give_theater(&mut g, first);
    let elsewhere = g.player_city_ids(2)[0];
    let spy = idle_spy(&mut g, elsewhere);
    host_menu(&mut g, spy, &["foment_unrest", "great_work_heist"]);
    assert_eq!(
        AdvancedAi::culture_heist_repost(&g, 0, spy, Some(elsewhere), &BTreeSet::from([first])),
        None
    );
}

#[test]
fn spies_at_home_post_one_to_each_theater_city() {
    let (mut g, first, second) = board();
    culture_race(&mut g);
    give_theater(&mut g, first);
    give_theater(&mut g, second);
    let home = g.player_city_ids(0)[0];
    let a = idle_spy(&mut g, home);
    let b = idle_spy(&mut g, home);
    heist().advanced_spies(&mut g, 0, &conquest_on(1));
    let posted: BTreeSet<Option<u32>> = [a, b].iter().map(|spy| g.spies[spy].city).collect();
    assert_eq!(posted, BTreeSet::from([Some(first), Some(second)]));
}
