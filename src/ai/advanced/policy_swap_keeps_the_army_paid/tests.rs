use super::*;
use crate::game::TradeRoute;

/// Two majors on grassland, our city at (4, 4) under Chiefdom (one Military
/// and one Economic slot) with six Archers (1 Gold upkeep each) and
/// Conscription slotted (-1 Gold a unit: 6 Gold a turn). Rapid Deployment
/// unlocks Military First and Ideology Five-Year Plan, both on the Conquest
/// deck; neither Conscription nor Caravansaries is.
fn fixture(gold_per_turn: f64) -> Game {
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
    g.players[0].government = Some("chiefdom".to_string());
    g.players[0].civics.extend([
        crate::name!("code_of_laws"),
        crate::name!("state_workforce"),
        crate::name!("foreign_trade"),
        crate::name!("rapid_deployment"),
        crate::name!("ideology"),
    ]);
    for pos in [(4, 4), (4, 5), (5, 4), (5, 5), (3, 4), (3, 5)] {
        g.spawn_test_unit("archer", 0, pos);
    }
    g.players[0].policies = BTreeSet::from([crate::name!("conscription")]);
    g.players[0].gold = 300.0;
    g.players[0].gold_per_turn = gold_per_turn;
    g
}

/// Three of our routes: Caravansaries pays 2 Gold each, 6 a turn.
fn add_routes(g: &mut Game) {
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
}

/// The AI for one arm, its turn-start deck read from `g` and its journal
/// recording.
fn arm(g: &Game, on: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    if on {
        ai.enable_policy_swap_keeps_the_army_paid();
    }
    ai.turn_start_policies = g.players[0].policies.clone();
    ai.base.journal = crate::reasoning::Journal::recording();
    ai
}

fn holds(g: &Game, card: &str) -> bool {
    g.players[0].policies.contains(&Name::new(card))
}

fn journalled(ai: &AdvancedAi, text: &str) -> bool {
    ai.journal()
        .since(0)
        .thoughts
        .iter()
        .any(|thought| thought.detail.contains(text))
}

/// The projection reads the turn's income, each card's upkeep relief and its
/// Gold; Economic Union is priced at the pass's wartime marginal when there
/// is one. Off, nothing is computed.
#[test]
fn the_projection_prices_upkeep_relief_and_gold() {
    let mut g = fixture(4.0);
    add_routes(&mut g);
    assert!(arm(&g, false).army_pay(&g, 0, &[], None).is_none(), "off");
    let ai = arm(&g, true);
    let pay = ai
        .army_pay(&g, 0, &["caravansaries", "economic_union"], None)
        .unwrap();
    assert_eq!(pay.income("conscription"), 6.0);
    assert_eq!(pay.income("caravansaries"), 6.0);
    assert_eq!(
        pay.income("economic_union"),
        0.0,
        "adjacency Gold reads zero"
    );
    assert_eq!(pay.income("military_first"), 0.0);
    assert_eq!(pay.projected(&g.players[0].policies), 4.0);
    assert_eq!(pay.projected(&BTreeSet::new()), -2.0);
    let priced = ai.army_pay(&g, 0, &["economic_union"], Some(12.0)).unwrap();
    assert_eq!(priced.income("economic_union"), 12.0);
}

/// +4 a turn with 6 Gold of upkeep relief in Conscription: giving it up for
/// Military First would take income to -2. Off the card goes; on it stays.
#[test]
fn an_upkeep_card_whose_loss_turns_income_negative_stays() {
    let g = fixture(4.0);

    let mut off = g.clone();
    arm(&g, false).strategic_policies(&mut off, 0, GrandStrategy::Conquest);
    assert!(
        holds(&off, "military_first"),
        "off: the plan's card takes the slot"
    );
    assert!(!holds(&off, "conscription"));

    let ai = arm(&g, true);
    let mut on = g.clone();
    ai.strategic_policies(&mut on, 0, GrandStrategy::Conquest);
    assert!(holds(&on, "conscription"), "on: the upkeep card stays");
    assert!(!holds(&on, "military_first"));
    assert!(journalled(
        &ai,
        "policy-swap-keeps-the-army-paid: +4.0 Gold a turn would fall to -2.0 \
         without its 6 upkeep; military first would have taken its slot"
    ));
}

/// +10 a turn: without Conscription income is still +4, so the swap goes
/// through in both arms and nothing is journalled.
#[test]
fn income_that_stays_positive_swaps_in_both_arms() {
    let g = fixture(10.0);
    for on in [false, true] {
        let ai = arm(&g, on);
        let mut board = g.clone();
        ai.strategic_policies(&mut board, 0, GrandStrategy::Conquest);
        assert!(holds(&board, "military_first"), "on={on}");
        assert!(!holds(&board, "conscription"), "on={on}");
        assert!(
            !journalled(&ai, "policy-swap-keeps-the-army-paid"),
            "on={on}"
        );
    }
}

/// +8 a turn, Conscription (6 upkeep) and Caravansaries (6 Gold) both slotted
/// and both unwanted. Each drop alone leaves +2; both leave -4. Off both go;
/// on the second drop is projected after the first and its card stays.
#[test]
fn two_drops_in_one_pass_are_projected_together() {
    let mut g = fixture(8.0);
    add_routes(&mut g);
    g.players[0].policies.insert(crate::name!("caravansaries"));
    let ai = arm(&g, true);
    let pay = ai.army_pay(&g, 0, &[], None).unwrap();
    let deck = &g.players[0].policies;
    assert_eq!(pay.keeps(deck, "conscription", "military_first"), None);
    assert_eq!(pay.keeps(deck, "caravansaries", "five_year_plan"), None);

    let mut off = g.clone();
    arm(&g, false).strategic_policies(&mut off, 0, GrandStrategy::Conquest);
    assert!(holds(&off, "military_first") && holds(&off, "five_year_plan"));
    assert!(!holds(&off, "conscription") && !holds(&off, "caravansaries"));

    let mut on = g.clone();
    ai.strategic_policies(&mut on, 0, GrandStrategy::Conquest);
    let kept = ["conscription", "caravansaries"]
        .into_iter()
        .filter(|card| holds(&on, card))
        .count();
    assert_eq!(kept, 1, "{:?}", on.players[0].policies);
    assert!(journalled(&ai, "+8.0 Gold a turn would fall to -4.0"));
}

/// The base deck review gave Conscription's slot to Discipline before the
/// pass ran (the live t192 shape). At +4 the deck the review left projects
/// -2, so the pass puts Conscription back; at +10 it projects +4 and the
/// slot goes to the plan's card. Off, Conscription stays out.
#[test]
fn a_card_the_deck_review_gave_up_comes_back_while_income_projects_at_or_below_zero() {
    let mut g = fixture(4.0);
    let start = g.players[0].policies.clone();
    g.players[0].policies = BTreeSet::from([crate::name!("discipline")]);

    let mut off = g.clone();
    let mut stock = arm(&g, false);
    stock.turn_start_policies = start.clone();
    stock.strategic_policies(&mut off, 0, GrandStrategy::Conquest);
    assert!(
        !holds(&off, "conscription"),
        "off: the review's drop stands"
    );

    let mut ai = arm(&g, true);
    ai.turn_start_policies = start.clone();
    let mut on = g.clone();
    ai.strategic_policies(&mut on, 0, GrandStrategy::Conquest);
    assert!(holds(&on, "conscription"), "{:?}", on.players[0].policies);
    assert!(!holds(&on, "discipline"));
    assert!(journalled(
        &ai,
        "policy-swap-keeps-the-army-paid: +4.0 Gold a turn would fall to -2.0 \
         without its 6 upkeep; the deck review had already given its slot away"
    ));

    g.players[0].gold_per_turn = 10.0;
    let mut ai = arm(&g, true);
    ai.turn_start_policies = start;
    let mut rich = g.clone();
    ai.strategic_policies(&mut rich, 0, GrandStrategy::Conquest);
    assert!(!holds(&rich, "conscription"));
    assert!(holds(&rich, "military_first"));
}
