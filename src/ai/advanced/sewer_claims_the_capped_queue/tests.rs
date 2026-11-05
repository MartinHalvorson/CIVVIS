use super::*;
use crate::ai::advanced::genes::GENES;

/// One major with one inland city at its housing, Sanitation known, plenty of
/// Production and an idle queue. Returns the game, our city and the plan.
fn capped_city() -> (Game, u32, StrategicPlan) {
    let mut game = Game::new_full(1, 32, 20, 936_221, 200, 0, false);
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    game.barb_camps.clear();
    game.barb_naval_camps.clear();
    let site = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| !game.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .min()
        .unwrap();
    let city = game.place_city(0, site, None);
    game.players[0].techs.insert(crate::name!("sanitation"));
    set_at_housing(&mut game, city);
    game.cities.get_mut(&city).unwrap().queue.clear();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: game.turn,
        rush: false,
    };
    (game, city, plan)
}

fn set_at_housing(game: &mut Game, city: u32) {
    let housing = game.city_housing(&game.cities[&city]);
    let pop = (housing.floor() as i32).max(CAPPED_SEWER_MIN_POP);
    game.cities.get_mut(&city).unwrap().pop = pop;
    let sewer = Item::Building {
        building: crate::name!("sewer"),
    };
    // Bank the Sewer almost whole, so it finishes inside the cap.
    let cost = game.item_cost_for_city(0, city, &sewer);
    game.cities
        .get_mut(&city)
        .unwrap()
        .production_progress
        .insert(Game::item_progress_key(&sewer), cost - 1.0);
}

fn seat() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_sewer_claims_the_capped_queue();
    ai
}

fn is_sewer(item: Option<&Item>) -> bool {
    matches!(item, Some(Item::Building { building }) if *building == "sewer")
}

#[test]
fn the_gene_is_a_registered_reversible_opt_in() {
    assert!(GENES.iter().any(|gene| gene.opt_in()
        && gene.field == "sewer_claims_the_capped_queue"
        && gene.tag == "sewer-claims-the-capped-queue"));
    let mut ai = AdvancedAi::new();
    assert!(!ai.sewer_claims_the_capped_queue, "production ships it off");
    ai.enable_sewer_claims_the_capped_queue();
    assert!(ai.sewer_claims_the_capped_queue);
    ai.disable_sewer_claims_the_capped_queue();
    assert!(!ai.sewer_claims_the_capped_queue);
}

#[test]
fn a_capped_city_claims_its_sewer_only_with_the_gene_on() {
    let (mut game, city, plan) = capped_city();
    AdvancedAi::new().claim_capped_sewer(&mut game, 0, &plan);
    assert!(game.cities[&city].queue.is_empty(), "off, nothing runs");

    let ai = seat();
    ai.claim_capped_sewer(&mut game, 0, &plan);
    let head = game.cities[&city].queue.first().cloned();
    assert!(
        is_sewer(head.as_ref()),
        "on, the Sewer heads the queue: {head:?}"
    );
    assert!(
        ai.capped_sewer_holds(&game, 0, city, &plan, head.as_ref().unwrap()),
        "and the governor's rescoring leaves it alone"
    );
}

#[test]
fn room_a_small_city_a_threat_or_no_sanitation_keep_the_queue() {
    let ai = seat();

    let (mut game, city, plan) = capped_city();
    let pop = game.city_housing(&game.cities[&city]).floor() as i32 - 3;
    game.cities.get_mut(&city).unwrap().pop = pop.max(1);
    ai.claim_capped_sewer(&mut game, 0, &plan);
    assert!(game.cities[&city].queue.is_empty(), "room to grow");

    let (mut game, city, plan) = capped_city();
    game.cities.get_mut(&city).unwrap().pop = CAPPED_SEWER_MIN_POP - 1;
    ai.claim_capped_sewer(&mut game, 0, &plan);
    assert!(game.cities[&city].queue.is_empty(), "too small");

    let (mut game, city, mut plan) = capped_city();
    plan.threatened_city = Some(city);
    ai.claim_capped_sewer(&mut game, 0, &plan);
    assert!(game.cities[&city].queue.is_empty(), "threatened");

    let (mut game, city, plan) = capped_city();
    game.players[0].techs.remove(&crate::name!("sanitation"));
    ai.claim_capped_sewer(&mut game, 0, &plan);
    assert!(game.cities[&city].queue.is_empty(), "no Sanitation");
}

#[test]
fn the_science_race_keeps_its_queue_and_a_market_does_not() {
    let (game, city, _) = capped_city();
    let may = |item: Item| AdvancedAi::capped_sewer_may_defer(&game, 0, city, &item, false);
    for building in [
        "research_lab",
        "factory",
        "nuclear_power_plant",
        "library",
        "university",
    ] {
        assert!(
            !may(Item::Building {
                building: Name::new(building)
            }),
            "{building} keeps its queue"
        );
    }
    assert!(
        !may(Item::District {
            district: crate::name!("spaceport"),
            pos: game.cities[&city].pos,
        }),
        "a Spaceport keeps its queue"
    );
    assert!(
        may(Item::Building {
            building: crate::name!("market")
        }),
        "a routine building waits"
    );
}
