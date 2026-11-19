use super::*;
use crate::ai::commercial_hub::tests::emperor_empire;
use crate::game::install_test_district;

fn plan() -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: 70,
        rush: false,
    }
}

/// A seat on `target` past its opening book with the city target met, the
/// gene on or off.
fn controller(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    ai.base.book_pos = 4;
    ai.base.w.city_target = 6.0;
    if gene {
        ai.enable_trade_district_follows_the_campus();
    }
    ai
}

fn unit(kind: &str) -> Item {
    Item::Unit {
        unit: Name::new(kind),
    }
}

fn building(kind: &str) -> Item {
    Item::Building {
        building: Name::new(kind),
    }
}

/// The six-city shape at turn 70 Online (every city four citizens, a Campus
/// and its Library, Currency in hand), at peace, every queue holding `head`
/// with nothing invested.
fn empire(tag: &str, head: &Item) -> (Game, Vec<u32>) {
    let (mut game, ranked) = emperor_empire(tag, 96_301);
    for cid in &ranked {
        game.cities.get_mut(cid).unwrap().queue = vec![head.clone()];
    }
    (game, ranked)
}

fn heads(game: &Game, ranked: &[u32]) -> Vec<Option<Item>> {
    ranked
        .iter()
        .map(|cid| game.cities[cid].queue.first().cloned())
        .collect()
}

fn trade_districts(game: &Game, heads: &[Option<Item>]) -> Vec<usize> {
    (0..heads.len())
        .filter(|rank| {
            heads[*rank]
                .as_ref()
                .is_some_and(|item| AdvancedAi::is_trade_district(game, item))
        })
        .collect()
}

#[test]
fn trade_district_follows_the_campus_is_an_opt_in_off_in_both_controllers() {
    crate::ai::advanced::test_support::opt_in_off_in_both_controllers(
        "trade-district-follows-the-campus",
        |ai| ai.trade_district_follows_the_campus,
    );
}

/// One Commercial Hub a turn goes ahead of a soldier in training at peace,
/// the soonest done first; the claimed Hub holds, and the other cities keep
/// their soldier until their own turn.
#[test]
fn one_trade_district_a_turn_goes_ahead_of_a_soldier_at_peace() {
    let soldier = unit("crossbowman");
    let (mut game, ranked) = empire("TRADEDISTRICTFIRST", &soldier);
    let ai = controller(VictoryTarget::Science, true);
    ai.claim_trade_district(&mut game, 0, &plan());
    let after = heads(&game, &ranked);
    assert_eq!(
        trade_districts(&game, &after),
        vec![0],
        "the most productive city claims first: {after:?}"
    );
    assert!(
        ai.trade_district_claim_holds(&game, 0, ranked[0], &plan(), after[0].as_ref().unwrap()),
        "the claimed district holds"
    );
    assert!(
        !ai.trade_district_claim_holds(&game, 0, ranked[1], &plan(), &soldier),
        "a soldier is not the claim"
    );
    ai.claim_trade_district(&mut game, 0, &plan());
    let after = heads(&game, &ranked);
    assert_eq!(
        trade_districts(&game, &after),
        vec![0, 1],
        "a second turn claims the second: {after:?}"
    );
    for (rank, head) in after.iter().enumerate().skip(2) {
        assert_eq!(head.as_ref(), Some(&soldier), "rank {rank}");
    }
}

/// A standing Commercial Hub claims its Market and a standing Harbor its
/// Lighthouse, both in the same turn and ahead of a University; a city that
/// already holds a Market claims nothing more.
#[test]
fn a_standing_district_claims_its_trade_building_past_a_university() {
    let university = building("university");
    let (mut game, ranked) = empire("TRADEBUILDING", &university);
    game.players[0]
        .techs
        .insert(Name::new("celestial_navigation"));
    install_test_district(&mut game, ranked[1], "commercial_hub");
    install_test_district(&mut game, ranked[2], "harbor");
    install_test_district(&mut game, ranked[3], "commercial_hub");
    game.cities
        .get_mut(&ranked[3])
        .unwrap()
        .buildings
        .push(Name::new("market"));
    let ai = controller(VictoryTarget::Diplomacy, true);
    ai.claim_trade_district(&mut game, 0, &plan());
    let after = heads(&game, &ranked);
    assert_eq!(after[1], Some(building("market")), "{after:?}");
    assert_eq!(after[2], Some(building("lighthouse")), "{after:?}");
    assert_eq!(after[3], Some(university.clone()), "{after:?}");
    assert!(ai.trade_district_claim_holds(&game, 0, ranked[1], &plan(), &building("market")));
    // The district claim does not pass a University: only the building does.
    for rank in [0, 4, 5] {
        assert_eq!(after[rank], Some(university.clone()), "rank {rank}");
    }
}

/// The second slot is the trade district's: an Industrial Zone nothing is
/// invested in gives way to the Hub; a Settler, a Builder, a Granary and a
/// Library keep their queue.
#[test]
fn an_unstarted_industrial_zone_waits_and_the_kept_heads_keep() {
    let (mut game, ranked) = empire("TRADEPASSES", &unit("settler"));
    game.players[0].techs.insert(Name::new("apprenticeship"));
    let zone_site = game
        .district_sites(ranked[5], Name::new("industrial_zone"))
        .into_iter()
        .next()
        .expect("an Industrial Zone site");
    let zone = Item::District {
        district: Name::new("industrial_zone"),
        pos: zone_site,
    };
    game.cities.get_mut(&ranked[5]).unwrap().queue = vec![zone.clone()];
    game.cities.get_mut(&ranked[1]).unwrap().queue = vec![unit("builder")];
    game.cities.get_mut(&ranked[2]).unwrap().queue = vec![building("granary")];
    game.cities.get_mut(&ranked[3]).unwrap().queue = vec![building("library")];
    let before = heads(&game, &ranked);
    let ai = controller(VictoryTarget::Science, true);
    for _ in 0..3 {
        ai.claim_trade_district(&mut game, 0, &plan());
    }
    let after = heads(&game, &ranked);
    assert_eq!(trade_districts(&game, &after), vec![5], "{after:?}");
    assert_ne!(after[5], Some(zone), "the Industrial Zone gives way");
    assert_eq!(after[..5], before[..5], "the kept heads keep their queue");
}

/// A city without its Library, a threatened city and a city attacked this
/// turn claim nothing; an idle queue is claimed like a soldier's.
#[test]
fn the_library_comes_first_and_a_threatened_city_keeps_its_queue() {
    let soldier = unit("crossbowman");
    let (mut game, ranked) = empire("TRADECALM", &soldier);
    let ai = controller(VictoryTarget::Science, true);
    // The cities of this board that can place a Hub in time.
    let open: Vec<usize> = (0..ranked.len())
        .filter(|rank| {
            ai.trade_claim_item(&game, 0, ranked[*rank], &plan())
                .is_some()
        })
        .collect();
    assert!(open.len() >= 5, "five cities can place a Hub: {open:?}");
    let (no_library, threatened, attacked, idle) = (open[0], open[1], open[2], open[3]);
    game.cities
        .get_mut(&ranked[no_library])
        .unwrap()
        .buildings
        .retain(|built| *built != "library");
    game.cities
        .get_mut(&ranked[attacked])
        .unwrap()
        .last_attacked = game.turn;
    game.cities.get_mut(&ranked[idle]).unwrap().queue.clear();
    let mut plan = plan();
    plan.threatened_city = Some(ranked[threatened]);
    let named = ai.trade_claim_item(&game, 0, ranked[idle], &plan);
    assert!(
        named
            .as_ref()
            .is_some_and(|(item, kind, _)| *kind == TradeClaim::District
                && AdvancedAi::is_trade_district(&game, item)),
        "the idle city names its district: {named:?}"
    );
    assert!(
        !ai.base.settler_due(&game, 0, ranked[idle], ranked.len(), 0),
        "no Settler is due at the city target"
    );
    for _ in 0..6 {
        ai.claim_trade_district(&mut game, 0, &plan);
    }
    let after = heads(&game, &ranked);
    assert_eq!(
        trade_districts(&game, &after),
        open[3..].to_vec(),
        "the calm cities with a Library claim, the idle one too: {after:?}"
    );
}

/// The gates: off, and on a Domination, Score, Culture or Religion seat,
/// nothing moves and nothing holds.
#[test]
fn the_gene_and_the_seat_gate_every_claim() {
    let soldier = unit("crossbowman");
    let (mut game, ranked) = empire("TRADEGATES", &soldier);
    install_test_district(&mut game, ranked[1], "commercial_hub");
    let before = heads(&game, &ranked);
    let market = building("market");
    let off = controller(VictoryTarget::Science, false);
    let mut board = game.clone();
    off.claim_trade_district(&mut board, 0, &plan());
    assert_eq!(heads(&board, &ranked), before, "off");
    assert!(!off.trade_district_claim_holds(&game, 0, ranked[1], &plan(), &market));
    for target in [
        VictoryTarget::Domination,
        VictoryTarget::Score,
        VictoryTarget::Culture,
        VictoryTarget::Religion,
    ] {
        let ai = controller(target, true);
        let mut board = game.clone();
        ai.claim_trade_district(&mut board, 0, &plan());
        assert_eq!(heads(&board, &ranked), before, "{target:?}");
        assert!(!ai.trade_district_claim_holds(&game, 0, ranked[1], &plan(), &market));
    }
    for target in [VictoryTarget::Science, VictoryTarget::Diplomacy] {
        let ai = controller(target, true);
        let mut board = game.clone();
        ai.claim_trade_district(&mut board, 0, &plan());
        let after = heads(&board, &ranked);
        assert_eq!(after[1], Some(market.clone()), "{target:?}: {after:?}");
        assert_eq!(trade_districts(&board, &after).len(), 1, "{target:?}");
    }
}

// ---- `culture-trade-district-follows-the-theater` ----

/// A Culture seat past its opening book, `culture-trade-district-follows-the-theater`
/// on or off (`trade-district-follows-the-campus` off).
fn culture_controller(gene: bool) -> AdvancedAi {
    let mut ai = controller(VictoryTarget::Culture, false);
    if gene {
        ai.enable_culture_trade_district_follows_the_theater();
    }
    ai
}

/// `empire` with Drama and Poetry and Humanism known, a Theater Square in
/// the cities ranked `theaters` and its Amphitheater in those ranked
/// `amphitheaters`.
fn culture_empire(
    tag: &str,
    head: &Item,
    theaters: &[usize],
    amphitheaters: &[usize],
) -> (Game, Vec<u32>) {
    let (mut game, ranked) = empire(tag, head);
    for civic in ["drama_poetry", "humanism"] {
        game.players[0].civics.insert(Name::new(civic));
    }
    for rank in theaters {
        install_test_district(&mut game, ranked[*rank], "theater_square");
    }
    for rank in amphitheaters {
        game.cities
            .get_mut(&ranked[*rank])
            .unwrap()
            .buildings
            .push(Name::new("amphitheater"));
    }
    (game, ranked)
}

#[test]
fn culture_trade_district_follows_the_theater_is_an_opt_in_off_in_both_controllers() {
    crate::ai::advanced::test_support::opt_in_off_in_both_controllers(
        "culture-trade-district-follows-the-theater",
        |ai| ai.culture_trade_district_follows_the_theater,
    );
}

/// On a Culture seat the Theater Square and its Amphitheater open the
/// district claim: a city with a Theater Square but no Amphitheater, and a
/// city with a Campus and its Library but no Theater Square, claim nothing.
#[test]
fn a_culture_seat_claims_its_trade_district_after_the_amphitheater() {
    let soldier = unit("crossbowman");
    let (mut game, ranked) =
        culture_empire("CULTURETRADEDISTRICT", &soldier, &[0, 1, 2, 3], &[0, 1, 2]);
    let ai = culture_controller(true);
    let open: Vec<usize> = (0..ranked.len())
        .filter(|rank| {
            ai.trade_claim_item(&game, 0, ranked[*rank], &plan())
                .is_some()
        })
        .collect();
    assert!(
        open.len() >= 2,
        "Amphitheater cities can place a Hub: {open:?}"
    );
    assert!(
        open.iter().all(|rank| *rank <= 2),
        "only the Amphitheater cities name a district: {open:?}"
    );
    for _ in 0..6 {
        ai.claim_trade_district(&mut game, 0, &plan());
    }
    let after = heads(&game, &ranked);
    assert_eq!(trade_districts(&game, &after), open, "{after:?}");
    for rank in 3..ranked.len() {
        assert_eq!(after[rank], Some(soldier.clone()), "rank {rank}");
    }
}

/// A Culture seat's Market passes a University but never a Theater
/// building or an unstarted Theater Square, nor a Builder; the Science
/// seat's gene, on the same board, passes all three Theater heads as
/// before.
#[test]
fn a_culture_seats_market_never_passes_a_theater_head() {
    let (mut game, ranked) =
        culture_empire("CULTURETRADEMARKET", &building("university"), &[1, 2], &[2]);
    let theater_site = game
        .district_sites(ranked[3], Name::new("theater_square"))
        .into_iter()
        .next()
        .expect("a Theater Square site");
    let theater = Item::District {
        district: Name::new("theater_square"),
        pos: theater_site,
    };
    for rank in 0..5 {
        install_test_district(&mut game, ranked[rank], "commercial_hub");
    }
    game.cities.get_mut(&ranked[1]).unwrap().queue = vec![building("amphitheater")];
    game.cities.get_mut(&ranked[2]).unwrap().queue = vec![building("art_museum")];
    game.cities.get_mut(&ranked[3]).unwrap().queue = vec![theater.clone()];
    game.cities.get_mut(&ranked[4]).unwrap().queue = vec![unit("builder")];
    let before = heads(&game, &ranked);

    let mut board = game.clone();
    culture_controller(true).claim_trade_district(&mut board, 0, &plan());
    let after = heads(&board, &ranked);
    assert_eq!(after[0], Some(building("market")), "{after:?}");
    assert_eq!(
        after[1..5],
        before[1..5],
        "Theater heads and the Builder keep"
    );

    let mut board = game.clone();
    controller(VictoryTarget::Science, true).claim_trade_district(&mut board, 0, &plan());
    let after = heads(&board, &ranked);
    for rank in 0..4 {
        assert_eq!(
            after[rank],
            Some(building("market")),
            "Science rank {rank}: {after:?}"
        );
    }
    assert_eq!(after[4], before[4], "the Builder keeps");
}

/// The Culture gene opens the Culture seat alone: off, nothing moves; on a
/// Science, Diplomatic or other seat it claims nothing, and with both genes
/// on a Science seat plays exactly as with the Science gene alone.
#[test]
fn the_culture_gene_gates_its_own_seat() {
    let soldier = unit("crossbowman");
    let (mut game, ranked) = culture_empire("CULTURETRADEGATES", &soldier, &[0, 1], &[0, 1]);
    install_test_district(&mut game, ranked[2], "commercial_hub");
    let before = heads(&game, &ranked);
    let market = building("market");
    let mut board = game.clone();
    let off = culture_controller(false);
    off.claim_trade_district(&mut board, 0, &plan());
    assert_eq!(heads(&board, &ranked), before, "off");
    assert!(!off.trade_district_claim_holds(&game, 0, ranked[2], &plan(), &market));
    for target in [
        VictoryTarget::Science,
        VictoryTarget::Diplomacy,
        VictoryTarget::Domination,
        VictoryTarget::Score,
        VictoryTarget::Religion,
    ] {
        let mut ai = controller(target, false);
        ai.enable_culture_trade_district_follows_the_theater();
        let mut board = game.clone();
        ai.claim_trade_district(&mut board, 0, &plan());
        assert_eq!(heads(&board, &ranked), before, "{target:?}");
    }
    let on = culture_controller(true);
    let mut board = game.clone();
    on.claim_trade_district(&mut board, 0, &plan());
    assert_eq!(heads(&board, &ranked)[2], Some(market.clone()));
    assert!(on.trade_district_claim_holds(&board, 0, ranked[2], &plan(), &market));
    for target in [VictoryTarget::Science, VictoryTarget::Diplomacy] {
        let science = controller(target, true);
        let mut both = controller(target, true);
        both.enable_culture_trade_district_follows_the_theater();
        let (mut alone, mut together) = (game.clone(), game.clone());
        for _ in 0..3 {
            science.claim_trade_district(&mut alone, 0, &plan());
            both.claim_trade_district(&mut together, 0, &plan());
        }
        assert_eq!(
            heads(&together, &ranked),
            heads(&alone, &ranked),
            "{target:?}"
        );
    }
}
