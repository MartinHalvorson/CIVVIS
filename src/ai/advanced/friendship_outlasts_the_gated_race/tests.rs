use super::*;
use crate::ai::advanced::launcher_war_every_lane::{DIPLOMATIC_LAUNCHER_EDGE, LANE_LAUNCHER_EDGE};
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::reasoning::Journal;

const GENE: &str = "friendship-outlasts-the-gated-race";

/// Three majors in contact, each with a capital, at turn 30: the protective
/// friendship fixture.
fn three_majors() -> Game {
    let mut game = Game::new_full(3, 24, 16, 7_923, 300, 0, false);
    for pid in 0..3 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("every major starts with a settler");
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
    }
    for pid in 0..3 {
        for other in pid + 1..3 {
            game.record_contact(pid, other);
        }
    }
    game.turn = 30;
    game
}

#[derive(Clone, Copy)]
struct Genes {
    keeps_the_friend: bool,
    denial_gate: bool,
    /// Turn this gene on and off again before the turn.
    toggled: bool,
}

const BOTH: Genes = Genes {
    keeps_the_friend: true,
    denial_gate: true,
    toggled: false,
};

/// Seat 1 was our declared friend until turn 31 and is racing to win; the
/// turn-30 frame records the expiry, then at turn 31 the live mirror drops
/// the friendship and our power is `ours` against its `theirs`. Returns the
/// board after seat 0's turn-31 friendship step, and the agent.
fn lapsed_racer(target: VictoryTarget, ours: f64, theirs: f64, genes: Genes) -> (Game, AdvancedAi) {
    lapsed_racer_on(&three_majors(), target, ours, theirs, genes)
}

/// [`lapsed_racer`] from a given board, so arms compared byte for byte
/// share one.
fn lapsed_racer_on(
    base: &Game,
    target: VictoryTarget,
    ours: f64,
    theirs: f64,
    genes: Genes,
) -> (Game, AdvancedAi) {
    let mut game = base.clone();
    let mut ai = AdvancedAi::targeting(target);
    ai.enable_befriend_the_strongest();
    ai.enable_friendship_keeps_the_peace();
    ai.enable_friendship_ends_at_the_race();
    if genes.keeps_the_friend || genes.toggled {
        ai.enable_friendship_outlasts_the_gated_race();
    }
    if genes.toggled {
        ai.disable_friendship_outlasts_the_gated_race();
    }
    if genes.denial_gate {
        ai.enable_denial_war_needs_the_lane_edge();
    }
    ai.base.journal = Journal::recording();
    game.players[0].friends_until.insert(1, 31);
    game.players[1].friends_until.insert(0, 31);
    ai.propose_protective_friendship(&mut game, 0);
    game.pending_deals.clear();
    game.turn = 31;
    game.players[0].friends_until.remove(&1);
    game.players[1].friends_until.remove(&0);
    game.players[1].dvp = 10;
    std::sync::Arc::make_mut(&mut game.observed_military_power).extend([(0, ours), (1, theirs)]);
    assert!(ai.friend_is_racing(&game, 1), "fixture: seat 1 is racing");
    ai.propose_protective_friendship(&mut game, 0);
    (game, ai)
}

fn renewed(game: &Game) -> bool {
    game.pending_deals
        .iter()
        .any(|deal| deal.from == 0 && deal.to == 1 && deal.friendship)
}

fn lines(ai: &AdvancedAi, headline: &str) -> Vec<String> {
    ai.journal()
        .since(0)
        .thoughts
        .into_iter()
        .filter(|thought| thought.headline.starts_with(headline))
        .map(|thought| thought.detail)
        .collect()
}

#[test]
fn the_gene_is_opt_in_and_ships_off_in_both_controllers() {
    opt_in_off_in_both_controllers(GENE, |ai| ai.friendship_outlasts_the_gated_race);
}

/// Live 202929Z t134: Georgia's friendship lapsed while it raced, at 352
/// power against 383, and the race rule let it go.
#[test]
fn a_racing_friend_under_the_edge_is_renewed() {
    let off = Genes {
        keeps_the_friend: false,
        denial_gate: true,
        toggled: false,
    };
    let (game, ai) = lapsed_racer(VictoryTarget::Culture, 352.0, 383.0, off);
    assert!(!renewed(&game), "control: the race ends the friendship");
    let held = lines(&ai, "Not renewing the friendship");
    assert_eq!(held.len(), 1, "{held:?}");
    assert!(
        held[0].contains("friendship-ends-at-the-race"),
        "{}",
        held[0]
    );
    assert!(lines(&ai, "Keeping the friendship").is_empty());

    let (game, ai) = lapsed_racer(VictoryTarget::Culture, 352.0, 383.0, BOTH);
    assert!(renewed(&game), "the gene: the friend is asked again");
    assert!(lines(&ai, "Not renewing the friendship").is_empty());
    let civ = &game.players[1].civ;
    let kept: Vec<_> = ai
        .journal()
        .since(0)
        .thoughts
        .into_iter()
        .filter(|thought| thought.detail.contains(GENE))
        .collect();
    assert_eq!(kept.len(), 1, "one line");
    assert_eq!(
        kept[0].headline,
        format!("Keeping the friendship with {civ}")
    );
    assert_eq!(
        kept[0].detail,
        "friendship-outlasts-the-gated-race: they are racing to win, but a war on them \
         needs 1.2 times their steady power; 352 against 383"
    );
    assert_eq!(lines(&ai, "Renewing the friendship").len(), 1);
}

#[test]
fn at_the_lane_edge_the_race_still_ends_the_friendship() {
    for (target, edge) in [
        (VictoryTarget::Culture, LANE_LAUNCHER_EDGE),
        (VictoryTarget::Religion, LANE_LAUNCHER_EDGE),
        (VictoryTarget::Science, LANE_LAUNCHER_EDGE),
        (VictoryTarget::Diplomacy, DIPLOMATIC_LAUNCHER_EDGE),
    ] {
        let at = edge * 100.0;
        for (ours, kept) in [(at - 0.01, true), (at, false), (at + 0.01, false)] {
            let (game, ai) = lapsed_racer(target, ours, 100.0, BOTH);
            assert_eq!(renewed(&game), kept, "{target:?} at {ours}");
            assert_eq!(
                lines(&ai, "Keeping the friendship").len(),
                usize::from(kept),
                "{target:?} at {ours}"
            );
        }
    }
}

/// The gene needs `denial-war-needs-the-lane-edge`: without the gate the
/// war it would refuse is not refused, and the race rule stands.
#[test]
fn without_the_denial_gate_the_race_still_ends_the_friendship() {
    let alone = Genes {
        keeps_the_friend: true,
        denial_gate: false,
        toggled: false,
    };
    let (game, ai) = lapsed_racer(VictoryTarget::Culture, 352.0, 383.0, alone);
    assert!(!renewed(&game));
    assert!(lines(&ai, "Keeping the friendship").is_empty());
}

/// A Domination seat neither renews a lapsed friend at once nor reads the
/// gate, so the gene changes nothing there.
#[test]
fn a_domination_seat_is_unchanged() {
    let (game, ai) = lapsed_racer(VictoryTarget::Domination, 352.0, 383.0, BOTH);
    assert!(!renewed(&game));
    assert!(lines(&ai, "Keeping the friendship").is_empty());
    let mut lapsed = BTreeSet::new();
    lapsed.insert(1);
    assert_eq!(ai.gated_race_keeps_the_friend(&game, 0, 1, &lapsed), None);
}

/// With either gene off the friendship step plays byte for byte as before:
/// this gene on without the denial gate, and this gene turned on and off
/// again beside the gate, against the same board without it.
#[test]
fn off_either_gene_the_turn_is_byte_identical() {
    let base = three_majors();
    let snapshot = |genes: Genes, target: VictoryTarget, ours: f64| {
        let (game, _) = lapsed_racer_on(&base, target, ours, 100.0, genes);
        serde_json::to_value(&game).unwrap()
    };
    let gene = |keeps_the_friend: bool, denial_gate: bool, toggled: bool| Genes {
        keeps_the_friend,
        denial_gate,
        toggled,
    };
    for target in [
        VictoryTarget::Science,
        VictoryTarget::Culture,
        VictoryTarget::Religion,
        VictoryTarget::Diplomacy,
        VictoryTarget::Domination,
    ] {
        for ours in [50.0, 500.0] {
            assert!(
                snapshot(gene(true, false, false), target, ours)
                    == snapshot(gene(false, false, false), target, ours),
                "{target:?} at {ours}: this gene alone"
            );
            assert!(
                snapshot(gene(false, true, true), target, ours)
                    == snapshot(gene(false, true, false), target, ours),
                "{target:?} at {ours}: this gene toggled off beside the gate"
            );
        }
    }
}
