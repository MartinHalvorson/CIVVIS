use super::super::StrategicPlan;
use super::*;
use crate::{
    ai::BasicAi,
    game::{Game, LiveGreatPersonActivationNeed, ObservedPublicEmpireStats},
};

pub(super) fn found_capitals(game: &mut Game) {
    let majors: Vec<_> = game
        .players
        .iter()
        .filter(|player| !player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .collect();
    for pid in majors {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("every major starts with a settler");
        game.found_city_for(pid, game.units[&settler].pos, None);
    }
    game.current = 0;
}

fn science_plan(turn: u32) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: turn,
        rush: false,
    }
}

pub(super) fn open_land_near(game: &Game, center: crate::Pos, radius: i32) -> crate::Pos {
    game.wdisk(center, radius)
        .into_iter()
        .find(|position| {
            *position != center
                && game.city_at(*position).is_none()
                && game
                    .map
                    .get(*position)
                    .is_some_and(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile))
        })
        .expect("the fixture has a nearby open land tile")
}

/// `beeline-orders-by-value`: the Science beeline walks Rocketry's
/// prerequisites by `tech_value`, not by printed price with ties by name.
/// Off, the three 50-cost Ancient steps go archery, sailing, writing —
/// alphabetical — whatever they unlock; on, the step is the value argmax
/// over the same chain steps, and the goal is unchanged.
#[test]
fn beeline_orders_by_value_walks_the_chain_by_value() {
    let mut game = Game::new_full(1, 24, 16, 91_007, 300, 0, false);
    found_capitals(&mut game);
    game.turn = 8;
    for tech in ["pottery", "animal_husbandry", "mining"] {
        game.players[0].techs.insert(crate::name::Name::new(tech));
    }
    let plan = science_plan(game.turn);
    let steps: Vec<crate::name::Name> = crate::ai::BasicAi::era_window_techs(&game, 0)
        .into_iter()
        .filter(|tech| AdvancedAi::new().tech_leads_to(&game, tech, "rocketry"))
        .collect();
    assert!(
        steps.len() >= 3,
        "the fixture offers several chain steps: {steps:?}"
    );

    let off = AdvancedAi::targeting(VictoryTarget::Science);
    assert!(!off.beeline_orders_by_value, "the gene ships off");
    let mut off_game = game.clone();
    off_game.players[0].research = None;
    off.advanced_research(&mut off_game, 0, &plan);
    let cheapest = steps
        .iter()
        .min_by(|a, b| {
            off_game.rules.techs[a.as_str()]
                .cost
                .total_cmp(&off_game.rules.techs[b.as_str()].cost)
                .then(a.cmp(b))
        })
        .copied();
    assert_eq!(
        off_game.players[0].research.as_deref(),
        cheapest.as_ref().map(|tech| tech.as_str()),
        "off, the beeline takes the cheapest step, ties by name"
    );

    let mut on = AdvancedAi::targeting(VictoryTarget::Science);
    on.enable_beeline_orders_by_value();
    let best = steps
        .iter()
        .map(|tech| (on.tech_value(&game, 0, tech.as_str(), plan.strategy), *tech))
        .max_by(|a, b| a.0.total_cmp(&b.0).then_with(|| b.1.cmp(&a.1)))
        .map(|(_, tech)| tech);
    let mut on_game = game.clone();
    on_game.players[0].research = None;
    on.advanced_research(&mut on_game, 0, &plan);
    assert_eq!(
        on_game.players[0].research.as_deref(),
        best.as_ref().map(|tech| tech.as_str()),
        "on, the best-valued chain step"
    );
    assert_ne!(
        best, cheapest,
        "the fixture must separate the two orders, or it proves nothing"
    );
    assert_eq!(
        AdvancedAi::science_victory_tech_goal(&on_game, 0, GrandStrategy::Science),
        AdvancedAi::science_victory_tech_goal(&off_game, 0, GrandStrategy::Science),
        "the goal itself is the same either way"
    );
    on.disable_beeline_orders_by_value();
    assert!(!on.beeline_orders_by_value);
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.tag == "beeline-orders-by-value" && gene.opt_in()));
}

#[test]
fn science_target_backfills_an_unfinished_ancient_tech_before_rocketry() {
    let mut game = Game::new_full(1, 24, 16, 91_001, 300, 0, false);
    found_capitals(&mut game);
    game.turn = game.max_turns / 2;

    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    let techs: Vec<_> = game
        .rules
        .techs
        .keys()
        .filter(|tech| ai.tech_leads_to(&game, tech, "rocketry"))
        .copied()
        .collect();
    game.players[0]
        .techs
        .extend(techs.into_iter().filter(|tech| tech.as_str() != "rocketry"));
    game.players[0].techs.insert(crate::name!("mining"));
    game.players[0]
        .live_great_person_activation_needs
        .push(LiveGreatPersonActivationNeed {
            kind: "general".to_string(),
            individual: None,
            required_district: None,
            ..LiveGreatPersonActivationNeed::default()
        });
    assert_eq!(
        BasicAi::live_great_person_tech_goal(&game, 0).as_deref(),
        Some("bronze_working"),
        "the fixture needs an unrelated live Great Person technology goal"
    );

    let plan = science_plan(game.turn);
    ai.advanced_research(&mut game, 0, &plan);

    // Any unfinished Ancient node is the backfill. This used to name
    // Bronze Working, which won the scorer only because its lookahead
    // counted other civilizations' Iron Working uniques (the Kongo Shield
    // Bearer, Hypaspist and Toa); counted honestly, Irrigation edges it.
    let picked = game.players[0].research.clone().expect("a research pick");
    assert!(
            picked != "rocketry" && game.rules.techs[picked.as_str()].era == 0,
            "an explicit Science target must clear an unfinished Ancient era before its Rocketry beeline, not {picked}"
        );
}

#[test]
fn domination_capital_focus_is_an_independently_reversible_opt_in() {
    assert!(!AdvancedAi::new().domination_capital_focus);
    assert!(!AdvancedAi::legacy().domination_capital_focus);
    let mut ai = AdvancedAi::new();
    ai.enable_domination_capital_focus();
    assert!(ai.domination_capital_focus);
    ai.disable_domination_capital_focus();
    assert!(!ai.domination_capital_focus);
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.tag == "domination-capital-focus" && gene.opt_in()));
}

#[test]
fn domination_target_aims_at_an_uncontrolled_original_capital_before_a_convenient_city() {
    check_domination_front_capital(2);
}

#[test]
fn domination_target_keeps_the_active_fronts_capital_when_another_is_cheaper() {
    check_domination_front_capital(3);
}

fn check_domination_front_capital(majors: usize) {
    let mut game = Game::new_full(majors, 48, 28, 91_002, 300, 0, false);
    found_capitals(&mut game);
    game.turn = 200;
    game.record_contact(0, 1);
    let capital = game.player_city_ids(1)[0];
    let capital_pos = game.cities[&capital].pos;
    let outpost = game.found_city_for(1, open_land_near(&game, capital_pos, 4), None);
    game.cities.get_mut(&capital).unwrap().hp = 200;
    game.cities.get_mut(&capital).unwrap().wall_hp = 400;
    game.cities.get_mut(&capital).unwrap().buildings.extend([
        crate::name!("walls"),
        crate::name!("medieval_walls"),
        crate::name!("renaissance_walls"),
    ]);
    for _ in 0..3 {
        game.spawn_test_unit("giant_death_robot", 1, capital_pos);
    }
    game.cities.get_mut(&outpost).unwrap().hp = 25;
    game.cities.get_mut(&outpost).unwrap().wall_hp = 0;
    game.cities.get_mut(&outpost).unwrap().pop = 14;
    if majors == 3 {
        // A nearby friendly population base makes the capital holdable;
        // the rich enemy outpost remains the generic scorer's bargain.
        let support = game.found_city_for(0, open_land_near(&game, capital_pos, 2), None);
        game.cities.get_mut(&support).unwrap().pop = 30;
        let home = game.cities[&game.player_city_ids(0)[0]].pos;
        for _ in 0..6 {
            game.spawn_test_unit("giant_death_robot", 0, home);
        }
    }
    let _capital_observer = game.spawn_test_unit("scout", 0, capital_pos);
    let _outpost_observer = game.spawn_test_unit("scout", 0, game.cities[&outpost].pos);

    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.belief.observe(&game, 0);
    let outpost_value =
        ai.campaign_city_value(&game, 0, &game.cities[&outpost], GrandStrategy::Conquest);
    let capital_value =
        ai.campaign_city_value(&game, 0, &game.cities[&capital], GrandStrategy::Conquest);
    assert!(
        outpost_value < capital_value,
        "the fixture needs the generic scorer to prefer the convenient city: \
             outpost={outpost_value}, capital={capital_value}"
    );
    assert!(
        !AdvancedAi::should_defer_city_capture(&game, 0, capital),
        "the required capital must be an operationally valid target"
    );

    if majors == 3 {
        game.record_contact(0, 2);
        game.at_war.insert((0, 1));
        assert_eq!(
            ai.domination_capital_target(&game, 0)
                .map(|(owner, _)| owner),
            Some(2),
            "the unengaged rival must own the cheaper global capital"
        );
    }
    if majors == 3 {
        assert_eq!(
            ai.assess(&game, 0).target_city,
            Some(outpost),
            "the off arm keeps the existing generic-city fallback"
        );
        ai.enable_domination_capital_focus();
    }
    let plan = ai.assess(&game, 0);
    assert_eq!(
        plan.strategy,
        if majors == 3 {
            // The forward support city is exposed to the garrison.
            // Retain the defensive posture while naming this front.
            GrandStrategy::Recovery
        } else {
            GrandStrategy::Conquest
        }
    );
    assert_eq!(plan.target_player, Some(1));
    assert_eq!(plan.target_city, Some(capital));
}

#[test]
fn domination_capital_routing_uses_current_ownership_and_includes_home_capital() {
    let mut game = Game::new_full(3, 48, 28, 91_006, 300, 0, false);
    found_capitals(&mut game);
    game.turn = 200;
    game.record_contact(0, 1);
    game.record_contact(0, 2);
    let taken = game.player_city_ids(1)[0];
    let outpost = game.found_city_for(1, open_land_near(&game, game.cities[&taken].pos, 4), None);
    game.cities.get_mut(&taken).unwrap().owner = 0;
    game.cities.get_mut(&outpost).unwrap().pop = 20;
    let remaining = game.player_city_ids(2)[0];
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert_eq!(
        ai.domination_capital_target_for(&game, 0, Some(2)),
        Some((2, remaining))
    );
    assert_eq!(ai.domination_capital_target_for(&game, 0, Some(1)), None);

    // Another conqueror can hold multiple original capitals. Route to
    // their present owner, including when our own capital needs retaking.
    let home = game
        .cities
        .values()
        .find(|c| c.original_owner == 0)
        .unwrap()
        .id;
    game.cities.get_mut(&home).unwrap().owner = 2;
    game.cities.get_mut(&remaining).unwrap().owner = 0;
    assert_eq!(
        ai.domination_capital_target_for(&game, 0, Some(2)),
        Some((2, home)),
        "our own lost original capital is still required"
    );
    assert_eq!(ai.domination_capital_target_for(&game, 0, Some(1)), None);
}

#[test]
fn domination_target_ignores_capitals_already_held_by_its_team() {
    let mut game = Game::new_full(3, 36, 22, 91_005, 300, 0, false);
    found_capitals(&mut game);
    game.players[0].team = Some(1);
    game.players[1].team = Some(1);
    game.turn = 200;
    game.record_contact(0, 2);

    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert_eq!(
        ai.domination_capital_target(&game, 0),
        Some((2, game.player_city_ids(2)[0])),
        "a teammate retaining its own capital is already sufficient for a team victory"
    );
}

#[test]
fn science_denial_aims_at_the_rival_spaceport_before_a_convenient_city() {
    let mut game = Game::new_full(2, 48, 28, 91_003, 300, 0, false);
    found_capitals(&mut game);
    game.turn = 190;
    game.record_contact(0, 1);
    let spaceport_city = game.found_city_for(
        1,
        open_land_near(&game, game.cities[&game.player_city_ids(0)[0]].pos, 10),
        Some("Launch Complex".to_string()),
    );
    assert!(AdvancedAi::city_within_declaration_range(
        &game,
        0,
        game.cities[&spaceport_city].pos
    ));
    let district = game.cities[&spaceport_city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&spaceport_city].pos)
        .expect("a founded city owns a district tile");
    game.map.tiles.get_mut(&district).unwrap().district = Some(crate::name!("spaceport"));
    game.cities
        .get_mut(&spaceport_city)
        .unwrap()
        .districts
        .insert(crate::name!("spaceport"), district);
    game.players[1]
        .science_projects
        .insert("exoplanet_expedition".to_string());

    let ai = AdvancedAi::new();
    assert_eq!(
        ai.victory_denial(&game, 0),
        Some((1, GrandStrategy::Conquest))
    );
    let plan = ai.assess(&game, 0);
    assert_eq!(plan.strategy, GrandStrategy::Conquest);
    assert_eq!(plan.target_player, Some(1));
    assert_eq!(plan.target_city, Some(spaceport_city));
}

/// Four majors at turn 190: rival 1 leads Culture but cannot be reached,
/// rival 3 has launched the Exoplanet Expedition. `stock_denial_lead_time`
/// makes that Science clock an actionable denial against player 3.
fn launched_science_threat_board() -> Game {
    let mut game = Game::new_full(4, 48, 28, 91_004, 300, 0, false);
    found_capitals(&mut game);
    game.turn = 190;
    for rival in 1..4 {
        game.record_contact(0, rival);
    }
    std::sync::Arc::make_mut(&mut game.observed_public_empire_stats).insert(
        0,
        ObservedPublicEmpireStats {
            domestic_tourists: Some(10),
            ..ObservedPublicEmpireStats::default()
        },
    );
    std::sync::Arc::make_mut(&mut game.observed_public_empire_stats).insert(
        1,
        ObservedPublicEmpireStats {
            foreign_tourists: Some(150),
            ..ObservedPublicEmpireStats::default()
        },
    );
    std::sync::Arc::make_mut(&mut game.observed_public_empire_stats).insert(
        2,
        ObservedPublicEmpireStats {
            domestic_tourists: Some(150),
            ..ObservedPublicEmpireStats::default()
        },
    );
    std::sync::Arc::make_mut(&mut game.observed_public_empire_stats).insert(
        3,
        ObservedPublicEmpireStats {
            domestic_tourists: Some(20),
            ..ObservedPublicEmpireStats::default()
        },
    );
    game.players[3]
        .science_projects
        .insert("exoplanet_expedition".to_string());

    game
}

#[test]
fn an_unactionable_leader_does_not_mask_an_actionable_victory_threat() {
    let game = launched_science_threat_board();
    let mut ai = AdvancedAi::new();
    ai.stock_denial_lead_time = true;
    assert_eq!(
        ai.victory_denial(&game, 0),
        Some((1, GrandStrategy::Culture)),
        "the public signal keeps reporting the leading Culture clock"
    );
    assert_eq!(
        ai.actionable_victory_denial(&game, 0),
        Some((3, GrandStrategy::Conquest)),
        "the army planner falls through to the launched Science threat"
    );
    let plan = ai.assess(&game, 0);
    assert_eq!(plan.strategy, GrandStrategy::Conquest);
    assert_eq!(plan.target_player, Some(3));
}

/// `denial-outranks-expansion`: the same board, but the seat has a lane.
/// One city against a target of several keeps the lane's branch on
/// "Expansion", and that branch sits above the denial in the chain — so
/// with the gene off the launched rival is never answered. With it on the
/// denial is asked first and the plan is the same Conquest against player
/// 3 that a lane-less seat already chooses.
#[test]
fn a_rival_close_to_winning_outranks_the_lanes_expansion_only_with_the_gene() {
    let game = launched_science_threat_board();
    // A Culture lane, so the off-case cannot land on Conquest-against-3 by
    // way of the lane's own strategy; the live seat's Domination lane hit
    // the same Expansion branch for the same reason.
    let mut lane = AdvancedAi::targeting(VictoryTarget::Culture);
    lane.stock_denial_lead_time = true;
    // `deny-while-targeted` is what lets a seat with a lane see a denial at
    // all; this gene is about what happens to that denial once it exists.
    lane.deny_while_targeted = true;
    assert_eq!(
        lane.actionable_victory_denial(&game, 0),
        Some((3, GrandStrategy::Conquest)),
        "the denial is actionable regardless of the lane"
    );
    let plan = lane.assess(&game, 0);
    assert_ne!(
        (plan.strategy, plan.target_player),
        (GrandStrategy::Conquest, Some(3)),
        "with the gene off the lane's expansion rule shadows the denial: {plan:?}"
    );

    lane.enable_denial_outranks_expansion();
    let plan = lane.assess(&game, 0);
    assert_eq!(plan.strategy, GrandStrategy::Conquest);
    assert_eq!(
        plan.target_player,
        Some(3),
        "the denial is answered ahead of the lane"
    );
}
fn only_launched_science_threat_board() -> Game {
    let mut game = launched_science_threat_board();
    std::sync::Arc::make_mut(&mut game.observed_public_empire_stats)
        .get_mut(&1)
        .unwrap()
        .foreign_tourists = Some(0);
    game
}

fn targeted_science_counter(target: VictoryTarget) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    ai.stock_denial_lead_time = true;
    ai.enable_deny_while_targeted();
    ai.enable_denial_outranks_expansion();
    ai.enable_counter_in_lane();
    ai
}

#[test]
fn a_science_race_counter_does_not_replace_an_assigned_culture_goal() {
    let game = only_launched_science_threat_board();
    let ai = targeted_science_counter(VictoryTarget::Culture);
    assert_eq!(
        ai.actionable_victory_denial(&game, 0),
        Some((3, GrandStrategy::Science)),
        "the launch threat remains visible to the denial machinery"
    );
    assert_eq!(ai.assess(&game, 0).strategy, GrandStrategy::Culture);
}

#[test]
fn a_direct_army_counter_still_interrupts_an_assigned_culture_goal() {
    let game = only_launched_science_threat_board();
    let mut ai = targeted_science_counter(VictoryTarget::Culture);
    ai.disable_counter_in_lane();
    assert_eq!(
        ai.actionable_victory_denial(&game, 0),
        Some((3, GrandStrategy::Conquest))
    );
    let plan = ai.assess(&game, 0);
    assert_eq!(plan.strategy, GrandStrategy::Conquest);
    assert_eq!(plan.target_player, Some(3));
}

#[test]
fn science_and_adaptive_seats_keep_their_science_race_counter() {
    let game = only_launched_science_threat_board();
    let science = targeted_science_counter(VictoryTarget::Science);
    let mut adaptive = AdvancedAi::new();
    adaptive.stock_denial_lead_time = true;
    adaptive.enable_counter_in_lane();
    for ai in [science, adaptive] {
        assert_eq!(
            ai.actionable_victory_denial(&game, 0),
            Some((3, GrandStrategy::Science))
        );
        assert_eq!(ai.assess(&game, 0).strategy, GrandStrategy::Science);
    }
}

// -----------------------------------------------------------------
// `counter-culture-by-conquest`
// -----------------------------------------------------------------

fn culture_threat(progress: i32) -> VictoryFocus {
    VictoryFocus {
        strategy: GrandStrategy::Culture,
        progress,
    }
}

/// Two majors with capitals, and a rival Theatre Square to aim at.
fn culture_board() -> (Game, u32) {
    let mut game = Game::new_full(2, 24, 16, 91_777, 300, 0, false);
    found_capitals(&mut game);
    game.turn = game.max_turns / 2;
    let rival_city = game
        .player_city_ids(1)
        .into_iter()
        .next()
        .expect("the rival founded a capital");
    game.cities
        .get_mut(&rival_city)
        .unwrap()
        .districts
        .insert(crate::name!("theater_square"), Default::default());
    (game, rival_city)
}

#[test]
fn domination_lane_hands_over_is_a_native_opt_in_off_in_both_controllers() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "domination-lane-hands-over",
        |ai| ai.domination_lane_hands_over,
    );
}

/// `domination-lane-hands-over`: four cities on a board whose lane target is
/// higher. Gene off, the lane keeps reading Expansion — the branch that held
/// the live seat at 8 and 9 cities. Gene on, it follows its lane to war.
#[test]
fn a_domination_lane_with_the_opening_band_in_hand_goes_to_war_only_with_the_gene() {
    let mut game = Game::new_full(4, 48, 28, 91_004, 300, 0, false);
    found_capitals(&mut game);
    game.turn = 120;
    for rival in 1..4 {
        game.record_contact(0, rival);
    }
    let capital = game.player_city_ids(0)[0];
    let center = game.cities[&capital].pos;
    let mut radius = 4;
    while game.player_city_ids(0).len() < super::super::DOMINATION_HANDOVER_CITIES {
        let site = open_land_near(&game, center, radius);
        game.found_city_for(0, site, None);
        radius += 1;
        assert!(radius < 12, "could not place four cities near {center:?}");
    }
    assert_eq!(
        game.player_city_ids(0).len(),
        super::super::DOMINATION_HANDOVER_CITIES
    );

    let mut lane = AdvancedAi::targeting(VictoryTarget::Domination);
    // The live seat's forced pair, which is what grows the target past the
    // land: 6 wanted, then 10, on a map that holds five or six.
    lane.enable_rapid_city_expansion_2();
    lane.enable_expansion_scales_with_difficulty();
    let plan = lane.assess(&game, 0);
    assert!(
        plan.desired_cities > super::super::DOMINATION_HANDOVER_CITIES,
        "the fixture must sit under the lane's own target: {plan:?}"
    );
    assert_eq!(
        plan.strategy,
        GrandStrategy::Expansion,
        "gene off: the lane waits for a target the map may never meet: {plan:?}"
    );

    lane.enable_domination_lane_hands_over();
    // ⚠ Four cities alone are not enough: the hand-over also needs an army
    // that clears the elective-war bar against the weakest rival. Game 6
    // declared at power 198 vs 384 and was down to one city by t170.
    let mut outgunned = game.clone();
    for rival in 1..4 {
        let capital = outgunned.player_city_ids(rival)[0];
        let around = outgunned.cities[&capital].pos;
        for _ in 0..8 {
            let site = open_land_near(&outgunned, around, 2);
            outgunned.spawn_test_unit("swordsman", rival, site);
        }
    }
    let plan = lane.assess(&outgunned, 0);
    assert_eq!(
        plan.strategy,
        GrandStrategy::Expansion,
        "gene on but outgunned: no war at half the rivals' power: {plan:?}"
    );

    for _ in 0..8 {
        let site = open_land_near(&game, center, 2);
        game.spawn_test_unit("swordsman", 0, site);
    }
    let plan = lane.assess(&game, 0);
    assert_eq!(
        plan.strategy,
        GrandStrategy::Conquest,
        "gene on with the army to back it: four cities in hand, the lane goes to war: {plan:?}"
    );
}

#[test]
fn denial_outranks_expansion_is_a_native_opt_in_off_in_both_controllers() {
    super::super::test_support::opt_in_off_in_both_controllers("denial-outranks-expansion", |ai| {
        ai.denial_outranks_expansion
    });
}

#[test]
fn counter_culture_by_conquest_is_a_native_opt_in_off_in_both_controllers() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "counter-culture-by-conquest",
        |ai| ai.counter_culture_by_conquest,
    );
}

/// The premise: every other lane can answer a leader with war, and Culture
/// cannot.
#[test]
fn culture_is_the_only_lane_that_never_reaches_conquest() {
    let (game, _) = culture_board();
    let ai = AdvancedAi::new();
    let urgent = 95;
    for (strategy, expected) in [
        (GrandStrategy::Science, GrandStrategy::Conquest),
        (GrandStrategy::Religion, GrandStrategy::Conquest),
        (GrandStrategy::Expansion, GrandStrategy::Conquest),
        (GrandStrategy::Culture, GrandStrategy::Culture),
    ] {
        let pressure = VictoryFocus {
            strategy,
            progress: urgent,
        };
        assert_eq!(
            ai.denial_response_for_pressure(&game, 0, 0, 1, pressure),
            Some(expected),
            "{strategy:?} answers a leader this way with the gene off"
        );
    }
}

#[test]
fn the_gene_answers_a_culture_leader_with_war() {
    let (game, _) = culture_board();
    let mut ai = AdvancedAi::new();
    ai.enable_counter_culture_by_conquest();
    assert_eq!(
        ai.denial_response_for_pressure(&game, 0, 0, 1, culture_threat(95)),
        Some(GrandStrategy::Conquest)
    );
}

/// The urgency bar is the shipped one and the gene does not lower it.
#[test]
fn a_culture_rival_below_the_bar_is_still_no_threat_either_way() {
    let (game, _) = culture_board();
    let mut ai = AdvancedAi::new();
    assert_eq!(
        ai.denial_response_for_pressure(&game, 0, 0, 1, culture_threat(50)),
        None
    );
    ai.enable_counter_culture_by_conquest();
    assert_eq!(
        ai.denial_response_for_pressure(&game, 0, 0, 1, culture_threat(50)),
        None,
        "the gene changes the answer, never the bar"
    );
}

#[test]
fn the_campaign_is_aimed_at_the_rivals_great_works() {
    let (game, rival_city) = culture_board();
    let mut ai = AdvancedAi::new();
    assert_eq!(
        ai.victory_suppression_city(&game, 0, 1, culture_threat(95)),
        None,
        "off, a Culture threat names no suppression city"
    );
    ai.enable_counter_culture_by_conquest();
    assert_eq!(
        ai.victory_suppression_city(&game, 0, 1, culture_threat(95)),
        Some(rival_city),
        "on, the Theatre Square city is the campaign's first target"
    );
}

/// The other two arms are untouched, gene or no gene.
#[test]
fn the_science_and_religion_suppression_arms_are_unchanged() {
    let (mut game, rival_city) = culture_board();
    game.cities
        .get_mut(&rival_city)
        .unwrap()
        .districts
        .insert(crate::name!("spaceport"), Default::default());
    let mut ai = AdvancedAi::new();
    let science = VictoryFocus {
        strategy: GrandStrategy::Science,
        progress: 95,
    };
    assert_eq!(
        ai.victory_suppression_city(&game, 0, 1, science),
        Some(rival_city)
    );
    ai.enable_counter_culture_by_conquest();
    assert_eq!(
        ai.victory_suppression_city(&game, 0, 1, science),
        Some(rival_city)
    );
}
