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
    // board, and it is the premium and the deferral that the flag gates.
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
fn a_coastal_city_trigger_is_cheap_only_while_a_settler_is_walking() {
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
        "a whole extra city is not a boost decision"
    );
    let capital = game.cities[&game.player_city_ids(0)[0]].pos;
    game.spawn_unit("settler", 0, capital);
    assert_eq!(
        ai_class(&game, &spec),
        BoostTriggerClass::Cheap(BoostAction::CoastalCity)
    );
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
            panic!("{node} ({}) is cheap on an empty opening board", boost.trigger);
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
        live.iter().any(|objective| objective.node == name!("machinery")),
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

// ---- research deferral -------------------------------------------------

/// A capital rich enough that an ancient node is a two-turn purchase, which
/// is the only regime in which deferring for a boost changes anything: below
/// it the node outlives its own trigger and the engine's mid-research credit
/// lands on it anyway.
fn fast_research_board(seed: u64) -> (Game, Name) {
    let mut game = capital_board(seed);
    game.turn = 40;
    let cid = game.player_city_ids(0)[0];
    game.cities.get_mut(&cid).unwrap().pop = 40;
    game.players[0].techs.insert(name!("archery"));
    let capital = game.cities[&cid].pos;
    game.spawn_unit("archer", 0, capital);
    game.spawn_unit("archer", 0, capital);
    let rate = AdvancedAi::research_rate(&game, 0, true);
    let ordinary = game
        .available_techs(0)
        .into_iter()
        .min_by(|a, b| game.tech_cost(a.as_str()).total_cmp(&game.tech_cost(b.as_str())))
        .expect("the tree offers a node");
    assert!(
        game.tech_cost(ordinary.as_str()) / rate <= BOOST_DEFER_TURNS,
        "the board must buy the cheapest node inside the deferral window"
    );
    (game, ordinary)
}

fn commit(ai: &AdvancedAi, node: Name, turn: u32, action: BoostAction, trigger: &str) {
    *ai.boost_planner_frame.borrow_mut() = BoostPlannerFrame {
        stamp: Some((turn, 0)),
        objectives: vec![BoostSideObjective {
            node,
            techs: true,
            trigger: trigger.to_string(),
            action,
            deadline: turn + 20,
            payout: 500.0,
        }],
        defence_stand_down: false,
    };
}

fn commit_mine(ai: &AdvancedAi, node: Name, turn: u32) {
    commit(
        ai,
        node,
        turn,
        BoostAction::Improvement {
            improvement: "mine".to_string(),
            on: TileRequirement::Any,
        },
        "improvement:mine",
    );
}

fn commit_archer(ai: &AdvancedAi, node: Name, turn: u32) {
    commit(
        ai,
        node,
        turn,
        BoostAction::Unit("archer".to_string()),
        "units_of:archer",
    );
}

/// Queue an Archer in the capital and invest production until it is `turns`
/// turns from done; returns the turns the queue front actually needs.
fn queue_archer_within(game: &mut Game, turns: f64) -> f64 {
    let cid = game.player_city_ids(0)[0];
    let archer = Item::Unit {
        unit: name!("archer"),
    };
    let city = game.cities.get_mut(&cid).unwrap();
    city.queue = vec![archer.clone()];
    city.production = 0.0;
    let per_turn =
        (game.city_yields(cid).production * game.item_prod_mult(0, cid, Some(&archer))).max(1.0);
    let outstanding = game.item_remaining_cost_for_city(0, cid, &archer);
    game.cities.get_mut(&cid).unwrap().production = (outstanding - turns * per_turn).max(0.0);
    game.item_remaining_cost_for_city(0, cid, &archer) / per_turn
}

#[test]
fn the_deferral_waits_only_for_a_committed_trigger_inside_its_window() {
    let (mut game, ordinary) = fast_research_board(71_040);
    let capital = game.cities[&game.player_city_ids(0)[0]].pos;
    let ai = armed();
    let available: Vec<Name> = game.available_techs(0);
    let rate = AdvancedAi::research_rate(&game, 0, true);
    let finish_turns = game.tech_cost(ordinary.as_str()) / rate;

    // No Builder stands, so the improvement is possible and not in progress.
    commit_mine(&ai, ordinary, game.turn);
    assert_eq!(
        ai.boost_planner_defer_pick(&game, 0, &available, &ordinary, None, true),
        None,
        "a merely possible trigger is not a commitment"
    );

    // A Builder does commit the charge, but this node outlives it: the boost
    // lands mid-research and the engine credits it, so nothing is deferred.
    game.spawn_unit("builder", 0, capital);
    assert!(finish_turns > BOOST_BUILDER_FIRE_TURNS);
    commit_mine(&ai, ordinary, game.turn);
    assert_eq!(
        ai.boost_planner_defer_pick(&game, 0, &available, &ordinary, None, true),
        None,
        "a node the trigger beats home needs no deferral"
    );

    // A queued Archer landing inside the window, after the node would have
    // been bought outright: this is the one case the deferral exists for.
    let fire_turns = queue_archer_within(&mut game, finish_turns + 0.5);
    assert!(
        finish_turns < fire_turns && fire_turns <= BOOST_DEFER_TURNS,
        "the board must put the trigger after the node and inside the window: \
         node {finish_turns:.2}, trigger {fire_turns:.2}"
    );
    commit_archer(&ai, ordinary, game.turn);
    let deferral = ai
        .boost_planner_defer_pick(&game, 0, &available, &ordinary, None, true)
        .expect("a committed trigger inside the window defers");
    assert_eq!(deferral.node, ordinary);
    assert_eq!(deferral.trigger, "units_of:archer");
    assert_ne!(deferral.pick, ordinary);
    // The replacement costs no more than the window it buys — the bound that
    // keeps the lane's next unlock from slipping past the boost's own wait.
    assert!(
        game.tech_cost(deferral.pick.as_str()) / rate <= BOOST_DEFER_TURNS,
        "the lane's next unlock is never pushed back past the boost window"
    );

    // The same trigger, pushed past the window, is not waited for.
    let far = queue_archer_within(&mut game, BOOST_DEFER_TURNS + 2.0);
    assert!(far > BOOST_DEFER_TURNS);
    commit_archer(&ai, ordinary, game.turn);
    assert_eq!(
        ai.boost_planner_defer_pick(&game, 0, &available, &ordinary, None, true),
        None,
        "a trigger landing after the window is not worth a deferral"
    );

    // Off, the picker is untouched.
    queue_archer_within(&mut game, finish_turns + 0.5);
    let plain_ai = AdvancedAi::new();
    commit_archer(&plain_ai, ordinary, game.turn);
    assert_eq!(
        plain_ai.boost_planner_defer_pick(&game, 0, &available, &ordinary, None, true),
        None
    );
}

#[test]
fn the_deferral_replacement_must_stay_on_the_lanes_own_beeline() {
    let (mut game, ordinary) = fast_research_board(71_041);
    let rate = AdvancedAi::research_rate(&game, 0, true);
    let want = game.tech_cost(ordinary.as_str()) / rate + 0.5;
    queue_archer_within(&mut game, want);
    let ai = armed();
    let available: Vec<Name> = game.available_techs(0);
    commit_archer(&ai, ordinary, game.turn);
    assert!(
        ai.boost_planner_defer_pick(&game, 0, &available, &ordinary, None, true)
            .is_some(),
        "with no forced goal the deferral fires on this board"
    );

    // With a forced lane goal, the replacement leads to that goal or there is
    // no deferral: the lane loses order, never progress.
    for goal in ["mining", "pottery", "animal_husbandry", "currency", "writing"] {
        if let Some(deferral) =
            ai.boost_planner_defer_pick(&game, 0, &available, &ordinary, Some(goal), true)
        {
            assert!(
                ai.tech_leads_to(&game, deferral.pick.as_str(), goal),
                "{} does not lead to {goal}",
                deferral.pick
            );
        }
    }
    // A goal nothing available leads to leaves the pick alone.
    assert_eq!(
        ai.boost_planner_defer_pick(
            &game,
            0,
            &available,
            &ordinary,
            Some("a_goal_no_tech_leads_to"),
            true
        ),
        None
    );
}

#[test]
fn a_node_that_outlives_its_own_trigger_is_never_deferred() {
    let mut game = capital_board(71_042);
    game.turn = 40;
    let capital = game.cities[&game.player_city_ids(0)[0]].pos;
    game.spawn_unit("builder", 0, capital);
    let ai = armed();
    let available: Vec<Name> = game.available_techs(0);
    let ordinary = name!("machinery");
    commit_mine(&ai, ordinary, game.turn);
    // Machinery is a long node against an opening empire's beakers, so the
    // engine's own mid-research credit reaches it and nothing is deferred.
    let rate = AdvancedAi::research_rate(&game, 0, true);
    assert!(game.tech_cost("machinery") / rate > BOOST_DEFER_TURNS);
    assert_eq!(
        ai.boost_planner_defer_pick(&game, 0, &available, &ordinary, None, true),
        None
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
    assert_eq!(
        plain_ai.boost_planner_site_premium(&game, 0, capital, 100.0),
        0.0
    );
    assert_eq!(
        plain_ai.boost_planner_defer_pick(
            &game,
            0,
            &game.available_techs(0),
            &name!("machinery"),
            None,
            true
        ),
        None
    );
    // Nothing was memoised either: the flag is checked before the frame.
    assert_eq!(plain_ai.boost_planner_frame.borrow().stamp, None);
}

