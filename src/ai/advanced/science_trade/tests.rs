use super::*;
use crate::ai::advanced::{test_support::opt_in_off_in_both_controllers, GrandStrategy};
use crate::game::{Action, AllianceState};
use crate::setup::GameSpeed;
use std::sync::Arc;

fn board() -> (Game, u32, u32, u32) {
    let mut g = Game::new(3, 24, 16, 914_3565, 250, 0);
    g.game_speed = GameSpeed::Standard;
    for pid in 0..3 {
        let pos = g.units[&g.player_unit_ids(pid)[0]].pos;
        g.found_city_for(pid, pos, None);
    }
    let origin = g.player_city_ids(0)[0];
    let first = g.player_city_ids(1)[0];
    let second = g.player_city_ids(2)[0];
    g.record_contact(0, 1);
    g.record_contact(0, 2);
    g.turn = 120;
    g.current = 0;
    g.players[0].gold = 500.0;
    g.players[0].gold_per_turn = 5.0;
    g.players[0].techs.insert(crate::name!("rocketry"));
    g.players[0].civics.insert(crate::name!("foreign_trade"));
    let pos = g.cities[&origin]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| *pos != g.cities[&origin].pos)
        .unwrap();
    let tile = g.map.tiles.get_mut(&pos).unwrap();
    tile.district = Some(crate::name!("spaceport"));
    tile.pillaged = false;
    g.cities
        .get_mut(&origin)
        .unwrap()
        .districts
        .insert(crate::name!("spaceport"), pos);
    let item = Item::Project {
        project: crate::name!("launch_earth_satellite"),
    };
    g.apply(0, &Action::Produce { city: origin, item }).unwrap();
    let production = g.city_yields(origin).production;
    Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
        origin,
        Yields {
            production: 30.0 - production,
            ..Yields::default()
        },
    );
    (g, origin, first, second)
}

fn treatment() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_trade_production_to_launch();
    ai
}

fn premium(g: &Game, origin: u32, production: f64) -> f64 {
    treatment().trade_production_to_launch_premium(
        g,
        0,
        Some(origin),
        Yields {
            production,
            ..Yields::default()
        },
    )
}

#[test]
fn launch_trade_is_an_independent_reversible_opt_in() {
    opt_in_off_in_both_controllers("trade-production-to-launch", |ai| {
        ai.trade_production_to_launch
    });
}

#[test]
fn active_launch_changes_the_real_destination_choice() {
    let (mut g, origin, production_dest, gold_dest) = board();
    Arc::make_mut(&mut g.observed_route_options).extend([
        (
            (origin, production_dest),
            Yields {
                production: 4.0,
                ..Yields::default()
            },
        ),
        (
            (origin, gold_dest),
            Yields {
                gold: 12.0,
                ..Yields::default()
            },
        ),
    ]);
    assert!(g.can_establish_trade_route(0, origin, production_dest));
    assert!(g.can_establish_trade_route(0, origin, gold_dest));
    assert_eq!(
        AdvancedAi::new()
            .best_trade_route_destination(&g, 0, origin, GrandStrategy::Science)
            .unwrap()
            .1,
        gold_dest
    );
    assert_eq!(
        treatment()
            .best_trade_route_destination(&g, 0, origin, GrandStrategy::Science)
            .unwrap()
            .1,
        production_dest
    );
    // The same city and route lose the premium as soon as the launch queue ends.
    g.cities.get_mut(&origin).unwrap().queue.clear();
    assert_eq!(
        treatment()
            .best_trade_route_destination(&g, 0, origin, GrandStrategy::Science)
            .unwrap()
            .1,
        gold_dest
    );
}

#[test]
fn timing_uses_invested_work_and_whole_completion_turns() {
    let (mut g, origin, _, _) = board();
    let cost = g.item_cost_for_city(0, origin, &g.cities[&origin].queue[0]);
    assert_eq!(g.city_yields(origin).production, 30.0);
    assert_eq!(
        g.item_prod_mult(0, origin, g.cities[&origin].queue.first()),
        2.0
    );
    // The native engine includes +100% for its modeled space-project stack.
    g.cities.get_mut(&origin).unwrap().production = cost - 600.0;
    assert_eq!(premium(&g, origin, 4.0), 4.0); // 10 turns -> 9 turns.
    g.cities.get_mut(&origin).unwrap().production = cost - 59.0;
    assert_eq!(premium(&g, origin, 4.0), 0.0); // Both finish next turn.
    g.cities.get_mut(&origin).unwrap().production = cost - 128.0;
    assert_eq!(premium(&g, origin, 4.0), 4.0); // 3 turns -> 2 turns.
    g.game_speed = GameSpeed::Online;
    let cost = g.item_cost_for_city(0, origin, &g.cities[&origin].queue[0]);
    g.cities.get_mut(&origin).unwrap().production = cost - 128.0;
    assert_eq!(premium(&g, origin, 4.0), 4.0 / g.game_speed.scale(1.0));
}

#[test]
fn chosen_production_route_launches_earlier_through_normal_turns() {
    let (mut g, origin, production_dest, gold_dest) = board();
    let routes = [
        (
            (origin, production_dest),
            Yields {
                production: 4.0,
                ..Yields::default()
            },
        ),
        (
            (origin, gold_dest),
            Yields {
                gold: 12.0,
                ..Yields::default()
            },
        ),
    ];
    Arc::make_mut(&mut g.observed_route_options).extend(routes);
    Arc::make_mut(&mut g.observed_route_yields).extend(routes);
    let uid = g.spawn_unit("trader", 0, g.cities[&origin].pos);
    let launch_turn = |mut game: Game, ai: AdvancedAi, destination: u32| {
        assert!(ai.advanced_trader_step(&mut game, 0, uid, GrandStrategy::Science));
        assert_eq!(game.routes.last().unwrap().dest, destination);
        for _ in 0..150 {
            game.apply(game.current, &Action::EndTurn).unwrap();
            if game.players[0]
                .science_projects
                .contains("launch_earth_satellite")
            {
                return game.turn;
            }
            assert!(!game.is_finished());
        }
        panic!("the queued launch must complete within the scenario budget");
    };
    let baseline = launch_turn(g.clone(), AdvancedAi::new(), gold_dest);
    let improved = launch_turn(g, treatment(), production_dest);
    assert!(
        improved < baseline,
        "launch turn: treatment {improved}, baseline {baseline}"
    );
}

#[test]
fn launch_premium_requires_a_legal_useful_solvent_origin() {
    let (g, origin, foreign, _) = board();
    assert!(premium(&g, origin, 4.0) > 0.0);
    assert_eq!(premium(&g, origin, 0.0), 0.0);
    assert_eq!(premium(&g, foreign, 4.0), 0.0);
    assert_eq!(
        treatment().trade_production_to_launch_premium(
            &g,
            0,
            None,
            Yields {
                production: 4.0,
                ..Yields::default()
            }
        ),
        0.0
    );
    let mut masked = g.clone();
    masked.victory_conditions.science = false;
    assert_eq!(premium(&masked, origin, 4.0), 0.0);
    let mut late = g.clone();
    late.turn = late.max_turns - 2;
    assert_eq!(premium(&late, origin, 4.0), 0.0);
    let mut insolvent = g.clone();
    insolvent.players[0].gold = 0.0;
    insolvent.players[0].gold_per_turn = -1.0;
    assert_eq!(premium(&insolvent, origin, 4.0), 0.0);
    let mut attacked = g.clone();
    attacked.cities.get_mut(&origin).unwrap().last_attacked = g.turn;
    assert_eq!(premium(&attacked, origin, 4.0), 0.0);
    let mut broken = g.clone();
    let pos = *broken.cities[&origin]
        .districts
        .get(crate::name!("spaceport"))
        .unwrap();
    broken.map.tiles.get_mut(&pos).unwrap().pillaged = true;
    assert_eq!(premium(&broken, origin, 4.0), 0.0);
}

#[test]
fn existing_routes_reduce_the_marginal_launch_bonus() {
    let (mut g, origin, dest, _) = board();
    let before = premium(&g, origin, 4.0);
    let uid = g.spawn_unit("trader", 0, g.cities[&origin].pos);
    g.apply(
        0,
        &Action::TradeRoute {
            unit: uid,
            city: dest,
        },
    )
    .unwrap();
    Arc::make_mut(&mut g.observed_route_yields).insert(
        (origin, dest),
        Yields {
            production: 30.0,
            ..Yields::default()
        },
    );
    assert!(g.city_yields(origin).production >= 60.0);
    assert!(premium(&g, origin, 4.0) < before);
}

#[test]
fn host_project_duration_overrides_the_native_timing_estimate() {
    let (mut g, origin, _, _) = board();
    assert!(premium(&g, origin, 4.0) > 0.0);
    for turns in [1.0, 40.0] {
        Arc::make_mut(&mut g.host_buildable).insert(
            origin,
            [(
                "project:launch_earth_satellite".to_string(),
                crate::game::HostMenuEntry {
                    cost: None,
                    turns: Some(turns),
                },
            )]
            .into_iter()
            .collect(),
        );
        // The host says either next turn (no acceleration possible), or so
        // late that this route still cannot deliver it within the horizon.
        assert_eq!(premium(&g, origin, 4.0), 0.0);
    }
}

#[test]
fn host_alliance_yields_are_counted_once_and_native_yields_still_get_the_bonus() {
    let (mut g, origin, dest, _) = board();
    let ai = AdvancedAi::new();
    for (kind, extra) in [
        (
            "research",
            Yields {
                science: 2.0,
                ..Yields::default()
            },
        ),
        (
            "cultural",
            Yields {
                culture: 2.0,
                ..Yields::default()
            },
        ),
        (
            "economic",
            Yields {
                gold: 4.0,
                ..Yields::default()
            },
        ),
        (
            "religious",
            Yields {
                faith: 2.0,
                ..Yields::default()
            },
        ),
    ] {
        g.players[0].alliances.insert(
            1,
            AllianceState {
                kind: kind.into(),
                points: 1.0,
                level: 1,
                ends: g.turn + 30,
            },
        );
        let score = |g: &Game| {
            ai.trade_route_destination_value_from(
                g,
                0,
                Some(origin),
                &g.cities[&dest],
                GrandStrategy::Science,
            )
        };
        Arc::make_mut(&mut g.observed_route_options).clear();
        let native = score(&g);
        let mut observed = g.trade_route_yields(0, dest);
        observed.science += extra.science;
        observed.culture += extra.culture;
        observed.gold += extra.gold;
        observed.faith += extra.faith;
        Arc::make_mut(&mut g.observed_route_options).insert((origin, dest), observed);
        assert!(
            (score(&g) - native).abs() < 1e-9,
            "{kind}: host and native equivalent yields must rank equally"
        );
        assert!((native - ai.yield_value(observed, GrandStrategy::Science) - 45.0).abs() < 1e-9);
    }
}
