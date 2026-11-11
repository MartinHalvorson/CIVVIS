use super::*;

/// A founded capital and a second city four to six tiles away, both idle at
/// population 4, every unit removed, on turn 40 of a 200-turn game in the
/// Classical era.
fn two_idle_cities() -> (Game, u32) {
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
        city.pop = 4;
        city.queue.clear();
    }
    game.turn = 40;
    game.world_era = 1;
    (game, capital)
}

/// The fixture in a Golden Age dedicated to Monumentality with `faith` banked.
fn golden(faith: f64) -> (Game, u32) {
    let (mut game, capital) = two_idle_cities();
    game.players[0].age = "golden".to_string();
    game.players[0]
        .dedications
        .insert(MONUMENTALITY.to_string());
    game.players[0].faith = faith;
    (game, capital)
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_faith_buys_the_settler();
    ai
}

fn settlers(game: &Game) -> usize {
    game.player_unit_ids(0)
        .into_iter()
        .filter(|uid| game.units[uid].kind == "settler")
        .count()
}

/// The blocked case: a Golden Age on Monumentality, a bank that covers the
/// price and a seat still free. The gene buys one Settler with Faith; off,
/// the Faith sits.
#[test]
fn monumentality_buys_a_settler_with_faith() {
    let (game, capital) = golden(1_000.0);
    let price = game
        .unit_purchase_cost(0, capital, "settler", "faith")
        .expect("fixture: Monumentality sells the Settler for Faith");
    assert!(price > 0.0 && price < 1_000.0);

    let stock = AdvancedAi::new();
    let mut off = game.clone();
    let plan = stock.assess(&off, 0);
    assert!(!stock.claim_faith_settler(&mut off, 0, &plan));
    assert_eq!(settlers(&off), 0, "off: nothing bought");
    assert_eq!(off.players[0].faith, 1_000.0);

    let ai = armed();
    let mut on = game.clone();
    let plan = ai.assess(&on, 0);
    assert!(
        ai.faith_settler_wanted(&on, 0, &plan).is_some(),
        "fixture: the plan wants more cities than the two it holds ({:?}, target {}, lane {:?})",
        plan.strategy,
        ai.settlement_target(&plan),
        ai.active_victory_target(&on)
    );
    assert!(ai.claim_faith_settler(&mut on, 0, &plan));
    assert_eq!(settlers(&on), 1, "one Settler bought");
    assert!(
        (on.players[0].faith - (1_000.0 - price)).abs() < 1e-6,
        "paid in Faith"
    );
    assert_eq!(on.players[0].gold, game.players[0].gold, "no Gold spent");
}

/// The bounds: no Golden Age, no Monumentality, a bank short of the price,
/// Recovery and a Religion lane each leave the Faith alone.
#[test]
fn the_purchase_waits_for_monumentality_the_price_and_the_lane() {
    let ai = armed();

    let (mut normal, _) = golden(1_000.0);
    normal.players[0].age = "normal".to_string();
    let plan = ai.assess(&normal, 0);
    assert!(
        !ai.claim_faith_settler(&mut normal, 0, &plan),
        "a Normal Age"
    );

    let (mut other, _) = golden(1_000.0);
    other.players[0].dedications.clear();
    let plan = ai.assess(&other, 0);
    assert!(
        !ai.claim_faith_settler(&mut other, 0, &plan),
        "another Dedication"
    );

    let (poor, capital) = golden(1_000.0);
    let price = poor
        .unit_purchase_cost(0, capital, "settler", "faith")
        .unwrap();
    let (mut poor, _) = golden(price - 1.0);
    let plan = ai.assess(&poor, 0);
    assert!(
        !ai.claim_faith_settler(&mut poor, 0, &plan),
        "a bank short of the price"
    );
    assert_eq!(settlers(&poor), 0);

    let (mut game, _) = golden(1_000.0);
    let mut plan = ai.assess(&game, 0);
    plan.strategy = GrandStrategy::Recovery;
    assert!(
        !ai.claim_faith_settler(&mut game, 0, &plan),
        "Recovery settles nothing"
    );

    let mut lane = armed();
    lane.victory_target = Some(VictoryTarget::Religion);
    let (mut game, _) = golden(1_000.0);
    let plan = lane.assess(&game, 0);
    assert!(
        !lane.claim_faith_settler(&mut game, 0, &plan),
        "a Religion lane keeps its Faith"
    );
}

/// One Settler past the ordinary pipeline's width, and no more.
#[test]
fn faith_adds_one_settler_to_the_pipeline() {
    let ai = armed();
    let (mut game, _) = golden(5_000.0);
    let plan = ai.assess(&game, 0);
    assert!(ai.claim_faith_settler(&mut game, 0, &plan));
    let plan = ai.assess(&game, 0);
    let bought = ai.claim_faith_settler(&mut game, 0, &plan);
    let plan = ai.assess(&game, 0);
    assert!(
        !ai.claim_faith_settler(&mut game, 0, &plan),
        "the width plus one is the most out at once"
    );
    assert_eq!(
        settlers(&game),
        1 + usize::from(bought),
        "{} Settler(s) out",
        settlers(&game)
    );
    assert!(settlers(&game) <= 1 + FAITH_SETTLER_EXTRA_WIDTH);
}

/// In a Golden Age that offers Monumentality the gene takes it first while a
/// seat is free; off, in a Normal Age, or with nothing to settle it does not.
#[test]
fn a_golden_age_is_dedicated_to_monumentality_while_seats_are_free() {
    let offered = |game: &mut Game| {
        game.players[0].age = "golden".to_string();
        game.players[0].dedications.clear();
        game.players[0].dedication_choices = 1;
    };
    let (mut game, _) = two_idle_cities();
    offered(&mut game);
    assert!(
        game.available_dedications(0)
            .iter()
            .any(|name| name.as_str() == MONUMENTALITY),
        "fixture: the Classical era offers Monumentality"
    );

    let stock = AdvancedAi::new();
    let mut off = game.clone();
    let plan = stock.assess(&off, 0);
    assert!(!stock.dedicate_to_monumentality(&mut off, 0, &plan));
    assert!(off.players[0].dedications.is_empty(), "off: no choice made");

    let ai = armed();
    let mut on = game.clone();
    let plan = ai.assess(&on, 0);
    assert!(ai.dedicate_to_monumentality(&mut on, 0, &plan));
    assert!(on.players[0].dedications.contains(MONUMENTALITY));
    assert!(AdvancedAi::monumentality_active(&on, 0));

    let mut normal = game.clone();
    normal.players[0].age = "normal".to_string();
    let plan = ai.assess(&normal, 0);
    assert!(
        !ai.dedicate_to_monumentality(&mut normal, 0, &plan),
        "a Normal Age banks Era Score; the ordinary choice stands"
    );

    let mut defending = game.clone();
    let mut plan = ai.assess(&defending, 0);
    plan.strategy = GrandStrategy::Recovery;
    assert!(
        !ai.dedicate_to_monumentality(&mut defending, 0, &plan),
        "Recovery settles nothing"
    );
}
