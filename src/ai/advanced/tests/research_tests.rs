//! Tests of `advanced/research.rs`: each calls methods of that module and no
//! other domain module's.
//!
//! Moved verbatim out of `advanced/tests.rs` (2026-11-04); a `super::` path
//! there is `super::super::` here. Shared fixtures stay in `tests.rs` and
//! arrive through `use super::*`.

use super::*;

/// `connect-the-luxury`: an owned, unimproved Plantation luxury with
/// Irrigation unknown names Irrigation; once Irrigation is in, nothing; and
/// the research step takes it ahead of the lane's beeline. Off, nothing.
#[test]
fn an_unconnected_luxury_names_its_tech_and_the_research_step_takes_it() {
    let (mut game, city, _) = empire_with_a_capital(71_118);
    let home = game.cities[&city].pos;
    let plot = game
        .nbrs(home)
        .into_iter()
        .find(|pos| {
            game.map
                .get(*pos)
                .is_some_and(|tile| tile.owner_city == Some(city) && !game.rules.is_water(tile))
        })
        .expect("fixture: an owned land plot beside the capital");
    {
        let tile = game.map.tiles.get_mut(&plot).expect("plot");
        tile.resource = Some(crate::name!("silk"));
        tile.improvement = None;
    }
    assert_eq!(
        game.rules.resources[&crate::name!("silk")].class,
        "luxury",
        "fixture: silk is a luxury"
    );
    game.players[0]
        .techs
        .retain(|tech| tech != &crate::name!("irrigation"));
    game.players[0]
        .techs
        .extend([crate::name!("pottery"), crate::name!("mining")]);
    let mut ai = AdvancedAi::new();
    assert!(!ai.connect_the_luxury, "the gene ships off");
    assert!(!AdvancedAi::legacy().connect_the_luxury);
    assert_eq!(ai.unconnected_luxury_tech(&game, 0), None, "off, nothing");
    ai.enable_connect_the_luxury();
    assert_eq!(
        ai.unconnected_luxury_tech(&game, 0),
        Some("irrigation"),
        "silk wants a Plantation, and a Plantation wants Irrigation"
    );
    let plan = StrategicPlan {
        strategy: GrandStrategy::Diplomacy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: game.turn,
        rush: false,
    };
    game.players[0].research = None;
    ai.advanced_research(&mut game, 0, &plan);
    assert_eq!(
        game.players[0].research.as_deref(),
        Some("irrigation"),
        "the research step takes the connecting tech ahead of the lane's beeline"
    );
    game.players[0].techs.insert(crate::name!("irrigation"));
    assert_eq!(
        ai.unconnected_luxury_tech(&game, 0),
        None,
        "with Irrigation in, the Plantation can take it"
    );
    ai.disable_connect_the_luxury();
    assert!(!ai.connect_the_luxury);
}

/// `boosted-bargain-first`: Mining with its Eureka in hand, a turn or two
/// from done for a pop-12 capital, is researched ahead of the Diplomacy
/// beeline; without the boost, or off, the beeline's own step stands.
#[test]
fn a_boosted_tech_two_turns_from_done_is_researched_first() {
    let (mut game, city, _) = empire_with_a_capital(71_120);
    game.cities.get_mut(&city).expect("capital").pop = 24;
    game.players[0]
        .techs
        .retain(|tech| tech != &crate::name!("mining"));
    game.players[0].research = None;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Diplomacy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::new();
    assert!(!ai.boosted_bargain_first, "the gene ships off");
    assert!(!ai.boosted_bargain_first_2, "v2 ships off too");
    assert!(
        !AdvancedAi::legacy().boosted_bargain_first
            && !AdvancedAi::legacy().boosted_bargain_first_2
    );
    game.players[0].boosted_techs.insert(crate::name!("mining"));
    let available = BasicAi::era_window_techs(&game, 0);
    assert_eq!(
        ai.boosted_bargain_tech(&game, 0, &available),
        None,
        "off, nothing"
    );
    ai.enable_boosted_bargain_first();
    assert!(ai.boosted_bargain_first);
    assert!(!ai.boosted_bargain_first_2);
    let science: f64 = game
        .player_city_ids(0)
        .into_iter()
        .map(|cid| game.city_yields(cid).science)
        .sum();
    let remaining = game.tech_cost("mining") * 0.6;
    assert!(
        remaining <= science.max(1.0) * BOOSTED_BARGAIN_TURNS,
        "fixture: boosted Mining ({remaining:.1}) is within two turns of {science:.1} science"
    );
    assert_eq!(
        ai.boosted_bargain_tech(&game, 0, &available),
        Some("mining"),
        "the boosted bargain is Mining"
    );
    ai.advanced_research(&mut game, 0, &plan);
    assert_eq!(
        game.players[0].research.as_deref(),
        Some("mining"),
        "the research step takes the bargain ahead of the beeline"
    );
    game.players[0]
        .boosted_techs
        .remove(&crate::name!("mining"));
    assert_eq!(
        ai.boosted_bargain_tech(&game, 0, &available),
        None,
        "without the Eureka there is no bargain"
    );
    ai.disable_boosted_bargain_first();
    assert!(!ai.boosted_bargain_first);
}

/// Version one reads price alone and interrupts Seasteads for boosted Mining.
/// Version two never enters while that lane goal has a legal prerequisite.
#[test]
fn boosted_bargain_v2_does_not_interrupt_a_forced_lane_goal() {
    let (mut game, city, _) = empire_with_a_capital(71_121);
    game.cities.get_mut(&city).expect("capital").pop = 24;
    game.players[0]
        .techs
        .retain(|tech| tech != &crate::name!("mining"));
    game.players[0].boosted_techs.insert(crate::name!("mining"));
    game.players[0].research = None;
    game.turn = game.max_turns / 2;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Diplomacy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: game.turn,
        rush: false,
    };

    let mut v1_game = game.clone();
    let mut v1 = AdvancedAi::new();
    v1.enable_boosted_bargain_first();
    v1.advanced_research(&mut v1_game, 0, &plan);
    assert_eq!(v1_game.players[0].research.as_deref(), Some("mining"));

    let mut v2 = AdvancedAi::new();
    v2.enable_boosted_bargain_first_2();
    v2.advanced_research(&mut game, 0, &plan);
    let selected = game.players[0].research.as_deref().expect("a lane step");
    assert_ne!(selected, "mining", "v2 must not force the side bargain");
    assert!(
        v2.tech_leads_to(&game, selected, "seasteads"),
        "the selected node must remain on the Diplomacy beeline; picked {selected}"
    );
}

/// Once no forcing contract owns research, v2 may use a one-turn Eureka to
/// break a close fallback decision. On this board Sailing is the ordinary
/// Recovery pick; boosted Animal Husbandry retains over ninety percent of its
/// value and finishes in one turn.
#[test]
fn boosted_bargain_v2_breaks_only_a_close_unforced_fallback() {
    let (mut game, city, _) = empire_with_a_capital(71_122);
    game.cities.get_mut(&city).expect("capital").pop = 60;
    game.players[0].research = None;
    game.players[0]
        .boosted_techs
        .insert(crate::name!("animal_husbandry"));
    let plan = StrategicPlan {
        strategy: GrandStrategy::Recovery,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: game.turn,
        rush: false,
    };

    let mut ordinary_game = game.clone();
    AdvancedAi::new().advanced_research(&mut ordinary_game, 0, &plan);
    assert_eq!(
        ordinary_game.players[0].research.as_deref(),
        Some("sailing"),
        "fixture: Sailing is the ordinary highest-value fallback"
    );

    let mut v2 = AdvancedAi::new();
    v2.enable_boosted_bargain_first_2();
    let available = BasicAi::era_window_techs(&game, 0);
    assert_eq!(
        v2.boosted_bargain_tech_2(
            &game,
            0,
            plan.strategy,
            &available,
            &crate::name!("sailing")
        ),
        Some(crate::name!("animal_husbandry"))
    );
    v2.advanced_research(&mut game, 0, &plan);
    assert_eq!(
        game.players[0].research.as_deref(),
        Some("animal_husbandry"),
        "the one-turn close-value Eureka breaks the fallback tie"
    );
}

/// `boosted-bargain-first-3` ships off in both constructors and, off, the
/// mechanism returns nothing on a board where it would otherwise fire.
#[test]
fn boosted_bargain_v3_ships_off_and_is_inert_off() {
    assert!(!AdvancedAi::new().boosted_bargain_first_3);
    assert!(!AdvancedAi::legacy().boosted_bargain_first_3);
    let (mut game, city, _) = empire_with_a_capital(71_123);
    game.at_war.clear();
    game.cities.get_mut(&city).expect("capital").pop = 60;
    game.players[0]
        .techs
        .retain(|tech| tech != &crate::name!("mining"));
    game.players[0].boosted_techs.insert(crate::name!("mining"));
    game.players[0].research = None;
    game.turn = game.max_turns / 2;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Diplomacy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: game.turn,
        rush: false,
    };
    let available = BasicAi::era_window_techs(&game, 0);
    let off = AdvancedAi::new();
    assert_eq!(
        off.boosted_bargain_tech_3(&game, 0, &plan, &available, None, None),
        None
    );
    let mut plain_game = game.clone();
    AdvancedAi::new().advanced_research(&mut plain_game, 0, &plan);
    let mut v3_off = AdvancedAi::new();
    v3_off.enable_boosted_bargain_first_3();
    v3_off.disable_boosted_bargain_first_3();
    v3_off.advanced_research(&mut game, 0, &plan);
    assert_eq!(
        game.players[0].research, plain_game.players[0].research,
        "enabled then disabled, the pick is the plain pick"
    );
}

/// On a peaceful board with boosted Mining one turn from done, v1 interrupts
/// the Diplomacy beeline (its pace), v2 refuses (its discipline), and v3
/// interrupts too: one turn buys a whole technology.
#[test]
fn boosted_bargain_v3_takes_the_one_turn_bargain_v1_takes_and_v2_refuses() {
    let (mut game, city, _) = empire_with_a_capital(71_124);
    game.at_war.clear();
    game.cities.get_mut(&city).expect("capital").pop = 60;
    game.players[0]
        .techs
        .retain(|tech| tech != &crate::name!("mining"));
    game.players[0].boosted_techs.insert(crate::name!("mining"));
    game.players[0].research = None;
    game.turn = game.max_turns / 2;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Diplomacy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: game.turn,
        rush: false,
    };
    let science: f64 = game
        .player_city_ids(0)
        .into_iter()
        .map(|cid| game.city_yields(cid).science)
        .sum();
    let remaining =
        game.tech_cost("mining") * (1.0 - AdvancedAi::boost_frac(&game, "mining", true));
    assert!(
        remaining <= science,
        "fixture: boosted Mining ({remaining:.0}) finishes within one turn of {science:.0} science"
    );

    let mut v1_game = game.clone();
    let mut v1 = AdvancedAi::new();
    v1.enable_boosted_bargain_first();
    v1.advanced_research(&mut v1_game, 0, &plan);
    assert_eq!(v1_game.players[0].research.as_deref(), Some("mining"));

    let mut v2_game = game.clone();
    let mut v2 = AdvancedAi::new();
    v2.enable_boosted_bargain_first_2();
    v2.advanced_research(&mut v2_game, 0, &plan);
    assert_ne!(v2_game.players[0].research.as_deref(), Some("mining"));

    let mut v3 = AdvancedAi::new();
    v3.enable_boosted_bargain_first_3();
    assert!(v3.boosted_bargain_first_3 && !v3.boosted_bargain_first && !v3.boosted_bargain_first_2);
    v3.advanced_research(&mut game, 0, &plan);
    assert_eq!(
        game.players[0].research.as_deref(),
        Some("mining"),
        "v3 interrupts the beeline for a one-turn Eureka"
    );
}

/// The v3 vetoes, each on the board of the test above: a major at war, a war
/// plan's stand-in (a threatened city), a Recovery plan, a lane target legal
/// within three turns, and a bargain two turns from done. Each returns the
/// lane's own step; lifting the veto returns the bargain.
#[test]
fn boosted_bargain_v3_refuses_under_pressure_or_when_the_lane_is_landing() {
    let (mut game, city, _) = empire_with_a_capital(71_125);
    game.at_war.clear();
    game.cities.get_mut(&city).expect("capital").pop = 60;
    game.players[0]
        .techs
        .retain(|tech| tech != &crate::name!("mining"));
    game.players[0].boosted_techs.insert(crate::name!("mining"));
    game.players[0].research = None;
    game.turn = game.max_turns / 2;
    let peaceful = StrategicPlan {
        strategy: GrandStrategy::Diplomacy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: game.turn,
        rush: false,
    };
    let available = BasicAi::era_window_techs(&game, 0);
    let mut v3 = AdvancedAi::new();
    v3.enable_boosted_bargain_first_3();
    let mining = Some(crate::name!("mining"));
    assert_eq!(
        v3.boosted_bargain_tech_3(&game, 0, &peaceful, &available, Some("seasteads"), None),
        mining,
        "fixture: the bargain stands on the peaceful board"
    );

    // A war with a living major.
    let mut war_game = game.clone();
    war_game.at_war.insert((0, 1));
    assert_eq!(
        v3.boosted_bargain_tech_3(&war_game, 0, &peaceful, &available, None, None),
        None
    );
    war_game.players[0].research = None;
    v3.advanced_research(&mut war_game, 0, &peaceful);
    assert_ne!(war_game.players[0].research.as_deref(), Some("mining"));

    // A defence posture: a threatened city.
    let threatened = StrategicPlan {
        threatened_city: Some(city),
        ..peaceful
    };
    assert_eq!(
        v3.boosted_bargain_tech_3(&game, 0, &threatened, &available, None, None),
        None
    );

    // A Recovery plan.
    let recovery = StrategicPlan {
        strategy: GrandStrategy::Recovery,
        ..peaceful
    };
    assert_eq!(
        v3.boosted_bargain_tech_3(&game, 0, &recovery, &available, None, None),
        None
    );

    // The lane target is legal and within three turns of done: the beeline
    // lands its node instead. Any cheap available node stands in for it.
    let near = available
        .iter()
        .find(|tech| tech.as_str() != "mining")
        .expect("another available node");
    let science: f64 = game
        .player_city_ids(0)
        .into_iter()
        .map(|cid| game.city_yields(cid).science)
        .sum();
    assert!(v3.beeline_step_cost(&game, 0, near.as_str(), true) <= science * 3.0);
    assert_eq!(
        v3.boosted_bargain_tech_3(&game, 0, &peaceful, &available, Some(near.as_str()), None),
        None
    );

    // Two turns from done is v1's bargain, not v3's.
    let mut slow_game = game.clone();
    slow_game.cities.get_mut(&city).expect("capital").pop = 24;
    let slow_science: f64 = slow_game
        .player_city_ids(0)
        .into_iter()
        .map(|cid| slow_game.city_yields(cid).science)
        .sum();
    let remaining =
        slow_game.tech_cost("mining") * (1.0 - AdvancedAi::boost_frac(&slow_game, "mining", true));
    assert!(
        remaining > slow_science && remaining <= slow_science * 2.0,
        "fixture: {remaining:.0} against {slow_science:.0} science is a two-turn bargain"
    );
    assert_eq!(
        v3.boosted_bargain_tech_3(&slow_game, 0, &peaceful, &available, None, None),
        None
    );
    let mut v1 = AdvancedAi::new();
    v1.enable_boosted_bargain_first();
    assert_eq!(
        v1.boosted_bargain_tech(&slow_game, 0, &available),
        Some("mining")
    );
}

#[test]
fn timed_war_research_follows_prerequisites_and_credits_progress() {
    let (mut game, _, objective) = timed_war_fixture(940_005);
    game.players[0].techs.clear();
    let goal = crate::name!("military_tactics");
    let full = AdvancedAi::war_remaining_research_cost(&game, 0, goal);
    let current = game.rules.tech_ancestors[goal.as_str()]
        .iter()
        .next()
        .cloned()
        .expect("the goal has a prerequisite path");
    game.players[0].research = Some(current.clone());
    game.players[0].research_progress = 7.0;
    assert_eq!(
        AdvancedAi::war_remaining_research_cost(&game, 0, goal),
        full - 7.0
    );

    game.players[0].research = None;
    game.players[0].research_progress = 0.0;
    let mut ai = AdvancedAi::new();
    ai.war_plan = Some(timed_plan(
        &game,
        objective,
        goal,
        crate::name!("pikeman"),
        None,
        None,
        WarPhase::Research,
    ));
    let strategic = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(objective),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    };
    ai.advanced_research(&mut game, 0, &strategic);
    let selected = game.players[0]
        .research
        .as_deref()
        .expect("the attack appointment selects a research step");
    assert!(ai.tech_leads_to(&game, selected, goal.as_str()));
}

/// The defect this repairs, reproduced from the live board that showed it.
///
/// Run `civvis-20260803T082856Z` finished turn 251 with seven cities, six
/// Campuses, six Universities and **zero Research Labs**, holding
/// Replaceable Parts (one of Chemistry's two prerequisites), Composites,
/// Plastics and Combined Arms — and not Sanitation, which unlocks only a
/// Sewer and a Medic. Chemistry was therefore never in `available_techs`,
/// so `tech_value` never scored it, so #958's 2.1-2.5x weight increase on
/// the Research Lab could not and did not change the outcome.
#[test]
fn a_university_empire_reaches_for_the_research_lab_across_a_worthless_prerequisite() {
    let mut game = Game::new(2, 32, 24, 8_801, 250, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("found city");
    let city = game.player_city_ids(0)[0];
    install_ai_test_district(&mut game, city, "campus");
    game.cities.get_mut(&city).unwrap().buildings =
        vec![crate::name!("library"), crate::name!("university")];
    // Everything Chemistry needs except Sanitation, exactly as the live
    // empire held it.
    for tech in [
        "writing",
        "education",
        "scientific_theory",
        "economics",
        "replaceable_parts",
    ] {
        game.players[0].techs.insert(Name::new(tech));
    }

    let ai = AdvancedAi::new();
    assert_eq!(
        ai.unreachable_science_building_tech(&game, 0),
        Some("chemistry"),
        "a Campus with a University and no Research Lab wants Chemistry"
    );
    assert!(
        !game
            .available_techs(0)
            .iter()
            .any(|tech| tech == "chemistry"),
        "and the argmax cannot see it: Chemistry is not an available tech"
    );
    assert!(
        ai.tech_leads_to(&game, "sanitation", "chemistry"),
        "the bridge runs through Sanitation, which unlocks only a Sewer"
    );

    // The legacy anchor must not acquire the goal.
    assert_eq!(
        AdvancedAi::legacy().unreachable_science_building_tech(&game, 0),
        None
    );
}

/// It must not send an empire shopping for a building it cannot use.
#[test]
fn an_empire_without_the_campus_chain_is_not_sent_after_chemistry() {
    let mut game = Game::new(2, 32, 24, 8_802, 250, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("found city");
    let ai = AdvancedAi::new();
    assert_eq!(
        ai.unreachable_science_building_tech(&game, 0),
        None,
        "no Campus, no goal"
    );

    let city = game.player_city_ids(0)[0];
    install_ai_test_district(&mut game, city, "campus");
    game.players[0].techs.insert(crate::name!("writing"));
    assert_eq!(
        ai.unreachable_science_building_tech(&game, 0),
        None,
        "a bare Campus has no University, so the Research Lab is not the next step"
    );
}

#[test]
fn nuclear_program_beelines_only_for_a_reachable_goal_and_stops_at_its_arsenal() {
    let (mut game, launch_city, fortress_city, spaceport_city, spaceport) = nuclear_war_fixture();
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(spaceport_city),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    };
    let goal = ai
        .nuclear_war_goal(&game, 0, GrandStrategy::Conquest, Some(1))
        .expect("the revealed launch site must be a nuclear goal");
    assert!(
        ai.nuclear_war_goal(&game, 0, GrandStrategy::Conquest, None)
            .is_some(),
        "a Domination-targeted research pass must retain the valid nuclear goal without a preselected rival"
    );

    let mut quiet = game.clone();
    quiet.players[1].science_projects.clear();
    quiet
        .cities
        .get_mut(&spaceport_city)
        .unwrap()
        .districts
        .remove(Name::new("spaceport"));
    quiet.map.tiles.get_mut(&spaceport).unwrap().district = None;
    quiet.cities.get_mut(&fortress_city).unwrap().wall_hp = 0;
    let quiet_value = ai.tech_value(&quiet, 0, "nuclear_fission", GrandStrategy::Conquest);
    let urgent_value = ai.tech_value(&game, 0, "nuclear_fission", GrandStrategy::Conquest);
    assert!(
        urgent_value > quiet_value + 20.0,
        "a revealed opponent Spaceport on an Expedition must turn Fission into a fast counter-beeline: \
             urgent={urgent_value:.1}, quiet={quiet_value:.1}"
    );
    let mut unforced = AdvancedAi::new();
    unforced.plan = Some(plan.clone());
    let planned_quiet = unforced.tech_value(&quiet, 0, "nuclear_fission", GrandStrategy::Conquest);
    let planned_urgent = unforced.tech_value(&game, 0, "nuclear_fission", GrandStrategy::Conquest);
    assert!(
        planned_urgent > planned_quiet + 20.0,
        "an ordinary AI's named Conquest target must also start the counter-program: \
             urgent={planned_urgent:.1}, quiet={planned_quiet:.1}"
    );

    let ancestors = game.rules.tech_ancestors["nuclear_fission"].clone();
    game.players[0]
        .techs
        .extend(ancestors.into_iter().map(|tech| Name::new(&tech)));
    let fission_era = game.rules.techs[&crate::name!("nuclear_fission")].era;
    let completed_eras: Vec<Name> = game
        .rules
        .techs
        .iter()
        .filter(|(_, spec)| spec.era < fission_era)
        .map(|(tech, _)| *tech)
        .collect();
    game.players[0].techs.extend(completed_eras);
    ai.advanced_research(&mut game, 0, &plan);
    assert_eq!(
        game.players[0].research.as_deref(),
        Some("nuclear_fission"),
        "the live research chooser actually starts the nuclear counter-program"
    );

    assert!(
        goal.desired_stockpile >= 2,
        "a rival on the Science clock needs an arsenal, not a lone speculative device"
    );
    assert!(
        goal.needs_thermonuclear,
        "the Spaceport outside a standard blast disk needs the larger weapon, not a cap full of ordinary devices"
    );
    game.players[0]
        .science_projects
        .insert("manhattan_project".to_string());
    let device = Item::Project {
        project: crate::name!("build_nuclear_device"),
    };
    let counts = ai.counts(&game, 0);
    let arming_value = ai.production_value(&game, 0, launch_city, &device, &plan, &counts);
    assert!(
        arming_value > 0.0,
        "the goal must open actual device production instead of refusing it: {arming_value:.1}"
    );
    let ivy = Item::Project {
        project: crate::name!("operation_ivy"),
    };
    let ivy_value = ai.production_value(&game, 0, launch_city, &ivy, &plan, &counts);
    assert!(
        ivy_value > arming_value,
        "the program prioritizes the prerequisite for the device that can actually reach the Spaceport: Ivy={ivy_value:.1}, standard device={arming_value:.1}"
    );
    game.players[0].counters.insert(
        "project_effect:nuclear_devices".to_string(),
        goal.desired_stockpile.saturating_sub(1),
    );
    game.players[0]
        .counters
        .insert("project_effect:thermonuclear_devices".to_string(), 1);
    let capped_value = ai.production_value(&game, 0, launch_city, &device, &plan, &counts);
    assert!(
        capped_value < -9_000.0,
        "once enough reachable targets are covered the AI stops hoarding devices: {capped_value:.1}"
    );
}

#[test]
fn science_lane_keeps_its_beeline_for_a_barbarian_that_cannot_strike_this_turn() {
    let mut game = Game::new_full(1, 30, 18, 7_221, 300, 0, true);
    let city = found_test_city(&mut game, 0);
    clear_barbarian_fixture(&mut game);
    let city_pos = game.cities[&city].pos;
    let barbarian = game.barb_pid.expect("the fixture has a barbarian seat");
    let outer_tile =
        game.map
            .tiles
            .keys()
            .copied()
            .find(|position| {
                game.wdist(city_pos, *position) == 3
                    && game.city_at(*position).is_none()
                    && game.unit_ids_at(*position).is_empty()
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .expect("the city needs visible outer ground");
    // A city-center sight ring is deliberately short on this fixture. Keep a
    // cheap observer beside the contact so the test proves the gate is
    // reacting to a known non-imminent raider, rather than merely ignoring
    // fog.
    let observer_tile =
        game.nbrs(outer_tile)
            .into_iter()
            .find(|position| {
                game.city_at(*position).is_none()
                    && game.unit_ids_at(*position).is_empty()
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .expect("the contact needs an observer tile");
    game.spawn_test_unit("scout", 0, observer_tile);
    let raider = game.spawn_test_unit("crossbowman", barbarian, outer_tile);
    let visible = game.player_vision_frame(0);
    assert!(
        BasicAi::barbarian_military_gap(&game, 0),
        "the fixture must still contain a real barbarian quality gap"
    );
    assert!(
        !AdvancedAi::imminent_city_attack(&game, 0, city, &visible),
        "a strong contact three tiles out must not be treated as a next-turn strike"
    );
    assert!(
        !game.attack_reach(raider).contains(&city_pos),
        "the fixture's outer contact must remain outside the attack envelope"
    );

    game.players[0].research = None;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let ai = AdvancedAi::new();
    ai.advanced_research(&mut game, 0, &plan);
    let selected = game.players[0]
        .research
        .as_deref()
        .expect("the Science seat should still choose a technology");
    assert!(
        ai.tech_leads_to(&game, selected, "rocketry"),
        "a non-imminent barbarian contact must not displace the Science beeline; selected {selected}"
    );
}

#[test]
fn adaptive_science_readiness_commits_to_the_rocketry_path() {
    let mut game = Game::new(2, 24, 16, 76_001, 300, 0);
    let ai = AdvancedAi::new();
    let rocketry_path: Vec<Name> = game
        .rules
        .techs
        .keys()
        .filter(|tech| ai.tech_leads_to(&game, tech, "rocketry"))
        .cloned()
        .collect();
    for tech in rocketry_path
        .iter()
        .filter(|tech| tech.as_str() != "rocketry")
    {
        game.players[0].techs.insert(*tech);
    }
    let rocketry_era = game.rules.techs[&crate::name!("rocketry")].era;
    let completed_eras: Vec<Name> = game
        .rules
        .techs
        .iter()
        .filter(|(_, spec)| spec.era < rocketry_era)
        .map(|(tech, _)| *tech)
        .collect();
    game.players[0].techs.extend(completed_eras);
    game.players[0].dvp = 10;

    let focus = ai.victory_focus(&game, 0);
    assert_eq!(focus.strategy, GrandStrategy::Science);
    assert!(focus.progress > 50);
    game.turn = game.max_turns / 2;

    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    ai.advanced_research(&mut game, 0, &plan);
    assert_eq!(game.players[0].research.as_deref(), Some("rocketry"));
}

#[test]
fn mature_diplomatic_plan_prepares_one_science_backup() {
    let mut game = Game::new(2, 24, 16, 76_002, 500, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    let site = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&city].pos)
        .unwrap();
    {
        let tile = game.map.tiles.get_mut(&site).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    let ai = AdvancedAi::new();
    let rocketry_path: Vec<Name> = game
        .rules
        .techs
        .keys()
        .filter(|tech| ai.tech_leads_to(&game, tech, "rocketry"))
        .cloned()
        .collect();
    for tech in rocketry_path
        .iter()
        .filter(|tech| tech.as_str() != "rocketry")
    {
        game.players[0].techs.insert(*tech);
    }
    let rocketry_era = game.rules.techs[&crate::name!("rocketry")].era;
    let completed_eras: Vec<Name> = game
        .rules
        .techs
        .iter()
        .filter(|(_, spec)| spec.era < rocketry_era)
        .map(|(tech, _)| *tech)
        .collect();
    game.players[0].techs.extend(completed_eras);
    game.turn = game.max_turns / 2 + 1;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Diplomacy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };

    assert!(ai.diplomatic_science_backup(&game, 0, &plan));
    ai.advanced_research(&mut game, 0, &plan);
    assert_eq!(game.players[0].research.as_deref(), Some("rocketry"));

    game.players[0].techs.insert(crate::name!("rocketry"));
    game.players[0].research = None;
    ai.science_production(&mut game, 0);
    assert!(matches!(
        game.cities[&city].queue.first(),
        Some(Item::District { district, .. }) if district == "spaceport"
    ));

    game.victory_conditions.science = false;
    assert!(!ai.diplomatic_science_backup(&game, 0, &plan));
}

#[test]
fn adaptive_research_routes_to_the_live_victory_plan() {
    let plan = |strategy| StrategicPlan {
        strategy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: 1,
        rush: false,
    };
    let ai = AdvancedAi::new();

    let mut science = Game::new_full(1, 20, 14, 762, 300, 0, false);
    science.turn = science.max_turns / 2;
    ai.advanced_research(&mut science, 0, &plan(GrandStrategy::Science));
    assert_eq!(
        science.players[0].research.as_deref(),
        Some("animal_husbandry"),
        "the cheapest available prerequisite toward Rocketry wins"
    );

    // Research must advance to the first unknown victory technology rather
    // than stop at a known rung while its project is being built.
    science.players[0].techs.insert(crate::name!("rocketry"));
    science.players[0]
        .science_projects
        .insert("launch_earth_satellite".to_string());
    assert_eq!(
        AdvancedAi::science_victory_tech_goal(&science, 0, GrandStrategy::Science),
        Some("satellites")
    );
    science.players[0].research = None;
    ai.advanced_research(&mut science, 0, &plan(GrandStrategy::Science));
    let next_science_tech = science.players[0]
        .research
        .as_deref()
        .expect("Science should keep researching toward Satellites");
    assert!(
        ai.tech_leads_to(&science, next_science_tech, "satellites"),
        "Science must keep its research slot on the next victory-tech path"
    );

    let mut culture = Game::new_full(1, 20, 14, 763, 300, 0, false);
    culture.turn = culture.max_turns / 2;
    ai.advanced_research(&mut culture, 0, &plan(GrandStrategy::Culture));
    assert_eq!(
        culture.players[0].research.as_deref(),
        Some("mining"),
        "the available prerequisite toward Printing wins"
    );

    let mut diplomacy = Game::new_full(1, 20, 14, 764, 300, 0, false);
    diplomacy.turn = diplomacy.max_turns / 2;
    ai.advanced_research(&mut diplomacy, 0, &plan(GrandStrategy::Diplomacy));
    let tech = diplomacy.players[0].research.as_deref().unwrap();
    assert!(
        ai.tech_leads_to(&diplomacy, tech, "seasteads"),
        "diplomatic research must advance toward Seasteads' victory point"
    );
    let civic = diplomacy.players[0].civic.as_deref().unwrap();
    assert!(
        ai.civic_leads_to(&diplomacy, civic, "global_warming_mitigation"),
        "diplomatic culture must advance toward Global Warming Mitigation's victory point"
    );
}

/// A late beeline used to make the live seat reach Advanced Flight before it
/// ever researched Bronze Working. Controllers may now work across three
/// consecutive eras, but no candidate may be farther than that rolling window
/// from the oldest unfinished era.
#[test]
fn research_keeps_a_three_era_window_before_a_later_beeline() {
    let setup = || {
        let mut game = Game::new_full(1, 20, 14, 76_004, 300, 0, false);
        let known_ancient: Vec<Name> = game
            .rules
            .techs
            .iter()
            .filter(|(tech, spec)| spec.era == 0 && tech.as_str() != "bronze_working")
            .map(|(tech, _)| *tech)
            .collect();
        game.players[0].techs.extend(known_ancient);
        // These two known prerequisites make the late Science milestone legal
        // without making Bronze Working an ancestor of it, matching the
        // beeline shape that left the Ancient side branch behind in the live
        // run.
        game.players[0].techs.extend([
            crate::name!("currency"),
            crate::name!("horseback_riding"),
            crate::name!("machinery"),
            crate::name!("radio"),
            crate::name!("chemistry"),
        ]);
        assert!(
            game.available_techs(0)
                .contains(&crate::name!("bronze_working")),
            "fixture: Bronze Working is a legal Ancient choice"
        );
        assert!(
            game.available_techs(0).contains(&crate::name!("rocketry")),
            "fixture: the late Science beeline is also legal"
        );
        assert!(
            game.available_techs(0)
                .contains(&crate::name!("apprenticeship")),
            "fixture: the third-era technology is legal"
        );
        assert!(
            game.available_techs(0).contains(&crate::name!("printing")),
            "fixture: the fourth-era technology is legal"
        );
        game
    };
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: 1,
        rush: false,
    };

    let mut basic = setup();
    let initial_window = BasicAi::era_window_techs(&basic, 0);
    assert!(
        initial_window.contains(&crate::name!("bronze_working"))
            && initial_window.contains(&crate::name!("apprenticeship")),
        "the Ancient and third-era choices share the three-era window: {initial_window:?}"
    );
    assert!(
        initial_window
            .iter()
            .all(|tech| basic.rules.techs[tech].era <= 2),
        "an unfinished Ancient era permits only Ancient through Medieval choices: {initial_window:?}"
    );
    assert!(
        !initial_window.contains(&crate::name!("printing"))
            && !initial_window.contains(&crate::name!("rocketry")),
        "a fourth or later era must stay outside the three-era window: {initial_window:?}"
    );
    BasicAi::new().research(&mut basic, 0);
    let basic_pick = Name::new(
        basic.players[0]
            .research
            .as_deref()
            .expect("a legal research pick"),
    );
    assert!(
        initial_window.contains(&basic_pick),
        "Basic AI must select only from the rolling window"
    );

    let mut advanced = setup();
    let advanced_window = BasicAi::era_window_techs(&advanced, 0);
    AdvancedAi::new().advanced_research(&mut advanced, 0, &plan);
    let advanced_pick = Name::new(
        advanced.players[0]
            .research
            .as_deref()
            .expect("a legal research pick"),
    );
    assert!(
        advanced_window.contains(&advanced_pick)
            && advanced.rules.techs[&advanced_pick].era <= 2,
        "a Science beeline must stay within the Ancient-to-Medieval window rather than leap to Rocketry"
    );

    advanced.players[0].research = None;
    advanced.players[0]
        .techs
        .insert(crate::name!("bronze_working"));
    let next_era = BasicAi::era_window_techs(&advanced, 0);
    assert!(
        !next_era.is_empty()
            && next_era
                .iter()
                .all(|tech| advanced.rules.techs[tech].era <= 3),
        "after Ancient is complete, the Classical-to-Renaissance window may open: {next_era:?}"
    );
    assert!(
        next_era.contains(&crate::name!("printing"))
            && !next_era.contains(&crate::name!("rocketry")),
        "clearing Ancient rolls the cap forward by one era but still excludes Rocketry"
    );
}

#[test]
fn targeted_science_takes_an_unlocked_milestone_outside_the_backlog_window() {
    let mut game = Game::new_full(1, 20, 14, 76_005, 300, 0, false);
    let known_ancient: Vec<Name> = game
        .rules
        .techs
        .iter()
        .filter(|(tech, spec)| spec.era == 0 && tech.as_str() != "bronze_working")
        .map(|(tech, _)| *tech)
        .collect();
    game.players[0].techs.extend(known_ancient);
    game.players[0].techs.extend([
        crate::name!("currency"),
        crate::name!("horseback_riding"),
        crate::name!("machinery"),
        crate::name!("radio"),
        crate::name!("chemistry"),
    ]);
    game.turn = game.max_turns / 2;

    assert!(
        game.available_techs(0).contains(&crate::name!("rocketry")),
        "the milestone must be legal before it can bypass the research floor"
    );
    assert!(
        !BasicAi::era_window_techs(&game, 0).contains(&crate::name!("rocketry")),
        "the Ancient backlog should still exclude Rocketry from ordinary choices"
    );

    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    ai.advanced_research(&mut game, 0, &plan);

    assert_eq!(
        game.players[0].research.as_deref(),
        Some("rocketry"),
        "an explicit Science race must not delay an already-unlocked victory milestone"
    );
}

#[test]
fn coastal_science_takes_one_era_gated_harbor_tech_package() {
    let mut game = Game::new_full(1, 20, 14, 76_003, 300, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("found city");
    let city = game.player_city_ids(0)[0];
    let coast = game
        .nbrs(game.cities[&city].pos)
        .into_iter()
        .next()
        .expect("city neighbor");
    let tile = game.map.tiles.get_mut(&coast).unwrap();
    tile.terrain = crate::name!("coast");
    tile.feature = None;
    tile.resource = None;
    tile.hills = false;
    game.world_era = 2;

    let ai = AdvancedAi::new();
    assert_eq!(
        ai.science_harbor_research_goal(&game, 0, GrandStrategy::Science),
        Some("celestial_navigation")
    );
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    ai.advanced_research(&mut game, 0, &plan);
    let selected = game.players[0]
        .research
        .as_deref()
        .expect("the Harbor package should select a prerequisite");
    assert!(
        ai.tech_leads_to(&game, selected, "celestial_navigation"),
        "the bounded support package must research Harbor prerequisites"
    );
}

#[test]
fn wide_live_opening_unlocks_a_real_government_before_the_science_beeline() {
    let mut game = Game::new_full(1, 20, 14, 765, 300, 0, false);
    game.players[0].civics.extend([
        crate::name!("code_of_laws"),
        crate::name!("craftsmanship"),
        crate::name!("foreign_trade"),
        crate::name!("early_empire"),
        crate::name!("state_workforce"),
        crate::name!("military_tradition"),
    ]);
    game.players[0].civic = None;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: PRODUCTION_CITY_TARGET_FLOOR,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut live = AdvancedAi::targeting(VictoryTarget::Science);
    live.enable_wide_map_capacity();

    live.advanced_research(&mut game, 0, &plan);

    assert_eq!(
        game.players[0].civic.as_deref(),
        Some("political_philosophy"),
        "Games and Drama must not delay the first tier-one government"
    );
}

#[test]
fn ordinary_religious_plan_routes_research_to_astrology() {
    let mut game = Game::new_full(1, 20, 14, 76_102, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Religion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };

    AdvancedAi::new().advanced_research(&mut game, 0, &plan);
    assert_eq!(game.players[0].research.as_deref(), Some("astrology"));
}

#[test]
fn enter_the_prophet_race_2_waits_for_a_feasible_commitment() {
    let mut game = Game::new_full(4, 34, 20, 76_108, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    for tech in ["animal_husbandry", "mining"] {
        game.players[0].techs.insert(Name::new(tech));
    }
    let plan = StrategicPlan {
        strategy: GrandStrategy::Diplomacy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut v2 = AdvancedAi::targeting(VictoryTarget::Diplomacy);

    assert!(!v2.enter_the_prophet_race_2, "the family member is opt-in");
    v2.enable_enter_the_prophet_race_2();
    assert!(v2.enter_the_prophet_race_2);

    assert!(v2.prophet_race_open_for(&game, 0));
    assert!(
        !v2.religious_opening_viable(&game, 0),
        "one city cannot pay a secondary lane's district commitment"
    );
    assert!(!v2.prophet_race_enterable_for(&game, 0, Some(VictoryTarget::Diplomacy)));
    v2.advanced_research(&mut game, 0, &plan);
    assert_ne!(
        game.players[0].research.as_deref(),
        Some("astrology"),
        "v2 must not charge the research fee before the production path exists"
    );

    let capital = game.player_city_ids(0)[0];
    let anchor = game.cities[&capital].pos;
    found_nearby_test_city(&mut game, 0, anchor);
    game.players[0].research = None;
    assert!(
        v2.religious_opening_viable(&game, 0),
        "two cities and a placeable Holy Site make this seat a real contender"
    );
    assert!(v2.prophet_race_enterable_for(&game, 0, Some(VictoryTarget::Diplomacy)));
    v2.advanced_research(&mut game, 0, &plan);
    assert_eq!(game.players[0].research.as_deref(), Some("astrology"));

    v2.take_turn_inner(&mut game, 0);
    assert!(v2.base.enter_prophet_race);
    assert!(v2.base.pursue_religion);

    game.players[0].religion = Some("Feasible Faith".to_string());
    v2.take_turn_inner(&mut game, 0);
    assert!(
        !v2.base.enter_prophet_race,
        "a founder no longer pays an entry fee"
    );
    assert!(
        v2.base.pursue_religion,
        "the admitted package still uses the religion it won"
    );
}

#[test]
fn skip_the_prophet_race_2_leaves_only_a_last_call_race() {
    let mut game = Game::new_full(6, 74, 46, 76_107, 250, 0, false);
    let mut cities = Vec::new();
    for pid in 0..6 {
        cities.push(found_test_city(&mut game, pid));
    }
    game.current = 0;
    for tech in ["animal_husbandry", "mining"] {
        game.players[0].techs.insert(Name::new(tech));
    }

    let open_slots = game.max_religions();
    assert!(open_slots < cities.len());
    for (rival, city) in cities.iter().copied().enumerate().skip(1).take(open_slots) {
        install_ai_test_district(&mut game, city, "holy_site");
        let rate = game
            .great_person_points_per_turn(rival)
            .get("prophet")
            .copied()
            .expect("an active Holy Site earns Prophet points");
        let cost = game.gp_cost(rival, "prophet");
        game.players[rival].gpp.insert(
            "prophet".to_string(),
            cost - rate * (PROPHET_RACE_LAST_CALL_TURNS - 1.0),
        );
    }

    let plan = StrategicPlan {
        strategy: GrandStrategy::Diplomacy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut v2 = AdvancedAi::targeting(VictoryTarget::Diplomacy);
    v2.enable_enter_the_prophet_race_2();
    v2.enable_skip_the_prophet_race_2();
    assert!(v2.skip_the_prophet_race_2);
    assert!(v2.skip_prophet_race_2_for(&game, 0, Some(VictoryTarget::Diplomacy)));
    assert!(!v2.prophet_race_enterable_for(&game, 0, Some(VictoryTarget::Diplomacy)));
    assert!(
        !v2.skip_prophet_race_2_for(&game, 0, Some(VictoryTarget::Religion)),
        "an explicit Religion lane keeps its Prophet race"
    );

    v2.advanced_research(&mut game, 0, &plan);
    assert_ne!(
        game.players[0].research.as_deref(),
        Some("astrology"),
        "the last-call gate blocks the race's research entry fee too"
    );
    v2.take_turn_inner(&mut game, 0);
    assert!(
        v2.base.skip_prophet_race,
        "the late call also closes the Holy Site reservation"
    );
    assert!(
        !v2.base.enter_prophet_race,
        "the bridge does not reopen the race after v2 withdrew"
    );
    assert!(
        !v2.base.pursue_religion,
        "the prize gate follows the same last-call decision"
    );

    // One unfilled slot leaves the non-committed seat a genuine race, so v2
    // does not repeat the unconditional withdrawal version 1 made.
    let last_rival = open_slots;
    game.players[last_rival]
        .gpp
        .insert("prophet".to_string(), 0.0);
    assert!(!v2.skip_prophet_race_2_for(&game, 0, Some(VictoryTarget::Diplomacy)));
    // `enter-the-prophet-race-2` admits the race only from two cities with a
    // placeable Holy Site; give the seat its second city and the race is on.
    let anchor = game.cities[&cities[0]].pos;
    found_nearby_test_city(&mut game, 0, anchor);
    assert!(
        v2.religious_opening_viable(&game, 0),
        "two cities and a placeable Holy Site admit the seat"
    );
    assert!(!v2.skip_prophet_race_2_for(&game, 0, Some(VictoryTarget::Diplomacy)));
    assert!(v2.prophet_race_enterable_for(&game, 0, Some(VictoryTarget::Diplomacy)));
}

#[test]
fn an_explicit_science_seat_stays_out_of_the_prophet_race() {
    let mut game = Game::new_full(4, 30, 18, 76_106, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    game.players[0].research = None;
    game.players[0].techs.clear();
    for tech in ["animal_husbandry", "mining"] {
        game.players[0].techs.insert(Name::new(tech));
    }

    let mut treated = AdvancedAi::targeting(VictoryTarget::Science);
    treated.enable_enter_the_prophet_race_2();

    assert!(!treated.prophet_race_enabled_for(Some(VictoryTarget::Science)));
    treated.advanced_research(&mut game, 0, &plan);
    assert_ne!(
        game.players[0].research.as_deref(),
        Some("astrology"),
        "an explicit Science lane must not take the Prophet dead end"
    );
}

/// Over a whole six-player game the commonest reason an upgrade was
/// unavailable was not distance up the tree but a node its owner had
/// simply never taken: Archery, Iron Working and Machinery accounted for
/// most of it, with empires holding ten to thirty technologies still
/// fielding the Slingers and Warriors those nodes retire.
#[test]
fn research_prices_the_node_that_retires_the_army_already_in_the_field() {
    let mut g = Game::new(2, 24, 16, 21, 80, 0);
    let ai = AdvancedAi::new();
    let home = g.units[&g.player_unit_ids(0)[0]].pos;
    let slingers = |g: &Game| {
        g.units
            .values()
            .filter(|unit| unit.owner == 0 && unit.kind == "slinger")
            .count()
    };
    let raise = |g: &mut Game, wanted: usize| {
        let before = slingers(g);
        for pos in g.wdisk(home, 4) {
            if slingers(g) >= before + wanted {
                break;
            }
            g.spawn_test_unit("slinger", 0, pos);
        }
        slingers(g) - before
    };

    // Measure the second batch against the first: the other terms that
    // read the empire's units settle after the first Slinger, so only the
    // waiting upgrades separate the two readings.
    let grown = raise(&mut g, 3);
    assert!(grown >= 3, "test needs a garrison, spawned {grown}");
    let three = ai.tech_value(&g, 0, "archery", GrandStrategy::Science);
    let grown = raise(&mut g, 3);
    assert!(grown >= 3, "test needs a bigger garrison, spawned {grown}");
    let six = ai.tech_value(&g, 0, "archery", GrandStrategy::Science);

    // `tech_value` divides by the square root of the technology's cost,
    // so three more Slingers waiting on Archery (15 to 25 apiece, at the
    // peacetime rate) read as a few points here, not forty.
    assert!(six > three + 4.0, "three={three} six={six}");
    // A node that retires nothing this empire owns gains nothing.
    assert!(
        ai.tech_value(&g, 0, "iron_working", GrandStrategy::Science) < six,
        "no unit here upgrades through Iron Working"
    );
}

#[test]
fn temp_seed_search_researchlane() {
    std::panic::set_hook(Box::new(|_| {}));
    let mut good = Vec::new();
    for seed in 91_440..91_600u64 {
        std::env::set_var("CIVVIS_FIXTURE_SEED_RESEARCHLANE", seed.to_string());
        if std::panic::catch_unwind(advanced_research_protects_a_science_lane_from_a_nearby_barbarian_quality_gap).is_ok() {
            good.push(seed);
        }
    }
    let _ = std::panic::take_hook();
    panic!("TEMPSEEDS researchlane {good:?}");
}

#[test]
fn advanced_research_protects_a_science_lane_from_a_nearby_barbarian_quality_gap() {
    let mut game = Game::new_full(
        1,
        24,
        16,
        crate::rng::fixture_seed("RESEARCHLANE", 91_503),
        120,
        0,
        true,
    );
    game.current = 0;
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    clear_barbarian_fixture(&mut game);
    let city = game.player_city_ids(0)[0];
    let center = game.cities[&city].pos;
    let barbarian = game.barb_pid.expect("the fixture has a barbarian seat");
    let raider_position = game
        .nbrs(center)
        .into_iter()
        .find(|position| g_is_open_land(&game, *position) && game.units_at(*position).is_empty())
        .expect("the capital has an open neighboring tile");
    game.spawn_test_unit("crossbowman", barbarian, raider_position);
    for _ in 0..3 {
        game.spawn_test_unit("archer", 0, center);
    }
    game.players[0].research = None;

    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: game.turn,
        rush: false,
    };
    ai.advanced_research(&mut game, 0, &plan);

    let selected = game.players[0]
        .research
        .as_deref()
        .expect("the research slot should be filled");
    assert!(
        ai.tech_leads_to(&game, selected, "machinery"),
        "Science must take the cheapest step toward Machinery, not a victory detour; selected {selected}"
    );
}

#[test]
fn a_religious_basil_commits_to_conquest_and_divine_right() {
    let mut game = Game::new_full(2, 24, 16, 111, 200, 0, false);
    game.players[0].civ = "Byzantium".to_string();
    game.players[0].religion = Some("Eastern Orthodoxy".to_string());
    game.record_contact(0, 1);
    let ai = AdvancedAi::new();
    let plan = ai.assess(&game, 0);
    assert_eq!(plan.strategy, GrandStrategy::Conquest);
    assert_eq!(plan.target_player, Some(1));

    ai.advanced_research(&mut game, 0, &plan);
    let civic = game.players[0].civic.as_deref().unwrap();
    assert!(
        ai.civic_leads_to(&game, civic, "divine_right"),
        "{civic} should be on the direct Divine Right beeline"
    );
}

/// The beeline is the point of the appointment: while it runs, research is
/// forced onto the cheapest legal step toward Advanced Flight instead of the
/// argmax the lane would otherwise take.
#[test]
fn the_beeline_forces_research_along_the_chain() {
    let (mut game, _, _) = air_surge_fixture(941_104);
    // This unit test isolates the air-surge chain itself. The ordinary
    // controller now clears an older era before it opens the next one, so set
    // up the same mature research position rather than letting an unrelated
    // Ancient branch preempt Flight.
    let flight_era = game.rules.techs[&crate::name!("flight")].era;
    let completed_eras: Vec<Name> = game
        .rules
        .techs
        .iter()
        .filter(|(_, spec)| spec.era < flight_era)
        .map(|(tech, _)| *tech)
        .collect();
    game.players[0].techs.extend(completed_eras);
    let mut ai = air_surge_ai();
    ai.maintain_air_surge(&game, 0);
    assert_eq!(
        ai.air_surge_research_goal(&game, 0),
        Some(air_surge::AIR_SURGE_GOAL_TECH)
    );

    game.players[0].research = None;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    };
    ai.advanced_research(&mut game, 0, &plan);
    assert_eq!(
        game.players[0].research.as_deref(),
        Some("flight"),
        "the first step of `flight -> radio -> advanced_flight` is the only \
         available one"
    );

    // The goal retires the moment the breakthrough lands, so the appointment
    // never holds the research slot past its own purpose.
    air_surge_arm(&mut game);
    ai.maintain_air_surge(&game, 0);
    assert!(ai.air_surge_research_goal(&game, 0).is_none());
}

#[test]
fn a_modernized_ground_army_finishes_the_appointed_air_surge() {
    let (mut game, _, _) = air_surge_fixture(941_154);
    game.at_war.insert((0, 1));
    game.at_war.insert((1, 0));
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    for _ in 0..2 {
        game.spawn_test_unit("artillery", 0, home);
    }
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_air_surge_2();
    ai.maintain_air_surge(&game, 0);
    assert!(ai.air_surge_active());
    assert!(
        ai.wartime_modernization_tech(&game, 0).is_some(),
        "a second land upgrade must actually compete for research"
    );
    game.players[0].techs.insert(crate::name!("composites"));
    assert!(ai.wartime_modernization_tech(&game, 0).is_some());
    assert_eq!(
        ai.air_surge_research_goal(&game, 0),
        Some(air_surge::AIR_SURGE_GOAL_TECH),
        "once the tank line is modernized, another land upgrade cannot indefinitely postpone the wing"
    );
}

/// The blind spot: a rival that has researched the whole chain to Rocketry and
/// launched nothing reads zero on the shipped ladder.
#[test]
fn a_rival_at_the_end_of_the_chain_reads_no_science_threat_at_all() {
    let mut game = Game::new(3, 30, 20, 9_101, 250, 0);
    game.found_city_for(1, (12, 10), None);
    let chain: Vec<Name> = game
        .rules
        .techs
        .keys()
        .filter(|tech| AdvancedAi::new().tech_leads_to(&game, tech, "rocketry"))
        .map(|tech| Name::new(tech.as_str()))
        .collect();
    assert!(!chain.is_empty(), "fixture: the chain exists");
    for tech in &chain {
        game.players[1].techs.insert(*tech);
    }
    assert!(
        !game.players[1]
            .science_projects
            .contains("launch_earth_satellite"),
        "fixture: nothing launched"
    );

    let shipped = AdvancedAi::new();
    let mut ai = AdvancedAi::new();
    ai.enable_science_chain_alarm();

    // The shipped ladder scores the whole chain as nothing, so Science is not
    // even this rival's named race.
    assert_ne!(
        shipped.rival_pressure(&game, 1).0,
        GrandStrategy::Science,
        "the shipped model does not see it"
    );
    let (lane, progress) = ai.rival_pressure(&game, 1);
    assert_eq!(lane, GrandStrategy::Science, "the gene does");
    assert!(
        progress >= 40,
        "the whole chain is worth the whole ramp ({progress})"
    );
}

/// The ladder supplies the goal the civic chooser is missing, and stops as
/// soon as any tier-2 gate is owned. See `AdvancedAi::government_ladder`.
#[test]
fn the_government_ladder_aims_at_the_tier_two_civic_its_lane_would_adopt() {
    let (mut game, _capital, _home) = empire_with_a_capital(71_401);
    let mut ai = AdvancedAi::new();
    ai.enable_government_ladder();

    // Rung one is not owned yet, so the ladder has nothing to climb and the
    // shipped `first_government` arm keeps the field to itself.
    assert_eq!(
        ai.government_ladder_goal(&game, 0, GrandStrategy::Expansion),
        None,
        "before Political Philosophy the ladder stands down"
    );

    game.players[0]
        .civics
        .insert(crate::name!("political_philosophy"));

    // Every lane but Conquest and Religion takes the government
    // `strategic_government` ranks first among the three: Merchant Republic.
    assert_eq!(
        ai.government_ladder_goal(&game, 0, GrandStrategy::Expansion),
        Some("exploration"),
        "the economic lanes climb toward Merchant Republic"
    );
    assert_eq!(
        ai.government_ladder_goal(&game, 0, GrandStrategy::Science),
        Some("exploration"),
        "so does Science"
    );
    assert_eq!(
        ai.government_ladder_goal(&game, 0, GrandStrategy::Conquest),
        Some("divine_right"),
        "Conquest climbs toward Monarchy, which its own priority list prefers"
    );
    assert_eq!(
        ai.government_ladder_goal(&game, 0, GrandStrategy::Religion),
        Some("reformed_church"),
        "Religion climbs toward Theocracy"
    );

    // A lateral move between two six-slot governments is not worth a second
    // 340-to-440-cost detour, so one tier-2 gate closes the ladder.
    game.players[0].civics.insert(crate::name!("divine_right"));
    assert_eq!(
        ai.government_ladder_goal(&game, 0, GrandStrategy::Science),
        None,
        "a tier-2 gate already owned ends the climb"
    );

    // And the climb does not start in the back half of the game, where a
    // policy slot has almost nothing left to pay back.
    game.players[0].civics.remove(&crate::name!("divine_right"));
    game.turn = game.max_turns / 2 + 1;
    assert_eq!(
        ai.government_ladder_goal(&game, 0, GrandStrategy::Science),
        None,
        "past the halfway turn the detour is the cost the shipped ratio avoids"
    );

    // Off, it is an exact no-op.
    game.turn = 1;
    ai.disable_government_ladder();
    assert_eq!(
        ai.government_ladder_goal(&game, 0, GrandStrategy::Science),
        None,
        "the gene is the whole behaviour"
    );
}

/// Version two of the ladder climbs past tier two, and keeps climbing past
/// the half-clock while the field out-slots the empire. See
/// `AdvancedAi::government_ladder_rung`.
#[test]
fn the_second_government_ladder_climbs_until_the_slots_are_level() {
    let (mut game, _capital, _home) = empire_with_a_capital(71_503);
    let mut ai = AdvancedAi::new();
    ai.enable_government_ladder_2();

    assert_eq!(
        ai.government_ladder_goal(&game, 0, GrandStrategy::Diplomacy),
        None,
        "before Political Philosophy the ladder stands down, as in version one"
    );
    game.players[0]
        .civics
        .insert(crate::name!("political_philosophy"));
    game.players[0].government = Some("classical_republic".to_string());

    // The cheapest gate that raises capacity above the current four slots.
    let first = ai.government_ladder_goal(&game, 0, GrandStrategy::Diplomacy);
    assert!(
        first.is_some(),
        "a four-slot government has somewhere to climb to"
    );
    let first = first.unwrap();
    assert!(
        game.rules
            .governments
            .iter()
            .any(|(name, spec)| spec.civic.as_deref() == Some(first)
                && name.as_str() != "classical_republic"),
        "the goal is some government's own gate civic"
    );

    // ★ The difference from version one: owning that gate does NOT end the
    // climb while a richer government is still out of reach.
    game.players[0].civics.insert(Name::new(first));
    game.players[0].government = Some("merchant_republic".to_string());
    let second = ai.government_ladder_goal(&game, 0, GrandStrategy::Diplomacy);
    assert!(
        second.is_some() && second != Some(first),
        "version one retired here; version two takes the next rung: {second:?}"
    );

    // Past the half clock, level with the field, the shipped window stands.
    game.turn = game.max_turns / 2 + 5;
    game.players[1].government = Some("merchant_republic".to_string());
    assert_eq!(
        ai.government_ladder_goal(&game, 0, GrandStrategy::Diplomacy),
        None,
        "level on slots, the climb stops at the shipped half-clock window"
    );

    // Behind the field on slots, it keeps climbing to three quarters.
    game.players[1].government = Some("digital_democracy".to_string());
    assert!(
        ai.government_ladder_goal(&game, 0, GrandStrategy::Diplomacy)
            .is_some(),
        "a rival with more slots holds the window open"
    );
    game.turn = (game.max_turns as f64 * 0.8) as u32;
    assert_eq!(
        ai.government_ladder_goal(&game, 0, GrandStrategy::Diplomacy),
        None,
        "and it still closes at three quarters"
    );

    // Off, it is an exact no-op.
    ai.disable_government_ladder_2();
    game.turn = 10;
    assert_eq!(
        ai.government_ladder_goal(&game, 0, GrandStrategy::Diplomacy),
        None,
        "with neither version on, the ladder is silent"
    );
}

/// Live Rome t80 had ten Warriors, Education and Apprenticeship, but neither
/// Bronze Working nor Iron Working. The Science lane bypassed the upgrade
/// score because the scored technology was not yet a legal research choice.
#[test]
fn wartime_modernization_routes_science_through_missing_prerequisites() {
    let mut g = Game::new(2, 24, 16, 21, 250, 0);
    g.players[0].civ = "Rome".to_string();
    let home = g.units[&g.player_unit_ids(0)[0]].pos;
    g.units.retain(|_, unit| unit.owner != 0);
    for _ in 0..10 {
        g.spawn_test_unit("warrior", 0, home);
    }
    for tech in [
        "pottery",
        "mining",
        "writing",
        "currency",
        "apprenticeship",
        "education",
    ] {
        g.players[0].techs.insert(Name::new(tech));
    }
    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    assert_eq!(ai.wartime_modernization_tech(&g, 0), None);
    g.at_war.insert((0, 1));
    assert_eq!(
        ai.wartime_modernization_tech(&g, 0).as_deref(),
        Some("iron_working")
    );
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: g.turn,
        rush: false,
    };
    g.players[0].research = None;
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("bronze_working"));
    g.players[0].techs.insert(crate::name!("bronze_working"));
    g.players[0].research = None;
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("iron_working"));
    g.players[0].techs.insert(crate::name!("iron_working"));
    assert_eq!(
        ai.wartime_modernization_tech(&g, 0),
        None,
        "known upgrades must release the Science lane even when Gold is missing"
    );
    g.players[0].research = None;
    ai.advanced_research(&mut g, 0, &plan);
    assert_ne!(g.players[0].research.as_deref(), Some("iron_working"));
}

#[test]
fn wartime_modernization_ignores_single_stragglers_and_city_state_wars() {
    let mut g = Game::new(2, 24, 16, 21, 250, 0);
    let home = g.units[&g.player_unit_ids(0)[0]].pos;
    g.units.retain(|_, unit| unit.owner != 0);
    g.spawn_test_unit("archer", 0, home);
    g.at_war.insert((0, 1));
    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    assert_eq!(ai.wartime_modernization_tech(&g, 0), None);
    g.spawn_test_unit("archer", 0, home);
    assert_eq!(
        ai.wartime_modernization_tech(&g, 0).as_deref(),
        Some("machinery")
    );
    g.players[1].is_minor = true;
    assert_eq!(ai.wartime_modernization_tech(&g, 0), None);
}

#[test]
fn culture_unlocks_bands_before_the_late_government_ladder() {
    let (mut game, _, _) = empire_with_a_capital(79_174);
    game.at_war.clear();
    game.turn = 174;
    game.max_turns = 250;
    let withheld = [
        "cold_war",
        "professional_sports",
        "cultural_heritage",
        "space_race",
        "environmentalism",
        "social_media",
        "corporate_libertarianism",
        "digital_democracy",
        "synthetic_technocracy",
    ];
    game.players[0].civics = game
        .rules
        .civics
        .keys()
        .copied()
        .filter(|c| !withheld.contains(&c.as_str()))
        .collect();
    game.players[0].government = Some("democracy".into());
    game.players[1].government = Some("corporate_libertarianism".into());
    game.players[0].civic = None;
    game.players[0]
        .boosted_civics
        .insert(crate::name!("professional_sports"));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_government_ladder_2();
    ai.government_capacity_fallback = true;
    ai.enable_culture_cold_war_window();
    ai.chase_every_boost = true;
    let mut plan = StrategicPlan {
        strategy: GrandStrategy::Culture,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: game.turn,
        rush: false,
    };
    assert_eq!(ai.culture_civic_goal(&game, 0), Some("cold_war"));
    assert!(ai
        .government_ladder_goal(&game, 0, GrandStrategy::Culture)
        .is_some());
    let mut parks = game.clone();
    parks.players[0]
        .civics
        .remove(&crate::name!("conservation"));
    ai.advanced_research(&mut parks, 0, &plan);
    assert_eq!(parks.players[0].civic.as_deref(), Some("conservation"));
    parks.players[0].civic = None;
    ai.culture_cold_war_window = false;
    ai.advanced_research(&mut parks, 0, &plan);
    assert_eq!(
        parks.players[0].civic.as_deref(),
        Some("professional_sports")
    );
    ai.enable_culture_cold_war_window();
    let mut culture = game.clone();
    ai.advanced_research(&mut culture, 0, &plan);
    assert_eq!(culture.players[0].civic.as_deref(), Some("cold_war"));
    culture.players[0].civics.insert(crate::name!("cold_war"));
    culture.players[0].civic = None;
    ai.advanced_research(&mut culture, 0, &plan);
    assert_eq!(
        culture.players[0].civic.as_deref(),
        Some("professional_sports")
    );
    plan.strategy = GrandStrategy::Recovery;
    let mut recovery = game.clone();
    ai.advanced_research(&mut recovery, 0, &plan);
    assert_eq!(recovery.players[0].civic.as_deref(), Some("cold_war"));
    plan.strategy = GrandStrategy::Culture;
    ai.culture_cold_war_window = false;
    ai.advanced_research(&mut game, 0, &plan);
    assert_eq!(
        game.players[0].civic.as_deref(),
        Some("professional_sports")
    );
}

#[test]
fn assigned_culture_unlocks_faith_units_through_counterplans_after_six_slots() {
    let (mut game, _, _) = empire_with_a_capital(79_175);
    game.at_war.clear();
    game.turn = 150;
    game.max_turns = 500;
    game.game_speed = crate::setup::GameSpeed::Online;
    let withheld = [
        "cold_war",
        "professional_sports",
        "cultural_heritage",
        "space_race",
        "environmentalism",
        "social_media",
        "suffrage",
        "class_struggle",
        "totalitarianism",
        "corporate_libertarianism",
        "digital_democracy",
        "synthetic_technocracy",
    ];
    game.players[0].civics = game
        .rules
        .civics
        .keys()
        .copied()
        .filter(|civic| !withheld.contains(&civic.as_str()))
        .collect();
    game.players[0].government = Some("monarchy".into());
    game.players[0].civic = None;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_government_ladder_2();
    ai.government_capacity_fallback = false;
    ai.enable_culture_cold_war_window();
    assert!(ai
        .government_ladder_goal(&game, 0, GrandStrategy::Culture)
        .is_some());
    let mut plan = StrategicPlan {
        strategy: GrandStrategy::Recovery,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: game.turn,
        rush: false,
    };
    for posture in [
        GrandStrategy::Recovery,
        GrandStrategy::Science,
        GrandStrategy::Expansion,
    ] {
        plan.strategy = posture;
        let mut bands = game.clone();
        ai.advanced_research(&mut bands, 0, &plan);
        assert_eq!(
            bands.players[0].civic.as_deref(),
            Some("cold_war"),
            "{posture:?}"
        );
        let mut parks = game.clone();
        parks.players[0]
            .civics
            .remove(&crate::name!("conservation"));
        ai.advanced_research(&mut parks, 0, &plan);
        assert_eq!(
            parks.players[0].civic.as_deref(),
            Some("conservation"),
            "{posture:?}"
        );
    }
    let mut unlocked = game.clone();
    unlocked.players[0].civics.insert(crate::name!("cold_war"));
    ai.advanced_research(&mut unlocked, 0, &plan);
    assert_eq!(
        unlocked.players[0].civic.as_deref(),
        Some("suffrage"),
        "resume government capacity after the Culture unlocks"
    );
    let mut opening = game.clone();
    opening.players[0].government = Some("classical_republic".into());
    ai.advanced_research(&mut opening, 0, &plan);
    assert_ne!(
        opening.players[0].civic.as_deref(),
        Some("cold_war"),
        "fewer than six slots retain the capacity priority"
    );
    ai.disable_culture_cold_war_window();
    let mut off = game;
    ai.advanced_research(&mut off, 0, &plan);
    assert_ne!(
        off.players[0].civic.as_deref(),
        Some("cold_war"),
        "the disabled policy retains its old ordering"
    );
}

/// See `advanced/faith_veto.rs`: a faithless Domination seat enters the
/// Prophet race once a rival faith holds two of four majors (the
/// early-warning bar) and a slot is open, and only under the gene.
#[test]
fn a_rival_faith_at_the_bar_opens_the_prophet_race_for_a_faithless_conqueror() {
    let mut game = Game::new_full(4, 34, 20, 76_108, 120, 0, false);
    for pid in 0..3 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        let at = game.units[&settler].pos;
        game.remove_unit(settler);
        game.found_city_for(pid, at, None);
    }
    for tech in ["animal_husbandry", "mining"] {
        game.players[0].techs.insert(Name::new(tech));
    }
    let capital = game.player_city_ids(0)[0];
    let anchor = game.cities[&capital].pos;
    found_nearby_test_city(&mut game, 0, anchor);
    let faith = "Rival Faith".to_string();
    game.players[1].religion = Some(faith.clone());
    for pid in [1, 2] {
        for city in game.player_city_ids(pid) {
            game.cities
                .get_mut(&city)
                .unwrap()
                .pressure
                .insert(faith.clone(), 5_000.0);
        }
    }
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(!ai.faith_veto_due(&game, 0), "off");
    ai.enable_found_against_a_rival_faith();
    assert!(ai.faith_veto_due(&game, 0));
    assert!(ai.prophet_race_enterable_for(&game, 0, Some(VictoryTarget::Domination)));
    ai.advanced_research(&mut game, 0, &plan);
    assert_eq!(game.players[0].research.as_deref(), Some("astrology"));
    // No rival faith at the bar: no race.
    let mut quiet = game.clone();
    for city in quiet.player_city_ids(2) {
        quiet.cities.get_mut(&city).unwrap().pressure.clear();
    }
    assert!(!ai.faith_veto_due(&quiet, 0));
}
