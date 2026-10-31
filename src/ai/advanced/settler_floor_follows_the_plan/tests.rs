use super::*;
use crate::ai::advanced::early_settler_floor::EARLY_SETTLER_FLOOR_STANDARD_TURN;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

/// A founded capital and a second city four to six tiles away, both idle at
/// population 6, every unit removed, past the band turn.
fn two_idle_cities_past_the_band() -> (Game, u32, u32) {
    let mut game = Game::new_full(1, 32, 20, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("the player opens with a settler");
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    let sites: Vec<Pos> = game.map.tiles.keys().copied().collect();
    let mut second = None;
    for site in sites {
        if game.wdist(site, home) < 4 || game.wdist(site, home) > 6 {
            continue;
        }
        let settler = game.spawn_unit("settler", 0, site);
        if game.apply(0, &Action::FoundCity { unit: settler }).is_ok() {
            second = game
                .player_city_ids(0)
                .into_iter()
                .find(|cid| game.cities[cid].pos == site);
            break;
        }
        game.remove_unit(settler);
    }
    let second = second.expect("a legal second site");
    for cid in [capital, second] {
        let city = game.cities.get_mut(&cid).unwrap();
        city.pop = 6;
        city.queue.clear();
    }
    game.turn = game.standard_duration(EARLY_SETTLER_FLOOR_STANDARD_TURN) + 5;
    (game, capital, second)
}

fn settlers_queued(game: &Game) -> Vec<u32> {
    game.player_city_ids(0)
        .into_iter()
        .filter(|cid| {
            matches!(
                game.cities[cid].queue.first(),
                Some(Item::Unit { unit }) if *unit == "settler"
            )
        })
        .collect()
}

#[test]
fn settler_floor_follows_the_plan_is_opt_in() {
    opt_in_off_in_both_controllers("settler-floor-follows-the-plan", |ai| {
        ai.settler_floor_follows_the_plan
    });
}

/// Past the band turn the shipped floor is off; under the gene it queues one
/// Settler while the plan's target is short, and the floor keeps it.
#[test]
fn past_the_band_the_floor_follows_the_plan() {
    let (game, _capital, _second) = two_idle_cities_past_the_band();
    let mut stock = AdvancedAi::new();
    stock.enable_early_settler_floor();
    let mut off = game.clone();
    let plan = stock.assess(&off, 0);
    assert!(
        plan.desired_cities > 2,
        "fixture: the plan wants more cities"
    );
    stock.claim_early_settler_floor(&mut off, 0, &plan);
    assert!(
        settlers_queued(&off).is_empty(),
        "stock: no claim past the band"
    );

    let mut ai = AdvancedAi::new();
    ai.enable_early_settler_floor();
    ai.enable_settler_floor_follows_the_plan();
    let mut on = game.clone();
    let plan = ai.assess(&on, 0);
    assert_eq!(ai.early_settler_floor(&on, &plan), plan.desired_cities);
    ai.claim_early_settler_floor(&mut on, 0, &plan);
    let queued = settlers_queued(&on);
    assert_eq!(queued.len(), 1, "one Settler at a time");
    assert!(ai.early_settler_floor_holds(&on, 0, queued[0], &plan));
    ai.claim_early_settler_floor(&mut on, 0, &plan);
    assert_eq!(
        settlers_queued(&on).len(),
        1,
        "no second while one is queued"
    );
}

/// The plan's target already met: nothing is claimed.
#[test]
fn a_met_target_claims_nothing() {
    let (game, _capital, _second) = two_idle_cities_past_the_band();
    let mut ai = AdvancedAi::new();
    ai.enable_early_settler_floor();
    ai.enable_settler_floor_follows_the_plan();
    let mut on = game.clone();
    let mut plan = ai.assess(&on, 0);
    plan.desired_cities = 2;
    ai.claim_early_settler_floor(&mut on, 0, &plan);
    assert!(settlers_queued(&on).is_empty());
}
