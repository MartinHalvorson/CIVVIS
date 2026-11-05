use std::collections::BTreeSet;
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

/// A Spy of seat 0 at `level` standing in `city`, free to act. `sourced`:
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
