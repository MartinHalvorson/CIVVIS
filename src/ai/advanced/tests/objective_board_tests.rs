//! Tests of `advanced/objective_board.rs`: each calls methods of that module and no
//! other domain module's.
//!
//! Moved verbatim out of `advanced/tests.rs` (2026-11-04); a `super::` path
//! there is `super::super::` here. Shared fixtures stay in `tests.rs` and
//! arrive through `use super::*`.

use super::*;

/// The defect this exists for: the army target was a headcount keyed to
/// our own city count, so an empire being outweighed two to one declared
/// its army finished and went back to building markets.
#[test]
fn the_army_target_rises_when_the_enemy_outweighs_us() {
    let (mut game, _capital, home) = empire_with_a_capital(71_105);
    let mut sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(position, tile)| {
            game.wdist(**position, home) >= 15
                && game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
        })
        .map(|(position, _)| *position)
        .collect();
    sites.sort_unstable();
    let muster = sites[0];

    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_army_target_weighs_the_enemy();
    let shipped = AdvancedAi::targeting(VictoryTarget::Domination);

    // An army of ours that comfortably outweighs theirs. `empire_with_a_capital`
    // strips our starting units, so this is the whole of our power.
    for step in 0..6 {
        game.spawn_test_unit("warrior", 0, anchor_at(&game, home, 1 + step));
    }
    game.spawn_test_unit("warrior", 1, muster);
    assert_eq!(
        ai.enemy_weighted_army_target(&game, 0, 12),
        12,
        "while we are ahead the shipped target is exactly what it was"
    );

    // They pass us. The target rises with the gap and never falls below
    // the shipped floor.
    for _ in 0..12 {
        game.spawn_test_unit("warrior", 1, muster);
    }
    let raised = ai.enemy_weighted_army_target(&game, 0, 12);
    assert!(
        raised > 12,
        "an army twice ours must want more than the peacetime twelve: {raised}"
    );
    assert!(raised <= 24, "the 2x ceiling holds: {raised}");

    // However far ahead they run, the empire is not asked to become
    // nothing but soldiers.
    for _ in 0..60 {
        game.spawn_test_unit("warrior", 1, muster);
    }
    assert_eq!(
        ai.enemy_weighted_army_target(&game, 0, 12),
        24,
        "a runaway rival is still capped at WARTIME_ARMY_CEILING"
    );

    // Peace with the same strong rival: the wartime scaling must not apply.
    game.at_war.remove(&(0, 1));
    game.at_war.remove(&(1, 0));
    assert_eq!(
        ai.enemy_weighted_army_target(&game, 0, 12),
        12,
        "a rival we are not fighting does not size our army"
    );

    // And the shipped agent never moves at all, war or not.
    game.at_war.insert((0, 1));
    assert!(!shipped.army_target_weighs_the_enemy);
    assert_eq!(shipped.enemy_weighted_army_target(&game, 0, 12), 12);
}

/// The defect this exists for: both army targets were blind until the war
/// started. Run `civvis-20260803T220954Z` founded seven cities, Mali
/// declared at t157 holding 894 military against our 481, and six of the
/// seven fell at loyalty 100 — sieges, not revolts, including the capital.
/// A target that asks "who could kill me" only after the declaration asks
/// too late; deterrence has to exist while the peace still holds.
#[test]
fn the_army_target_deters_the_strongest_met_major_in_peacetime() {
    let (mut game, _capital, home) = empire_with_a_capital(71_106);
    let mut sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(position, tile)| {
            game.wdist(**position, home) >= 15
                && game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
        })
        .map(|(position, _)| *position)
        .collect();
    sites.sort_unstable();
    let muster = sites[0];

    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_peacetime_deterrence();

    // Everyone is at peace, and the rival has been met.
    game.at_war.remove(&(0, 1));
    game.at_war.remove(&(1, 0));
    game.players[0].met.insert(1);
    game.players[1].met.insert(0);

    // While we outweigh the met rival the shipped target stands.
    for step in 0..6 {
        game.spawn_test_unit("warrior", 0, anchor_at(&game, home, 1 + step));
    }
    game.spawn_test_unit("warrior", 1, muster);
    assert_eq!(
        ai.enemy_weighted_army_target(&game, 0, 12),
        12,
        "a weaker met major does not size our peacetime army"
    );

    // They pass us in peace. The target rises — but under the deterrence
    // ceiling, which sits far below the wartime doubling.
    for _ in 0..12 {
        game.spawn_test_unit("warrior", 1, muster);
    }
    let raised = ai.enemy_weighted_army_target(&game, 0, 12);
    assert!(
        raised > 12,
        "a met major outweighing us must raise the peacetime target: {raised}"
    );
    assert!(
        raised <= 18,
        "deterrence wants a standing army, not a war footing: {raised}"
    );

    // However far ahead they run, peace never asks for a war footing.
    for _ in 0..60 {
        game.spawn_test_unit("warrior", 1, muster);
    }
    assert_eq!(
        ai.enemy_weighted_army_target(&game, 0, 12),
        18,
        "the peacetime ceiling holds however strong the met major grows"
    );

    // A major this seat has never met cannot size its army — the floor
    // reads no information fog has not released.
    game.players[0].met.remove(&1);
    assert_eq!(
        ai.enemy_weighted_army_target(&game, 0, 12),
        12,
        "an unmet major must not reach the peacetime target through fog"
    );

    // ⚠ `has_met` answers `true` for a TEAMMATE, so an ally's army would
    // otherwise deter us. `ai_eval` seats a free-for-all (`teams` empty,
    // every `player.team` None), so this arm is unreachable there and the
    // guard is inert in every evaluated game — but it is reachable in a
    // team lobby, which is why it is pinned rather than argued.
    game.players[0].met.insert(1);
    game.players[0].team = Some(1);
    game.players[1].team = Some(1);
    assert_eq!(
        ai.enemy_weighted_army_target(&game, 0, 12),
        12,
        "an ally on our own team must not size our army"
    );
    game.players[0].team = None;
    game.players[1].team = None;

    // War with the same rival escalates past the deterrence ceiling: the
    // wartime term takes over under its larger ceiling.
    game.players[0].met.insert(1);
    game.at_war.insert((0, 1));
    ai.enable_army_target_weighs_the_enemy();
    assert_eq!(
        ai.enemy_weighted_army_target(&game, 0, 12),
        24,
        "once the war is real the wartime ceiling governs, not the floor"
    );

    // The shipped agent never moves at all.
    let shipped = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(!shipped.peacetime_deterrence);
    game.at_war.remove(&(0, 1));
    assert_eq!(shipped.enemy_weighted_army_target(&game, 0, 12), 12);
}

/// The window has two edges. The horizon refuses a beeline so deep it is the
/// empire's whole research policy; the clock (its own test below) refuses one
/// that cannot finish. Inside both, the surge appoints — including with the
/// Bomber already in hand, where the buildout is all that is left.
#[test]
fn the_surge_window_is_bounded_by_the_horizon_and_the_clock() {
    let (mut game, _, _) = air_surge_fixture(941_101);
    let ai = air_surge_ai();

    assert_eq!(AdvancedAi::air_surge_missing_techs(&game, 0), 3);
    assert!(ai.air_surge_open(&game, 0));

    // Past the horizon the window is shut however affordable the chain is.
    let mut distant = game.clone();
    for tech in air_surge_prerequisites(&distant)
        .into_iter()
        .take(air_surge::AIR_SURGE_TECH_HORIZON)
    {
        distant.players[0].techs.remove(&tech);
    }
    assert!(AdvancedAi::air_surge_missing_techs(&distant, 0) > air_surge::AIR_SURGE_TECH_HORIZON);
    assert!(!ai.air_surge_open(&distant, 0));

    // And with the Bomber already in hand there is nothing left to beeline,
    // but the surge still opens — the buildout is the rest of the plan.
    air_surge_arm(&mut game);
    assert_eq!(AdvancedAi::air_surge_missing_techs(&game, 0), 0);
    assert!(ai.air_surge_open(&game, 0));
}

/// A Bomber cannot take a city; it can only empty one. The capture body is a
/// cavalry line by preference, and the surge says so when the empire can
/// field none.
#[test]
fn the_capture_body_is_cavalry_when_the_empire_can_field_one() {
    let (mut game, _, _) = air_surge_fixture(941_103);
    let (body, is_cavalry) =
        AdvancedAi::air_surge_body(&game, 0).expect("some land body is always buildable");
    assert!(is_cavalry, "{body} was chosen over the cavalry lines");
    assert!(matches!(
        game.rules.units[body].promotion_class.as_str(),
        "light_cavalry" | "heavy_cavalry"
    ));

    // Strip every cavalry technology and the surge falls back to melee rather
    // than refusing to attack at all.
    for tech in ["horseback_riding", "stirrups", "military_tactics"] {
        game.players[0].techs.remove(&Name::new(tech));
    }
    let (fallback, is_cavalry) =
        AdvancedAi::air_surge_body(&game, 0).expect("melee is always available");
    assert!(!is_cavalry, "{fallback} is still a cavalry line");
    assert!(game.rules.units[fallback].is_melee_capable());
}

/// The counter. A Bomber is as useful against a civilization attacking us as
/// against one we chose, and the empire that most needs a wing is the one
/// already being invaded — so a surge appointed into a running war arms
/// without a declaration, and keeps its beeline while it does.
#[test]
fn a_surge_appointed_into_a_running_war_arms_without_declaring() {
    let (mut game, _, objective) = air_surge_fixture(crate::rng::fixture_seed("AUTO_24", 941_111));
    game.at_war.insert((0, 1));
    game.at_war.insert((1, 0));
    let mut ai = air_surge_ai();
    ai.maintain_air_surge(&game, 0);

    let plan = ai
        .air_surge_plan
        .clone()
        .expect("a running war is a counter, not a refusal");
    assert!(plan.opened_at_war);
    assert_eq!(
        plan.target_player, 1,
        "the counter arms against the invader"
    );
    assert_eq!(plan.objective_city, objective);
    assert_eq!(
        plan.phase,
        air_surge::AirSurgePhase::Exploit,
        "there is nothing left to declare"
    );

    // ⚠ The beeline is keyed off the technology, not the phase: a counter is
    // in `Exploit` from its first turn and is exactly the appointment that
    // most needs the three techs.
    assert_eq!(
        ai.air_surge_research_goal(&game, 0),
        Some(air_surge::AIR_SURGE_GOAL_TECH)
    );
    assert!(
        !ai.air_surge_opening(&mut game, 0, 1),
        "a counter never consumes the war-opening decision"
    );

    // Peace closes it, exactly as a declaration of our own would have been.
    game.at_war.remove(&(0, 1));
    game.at_war.remove(&(1, 0));
    ai.maintain_air_surge(&game, 0);
    assert!(!ai.air_surge_active());
    assert_eq!(
        ai.air_surge_census.aborts.get("peace closed the war"),
        Some(&1)
    );
}

#[test]
fn culture_fortifies_before_a_visible_peacetime_siege_party_attacks() {
    let (mut game, city, home) = empire_with_a_capital(65_3504);
    game.at_war.clear();
    game.turn = 65;
    game.players[0].met.insert(1);
    game.players[0].techs.insert(crate::name!("masonry"));
    let nearby: Vec<Pos> = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| game.wdist(home, *pos) == 1)
        .take(3)
        .collect();
    assert_eq!(nearby.len(), 3);
    for (kind, pos) in ["catapult", "warrior", "archer"].into_iter().zip(nearby) {
        game.spawn_test_unit(kind, 1, pos);
    }
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_peacetime_deterrence();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Culture,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: game.turn,
        rush: false,
    };
    let wall = Item::Building {
        building: crate::name!("walls"),
    };
    assert!(game.can_produce(0, city, &wall));
    let mut disabled = ai.clone();
    disabled.peacetime_deterrence = false;
    assert!(disabled
        .culture_border_siege_walls_item(&game, 0, city, &plan)
        .is_none());
    let mut friendly = game.clone();
    friendly.players[0].friends_until.insert(1, game.turn + 30);
    assert!(ai
        .culture_border_siege_walls_item(&friendly, 0, city, &plan)
        .is_none());
    let mut no_siege = game.clone();
    let siege = no_siege
        .units
        .values()
        .find(|u| u.owner == 1 && u.kind == "catapult")
        .unwrap()
        .id;
    no_siege.units.remove(&siege);
    assert!(ai
        .culture_border_siege_walls_item(&no_siege, 0, city, &plan)
        .is_none());
    let mut unknown = game.clone();
    unknown.players[0].met.remove(&1);
    assert!(ai
        .culture_border_siege_walls_item(&unknown, 0, city, &plan)
        .is_none());
    let mut unseen = game.clone();
    let visible = unseen.player_vision_frame(0);
    let hidden = unseen
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| unseen.wdist(home, *pos) <= 4 && !unseen.sees(&visible, *pos))
        .expect("the fixture has nearby fog outside the capital's view");
    for unit in unseen.units.values_mut().filter(|unit| unit.owner == 1) {
        unit.pos = hidden;
    }
    assert!(
        ai.culture_border_siege_walls_item(&unseen, 0, city, &plan)
            .is_none(),
        "nearby siege units in the fog cannot influence production"
    );
    let mut recovery = plan.clone();
    recovery.strategy = GrandStrategy::Recovery;
    assert!(ai
        .culture_border_siege_walls_item(&game, 0, city, &recovery)
        .is_none());
    let mut defender = game.clone();
    defender.cities.get_mut(&city).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("warrior"),
    }];
    ai.advanced_production(&mut defender, 0, &plan, false);
    assert_eq!(
        defender.cities[&city].queue[0],
        Item::Unit {
            unit: crate::name!("warrior")
        }
    );
    ai.advanced_production(&mut game, 0, &plan, false);
    assert_eq!(
        game.cities[&city].queue.first(),
        Some(&wall),
        "visible siege preparations should not wait for an actual declaration"
    );
}

/// See `BasicAi::lent_military_floor_base`: above the genome's own floor, a
/// lent war target is raised only by cities that finish the unit quickly.
#[test]
fn a_slow_city_leaves_the_lent_army_margin_to_fast_cities() {
    let (mut game, capital, _) = empire_with_a_capital(79_137);
    clear_barbarian_fixture(&mut game);
    let home = game.cities[&capital].pos;
    game.spawn_test_unit("scout", 0, home);
    game.spawn_test_unit("warrior", 0, home);
    game.turn = 60;
    game.players[0].techs.insert(crate::name!("pottery"));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.base.book_pos = 4;
    ai.base.w.mil_per_city = 3.0;
    ai.base.lent_military_floor_base = Some(1.0);
    ai.disable_recon_replacement();
    ai.disable_naval_recon();
    let set_production = |game: &mut Game, production: f64| {
        std::sync::Arc::make_mut(&mut game.observed_city_yield_adjustments).remove(&capital);
        let current = game.city_yields(capital);
        std::sync::Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
            capital,
            crate::rules::Yields {
                production: production - current.production,
                ..crate::rules::Yields::default()
            },
        );
    };
    let military = |game: &Game, ai: &AdvancedAi, held: usize| {
        ai.base
            .pick_item(game, 0, capital, 1, 1, 9, 9, 9, held, held, 0)
            .is_some_and(|item| {
                matches!(item, Item::Unit { unit } if game.rules.units[&unit].class == "military")
            })
    };
    set_production(&mut game, 1.0);
    assert!(
        military(&game, &ai, 0),
        "below the genome's floor every city builds"
    );
    assert!(
        !military(&game, &ai, 2),
        "a 1-production city leaves the lent margin: {:?}",
        ai.base.pick_item(&game, 0, capital, 1, 1, 9, 9, 9, 2, 2, 0)
    );
    set_production(&mut game, 200.0);
    assert!(
        military(&game, &ai, 2),
        "a fast city raises the lent margin"
    );
    ai.base.lent_military_floor_base = None;
    set_production(&mut game, 1.0);
    assert!(
        military(&game, &ai, 2),
        "without a loan the floor is the genome's own"
    );
}
