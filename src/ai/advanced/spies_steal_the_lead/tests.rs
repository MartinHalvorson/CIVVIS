use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::{StrategicPlan, VictoryTarget};
use crate::game::HostUnitFacts;

/// Three majors at peace and mutually known, seat 0 ours. Seat 1 holds
/// Rocketry (a space racer under `space-denial-spies-from-rocketry`) and a
/// Campus in its capital; seat 2 also has a Campus but no Rocketry.
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
    g.players[2].techs.insert(crate::name!("education"));
    let racer = g.player_city_ids(1)[0];
    let other = g.player_city_ids(2)[0];
    give_district(&mut g, racer, "campus");
    give_district(&mut g, other, "campus");
    (g, racer, other)
}

/// A district `kind` on a free owned land neighbour of `cid`, seen by seat 0.
fn give_district(g: &mut Game, cid: u32, kind: &str) -> Pos {
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
    tile.district = Some(Name::new(kind));
    tile.pillaged = false;
    tile.improvement = None;
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(Name::new(kind), site);
    g.players[0].explored.insert(site);
    site
}

/// A promoted Spy of seat 0 at `level` standing in `city`, free to act. `sourced`:
/// its Gain Sources there already ran (so the pass does not offer it).
fn idle_spy(g: &mut Game, city: u32, level: i64, sourced: bool) -> u32 {
    let id = g.next_id;
    g.next_id += 1;
    g.spies.insert(
        id,
        crate::game::Spy {
            id,
            owner: 0,
            level,
            // One promotion per level, so the pass does not spend the turn on
            // a promotion; neither touches Steal Tech Boost or Disrupt Rocketry.
            promotions: ["linguist", "quartermaster", "surveillance"]
                .into_iter()
                .take(level.max(0) as usize)
                .map(String::from)
                .collect(),
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

fn plan(strategy: GrandStrategy) -> StrategicPlan {
    StrategicPlan {
        strategy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: 0,
        rush: false,
    }
}

/// The live spy arming this gene joins.
fn keeps(target: VictoryTarget) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    ai.enable_space_denial_spies_from_rocketry();
    ai.enable_racer_keeps_the_spies();
    ai
}

fn steals(target: VictoryTarget) -> AdvancedAi {
    let mut ai = keeps(target);
    ai.enable_spies_steal_the_lead();
    ai
}

fn mission(g: &Game, spy: u32) -> Option<String> {
    g.spies[&spy]
        .mission
        .as_ref()
        .map(|mission| mission.kind.clone())
}

fn run(mut ai: AdvancedAi, strategy: GrandStrategy, level: i64, sourced: bool) -> Option<String> {
    let (mut g, racer, _) = board();
    let spy = idle_spy(&mut g, racer, level, sourced);
    ai.advanced_spies(&mut g, 0, &plan(strategy));
    assert_eq!(
        g.spies[&spy].city,
        Some(racer),
        "the Spy stays in the racer's city"
    );
    mission(&g, spy)
}

#[test]
fn spies_steal_the_lead_is_opt_in() {
    opt_in_off_in_both_controllers("spies-steal-the-lead", |ai| ai.spies_steal_the_lead);
}

#[test]
fn in_the_racers_city_a_science_seat_steals_instead_of_listening() {
    let off = run(
        keeps(VictoryTarget::Science),
        GrandStrategy::Science,
        2,
        true,
    );
    assert!(
        matches!(off.as_deref(), Some("listening_post" | "gain_sources")),
        "racer-keeps-the-spies alone waits on a safe operation ({off:?})"
    );
    let on = run(
        steals(VictoryTarget::Science),
        GrandStrategy::Science,
        2,
        true,
    );
    assert_eq!(on.as_deref(), Some(STEAL_OPERATION));
}

#[test]
fn a_green_spy_steals_instead_of_its_gain_sources_training() {
    let off = run(
        keeps(VictoryTarget::Science),
        GrandStrategy::Science,
        0,
        false,
    );
    assert_eq!(off.as_deref(), Some("gain_sources"), "the training step");
    let on = run(
        steals(VictoryTarget::Science),
        GrandStrategy::Science,
        0,
        false,
    );
    assert_eq!(on.as_deref(), Some(STEAL_OPERATION));
}

#[test]
fn an_offered_disrupt_rocketry_keeps_its_place() {
    let (mut g, racer, _) = board();
    give_district(&mut g, racer, "spaceport");
    let spy = idle_spy(&mut g, racer, 2, true);
    steals(VictoryTarget::Science).advanced_spies(&mut g, 0, &plan(GrandStrategy::Science));
    assert_eq!(mission(&g, spy).as_deref(), Some("disrupt_rocketry"));
}

#[test]
fn off_a_science_aim_nothing_changes() {
    for (target, strategy) in [
        (VictoryTarget::Domination, GrandStrategy::Conquest),
        (VictoryTarget::Culture, GrandStrategy::Culture),
    ] {
        for (level, sourced) in [(2, true), (0, false)] {
            assert_eq!(
                run(steals(target), strategy, level, sourced),
                run(keeps(target), strategy, level, sourced),
                "{target:?} level {level}"
            );
        }
    }
}

#[test]
fn the_plans_science_strategy_is_enough() {
    let on = run(
        steals(VictoryTarget::Score),
        GrandStrategy::Science,
        2,
        true,
    );
    assert_eq!(on.as_deref(), Some(STEAL_OPERATION));
}

#[test]
fn a_host_menu_without_the_steal_holds_it() {
    let (mut g, racer, _) = board();
    let spy = idle_spy(&mut g, racer, 2, true);
    Arc::make_mut(&mut g.host_unit_facts).insert(
        spy,
        HostUnitFacts {
            spy_missions: Some(BTreeSet::from(["listening_post".to_string()])),
            ..HostUnitFacts::default()
        },
    );
    steals(VictoryTarget::Science).advanced_spies(&mut g, 0, &plan(GrandStrategy::Science));
    assert_ne!(mission(&g, spy).as_deref(), Some(STEAL_OPERATION));
}

#[test]
fn outside_the_racers_cities_the_steal_is_not_withheld() {
    let ai = steals(VictoryTarget::Science);
    let science = plan(GrandStrategy::Science);
    assert!(!ai.racer_spy_withholds(&science, STEAL_OPERATION));
    assert!(ai.racer_spy_withholds(&science, "foment_unrest"));
    let stock = keeps(VictoryTarget::Science);
    assert!(stock.racer_spy_withholds(&science, STEAL_OPERATION));
    let conquest = plan(GrandStrategy::Conquest);
    let domination = steals(VictoryTarget::Domination);
    assert!(domination.racer_spy_withholds(&conquest, STEAL_OPERATION));
}

/// The board with seat 0 holding Spy capacity, an empty queue in its city,
/// and seat 2 knowing three more technologies than we do.
fn claim_board() -> (Game, u32) {
    let (mut g, _, _) = board();
    for civic in ["diplomatic_service", "nationalism"] {
        g.players[0].civics.insert(Name::new(civic));
    }
    for tech in ["astronomy", "mathematics"] {
        g.players[2].techs.insert(Name::new(tech));
    }
    let home = g.player_city_ids(0)[0];
    g.cities.get_mut(&home).unwrap().queue.clear();
    (g, home)
}

fn spy_item() -> Item {
    Item::Unit {
        unit: crate::name!("spy"),
    }
}

#[test]
fn a_science_seat_trains_one_spy_to_steal_with() {
    let (mut g, home) = claim_board();
    assert!(g.spy_capacity(0) >= 1, "fixture capacity");
    keeps(VictoryTarget::Science).claim_queues_for_steal_spies(
        &mut g,
        0,
        &plan(GrandStrategy::Science),
    );
    assert!(g.cities[&home].queue.is_empty(), "off: no claim");
    let ai = steals(VictoryTarget::Science);
    ai.claim_queues_for_steal_spies(&mut g, 0, &plan(GrandStrategy::Science));
    assert_eq!(g.cities[&home].queue.first(), Some(&spy_item()));
    // One at a time: the Spy in training holds the claim.
    let queued = g.cities[&home].queue.len();
    ai.claim_queues_for_steal_spies(&mut g, 0, &plan(GrandStrategy::Science));
    assert_eq!(g.cities[&home].queue.len(), queued);
}

#[test]
fn no_spy_is_trained_at_capacity_paused_or_without_a_target() {
    let run = |g: &mut Game, ai: &AdvancedAi, home: u32| {
        ai.claim_queues_for_steal_spies(g, 0, &plan(GrandStrategy::Science));
        g.cities[&home].queue.first() == Some(&spy_item())
    };
    // At capacity.
    let (mut g, home) = claim_board();
    for _ in 0..g.spy_capacity(0) {
        idle_spy(&mut g, home, 0, false);
    }
    assert!(!run(&mut g, &steals(VictoryTarget::Science), home));
    // Paused by `denial-spies-count-their-losses`.
    let (mut g, home) = claim_board();
    let mut paused = steals(VictoryTarget::Science);
    paused.base.spy_training_paused = true;
    assert!(!run(&mut g, &paused, home));
    // No rival knows three technologies we lack.
    let (mut g, home) = claim_board();
    for pid in 1..3 {
        let theirs: Vec<Name> = g.players[pid].techs.iter().copied().collect();
        g.players[0].techs.extend(theirs);
    }
    assert!(!run(&mut g, &steals(VictoryTarget::Science), home));
    // Off a Science aim.
    let (mut g, home) = claim_board();
    let ai = steals(VictoryTarget::Domination);
    ai.claim_queues_for_steal_spies(&mut g, 0, &plan(GrandStrategy::Conquest));
    assert!(g.cities[&home].queue.first() != Some(&spy_item()));
}

#[test]
fn a_rival_with_three_technologies_we_lack_and_a_seen_campus_is_a_target() {
    let (mut g, _) = claim_board();
    assert!(AdvancedAi::steal_targets(&g, 0).contains(&2));
    let campus = g.cities[&g.player_city_ids(2)[0]]
        .districts
        .iter()
        .find_map(|(district, tile)| (g.district_family(*district) == "campus").then_some(*tile));
    g.players[0]
        .explored
        .remove(&campus.expect("the fixture campus"));
    assert!(
        !AdvancedAi::steal_targets(&g, 0).contains(&2),
        "an unseen Campus"
    );
}

/// The board with the racer's Campus moved off its capital onto a second
/// city of the racer: (game, racer's capital, racer's Campus city, rival city).
fn post_board() -> (Game, u32, u32, u32) {
    let (mut g, racer, other) = board();
    let campus = g.cities[&racer]
        .districts
        .iter()
        .find_map(|(district, tile)| (g.district_family(*district) == "campus").then_some(*tile))
        .expect("the fixture campus");
    g.cities
        .get_mut(&racer)
        .unwrap()
        .districts
        .remove(Name::new("campus"));
    g.map.tiles.get_mut(&campus).unwrap().district = None;
    let centre = g.cities[&racer].pos;
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
    g.players[0].explored.insert(g.cities[&second].pos);
    give_district(&mut g, second, "campus");
    (g, racer, second, other)
}

#[test]
fn a_science_post_prefers_the_leaders_campus_city() {
    let (mut g, capital, second, other) = post_board();
    assert!(g.cities[&capital].is_capital);
    let spy = idle_spy(&mut g, other, 2, true);
    let legal = g.legal_spy_actions(0, spy);
    let racers = BTreeSet::from([1]);
    let none = BTreeSet::new();
    let post = |prefer: bool| {
        AdvancedAi::racer_spy_post_for(
            &g,
            spy,
            Some(other),
            Some(1),
            &racers,
            &none,
            &legal,
            None,
            prefer,
            &BTreeMap::new(),
            &racers,
        )
    };
    assert_eq!(post(false), Some(capital), "stock: the capital first");
    assert_eq!(post(true), Some(second), "the Campus to steal from");
}

#[test]
fn a_spy_in_a_campusless_leader_city_moves_to_its_campus() {
    let (mut g, capital, second, _) = post_board();
    let spy = idle_spy(&mut g, capital, 2, true);
    let legal = g.legal_spy_actions(0, spy);
    let racers = BTreeSet::from([1]);
    let none = BTreeSet::new();
    let post = |prefer: bool| {
        AdvancedAi::racer_spy_post_for(
            &g,
            spy,
            Some(capital),
            Some(1),
            &racers,
            &none,
            &legal,
            None,
            prefer,
            &BTreeMap::new(),
            &racers,
        )
    };
    assert_eq!(
        post(false),
        None,
        "stock: the Spy stays in the leader's city"
    );
    assert_eq!(
        post(true),
        Some(second),
        "it can neither steal nor disrupt here"
    );
    // Through the pass: the Science seat posts it, a Domination seat keeps it.
    let mut on = g.clone();
    steals(VictoryTarget::Science).advanced_spies(&mut on, 0, &plan(GrandStrategy::Science));
    assert_ne!(on.spies[&spy].city, Some(capital), "posted away");
}

fn host_menu(g: &mut Game, spy: u32, missions: &[&str]) {
    Arc::make_mut(&mut g.host_unit_facts).insert(
        spy,
        HostUnitFacts {
            spy_missions: Some(missions.iter().map(|m| m.to_string()).collect()),
            ..HostUnitFacts::default()
        },
    );
}

#[test]
fn the_host_menu_marks_a_city_dry_and_clears_it() {
    let (mut g, capital, _, _) = post_board();
    let spy = idle_spy(&mut g, capital, 2, true);
    let mut ai = steals(VictoryTarget::Science);
    host_menu(&mut g, spy, &["listening_post", "foment_unrest"]);
    ai.steal_note_dry_city(&g, 0, spy, Some(capital));
    assert_eq!(ai.steal_dry_cities.get(&capital), Some(&g.turn));
    host_menu(&mut g, spy, &["listening_post", STEAL_OPERATION]);
    ai.steal_note_dry_city(&g, 0, spy, Some(capital));
    assert!(!ai.steal_dry_cities.contains_key(&capital));
}

#[test]
fn a_spy_in_a_dry_leader_city_moves_on_when_no_campus_is_known() {
    let (mut g, capital, second, _) = post_board();
    // The board knows no Campus of the leader.
    let tile = g.cities[&second]
        .districts
        .iter()
        .find_map(|(district, tile)| (g.district_family(*district) == "campus").then_some(*tile))
        .expect("the fixture campus");
    g.players[0].explored.remove(&tile);
    assert!(!AdvancedAi::racer_spy_seen_campus(&g, 0, second));
    let spy = idle_spy(&mut g, capital, 2, true);
    let legal = g.legal_spy_actions(0, spy);
    let racers = BTreeSet::from([1]);
    let none = BTreeSet::new();
    let post = |dry: &BTreeMap<u32, u32>| {
        AdvancedAi::racer_spy_post_for(
            &g,
            spy,
            Some(capital),
            Some(1),
            &racers,
            &none,
            &legal,
            None,
            true,
            dry,
            &racers,
        )
    };
    assert_eq!(post(&BTreeMap::new()), None, "not shown dry: it waits");
    let dry = BTreeMap::from([(capital, g.turn)]);
    assert_eq!(post(&dry), Some(second), "shown dry: the next city");
    let both = BTreeMap::from([(capital, g.turn), (second, g.turn)]);
    assert_eq!(post(&both), None, "every city dry: it stays");
}

/// The host's menu and targets for `spy`: Listening Post at the city, Steal
/// Tech Boost at `steal_at` with `odds`.
fn host_targets(g: &mut Game, spy: u32, city: u32, steal_at: Pos, odds: Option<f64>) {
    let centre = g.cities[&city].pos;
    Arc::make_mut(&mut g.host_unit_facts).insert(
        spy,
        HostUnitFacts {
            spy_missions: Some(BTreeSet::from([
                "listening_post".to_string(),
                STEAL_OPERATION.to_string(),
            ])),
            spy_targets: Some(std::collections::BTreeMap::from([
                (
                    "listening_post".to_string(),
                    crate::game::HostSpyTarget {
                        pos: centre,
                        success: Some(1.0),
                        fail: None,
                    },
                ),
                (
                    STEAL_OPERATION.to_string(),
                    crate::game::HostSpyTarget {
                        pos: steal_at,
                        success: odds,
                        fail: None,
                    },
                ),
            ])),
            ..HostUnitFacts::default()
        },
    );
}

#[test]
fn a_campus_the_board_never_saw_is_stolen_from_at_the_hosts_plot() {
    let (mut g, racer, _) = board();
    let campus = g.cities[&racer]
        .districts
        .iter()
        .find_map(|(district, tile)| (g.district_family(*district) == "campus").then_some(*tile))
        .expect("the fixture campus");
    // The board forgets the Campus: neither the city nor the tile holds it.
    g.cities
        .get_mut(&racer)
        .unwrap()
        .districts
        .remove(Name::new("campus"));
    g.map.tiles.get_mut(&campus).unwrap().district = None;
    let spy = idle_spy(&mut g, racer, 2, true);
    host_targets(&mut g, spy, racer, campus, Some(0.7));
    let mut off = g.clone();
    keeps(VictoryTarget::Science).advanced_spies(&mut off, 0, &plan(GrandStrategy::Science));
    assert_ne!(
        mission(&off, spy).as_deref(),
        Some(STEAL_OPERATION),
        "without the gene the board cannot aim it"
    );
    steals(VictoryTarget::Science).advanced_spies(&mut g, 0, &plan(GrandStrategy::Science));
    assert_eq!(mission(&g, spy).as_deref(), Some(STEAL_OPERATION));
    assert_eq!(
        g.spies[&spy].mission.as_ref().map(|mission| mission.target),
        Some(campus),
        "aimed at the host's plot"
    );
}

#[test]
fn a_steal_below_the_host_odds_floor_is_not_started() {
    let run = |odds: f64| {
        let (mut g, racer, _) = board();
        let campus = g.cities[&racer]
            .districts
            .iter()
            .find_map(|(district, tile)| {
                (g.district_family(*district) == "campus").then_some(*tile)
            })
            .expect("the fixture campus");
        let spy = idle_spy(&mut g, racer, 2, true);
        host_targets(&mut g, spy, racer, campus, Some(odds));
        steals(VictoryTarget::Science).advanced_spies(&mut g, 0, &plan(GrandStrategy::Science));
        mission(&g, spy)
    };
    assert_eq!(run(0.7).as_deref(), Some(STEAL_OPERATION), "good odds");
    assert_ne!(
        run(STEAL_MIN_SUCCESS - 0.2).as_deref(),
        Some(STEAL_OPERATION),
        "below the floor"
    );
}

#[test]
fn a_steal_priced_under_the_floor_marks_the_city_dry() {
    let (mut g, racer, _) = board();
    let campus = g.cities[&racer]
        .districts
        .iter()
        .find_map(|(district, tile)| (g.district_family(*district) == "campus").then_some(*tile))
        .expect("the fixture campus");
    let spy = idle_spy(&mut g, racer, 2, true);
    let mut ai = steals(VictoryTarget::Science);
    host_targets(&mut g, spy, racer, campus, Some(STEAL_MIN_SUCCESS - 0.1));
    ai.steal_note_dry_city(&g, 0, spy, Some(racer));
    assert!(ai.steal_dry_cities.contains_key(&racer), "poor odds: dry");
    host_targets(&mut g, spy, racer, campus, Some(0.8));
    ai.steal_note_dry_city(&g, 0, spy, Some(racer));
    assert!(
        !ai.steal_dry_cities.contains_key(&racer),
        "good odds: cleared"
    );
}
