use super::super::test_support::opt_in_off_in_both_controllers;
use super::super::{AdvancedAi, GrandStrategy, StrategicPlan};
use super::*;
use crate::game::{Action, Game};
use crate::name;
use crate::rules::BoostSpec;

/// One founded capital and nothing else on the board.
fn capital_board(seed: u64) -> Game {
    let mut game = Game::new_full(1, 20, 14, seed, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("the player opens with a settler");
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    game
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_boost_planner();
    ai
}

fn plan() -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: 0,
        rush: false,
    }
}

fn trigger(trigger: &str, count: i64) -> BoostSpec {
    BoostSpec {
        trigger: trigger.to_string(),
        count,
        percent: None,
    }
}

/// The first tile the capital owns that is not the city centre.
fn owned_tile(game: &Game) -> Pos {
    let cid = game.player_city_ids(0)[0];
    let city = &game.cities[&cid];
    *city
        .owned_tiles
        .iter()
        .find(|pos| **pos != city.pos)
        .expect("the capital owns more than its centre")
}

/// Put `improvement` on an owned tile and return it.
fn improve(game: &mut Game, improvement: &str) -> Pos {
    let pos = owned_tile(game);
    let tile = game.map.tiles.get_mut(&pos).expect("the tile exists");
    tile.improvement = Some(Name::new(improvement));
    tile.pillaged = false;
    pos
}

#[test]
fn boost_planner_is_a_native_opt_in_off_in_both_controllers() {
    opt_in_off_in_both_controllers("boost-planner", |ai| ai.boost_planner);
}

// ---- the horizon ----------------------------------------------------

#[test]
fn the_horizon_walks_the_beeline_pickers_own_comparator_forward() {
    let game = capital_board(71_001);
    let ai = armed();
    let horizon = ai.boost_horizon(&game, 0, true, BOOST_HORIZON);
    assert_eq!(horizon.len(), BOOST_HORIZON, "six technologies of beeline");

    let mut seen: BTreeSet<Name> = BTreeSet::new();
    let mut previous_start = game.turn;
    for step in &horizon {
        assert!(
            !game.players[0].techs.contains(&step.node),
            "{} is already held",
            step.node
        );
        assert!(seen.insert(step.node), "{} appears twice", step.node);
        // Every prerequisite is held or already taken earlier in the walk.
        for need in &game.rules.techs[step.node.as_str()].requires {
            assert!(
                game.players[0].techs.contains(need) || seen.contains(need),
                "{} needs {need}, which the horizon has not taken",
                step.node
            );
        }
        assert!(
            step.start_turn >= previous_start,
            "start turns never move backwards"
        );
        assert!(step.deadline >= step.start_turn);
        previous_start = step.start_turn;
    }

    // Nothing is held back with the gene off: the horizon is a read of the
    // board, and it is the premium that the flag gates.
    assert_eq!(
        AdvancedAi::new().boost_horizon(&game, 0, true, BOOST_HORIZON),
        horizon,
        "the projection itself does not depend on the flag"
    );
    // The first step is the cheapest legal one by the beeline's own cost.
    let cheapest = game
        .available_techs(0)
        .into_iter()
        .min_by(|a, b| {
            ai.beeline_step_cost(&game, 0, a.as_str(), true)
                .total_cmp(&ai.beeline_step_cost(&game, 0, b.as_str(), true))
                .then_with(|| a.cmp(b))
        })
        .expect("the opening offers techs");
    assert_eq!(horizon[0].node, cheapest);
}

#[test]
fn the_node_under_study_leads_the_horizon_and_keeps_its_window_to_completion() {
    let mut game = capital_board(71_002);
    game.turn = 30;
    game.players[0].research = Some("mining".to_string());
    game.players[0].research_progress = 0.0;
    let ai = armed();
    let horizon = ai.boost_horizon(&game, 0, true, BOOST_HORIZON);
    assert_eq!(horizon[0].node, name!("mining"));
    assert_eq!(horizon[0].start_turn, 30, "it has already started");
    assert!(
        horizon[0].deadline > 30,
        "an in-progress node's window runs to completion: the engine credits \
         a boost that lands mid-research"
    );
    // A future step's deadline is the turn it starts, not the turn it ends.
    assert_eq!(horizon[1].deadline, horizon[1].start_turn);
    assert!(horizon[1].start_turn >= horizon[0].deadline);
}

#[test]
fn the_civic_horizon_is_shorter_than_the_technology_horizon() {
    let game = capital_board(71_003);
    let ai = armed();
    assert_eq!(BOOST_CIVIC_HORIZON, 4);
    assert_eq!(BOOST_HORIZON, 6);
    assert_eq!(
        ai.boost_horizon(&game, 0, false, BOOST_CIVIC_HORIZON).len(),
        BOOST_CIVIC_HORIZON
    );
}

// ---- the trigger cost table ------------------------------------------

#[test]
fn a_satisfied_trigger_is_never_planned_for() {
    let game = capital_board(71_010);
    let ai = armed();
    // One city stands, so "found a city" is already met.
    assert_eq!(
        ai.boost_trigger_class(&game, 0, &trigger("cities", 1)),
        BoostTriggerClass::Satisfied
    );
}

#[test]
fn an_improvement_trigger_is_cheap_only_once_we_already_build_that_improvement() {
    let mut game = capital_board(71_011);
    game.players[0].techs.insert(name!("mining"));
    let spec = trigger("improvement_on_resource:mine", 1);

    // Unlocked, but the empire has never put a Mine on the ground.
    assert_eq!(
        ai_class(&game, &spec),
        BoostTriggerClass::Expensive,
        "a new Builder habit is not cheap"
    );

    improve(&mut game, "mine");
    assert_eq!(
        ai_class(&game, &spec),
        BoostTriggerClass::Cheap(BoostAction::Improvement {
            improvement: "mine".to_string(),
            on: TileRequirement::AnyResource,
        })
    );

    // The plain `improvement:` form carries no tile requirement …
    assert_eq!(
        ai_class(&game, &trigger("improvement:mine", 3)),
        BoostTriggerClass::Cheap(BoostAction::Improvement {
            improvement: "mine".to_string(),
            on: TileRequirement::Any,
        })
    );
    // … and `improve_resource:` names the resource the tile must carry.
    let resource = game
        .rules
        .resources
        .iter()
        .find(|(_, spec)| spec.improvement.as_str() == "mine")
        .map(|(name, _)| name.to_string())
        .expect("some resource is worked by a Mine");
    game.players[0]
        .techs
        .extend(game.rules.resources[resource.as_str()].tech);
    assert_eq!(
        ai_class(&game, &trigger(&format!("improve_resource:{resource}"), 1)),
        BoostTriggerClass::Cheap(BoostAction::Improvement {
            improvement: "mine".to_string(),
            on: TileRequirement::Named(resource),
        })
    );

    // Without the gating technology nothing can be planned at all.
    let mut locked = capital_board(71_012);
    locked.players[0].techs.remove(&name!("mining"));
    improve(&mut locked, "mine");
    assert_eq!(
        ai_class(&locked, &spec),
        BoostTriggerClass::ImpossibleNow,
        "a trigger whose own technology is unresearched cannot be planned"
    );
}

fn ai_class(game: &Game, spec: &BoostSpec) -> BoostTriggerClass {
    armed().boost_trigger_class(game, 0, spec)
}

/// Grant every node that gates whatever a trigger names, so a classification
/// test measures the classification and not the permission.
fn unlock(game: &mut Game, trigger: &str) {
    for (gate, techs) in AdvancedAi::trigger_gates(game, trigger) {
        if techs {
            game.players[0].techs.insert(gate);
        } else {
            game.players[0].civics.insert(gate);
        }
    }
}

#[test]
fn a_unit_trigger_is_cheap_only_when_we_own_one_and_need_exactly_one_more() {
    let mut game = capital_board(71_013);
    game.players[0].techs.insert(name!("archery"));
    let three = trigger("units_of:archer", 3);
    let capital = game.cities[&game.player_city_ids(0)[0]].pos;

    assert_eq!(
        ai_class(&game, &three),
        BoostTriggerClass::Expensive,
        "owning none of the kind is a new army, not a side objective"
    );
    game.spawn_unit("archer", 0, capital);
    assert_eq!(
        ai_class(&game, &three),
        BoostTriggerClass::Expensive,
        "two more Archers is a build order, not a tie-break"
    );
    game.spawn_unit("archer", 0, capital);
    assert_eq!(
        ai_class(&game, &three),
        BoostTriggerClass::Cheap(BoostAction::Unit("archer".to_string())),
        "one we already field, and one more, is cheap"
    );
    game.spawn_unit("archer", 0, capital);
    assert_eq!(
        ai_class(&game, &three),
        BoostTriggerClass::Satisfied,
        "the third Archer fires it"
    );
}

#[test]
fn a_district_trigger_is_cheap_only_for_a_family_we_already_build() {
    let mut game = capital_board(71_014);
    let cid = game.player_city_ids(0)[0];
    let spec = trigger("district:campus", 2);
    assert_eq!(
        ai_class(&game, &spec),
        BoostTriggerClass::ImpossibleNow,
        "a district the empire may not build yet cannot be planned for"
    );
    unlock(&mut game, "district:campus");
    assert_eq!(
        ai_class(&game, &spec),
        BoostTriggerClass::Expensive,
        "a family the empire has never built is not a plan it already has"
    );
    let pos = owned_tile(&game);
    game.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(name!("campus"), pos);
    assert_eq!(
        ai_class(&game, &spec),
        BoostTriggerClass::Cheap(BoostAction::District("campus".to_string()))
    );
}

#[test]
fn a_city_site_is_never_a_boost_decision() {
    let mut game = capital_board(71_015);
    let spec = trigger("coastal_city", 1);
    if ai_class(&game, &spec) == BoostTriggerClass::Satisfied {
        // This seed founded on the coast; the rule under test needs an
        // unsatisfied trigger, and the classification above is the point.
        return;
    }
    assert_eq!(
        ai_class(&game, &spec),
        BoostTriggerClass::Expensive,
        "where the next city stands is not a boost decision"
    );
    // Not even with a Settler already walking: the site seam was cut in
    // review, and the class must not come back with it.
    let capital = game.cities[&game.player_city_ids(0)[0]].pos;
    game.spawn_unit("settler", 0, capital);
    assert_eq!(ai_class(&game, &spec), BoostTriggerClass::Expensive);
}

#[test]
fn strategic_spending_and_the_untouchable_triggers_never_become_objectives() {
    let mut game = capital_board(71_016);
    for gated in [
        "building:library",
        "building_near_mountain:university",
        "kill_with:archer",
    ] {
        unlock(&mut game, gated);
    }
    for spending in [
        "wonders",
        "war",
        "kills",
        "barbs_killed",
        "camps",
        "religion",
        "pantheon",
        "great_people",
        "national_park",
        "themed_buildings",
        "kill_with:archer",
        "great_person_of:scientist",
        "building:library",
        "building_near_mountain:university",
        "unit_and_improve:builder:horses",
    ] {
        assert_eq!(
            ai_class(&game, &trigger(spending, 1)),
            BoostTriggerClass::Expensive,
            "{spending} is strategic spending"
        );
    }
    for unreachable in [
        "pop",
        "total_pop",
        "met_civ",
        "met_city_states",
        "discover_continent",
        "alliances",
        "government_slots",
        "trade_routes",
        "specialty_districts",
        "tech:mining",
        "civic:early_empire",
        "a_trigger_the_engine_does_not_know",
    ] {
        assert_eq!(
            ai_class(&game, &trigger(unreachable, 9)),
            BoostTriggerClass::ImpossibleNow,
            "{unreachable} is not a decision the planner can take"
        );
    }
    // Every classification is exhaustive over the shipped rows: nothing
    // panics and nothing falls through into `Cheap` by accident.
    for (node, spec) in game.rules.techs.iter().chain(game.rules.civics.iter()) {
        let Some(boost) = spec.boost.as_ref() else {
            continue;
        };
        let class = armed().boost_trigger_class(&game, 0, boost);
        if let BoostTriggerClass::Cheap(_) = class {
            panic!(
                "{node} ({}) is cheap on an empty opening board",
                boost.trigger
            );
        }
    }
}

// ---- side objectives, the cap and the deadline -----------------------

#[test]
fn at_most_three_objectives_stand_and_the_richest_win() {
    let mut game = capital_board(71_020);
    let ai = armed();
    for objective in ai.boost_side_objectives(&game, 0) {
        assert!(objective.deadline > game.turn);
    }
    assert!(ai.boost_side_objectives(&game, 0).len() <= BOOST_MAX_ACTIVE);

    // Forced: four cheap candidates, one slot each, the cap keeps three.
    let candidates = vec![
        objective_named("alpha", 10.0, 40),
        objective_named("beta", 400.0, 40),
        objective_named("gamma", 300.0, 40),
        objective_named("delta", 200.0, 40),
    ];
    let mut sorted = candidates;
    sorted.sort_by(AdvancedAi::boost_by_payout);
    sorted.truncate(BOOST_MAX_ACTIVE);
    assert_eq!(
        sorted
            .iter()
            .map(|objective| objective.node.to_string())
            .collect::<Vec<_>>(),
        vec!["beta", "gamma", "delta"],
        "the cap binds on the research at stake"
    );
    game.turn += 1;
    assert!(ai.boost_side_objectives(&game, 0).len() <= BOOST_MAX_ACTIVE);
}

fn objective_named(node: &str, payout: f64, deadline: u32) -> BoostSideObjective {
    BoostSideObjective {
        node: Name::new(node),
        techs: true,
        trigger: "units_of:archer".to_string(),
        action: BoostAction::Unit("archer".to_string()),
        deadline,
        payout,
    }
}

#[test]
fn a_committed_objective_stands_until_its_deadline_and_then_expires() {
    let mut game = capital_board(71_021);
    game.turn = 10;
    let ai = armed();
    // A commitment the horizon would not offer, planted directly, so the
    // expiry rule is what is under test and nothing else.
    *ai.boost_planner_frame.borrow_mut() = BoostPlannerFrame {
        stamp: Some((10, 0)),
        objectives: vec![objective_named("machinery", 500.0, 13)],
        defence_stand_down: false,
    };
    game.players[0].techs.insert(name!("archery"));
    let capital = game.cities[&game.player_city_ids(0)[0]].pos;
    game.spawn_unit("archer", 0, capital);
    game.spawn_unit("archer", 0, capital);

    game.turn = 13;
    let live = ai.boost_side_objectives(&game, 0);
    assert!(
        live.iter()
            .any(|objective| objective.node == name!("machinery")),
        "on its deadline turn the commitment still stands"
    );

    game.turn = 14;
    assert!(
        !ai.boost_side_objectives(&game, 0)
            .iter()
            .any(|objective| objective.node == name!("machinery")),
        "past the deadline it is dropped"
    );
}

#[test]
fn a_collected_boost_ends_its_own_objective() {
    let mut game = capital_board(71_022);
    game.turn = 10;
    let ai = armed();
    *ai.boost_planner_frame.borrow_mut() = BoostPlannerFrame {
        stamp: Some((10, 0)),
        objectives: vec![objective_named("machinery", 500.0, 60)],
        defence_stand_down: false,
    };
    game.players[0].boosted_techs.insert(name!("machinery"));
    game.turn = 11;
    assert!(
        !ai.boost_side_objectives(&game, 0)
            .iter()
            .any(|objective| objective.node == name!("machinery")),
        "the discount is banked; there is nothing left to chase"
    );
}

#[test]
fn a_commitment_whose_node_comes_under_study_keeps_its_window_to_completion() {
    let mut game = capital_board(71_023);
    game.turn = 10;
    game.players[0].techs.insert(name!("archery"));
    let capital = game.cities[&game.player_city_ids(0)[0]].pos;
    game.spawn_unit("archer", 0, capital);
    game.spawn_unit("archer", 0, capital);
    let ai = armed();
    // Committed with the deadline a not-yet-started node carries: its
    // projected start turn.
    *ai.boost_planner_frame.borrow_mut() = BoostPlannerFrame {
        stamp: Some((10, 0)),
        objectives: vec![objective_named("machinery", 500.0, 12)],
        defence_stand_down: false,
    };
    // The empire starts Machinery on the deadline turn and is still on it
    // the turn after: the engine credits a boost mid-research, so the
    // commitment stands to the projected completion rather than expiring on
    // the very turn it became most valuable.
    game.players[0].research = Some("machinery".to_string());
    game.players[0].research_progress = 0.0;
    game.turn = 13;
    let live = ai.boost_side_objectives(&game, 0);
    let standing = live
        .iter()
        .find(|objective| objective.node == name!("machinery"))
        .expect("a commitment under study is kept past its start-turn deadline");
    assert_eq!(standing.deadline, 12, "the commitment keeps the deadline it was given");
    let completion = ai.boost_horizon(&game, 0, true, 1)[0].deadline;
    assert!(completion > 13, "an opening empire takes many turns over Machinery");
    assert_eq!(ai.boost_commitment_deadline(&game, 0, standing), completion);

    // Once research moves on, the old start-turn window is what is left,
    // and it has passed.
    game.players[0].research = Some("mining".to_string());
    game.turn = 14;
    assert!(
        !ai.boost_side_objectives(&game, 0)
            .iter()
            .any(|objective| objective.node == name!("machinery")),
        "a node no longer under study falls back to the window it was given"
    );
}

// ---- the premium ------------------------------------------------------

#[test]
fn the_premium_is_a_share_of_the_choice_and_only_of_a_named_one() {
    let mut game = capital_board(71_030);
    // Past the expansion band, so the opening-band stand-down is not what is
    // being measured here.
    game.turn = AdvancedAi::expansion_band_turn(&game) + 1;
    let ai = armed();
    let cid = game.player_city_ids(0)[0];
    *ai.boost_planner_frame.borrow_mut() = BoostPlannerFrame {
        stamp: Some((game.turn, 0)),
        objectives: vec![objective_named("machinery", 500.0, game.turn + 20)],
        defence_stand_down: false,
    };
    let archer = Item::Unit {
        unit: name!("archer"),
    };
    let warrior = Item::Unit {
        unit: name!("warrior"),
    };
    assert_eq!(
        ai.boost_planner_production_premium(&game, 0, cid, &archer, 200.0, &plan()),
        200.0 * BOOST_PREMIUM_PCT / 100.0,
        "fifteen percent of the item's own value"
    );
    assert_eq!(
        ai.boost_planner_production_premium(&game, 0, cid, &warrior, 200.0, &plan()),
        0.0,
        "an item no objective names is untouched"
    );
    assert_eq!(
        ai.boost_planner_production_premium(&game, 0, cid, &archer, -50.0, &plan()),
        0.0,
        "a premium never lifts a choice the planner priced at or below zero"
    );
}

/// `chase-every-boost` ships on and prices the same seams. Stacked, the two
/// premiums are each a share of the same positive base — v1 held under half
/// of it, this gene at fifteen percent — so together they never exceed 65 %
/// of the choice's own value and pay nothing on a choice worth nothing.
#[test]
fn stacked_on_chase_every_boost_the_premium_stays_a_bounded_share() {
    use super::super::chase_every_boost::{
        CHASE_BUILDER_VALUE_FRACTION, CHASE_PRODUCTION_RAW_FRACTION,
    };
    let mut game = capital_board(71_033);
    game.turn = AdvancedAi::expansion_band_turn(&game) + 1;
    game.players[0].techs.insert(name!("archery"));
    game.players[0].techs.insert(name!("mining"));
    let capital = game.cities[&game.player_city_ids(0)[0]].pos;
    game.spawn_unit("archer", 0, capital);
    game.spawn_unit("archer", 0, capital);
    let pos = improve(&mut game, "mine");
    let cid = game.player_city_ids(0)[0];
    let mut ai = armed();
    ai.enable_chase_every_boost();
    *ai.boost_planner_frame.borrow_mut() = BoostPlannerFrame {
        stamp: Some((game.turn, 0)),
        objectives: vec![
            objective_named("machinery", 500.0, game.turn + 20),
            BoostSideObjective {
                node: name!("apprenticeship"),
                techs: true,
                trigger: "improvement:mine".to_string(),
                action: BoostAction::Improvement {
                    improvement: "mine".to_string(),
                    on: TileRequirement::Any,
                },
                deadline: game.turn + 20,
                payout: 100.0,
            },
        ],
        defence_stand_down: false,
    };
    let archer = Item::Unit {
        unit: name!("archer"),
    };
    let planner_share = BOOST_PREMIUM_PCT / 100.0;
    for raw in [1.0, 40.0, 200.0, 5_000.0] {
        let stacked = ai.production_boost_premium(&game, 0, cid, &archer, raw)
            + ai.boost_planner_production_premium(&game, 0, cid, &archer, raw, &plan());
        assert!(
            stacked <= raw * (CHASE_PRODUCTION_RAW_FRACTION + planner_share) + 1e-9,
            "production: {stacked} on a value of {raw}"
        );
        let stacked = ai.builder_boost_premium(&game, pos, "mine", raw)
            + ai.boost_planner_builder_premium(&game, pos, "mine", raw);
        assert!(
            stacked <= raw * (CHASE_BUILDER_VALUE_FRACTION + planner_share) + 1e-9,
            "builder: {stacked} on a value of {raw}"
        );
    }
    for raw in [0.0, -1.0, -500.0] {
        assert_eq!(
            ai.production_boost_premium(&game, 0, cid, &archer, raw)
                + ai.boost_planner_production_premium(&game, 0, cid, &archer, raw, &plan()),
            0.0,
            "a choice worth nothing is lifted by neither premium"
        );
        assert_eq!(
            ai.builder_boost_premium(&game, pos, "mine", raw)
                + ai.boost_planner_builder_premium(&game, pos, "mine", raw),
            0.0
        );
    }
}

#[test]
fn the_builder_premium_respects_the_tile_requirement() {
    let mut game = capital_board(71_031);
    game.turn = AdvancedAi::expansion_band_turn(&game) + 1;
    game.players[0].techs.insert(name!("mining"));
    let ai = armed();
    let pos = improve(&mut game, "mine");
    let bare = *game.cities[&game.player_city_ids(0)[0]]
        .owned_tiles
        .iter()
        .find(|other| **other != pos)
        .expect("the capital owns a second tile");
    game.map.tiles.get_mut(&pos).unwrap().resource = Some(name!("iron"));
    game.map.tiles.get_mut(&bare).unwrap().resource = None;

    *ai.boost_planner_frame.borrow_mut() = BoostPlannerFrame {
        stamp: Some((game.turn, 0)),
        objectives: vec![BoostSideObjective {
            node: name!("wheel"),
            techs: true,
            trigger: "improvement_on_resource:mine".to_string(),
            action: BoostAction::Improvement {
                improvement: "mine".to_string(),
                on: TileRequirement::AnyResource,
            },
            deadline: game.turn + 20,
            payout: 100.0,
        }],
        defence_stand_down: false,
    };
    assert_eq!(
        ai.boost_planner_builder_premium(&game, pos, "mine", 40.0),
        40.0 * BOOST_PREMIUM_PCT / 100.0
    );
    assert_eq!(
        ai.boost_planner_builder_premium(&game, bare, "mine", 40.0),
        0.0,
        "the trigger counts a Mine on a resource, so a bare hill earns nothing"
    );
    assert_eq!(
        ai.boost_planner_builder_premium(&game, pos, "farm", 40.0),
        0.0,
        "another improvement on the same tile earns nothing"
    );
}

#[test]
fn defence_the_opening_band_and_a_launch_city_all_outrank_a_boost() {
    let mut game = capital_board(71_032);
    let ai = armed();
    let cid = game.player_city_ids(0)[0];
    let archer = Item::Unit {
        unit: name!("archer"),
    };
    fn frame(turn: u32) -> BoostPlannerFrame {
        BoostPlannerFrame {
            stamp: Some((turn, 0)),
            objectives: vec![objective_named("machinery", 500.0, turn + 20)],
            defence_stand_down: false,
        }
    }

    // Inside the expansion band and behind its pace: the production premium
    // stands down so a Settler's slot is never taken for a trigger.
    game.turn = AdvancedAi::expansion_band_turn(&game) / 2;
    assert!(
        AdvancedAi::expansion_pace(&game) > game.player_city_ids(0).len(),
        "halfway through the band the winning pace is ahead of one city"
    );
    *ai.boost_planner_frame.borrow_mut() = frame(game.turn);
    assert_eq!(
        ai.boost_planner_production_premium(&game, 0, cid, &archer, 200.0, &plan()),
        0.0
    );
    // The Builder half is untouched: a charge does not compete with a Settler.
    game.players[0].techs.insert(name!("mining"));
    let pos = improve(&mut game, "mine");
    *ai.boost_planner_frame.borrow_mut() = BoostPlannerFrame {
        stamp: Some((game.turn, 0)),
        objectives: vec![BoostSideObjective {
            node: name!("apprenticeship"),
            techs: true,
            trigger: "improvement:mine".to_string(),
            action: BoostAction::Improvement {
                improvement: "mine".to_string(),
                on: TileRequirement::Any,
            },
            deadline: game.turn + 20,
            payout: 100.0,
        }],
        defence_stand_down: false,
    };
    assert!(ai.boost_planner_builder_premium(&game, pos, "mine", 40.0) > 0.0);

    // A threatened city pays nothing at all.
    game.turn = AdvancedAi::expansion_band_turn(&game) + 1;
    *ai.boost_planner_frame.borrow_mut() = frame(game.turn);
    let mut besieged = plan();
    besieged.threatened_city = Some(cid);
    assert_eq!(
        ai.boost_planner_production_premium(&game, 0, cid, &archer, 200.0, &besieged),
        0.0
    );
    // And the empire-wide defence stand-down reaches the Builder too.
    *ai.boost_planner_frame.borrow_mut() = BoostPlannerFrame {
        stamp: Some((game.turn, 0)),
        objectives: vec![objective_named("machinery", 500.0, game.turn + 20)],
        defence_stand_down: true,
    };
    assert_eq!(
        ai.boost_planner_builder_premium(&game, pos, "mine", 40.0),
        0.0
    );
    assert_eq!(
        ai.boost_planner_production_premium(&game, 0, cid, &archer, 200.0, &plan()),
        0.0
    );
}

// ---- off, nothing moves ------------------------------------------------

#[test]
fn off_every_entry_point_returns_before_reading_anything() {
    let mut game = capital_board(71_050);
    game.turn = 40;
    game.players[0].techs.insert(name!("archery"));
    let capital = game.cities[&game.player_city_ids(0)[0]].pos;
    game.spawn_unit("archer", 0, capital);
    game.spawn_unit("archer", 0, capital);
    let cid = game.player_city_ids(0)[0];
    let plain_ai = AdvancedAi::new();
    let item = Item::Unit {
        unit: name!("archer"),
    };
    let pos = improve(&mut game, "mine");

    assert!(plain_ai.boost_side_objectives(&game, 0).is_empty());
    assert_eq!(
        plain_ai.boost_planner_production_premium(&game, 0, cid, &item, 200.0, &plan()),
        0.0
    );
    assert_eq!(
        plain_ai.boost_planner_builder_premium(&game, pos, "mine", 40.0),
        0.0
    );
    // Nothing was memoised either: the flag is checked before the frame.
    assert_eq!(plain_ai.boost_planner_frame.borrow().stamp, None);
}
