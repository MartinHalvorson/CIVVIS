use super::*;

const LOGISTICS: &str = "industrial_zone_logistics";

/// One city of ours with a standing Industrial Zone and `head` queued, Rocketry
/// known, and von Braun on the host's screen `missing` points away.
fn seat(head: Item, missing: f64) -> (Game, u32) {
    let mut g = Game::new_full(1, 24, 16, 91_771, 120, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| g.units[unit].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = g.player_city_ids(0)[0];
    g.players[0].techs.insert(crate::name!("rocketry"));
    g.players[0].techs.insert(crate::name!("apprenticeship"));
    let site = g.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| {
            let tile = &g.map.tiles[position];
            *position != g.cities[&city].pos
                && tile.district.is_none()
                && !g.rules.is_water(tile)
                && g.rules.is_passable(tile)
        })
        .unwrap();
    g.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("industrial_zone"));
    let held = g.cities.get_mut(&city).unwrap();
    held.districts.insert(crate::name!("industrial_zone"), site);
    held.queue = vec![head];
    offer(&mut g, "engineer", "wernher_von_braun", missing);
    (g, city)
}

fn offer(g: &mut Game, kind: &str, individual: &str, missing: f64) {
    let player = &mut g.players[0];
    player
        .live_great_person_offers
        .get_or_insert_with(Default::default)
        .insert(kind.to_string());
    player
        .live_great_person_offer_individuals
        .insert(kind.to_string(), individual.to_string());
    let cost = g.gp_cost(0, kind);
    g.players[0].gpp.insert(kind.to_string(), cost - missing);
}

fn plan() -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: 0,
        rush: false,
    }
}

fn seat_ai(gene: bool, target: VictoryTarget) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if gene {
        ai.enable_space_people_call_the_projects();
    }
    ai
}

fn scientist() -> AdvancedAi {
    seat_ai(true, VictoryTarget::Science)
}

fn monument() -> Item {
    Item::Building {
        building: crate::name!("monument"),
    }
}

fn logistics() -> Item {
    Item::Project {
        project: Name::new(LOGISTICS),
    }
}

#[test]
fn von_braun_on_offer_calls_logistics_only_with_the_gene() {
    let (mut g, city) = seat(monument(), 300.0);
    assert!(g.can_produce(0, city, &logistics()), "fixture: legal");

    seat_ai(false, VictoryTarget::Science).claim_queues_for_the_space_people(&mut g, 0, &plan());
    assert_eq!(g.cities[&city].queue.first(), Some(&monument()), "off");
    assert!(!g.players[0]
        .counters
        .contains_key("space_person_project_claims"));

    scientist().claim_queues_for_the_space_people(&mut g, 0, &plan());
    assert_eq!(g.cities[&city].queue.first(), Some(&logistics()), "on");
    assert_eq!(g.players[0].counters["space_person_project_claims"], 1);
}

#[test]
fn a_running_project_is_not_claimed_twice() {
    let (mut g, _) = seat(monument(), 300.0);
    let ai = scientist();
    ai.claim_queues_for_the_space_people(&mut g, 0, &plan());
    ai.claim_queues_for_the_space_people(&mut g, 0, &plan());
    assert_eq!(g.players[0].counters["space_person_project_claims"], 1);
}

#[test]
fn only_a_science_aim_past_rocketry_answers() {
    let (mut g, city) = seat(monument(), 300.0);
    seat_ai(true, VictoryTarget::Culture).claim_queues_for_the_space_people(&mut g, 0, &plan());
    assert_eq!(g.cities[&city].queue.first(), Some(&monument()), "culture");

    g.players[0].techs.remove(&crate::name!("rocketry"));
    scientist().claim_queues_for_the_space_people(&mut g, 0, &plan());
    assert_eq!(g.cities[&city].queue.first(), Some(&monument()), "rocketry");
}

#[test]
fn a_person_who_does_not_speed_the_race_calls_nothing() {
    let (mut g, city) = seat(monument(), 300.0);
    offer(&mut g, "engineer", "james_watt", 300.0);
    scientist().claim_queues_for_the_space_people(&mut g, 0, &plan());
    assert_eq!(g.cities[&city].queue.first(), Some(&monument()));
}

#[test]
fn a_gap_out_of_reach_calls_nothing() {
    let (mut g, city) = seat(monument(), SPACE_PERSON_MAX_MISSING + 1.0);
    scientist().claim_queues_for_the_space_people(&mut g, 0, &plan());
    assert_eq!(g.cities[&city].queue.first(), Some(&monument()));
}

#[test]
fn the_doubling_comes_before_the_lump_however_near_the_lump() {
    let (mut g, _) = seat(monument(), 500.0);
    offer(&mut g, "scientist", "carl_sagan", 50.0);
    let call = scientist().space_person_called(&g, 0, &plan()).unwrap();
    assert_eq!(call.individual, "wernher_von_braun");
}

#[test]
fn a_threatened_city_keeps_its_queue() {
    let (mut g, city) = seat(monument(), 300.0);
    let mut threatened = plan();
    threatened.threatened_city = Some(city);
    scientist().claim_queues_for_the_space_people(&mut g, 0, &threatened);
    assert_eq!(g.cities[&city].queue.first(), Some(&monument()));
}

#[test]
fn the_science_lanes_own_work_keeps_the_head() {
    for head in [
        Item::Building {
            building: crate::name!("research_lab"),
        },
        Item::Building {
            building: crate::name!("coal_power_plant"),
        },
        Item::Unit {
            unit: crate::name!("warrior"),
        },
    ] {
        let (mut g, city) = seat(head.clone(), 300.0);
        scientist().claim_queues_for_the_space_people(&mut g, 0, &plan());
        assert_eq!(g.cities[&city].queue.first(), Some(&head));
    }
}

#[test]
fn the_hold_stands_with_the_call_and_ends_with_it() {
    let (mut g, city) = seat(logistics(), 300.0);
    let ai = scientist();
    assert!(ai.space_person_project_holds(&g, 0, city, &plan(), &logistics()));
    assert!(!ai.space_person_project_holds(&g, 0, city, &plan(), &monument()));

    g.players[0].live_great_person_offer_individuals.clear();
    assert!(!ai.space_person_project_holds(&g, 0, city, &plan(), &logistics()));
}
