use super::*;
use std::collections::BTreeSet;

/// `disrupt-before-the-sources`' own fixture: seat 1, the space racer, has
/// landed its Earth Satellite and stands a seen Spaceport; a green Spy of
/// seat 0 stands in its capital offered Gain Sources and Disrupt Rocketry.
fn racer_with_a_pad() -> (Game, u32, u32) {
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
    let spy = g.next_id;
    g.next_id += 1;
    g.spies.insert(
        spy,
        crate::game::Spy {
            id: spy,
            owner: 0,
            level: 0,
            promotions: BTreeSet::new(),
            city: Some(capital),
            ready_turn: g.turn,
            mission: None,
            sources_city: None,
            sources_until: 0,
            captured_by: None,
        },
    );
    (g, capital, spy)
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

/// The live arming the gene joins: the Rocketry-trigger network and the
/// racer's Spies.
fn live() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_space_denial_spies_from_rocketry();
    ai.enable_racer_keeps_the_spies();
    ai
}

fn mission(g: &Game, spy: u32) -> Option<String> {
    g.spies[&spy]
        .mission
        .as_ref()
        .map(|mission| mission.kind.clone())
}

fn land(g: &mut Game, projects: &[&str]) {
    for project in projects {
        g.players[1].science_projects.insert(project.to_string());
    }
}

/// Withheld before the match point, released at it; never with the gene off.
#[test]
fn disrupt_is_held_until_the_racers_third_project() {
    let (mut g, _, spy) = racer_with_a_pad();
    let off = AdvancedAi::new();
    let mut on = AdvancedAi::new();
    on.enable_disrupt_waits_for_the_match_point();
    assert!(!off.disrupt_held_for_the_match_point(&g, spy));
    assert!(on.disrupt_held_for_the_match_point(&g, spy));
    let legal = g.legal_spy_actions(0, spy);
    let disrupts = |legal: &[Action]| {
        legal.iter().any(
            |action| matches!(action, Action::SpyMission { mission, .. } if mission == "disrupt_rocketry"),
        )
    };
    assert!(disrupts(&legal), "fixture: Disrupt is offered");
    assert!(!disrupts(&on.withhold_early_disrupts(
        &g,
        spy,
        legal.clone()
    )));
    assert!(disrupts(&off.withhold_early_disrupts(
        &g,
        spy,
        legal.clone()
    )));
    assert_eq!(on.disrupt_from_stages(), MATCH_POINT_STAGES);
    assert_eq!(
        off.disrupt_from_stages(),
        super::super::disrupt_before_the_sources::DISRUPT_FROM_STAGES
    );

    land(&mut g, &["launch_moon_landing", "launch_mars_colony"]);
    assert!(!on.disrupt_held_for_the_match_point(&g, spy));
    let legal = g.legal_spy_actions(0, spy);
    assert!(disrupts(&on.withhold_early_disrupts(&g, spy, legal)));
}

/// The green Spy `disrupt-before-the-sources` sends to Disrupt gains sources
/// instead before the match point, and disrupts at it.
#[test]
fn the_green_spy_waits_for_the_match_point() {
    let (g, _, spy) = racer_with_a_pad();
    let mut ai = live();
    ai.enable_disrupt_before_the_sources();
    ai.enable_disrupt_waits_for_the_match_point();
    let mut early = g.clone();
    ai.advanced_spies(&mut early, 0, &conquest_on(1));
    assert_ne!(mission(&early, spy).as_deref(), Some("disrupt_rocketry"));

    let mut late = g.clone();
    land(&mut late, &["launch_moon_landing", "launch_mars_colony"]);
    ai.advanced_spies(&mut late, 0, &conquest_on(1));
    assert_eq!(mission(&late, spy).as_deref(), Some("disrupt_rocketry"));
}
