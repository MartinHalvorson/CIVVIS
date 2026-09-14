use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::game::{Action, HostMenuEntry};
use crate::rules::Yields;
use crate::Pos;

fn board(cities: usize) -> (Game, u32, Item, StrategicPlan) {
    let mut g = Game::new(3, 32, 22, 914_3580, 250, 0);
    let starts: Vec<_> = (0..3)
        .map(|pid| g.units[&g.player_unit_ids(pid)[0]].pos)
        .collect();
    g.players[0].civ = "Rome".into();
    g.players[0].techs = ["mining", "astrology", "writing"]
        .into_iter()
        .map(Name::new)
        .collect();
    g.players[0].civics.clear();
    for pos in starts.into_iter().take(cities) {
        g.found_city_for(0, pos, None);
    }
    for cid in g.player_city_ids(0) {
        g.cities.get_mut(&cid).unwrap().pop = 4;
        let pos = bare_plot(&mut g, cid);
        g.cities
            .get_mut(&cid)
            .unwrap()
            .districts
            .insert(crate::name!("holy_site"), pos);
        g.map.tiles.get_mut(&pos).unwrap().district = Some(crate::name!("holy_site"));
    }
    let cid = g.player_city_ids(0)[0];
    let pos = bare_plot(&mut g, cid);
    let item = Item::District {
        district: crate::name!("campus"),
        pos,
    };
    g.turn = 60;
    g.current = 0;
    g.players[0].research = Some("bronze_working".into());
    g.players[0].research_progress = g.tech_cost("bronze_working") * 0.95;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: cities,
        assessed_turn: g.turn,
        rush: false,
    };
    assert!(g.can_produce(0, cid, &item));
    (g, cid, item, plan)
}

fn bare_plot(g: &mut Game, cid: u32) -> Pos {
    let pos = g.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .find(|p| *p != g.cities[&cid].pos && g.map.tiles[p].district.is_none())
        .unwrap();
    let tile = g.map.tiles.get_mut(&pos).unwrap();
    tile.terrain = crate::name!("grassland");
    tile.hills = false;
    tile.feature = None;
    tile.resource = None;
    tile.improvement = None;
    pos
}

fn treatment() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_lock_expiring_district_discount();
    ai
}

fn menu(g: &Game, cid: u32, item: &Item, plan: &StrategicPlan, district_score: f64) -> Vec<f64> {
    let items = [
        item.clone(),
        Item::Building {
            building: crate::name!("monument"),
        },
    ];
    let mut scores = vec![district_score, 100.0];
    treatment().adjust_expiring_district_discounts(g, 0, cid, &items, plan, &mut scores);
    scores
}

#[test]
fn discount_window_is_an_independent_reversible_opt_in() {
    opt_in_off_in_both_controllers("lock-expiring-district-discount", |ai| {
        ai.lock_expiring_district_discount
    });
}

#[test]
fn an_expiring_discount_can_decide_a_close_build_but_cannot_rescue_filler() {
    let (g, cid, item, plan) = board(2);
    assert_eq!(g.district_underbuilt_discount(0, "campus", false), 0.4);
    let near = menu(&g, cid, &item, &plan, 95.0);
    assert!(near[0] > near[1]);
    assert_eq!(near[1], 100.0);
    assert_eq!(menu(&g, cid, &item, &plan, 70.0), vec![70.0, 100.0]);
    assert_eq!(menu(&g, cid, &item, &plan, -10.0), vec![-10.0, 100.0]);
    let mut off = vec![95.0, 100.0];
    AdvancedAi::new().adjust_expiring_district_discounts(
        &g,
        0,
        cid,
        &[
            item,
            Item::Building {
                building: crate::name!("monument"),
            },
        ],
        &plan,
        &mut off,
    );
    assert_eq!(off, vec![95.0, 100.0]);
}

#[test]
fn real_production_shortlist_receives_the_discount_preference() {
    let (g, cid, item, plan) = board(2);
    let stock = AdvancedAi::new();
    let items = [item];
    let before = stock.production_values(&g, 0, cid, &items, &plan, stock.counts(&g, 0));
    let ai = treatment();
    let after = ai.production_values(&g, 0, cid, &items, &plan, ai.counts(&g, 0));
    assert!(before[0] > 0.0);
    assert!(after[0] > before[0]);
}

#[test]
fn the_selected_district_keeps_its_price_through_the_actual_research_completion() {
    let (mut g, cid, item, plan) = board(2);
    let original_price = g.item_cost_for_city(0, cid, &item);
    assert!(menu(&g, cid, &item, &plan, 95.0)[0] > 100.0);
    g.players[0].research_progress = g.tech_cost("bronze_working") - 1.0;
    Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
        cid,
        Yields {
            science: 10.0,
            ..Yields::default()
        },
    );
    let mut waiting = g.clone();
    g.apply(
        0,
        &Action::Produce {
            city: cid,
            item: item.clone(),
        },
    )
    .unwrap();
    for game in [&mut g, &mut waiting] {
        // Upkeep, including research, runs when this player's next turn starts.
        for _ in 0..game.players.len() {
            game.apply(game.current, &Action::EndTurn).unwrap();
            if game.current == 0 {
                break;
            }
        }
        assert_eq!(game.current, 0);
        assert!(game.players[0]
            .techs
            .contains(&crate::name!("bronze_working")));
    }
    assert_eq!(g.item_cost_for_city(0, cid, &item), original_price);
    assert!(waiting.item_cost_for_city(0, cid, &item) > original_price);
    assert_eq!(
        waiting.district_underbuilt_discount(0, "campus", false),
        0.0
    );
}

#[test]
fn stable_prices_early_research_and_existing_commitments_keep_the_old_scores() {
    let (g, cid, item, plan) = board(2);
    let unchanged = |game: &Game, plan: &StrategicPlan| {
        assert_eq!(menu(game, cid, &item, plan, 95.0), vec![95.0, 100.0])
    };
    let mut early = g.clone();
    early.players[0].research_progress = 0.5 * early.tech_cost("bronze_working");
    unchanged(&early, &plan);
    let mut unrelated = g.clone();
    unrelated.players[0].research = Some("irrigation".into());
    unrelated.players[0].research_progress = unrelated.tech_cost("irrigation") * 0.95;
    unchanged(&unrelated, &plan);
    let mut protected = g.clone();
    protected
        .cities
        .get_mut(&cid)
        .unwrap()
        .queue
        .push(Item::Unit {
            unit: crate::name!("settler"),
        });
    unchanged(&protected, &plan);
    let mut threatened = plan.clone();
    threatened.threatened_city = Some(cid);
    unchanged(&g, &threatened);
    let mut war = plan.clone();
    war.strategy = GrandStrategy::Conquest;
    unchanged(&g, &war);
    let (stable, cid, item, plan) = board(3); // Three completed vs. three unlocked still discounts Campus.
    assert_eq!(menu(&stable, cid, &item, &plan, 95.0), vec![95.0, 100.0]);
}

#[test]
fn government_plaza_and_civic_unlocks_use_the_engine_discount_rules() {
    let (mut g, cid, item, plan) = board(3);
    g.players[0].civics.insert(crate::name!("state_workforce"));
    let Item::District { pos, .. } = item else {
        unreachable!()
    };
    let plaza = Item::District {
        district: crate::name!("government_plaza"),
        pos,
    };
    assert_eq!(
        g.district_underbuilt_discount(0, "government_plaza", false),
        0.25
    );
    assert!(menu(&g, cid, &plaza, &plan, 95.0)[0] > 100.0);
    let (mut g, cid, item, plan) = board(2);
    g.players[0].research = None;
    g.players[0].civic = Some("state_workforce".into());
    g.players[0].civic_progress = g.civic_cost("state_workforce") * 0.95;
    assert!(menu(&g, cid, &item, &plan, 95.0)[0] > 100.0);
}

#[test]
fn a_host_price_mismatch_disables_the_model_forecast() {
    let (mut g, cid, item, plan) = board(2);
    let cost = g.item_cost_for_city(0, cid, &item);
    let quote = |cost| {
        [(
            "district:campus".into(),
            HostMenuEntry {
                cost: Some(cost),
                turns: None,
            },
        )]
        .into_iter()
        .collect()
    };
    Arc::make_mut(&mut g.host_buildable).insert(cid, quote(cost));
    assert!(menu(&g, cid, &item, &plan, 95.0)[0] > 100.0);
    Arc::make_mut(&mut g.host_buildable).insert(cid, quote(cost * 2.0));
    assert_eq!(menu(&g, cid, &item, &plan, 95.0), vec![95.0, 100.0]);
}
