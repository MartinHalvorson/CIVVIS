use std::collections::BTreeSet;

use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::{StrategicPlan, VictoryTarget};

/// Three majors at peace and mutually known, seat 0 ours. Seat 1, the space
/// racer, holds Rocketry and a second city; seat 2 is the war plan's rival.
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
    g.players[1].techs.insert(crate::name!("rocketry"));
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

/// The live arming the gene joins: the Rocketry-trigger network.
fn stock() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_space_denial_spies_from_rocketry();
    ai
}

fn keeps() -> AdvancedAi {
    let mut ai = stock();
    ai.enable_racer_keeps_the_spies();
    ai
}

fn mission(g: &Game, spy: u32) -> Option<String> {
    g.spies[&spy]
        .mission
        .as_ref()
        .map(|mission| mission.kind.clone())
}

#[test]
fn racer_keeps_the_spies_is_opt_in() {
    opt_in_off_in_both_controllers("racer-keeps-the-spies", |ai| ai.racer_keeps_the_spies);
}

#[test]
fn the_racers_are_read_only_under_the_gene() {
    let (g, _, _, _) = board();
    assert_eq!(stock().racer_spy_racers(&g, 0), BTreeSet::new());
    assert_eq!(keeps().racer_spy_racers(&g, 0), BTreeSet::from([1]));
}

#[test]
fn a_spy_in_the_war_targets_city_goes_to_the_racer_before_a_pad_is_seen() {
    // 050758Z's shape: Conquest on England (seat 2), Gaul (seat 1) holds
    // Rocketry and no pad of it is on the board.
    let run = |mut ai: AdvancedAi| {
        let (mut g, capital, _, rival) = board();
        let spy = idle_spy(&mut g, rival);
        ai.advanced_spies(&mut g, 0, &conquest_on(2));
        (g.spies[&spy].city, mission(&g, spy), capital, rival)
    };
    let (city, _, _, rival) = run(stock());
    assert_eq!(city, Some(rival), "stock: the Spy works the war target");
    let (city, op, capital, _) = run(keeps());
    assert_eq!(
        city,
        Some(capital),
        "the gene: it goes to the racer's capital"
    );
    assert_eq!(op, None, "and starts nothing on the way");
}

#[test]
fn a_spy_at_home_goes_to_the_racer_rather_than_the_war_target() {
    let run = |mut ai: AdvancedAi| {
        let (mut g, capital, second, _) = board();
        let home = g.player_city_ids(0)[0];
        let spy = idle_spy(&mut g, home);
        ai.advanced_spies(&mut g, 0, &conquest_on(2));
        (g.spies[&spy].city, capital, second)
    };
    let (city, capital, second) = run(stock());
    assert!(
        city != Some(capital) && city != Some(second),
        "stock: the Conquest posting is not the racer ({city:?})"
    );
    let (city, capital, _) = run(keeps());
    assert_eq!(city, Some(capital));
}

#[test]
fn a_seen_pad_outranks_the_capital() {
    let (mut g, _, second, rival) = board();
    give_pad(&mut g, second);
    let spy = idle_spy(&mut g, rival);
    keeps().advanced_spies(&mut g, 0, &conquest_on(2));
    assert_eq!(g.spies[&spy].city, Some(second));
}

#[test]
fn a_second_spy_takes_the_racers_next_city() {
    let (mut g, capital, second, rival) = board();
    let first = idle_spy(&mut g, rival);
    let home = g.player_city_ids(0)[0];
    let other = idle_spy(&mut g, home);
    keeps().advanced_spies(&mut g, 0, &conquest_on(2));
    let posted: BTreeSet<Option<u32>> =
        [first, other].iter().map(|spy| g.spies[spy].city).collect();
    assert_eq!(posted, BTreeSet::from([Some(capital), Some(second)]));
}

#[test]
fn no_capture_risk_operation_outside_the_racers_cities() {
    // The racer's cities are unknown to us, so the Spy cannot be posted
    // there; it still does not gamble its slot on the war target.
    let run = |mut ai: AdvancedAi| {
        let (mut g, capital, second, rival) = board();
        for cid in [capital, second] {
            let centre = g.cities[&cid].pos;
            g.players[0].explored.remove(&centre);
        }
        let spy = idle_spy(&mut g, rival);
        ai.advanced_spies(&mut g, 0, &conquest_on(2));
        (g.spies[&spy].city, mission(&g, spy), rival)
    };
    let (_, op, _) = run(stock());
    assert!(
        op.as_deref().is_some_and(AdvancedAi::racer_spy_risky),
        "stock: the table runs a capture-risk operation ({op:?})"
    );
    let (city, op, rival) = run(keeps());
    assert_eq!(city, Some(rival), "nowhere to go");
    assert!(
        !op.as_deref().is_some_and(AdvancedAi::racer_spy_risky),
        "the gene withholds it ({op:?})"
    );
}

#[test]
fn in_the_racers_city_a_safe_operation_until_the_disruption() {
    let run = |mut ai: AdvancedAi, pad: bool| {
        let (mut g, capital, _, _) = board();
        if pad {
            give_pad(&mut g, capital);
        }
        let spy = idle_spy(&mut g, capital);
        ai.advanced_spies(&mut g, 0, &conquest_on(1));
        (g.spies[&spy].city, mission(&g, spy), capital)
    };
    let (_, op, _) = run(stock(), false);
    assert!(
        op.as_deref().is_some_and(AdvancedAi::racer_spy_risky),
        "stock: the war on the racer runs {op:?}"
    );
    let (city, op, capital) = run(keeps(), false);
    assert_eq!(city, Some(capital), "the Spy stays");
    assert!(
        op.is_none() || matches!(op.as_deref(), Some("listening_post" | "gain_sources")),
        "no pad yet: only a safe operation ({op:?})"
    );
    let (_, op, _) = run(keeps(), true);
    assert_eq!(op.as_deref(), Some("disrupt_rocketry"));
}

#[test]
fn a_spy_in_a_city_that_stays_is_not_posted() {
    let (mut g, _, _, rival) = board();
    let spy = idle_spy(&mut g, rival);
    let legal = g.legal_spy_actions(0, spy);
    let stays = BTreeSet::from([1, 2]);
    assert_eq!(
        AdvancedAi::racer_spy_post(&g, spy, Some(rival), Some(1), &stays, &legal),
        None
    );
    let stays = BTreeSet::from([1]);
    assert!(AdvancedAi::racer_spy_post(&g, spy, Some(rival), Some(1), &stays, &legal).is_some());
    assert_eq!(
        AdvancedAi::racer_spy_post(&g, spy, Some(rival), None, &stays, &legal),
        None,
        "no racer, no post"
    );
}

#[test]
fn the_leader_is_the_racer_with_the_most_projects() {
    let (mut g, _, _, _) = board();
    g.players[2].techs.insert(crate::name!("rocketry"));
    g.players[2]
        .science_projects
        .insert("launch_earth_satellite".to_string());
    assert_eq!(
        AdvancedAi::racer_spy_leader(&g, &BTreeSet::from([1, 2])),
        Some(2)
    );
    assert_eq!(AdvancedAi::racer_spy_leader(&g, &BTreeSet::new()), None);
}

#[test]
fn slots_held_by_captured_agents_are_journalled_once() {
    let headline = "Spy slots held by captured agents";
    let count = |ai: &AdvancedAi| {
        ai.base
            .journal
            .since(0)
            .thoughts
            .iter()
            .filter(|thought| thought.headline == headline)
            .count()
    };
    let (mut g, _, _, _) = board();
    g.players[0]
        .civics
        .insert(crate::name!("diplomatic_service"));
    assert_eq!(g.spy_capacity(0), 1, "fixture: one slot");
    let persist = g.standard_duration(RACER_SPY_SLOT_PERSIST_TURNS);
    let mut ai = keeps();
    ai.base.journal = crate::reasoning::Journal::recording();
    ai.racer_spy_note_held_slots(&g, 0, 0, 1, false);
    assert_eq!(count(&ai), 0, "a city may still train one");
    let start = g.turn;
    for turn in start..start + persist {
        g.turn = turn;
        ai.racer_spy_note_held_slots(&g, 0, 0, 1, true);
    }
    assert_eq!(
        count(&ai),
        0,
        "a Spy in transit also leaves the export for a turn or two"
    );
    g.turn = start + persist;
    ai.racer_spy_note_held_slots(&g, 0, 0, 1, true);
    assert_eq!(
        count(&ai),
        1,
        "every city refused below capacity, turn after turn"
    );
    g.turn += 1;
    ai.racer_spy_note_held_slots(&g, 0, 0, 1, true);
    assert_eq!(count(&ai), 1, "not repeated inside the note window");
    let mut broken = keeps();
    broken.base.journal = crate::reasoning::Journal::recording();
    for turn in (start..start + 4 * persist).step_by(2) {
        g.turn = turn;
        broken.racer_spy_note_held_slots(&g, 0, 0, 1, true);
    }
    assert_eq!(count(&broken), 0, "a broken run starts again");
    let mut off = stock();
    off.base.journal = crate::reasoning::Journal::recording();
    for turn in start..=start + persist {
        g.turn = turn;
        off.racer_spy_note_held_slots(&g, 0, 0, 1, true);
    }
    assert_eq!(count(&off), 0, "gene off: nothing");
}
