//! Production-order regressions for the early Domination land eye.
use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

fn board_with_enemy(enemy: Pos) -> (Game, StrategicPlan) {
    let mut g = Game::new_full(2, 48, 30, 50_541, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    for pos in [(8, 8), (15, 8), (22, 8)] {
        g.found_city_for(0, pos, None);
    }
    g.found_city_for(1, enemy, None);
    g.players[0].explored.clear();
    for city in g.cities.values_mut() {
        city.pop = 5;
        city.queue.clear();
    }
    g.spawn_test_unit("scout", 0, (9, 8));
    for pos in [(8, 8), (15, 8), (22, 8)] {
        g.spawn_test_unit("warrior", 0, pos);
    }
    g.players[0].gold = 500.0;
    g.players[0].gold_per_turn = 20.0;
    g.players[0]
        .techs
        .extend([crate::name!("pottery"), crate::name!("writing")]);
    g.game_speed = crate::setup::GameSpeed::Online;
    g.turn = 25;
    g.current = 0;
    g.at_war.clear();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, plan)
}

fn board() -> (Game, StrategicPlan) {
    board_with_enemy((30, 20))
}

fn seat(on: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.skip_opening_book();
    ai.enable_recon_replacement();
    ai.enable_early_conquest_opening();
    ai.enable_first_capital_before_the_walls();
    if on {
        ai.enable_land_recon_before_the_first_capital();
    }
    ai
}

fn queued_scouts(g: &Game) -> usize {
    g.cities
        .values()
        .filter(|c| c.owner == 0)
        .filter(|c| {
            c.queue.iter().any(|i| {
        matches!(i, Item::Unit { unit } if g.rules.units[unit].promotion_class == "recon")
    })
        })
        .count()
}

#[test]
fn the_early_land_claim_is_opt_in_off_in_both_controllers() {
    opt_in_off_in_both_controllers("land-recon-before-the-first-capital", |ai| {
        ai.land_recon_before_the_first_capital
    });
}

#[test]
fn the_missing_second_eye_gets_one_idle_queue_and_never_duplicates_its_order() {
    let (mut g, plan) = board();
    seat(false).claim_first_capital_land_recon(&mut g, 0, &plan);
    assert_eq!(queued_scouts(&g), 0);
    let ai = seat(true);
    ai.claim_first_capital_land_recon(&mut g, 0, &plan);
    assert_eq!(
        queued_scouts(&g),
        1,
        "one existing Scout still needs its spare"
    );
    ai.claim_first_capital_land_recon(&mut g, 0, &plan);
    assert_eq!(queued_scouts(&g), 1, "the queued Scout fills the arm");
}

#[test]
fn the_early_claim_keeps_opening_defense_and_other_lanes_ahead() {
    for case in [
        "book", "one_city", "late", "threat", "war", "culture", "full_arm", "busy",
    ] {
        let (mut g, mut plan) = board();
        let mut ai = seat(true);
        match case {
            "book" => ai.base.book_pos = 3,
            "one_city" => {
                let keep = g.player_city_ids(0)[0];
                for cid in g.player_city_ids(0) {
                    if cid != keep {
                        g.cities.remove(&cid);
                    }
                }
            }
            "late" => g.turn = g.standard_duration(FIRST_CAPITAL_NAMING_DEADLINE) + 1,
            "threat" => plan.threatened_city = Some(g.player_city_ids(0)[0]),
            "war" => {
                g.at_war.insert((0, 1));
            }
            "culture" => ai.victory_target = Some(VictoryTarget::Culture),
            "full_arm" => {
                g.spawn_test_unit("scout", 0, (16, 8));
            }
            "busy" => {
                for c in g.cities.values_mut() {
                    c.queue.push(Item::Building {
                        building: crate::name!("monument"),
                    });
                }
            }
            _ => unreachable!(),
        }
        ai.claim_first_capital_land_recon(&mut g, 0, &plan);
        assert_eq!(queued_scouts(&g), 0, "{case}");
    }
}

#[test]
fn a_reachable_capital_keeps_the_opening_army_ahead_of_an_extra_eye() {
    let (mut g, plan) = board_with_enemy((25, 8));
    g.players[0].met.insert(1);
    g.players[1].met.insert(0);
    g.players[0].explored.extend(
        g.map
            .tiles
            .keys()
            .copied()
            .filter(|(x, y)| (8..=25).contains(x) && (7..=9).contains(y)),
    );
    let ai = seat(true);
    assert!(ai.first_capital_target(&g, 0).is_some());
    assert!(
        ai.base.recon_is_the_missing_arm(&g, 0),
        "unexplored land still needs the ordinary spare eye"
    );
    ai.claim_first_capital_land_recon(&mut g, 0, &plan);
    assert_eq!(queued_scouts(&g), 0, "the reachable opening has priority");
}

fn campus_competition() -> (Game, StrategicPlan, u32) {
    let (mut g, plan) = board();
    let capital = g
        .player_city_ids(0)
        .into_iter()
        .find(|cid| g.cities[cid].is_capital)
        .unwrap();
    for c in g.cities.values_mut() {
        c.buildings.push(crate::name!("granary"));
        if c.owner == 0 && c.id != capital {
            c.queue.push(Item::Building {
                building: crate::name!("monument"),
            });
        }
    }
    g.spawn_test_unit("settler", 0, (12, 10));
    for u in g.units.values_mut() {
        u.moves_left = 0.0;
    }
    std::sync::Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
        capital,
        crate::rules::Yields {
            production: 10.0,
            ..Default::default()
        },
    );
    (g, plan, capital)
}

#[test]
fn the_claimed_scout_survives_the_governors_first_zero_investment_frame() {
    let (base, plan, capital) = campus_competition();
    let scout = Item::Unit {
        unit: crate::name!("scout"),
    };
    for on in [false, true] {
        let mut g = base.clone();
        let mut ai = seat(on);
        g.apply(
            0,
            &Action::Produce {
                city: capital,
                item: scout.clone(),
            },
        )
        .unwrap();
        assert_eq!(g.item_invested_production(capital, &scout), 0.0);
        ai.advanced_production(&mut g, 0, &plan, false);
        assert_eq!(
            g.cities[&capital].queue.first() == Some(&scout),
            on,
            "the on arm must retain the Scout the off arm overwrites"
        );
    }
}

#[test]
fn the_hold_releases_to_a_full_arm_and_defense() {
    let (mut g, mut plan, capital) = campus_competition();
    let scout = Item::Unit {
        unit: crate::name!("scout"),
    };
    g.apply(
        0,
        &Action::Produce {
            city: capital,
            item: scout.clone(),
        },
    )
    .unwrap();
    let ai = seat(true);
    assert!(ai.first_capital_land_recon_holds(&g, 0, capital, &plan, &scout));
    assert!(!seat(false).first_capital_land_recon_holds(&g, 0, capital, &plan, &scout));
    plan.threatened_city = Some(capital);
    assert!(!ai.first_capital_land_recon_holds(&g, 0, capital, &plan, &scout));
    plan.threatened_city = None;
    g.spawn_test_unit("scout", 0, (16, 8));
    assert!(!ai.first_capital_land_recon_holds(&g, 0, capital, &plan, &scout));
}

#[test]
fn the_real_turn_leaves_a_second_land_eye_in_the_arm() {
    let (base, _, _) = campus_competition();
    let mut arms = Vec::new();
    for on in [false, true] {
        let mut g = base.clone();
        let mut ai = seat(on);
        ai.enable_capital_campus_before_the_plaza();
        ai.take_turn(&mut g, 0);
        let fielded = g
            .units
            .values()
            .filter(|u| u.owner == 0 && g.rules.units[&u.kind].promotion_class == "recon")
            .count();
        arms.push(fielded + queued_scouts(&g));
    }
    assert_eq!(
        arms,
        vec![1, 2],
        "the whole turn must preserve the new Scout, not just log an overwritten order"
    );
}
