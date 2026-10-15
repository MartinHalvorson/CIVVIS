use super::*;
use crate::game::TradeRoute;

/// Two majors on grassland, our city at (4, 4) with two Archers (one its
/// garrison), a Domination plan on the rival's city, Machinery researched so
/// each Archer can become a Crossbowman (+2 upkeep each).
fn fixture() -> (Game, StrategicPlan, Vec<u32>) {
    let mut g = Game::new_full(2, 24, 16, 366_400, 250, 0, false);
    g.units.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    g.found_city_for(0, (4, 4), None);
    g.found_city_for(1, (18, 10), None);
    g.current = 0;
    g.turn = 125;
    g.players[0].techs.insert(crate::name!("machinery"));
    g.players[0].gold = 1000.0;
    g.players[0].gold_per_turn = 18.0;
    let units = [(4, 4), (4, 5)]
        .into_iter()
        .map(|pos| g.spawn_test_unit("archer", 0, pos))
        .collect();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(g.player_city_ids(1)[0]),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, plan, units)
}

fn crossbows(g: &Game, units: &[u32]) -> usize {
    units
        .iter()
        .filter(|uid| g.units[*uid].kind == "crossbowman")
        .count()
}

/// Income +3 carries one +2 upgrade and not a second; income 0 carries none;
/// off, both are bought.
#[test]
fn the_modernizer_holds_upgrades_the_income_cannot_carry() {
    let (mut g, plan, units) = fixture();
    g.players[0].gold_per_turn = 3.0;
    let target = g.unit_gold_upgrade_offer(0, units[0]).unwrap().0;
    assert_eq!(AdvancedAi::upgrade_upkeep_delta(&g, units[0], target), 2.0);

    let stock = AdvancedAi::targeting(VictoryTarget::Domination);
    let mut off = g.clone();
    stock.fund_domination_upgrades(&mut off, 0, &plan);
    assert_eq!(crossbows(&off, &units), 2, "off: both upgraded");

    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_solvent_or_hold();
    ai.base.journal = crate::reasoning::Journal::recording();
    let mut on = g.clone();
    ai.fund_domination_upgrades(&mut on, 0, &plan);
    assert_eq!(
        crossbows(&on, &units),
        1,
        "+3 a turn carries one +2 upgrade"
    );
    assert!(
        ai.journal()
            .since(0)
            .thoughts
            .iter()
            .any(|thought| thought.detail.contains("solvent-or-hold")),
        "the hold is journalled"
    );

    let mut broke = g.clone();
    broke.players[0].gold_per_turn = 0.0;
    ai.fund_domination_upgrades(&mut broke, 0, &plan);
    assert_eq!(crossbows(&broke, &units), 0, "no income, no added upkeep");
}

/// While income is not positive an amenity or upkeep card gives its slot only
/// to a card of the same kind.
#[test]
fn the_guard_keeps_amenity_and_upkeep_cards_while_income_is_not_positive() {
    let (mut g, _, _) = fixture();
    assert_eq!(
        AdvancedAi::solvency_card_kind(&g, "liberalism"),
        Some("amenity")
    );
    assert_eq!(
        AdvancedAi::solvency_card_kind(&g, "retainers"),
        Some("amenity")
    );
    assert_eq!(
        AdvancedAi::solvency_card_kind(&g, "conscription"),
        Some("upkeep")
    );
    assert_eq!(
        AdvancedAi::solvency_card_kind(&g, "levee_en_masse"),
        Some("upkeep")
    );
    assert_eq!(AdvancedAi::solvency_card_kind(&g, "caravansaries"), None);

    g.players[0].gold_per_turn = -4.0;
    let mut ai = AdvancedAi::new();
    assert!(
        !ai.solvency_keeps_card(&g, 0, "liberalism", "caravansaries"),
        "off"
    );
    ai.enable_solvent_or_hold();
    assert!(ai.solvency_keeps_card(&g, 0, "liberalism", "caravansaries"));
    assert!(ai.solvency_keeps_card(&g, 0, "conscription", "professional_army"));
    assert!(
        !ai.solvency_keeps_card(&g, 0, "liberalism", "new_deal"),
        "an amenity card may give way to an amenity card"
    );
    assert!(
        !ai.solvency_keeps_card(&g, 0, "conscription", "levee_en_masse"),
        "an upkeep card may give way to an upkeep card"
    );
    assert!(!ai.solvency_keeps_card(&g, 0, "caravansaries", "liberalism"));
    g.players[0].gold_per_turn = 5.0;
    assert!(
        !ai.solvency_keeps_card(&g, 0, "liberalism", "caravansaries"),
        "positive income: the ordinary pass"
    );
}

/// Drained: the unit with the least strength per Gold of upkeep goes first
/// until income stops running negative, at most three a turn; the garrison
/// and the siege gun stay; off, or with the deficit covered, nothing moves.
#[test]
fn the_drain_disbands_the_cheapest_strength_and_spares_garrisons_and_guns() {
    let (mut g, _, units) = fixture();
    let knight = g.spawn_test_unit("knight", 0, (6, 6));
    let spear = g.spawn_test_unit("spearman", 0, (6, 8));
    let gun = g.spawn_test_unit("catapult", 0, (8, 6));
    g.players[0].gold = 0.0;
    g.players[0].gold_per_turn = -2.0;
    let ours = |g: &Game| g.player_unit_ids(0).len();
    let before = ours(&g);

    let mut off = g.clone();
    AdvancedAi::new().exit_the_drain(&mut off, 0);
    assert_eq!(ours(&off), before, "off: nothing disbanded");

    let mut ai = AdvancedAi::new();
    ai.enable_bankruptcy_exits_the_drain();
    ai.base.journal = crate::reasoning::Journal::recording();
    let mut small = g.clone();
    ai.exit_the_drain(&mut small, 0);
    assert!(
        !small.units.contains_key(&knight),
        "the knight is 12.5 strength a Gold of upkeep, the least"
    );
    assert_eq!(
        ours(&small),
        before - 1,
        "its 4 upkeep covers the 2 deficit"
    );
    assert!(ai
        .journal()
        .since(0)
        .thoughts
        .iter()
        .any(|thought| thought.detail.contains("bankruptcy-exits-the-drain")));

    let mut deep = g.clone();
    deep.players[0].gold_per_turn = -40.0;
    ai.exit_the_drain(&mut deep, 0);
    assert_eq!(ours(&deep), before - 3, "three a turn at most");
    assert!(deep.units.contains_key(&units[0]), "the garrison stays");
    assert!(deep.units.contains_key(&gun), "the siege gun stays");
    assert!(!deep.units.contains_key(&spear) && !deep.units.contains_key(&units[1]));

    let mut covered = g.clone();
    covered.players[0].gold = 50.0;
    ai.exit_the_drain(&mut covered, 0);
    assert_eq!(ours(&covered), before, "the treasury covers the deficit");
}

/// Drained, Caravansaries is worth 2 Gold a route; with the deficit covered
/// or the gene off, no card is pushed.
#[test]
fn drained_gold_cards_are_priced_by_our_routes() {
    let (mut g, _, _) = fixture();
    g.players[0].civics.insert(crate::name!("foreign_trade"));
    let origin = g.player_city_ids(0)[0];
    let dest = g.player_city_ids(1)[0];
    for _ in 0..3 {
        g.routes.push(TradeRoute {
            origin,
            dest,
            owner: 0,
            ends: g.turn + 20,
        });
    }
    g.players[0].gold = 0.0;
    g.players[0].gold_per_turn = -5.0;
    let mut ai = AdvancedAi::new();
    assert!(ai.drain_gold_cards(&g, 0).is_empty(), "off");
    ai.enable_bankruptcy_exits_the_drain();
    let cards = ai.drain_gold_cards(&g, 0);
    assert!(
        cards
            .iter()
            .any(|(card, gold)| *card == "caravansaries" && (*gold - 6.0).abs() < 1e-9),
        "{cards:?}"
    );
    g.players[0].gold = 100.0;
    assert!(ai.drain_gold_cards(&g, 0).is_empty(), "covered");
}

/// The action removes our own unit and refuses a rival's.
#[test]
fn disband_removes_only_our_own_unit() {
    let (mut g, _, units) = fixture();
    let theirs = g.spawn_test_unit("archer", 1, (17, 10));
    assert!(g.apply(0, &Action::DisbandUnit { unit: theirs }).is_err());
    assert!(g.units.contains_key(&theirs));
    g.apply(0, &Action::DisbandUnit { unit: units[1] }).unwrap();
    assert!(!g.units.contains_key(&units[1]));
}

/// A unit an invested siege needs stays even when it is the cheapest
/// strength to lose; the next candidate off the ring goes instead.
#[test]
fn the_drain_spares_a_unit_the_siege_needs() {
    let (mut g, _, units) = fixture();
    let rival = g.player_city_ids(1)[0];
    let ringed = g.spawn_test_unit("knight", 0, (16, 10));
    g.players[0].gold = 0.0;
    g.players[0].gold_per_turn = -1.0;
    let mut ai = AdvancedAi::new();
    ai.enable_bankruptcy_exits_the_drain();
    let mut free = g.clone();
    ai.exit_the_drain(&mut free, 0);
    assert!(
        !free.units.contains_key(&ringed),
        "fixture: with no siege the knight is the cheapest strength to lose"
    );
    ai.sieges.insert(
        rival,
        super::super::siege_train::Siege {
            stage: super::super::siege_train::SiegeStage::Invest,
            taker: None,
            entered: g.turn,
            assessed: g.turn,
            posts: BTreeMap::new(),
            short_since: None,
        },
    );
    // The 24-wide test map wraps, so the second Archer stands 10 tiles from
    // the rival city and the siege keeps it too; a Spearman beyond the muster
    // radius is the one off the ring.
    let away = g.spawn_test_unit("spearman", 0, (6, 1));
    ai.exit_the_drain(&mut g, 0);
    assert!(g.units.contains_key(&ringed), "the siege needs it");
    assert!(
        g.units.contains_key(&units[1]),
        "within the muster radius too"
    );
    assert!(
        !g.units.contains_key(&away),
        "the unit off the ring goes instead"
    );
    assert!(g.units.contains_key(&units[0]), "the garrison stays");
}

/// A gold card that covers the deficit may take an amenity card's slot; one
/// that does not, may not.
#[test]
fn a_gold_card_that_covers_the_deficit_may_take_an_amenity_slot() {
    let (mut g, _, _) = fixture();
    let origin = g.player_city_ids(0)[0];
    let dest = g.player_city_ids(1)[0];
    for _ in 0..3 {
        g.routes.push(TradeRoute {
            origin,
            dest,
            owner: 0,
            ends: g.turn + 20,
        });
    }
    assert_eq!(AdvancedAi::card_gold_estimate(&g, 0, "caravansaries"), 6.0);
    let mut ai = AdvancedAi::new();
    ai.enable_solvent_or_hold();
    g.players[0].gold_per_turn = -4.0;
    assert!(
        !ai.solvency_keeps_card(&g, 0, "liberalism", "caravansaries"),
        "6 Gold covers a 4 deficit"
    );
    g.players[0].gold_per_turn = -10.0;
    assert!(
        ai.solvency_keeps_card(&g, 0, "liberalism", "caravansaries"),
        "6 Gold does not cover a 10 deficit"
    );
}

/// The turn a gold card is slotted for the drain, nothing is disbanded.
#[test]
fn the_drain_waits_a_turn_for_a_slotted_gold_card() {
    let (mut g, _, _) = fixture();
    g.spawn_test_unit("knight", 0, (6, 6));
    g.players[0].gold = 0.0;
    g.players[0].gold_per_turn = -2.0;
    let mut ai = AdvancedAi::new();
    ai.enable_bankruptcy_exits_the_drain();
    ai.drain_card_turn.set(Some(g.turn));
    let before = g.player_unit_ids(0).len();
    ai.exit_the_drain(&mut g, 0);
    assert_eq!(g.player_unit_ids(0).len(), before);
}

/// A caller-priced card (Economic Union's wartime marginal) is judged by that
/// price: it takes an amenity card's slot when it covers the deficit.
#[test]
fn a_caller_priced_gold_card_is_judged_by_its_price() {
    let (mut g, _, _) = fixture();
    g.players[0].gold_per_turn = -9.0;
    let mut ai = AdvancedAi::new();
    ai.enable_solvent_or_hold();
    assert!(
        ai.solvency_keeps_card(&g, 0, "civil_prestige", "economic_union"),
        "unpriced, its adjacency Gold reads zero"
    );
    assert!(!ai.solvency_keeps_card_priced(&g, 0, "civil_prestige", "economic_union", Some(12.0)));
    assert!(ai.solvency_keeps_card_priced(&g, 0, "civil_prestige", "economic_union", Some(5.0)));
}
