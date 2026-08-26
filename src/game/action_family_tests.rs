use super::*;
use crate::ai::{AdvancedAi, Ai};

const FAMILIES: [ActionFamilies; 6] = [
    ActionFamilies::CORE,
    ActionFamilies::UNITS,
    ActionFamilies::PURCHASES,
    ActionFamilies::EMPIRE,
    ActionFamilies::DEALS,
    ActionFamilies::DIPLOMACY,
];
const OPTIONAL_FAMILIES: [ActionFamilies; 3] = [
    ActionFamilies::CORPORATIONS,
    ActionFamilies::PRODUCTS,
    ActionFamilies::FORMATIONS,
];

fn labels(actions: &[Action]) -> Vec<String> {
    actions.iter().map(|action| format!("{action:?}")).collect()
}

/// Is `part` a subsequence of `whole`? Skipping a family must only remove
/// actions; it must never add one or reorder the rest, because the order
/// of this list is part of what makes a game deterministic.
fn is_subsequence(part: &[String], whole: &[String]) -> bool {
    let mut rest = whole.iter();
    part.iter().all(|item| rest.any(|other| other == item))
}

/// A settled position: cities to buy in, units to order, and rivals to
/// deal with.
///
/// The last of those is the one that cannot be had by naming a turn. A
/// congress sits for only part of its cycle and a trade route needs a
/// trader already in the field, so which turn has a deal on the table
/// depends on the map and on how the AI played it. Stopping at a chosen
/// turn made a rich position a coincidence, and any change to either could
/// move it — turn 60 held one until the generator started filling lakes,
/// and turns 65, 70, 80 and 90 all did while 75 and 100 did not. Play on
/// until a deal is genuinely available instead.
/// Whether dropping any one of the six families from the full enumeration
/// removes at least one action — the position the partition test needs.
///
/// The fixture used to stop at the first settled position that offered a
/// deal and a diplomatic action, and nothing else. Which position that is
/// depends on every rule the played-out game runs through: #2573 let units
/// pass through their own units, the four AIs took different roads to turn
/// 60, and the first position with a deal and a denouncement on offer had
/// nothing left in `CORE` to drop — no research, civic or production choice
/// pending on that exact turn — so the partition test failed on `main` with
/// no change to the partition. Asking the fixture for the property the test
/// asserts keeps it from breaking on the next unrelated engine change.
fn every_family_drops_something(game: &Game, pid: usize) -> bool {
    let all = game.legal_actions_within(pid, ActionFamilies::ALL).len();
    FAMILIES.iter().all(|family| {
        game.legal_actions_within(pid, ActionFamilies::ALL.without(*family))
            .len()
            < all
    })
}

fn played_in_game() -> Game {
    let mut game = Game::new_with(GameOptions::new(4, 32, 22, 90_002, 400, 4));
    let mut ais = AdvancedAi::fleet(&game);
    while game.turn < 120 && game.winner.is_none() {
        let pid = game.current;
        ais[pid].take_turn(&mut game, pid);
        if game.winner.is_none() && game.current == pid {
            let _ = game.apply(pid, &Action::EndTurn);
        }
        let settled = game.turn >= 60 && game.winner.is_none();
        let deals = game.legal_actions_within(game.current, ActionFamilies::DEALS);
        let diplomacy =
            game.legal_actions_within(game.current, ActionFamilies::DIPLOMACY);
        if settled
            && every_family_drops_something(&game, game.current)
            && deals.iter().any(|action| {
                matches!(
                    action,
                    Action::AcceptDeal { .. }
                        | Action::RejectDeal { .. }
                        | Action::Trade { .. }
                        | Action::ChooseDedication { .. }
                        | Action::CongressVote { .. }
                )
            })
            && diplomacy.iter().any(|action| {
                matches!(
                    action,
                    Action::MakePeace { .. }
                        | Action::DeclareWar { .. }
                        | Action::DeclareWarWithCasusBelli { .. }
                        | Action::Denounce { .. }
                        | Action::SendDelegation { .. }
                        | Action::SendEmbassy { .. }
                        | Action::DemandGold { .. }
                        | Action::RequestPromise { .. }
                        | Action::ProposeDeal { .. }
                        | Action::ProposeDefensivePact { .. }
                        | Action::ProposeJointWar { .. }
                )
            })
        {
            break;
        }
    }
    game
}


#[test]
fn action_families_partition_the_full_enumeration() {
    let game = played_in_game();
    assert!(game.winner.is_none(), "the position must still be live");
    let pid = game.current;
    let all = labels(&game.legal_actions(pid));
    assert!(all.len() > 40, "expected a rich position, got {}", all.len());

    // Asking for everything is the enumeration callers already relied on.
    assert_eq!(labels(&game.legal_actions_within(pid, ActionFamilies::ALL)), all);

    // Dropping one family only ever removes actions.
    for family in FAMILIES {
        let without = labels(&game.legal_actions_within(pid, ActionFamilies::ALL.without(family)));
        assert!(
            is_subsequence(&without, &all),
            "dropping {family:?} added or reordered actions"
        );
        assert!(
            without.len() < all.len(),
            "dropping {family:?} removed nothing, so the gate is in the wrong place"
        );
    }

    // The cheap core plus the six families cover every action, so no
    // caller can lose one by naming the family it wants.
    let mut covered: BTreeSet<String> =
        labels(&game.legal_actions_within(pid, ActionFamilies::CHEAP))
            .into_iter()
            .collect();
    for family in FAMILIES.into_iter().chain(OPTIONAL_FAMILIES) {
        covered.extend(labels(&game.legal_actions_within(pid, family)));
    }
    assert_eq!(covered, all.into_iter().collect::<BTreeSet<_>>());
}

/// Each narrowed call site in the AI must still see every action of the
/// kind it filters for.
#[test]
fn narrowed_call_sites_still_see_their_own_kinds() {
    let game = played_in_game();
    let pid = game.current;
    let all = game.legal_actions(pid);
    type ActionCase = (ActionFamilies, fn(&Action) -> bool);
    let cases: [ActionCase; 10] = [
        (ActionFamilies::CHEAP, |action| {
            matches!(
                action,
                Action::CityStrike { .. }
                    | Action::EncampmentStrike { .. }
                    | Action::DeclareWar { .. }
                    | Action::DeclareWarWithCasusBelli { .. }
            )
        }),
        (ActionFamilies::CORE, |action| {
            matches!(
                action,
                Action::UpgradeUnit { .. }
                    | Action::AssignSpy { .. }
                    | Action::SpyMission { .. }
                    | Action::PromoteSpy { .. }
                    | Action::LevyMilitary { .. }
                    | Action::Research { .. }
                    | Action::Civic { .. }
                    | Action::Fortify { .. }
                    | Action::CityStrike { .. }
                    | Action::EncampmentStrike { .. }
            )
        }),
        (ActionFamilies::UNITS, |action| {
            matches!(
                action,
                Action::Move { .. }
                    | Action::Attack { .. }
                    | Action::Ranged { .. }
                    | Action::Spread { .. }
                    | Action::RemoveHeresy { .. }
                    | Action::TheologicalAttack { .. }
            )
        }),
        (
            ActionFamilies::PURCHASES | ActionFamilies::EMPIRE,
            |action| {
                matches!(
                    action,
                    Action::Buy { .. }
                        | Action::BuyBuilding { .. }
                        | Action::BuyDistrict { .. }
                        | Action::BuyPlot { .. }
                )
            },
        ),
        (ActionFamilies::EMPIRE, |action| {
            matches!(action, Action::WmdStrike { .. } | Action::SendEnvoy { .. })
        }),
        (ActionFamilies::DEALS, |action| {
            matches!(action, Action::Trade { .. } | Action::CongressVote { .. })
        }),
        (ActionFamilies::DIPLOMACY, |action| {
            matches!(
                action,
                Action::MakePeace { .. }
                    | Action::DeclareWar { .. }
                    | Action::DeclareWarWithCasusBelli { .. }
                    | Action::Denounce { .. }
                    | Action::SendDelegation { .. }
                    | Action::SendEmbassy { .. }
                    | Action::DemandGold { .. }
                    | Action::RequestPromise { .. }
                    | Action::ProposeDeal { .. }
                    | Action::ProposeDefensivePact { .. }
                    | Action::ProposeJointWar { .. }
            )
        }),
        (ActionFamilies::CORPORATIONS, |action| {
            matches!(action, Action::FoundCorporation { .. })
        }),
        (ActionFamilies::PRODUCTS, |action| {
            matches!(action, Action::MoveProduct { .. })
        }),
        (ActionFamilies::FORMATIONS, |action| {
            matches!(
                action,
                Action::CombineUnits { .. } | Action::LinkUnits { .. }
            )
        }),
    ];
    for (families, wanted) in cases {
        let narrow: Vec<String> = labels(
            &game
                .legal_actions_within(pid, families)
                .into_iter()
                .filter(wanted)
                .collect::<Vec<_>>(),
        );
        let full: Vec<String> = labels(
            &all.iter()
                .filter(|action| wanted(action))
                .cloned()
                .collect::<Vec<_>>(),
        );
        assert_eq!(narrow, full, "{families:?} lost an action it promised");
    }
}
