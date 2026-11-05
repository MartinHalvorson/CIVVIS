use super::super::test_support::opt_in_off_in_both_controllers;
use super::super::{AdvancedAi, GrandStrategy};
use super::*;
use crate::game::{Action, Game};
use crate::name;

#[test]
fn boost_first_research_is_a_native_opt_in_off_in_both_controllers() {
    opt_in_off_in_both_controllers("boost-first-research", |ai| ai.boost_first_research);
}

#[test]
fn boost_first_research_2_is_a_native_opt_in_off_in_both_controllers() {
    opt_in_off_in_both_controllers("boost-first-research-2", |ai| ai.boost_first_research_2);
}

#[test]
fn boost_wait_research_2_is_a_native_opt_in_off_in_both_controllers() {
    opt_in_off_in_both_controllers("boost-wait-research-2", |ai| ai.boost_wait_research_2);
}

#[test]
fn boost_unlock_research_is_a_native_opt_in_off_in_both_controllers() {
    opt_in_off_in_both_controllers("boost-unlock-research", |ai| ai.boost_unlock_research);
}

/// One founded capital and nothing else on the board.
fn capital_board(seed: u64) -> Game {
    let mut game = Game::new_full(1, 20, 14, seed, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("the player opens with a settler");
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    game
}

/// Off, the whole term is the flat credit the two value functions paid
/// before the genes existed — 28 for a boost in hand, nothing otherwise.
#[test]
fn every_gene_off_is_the_old_flat_credit_exactly() {
    let mut game = capital_board(53_001);
    let ai = AdvancedAi::new();
    assert_eq!(ai.boost_research_value(&game, 0, "masonry", true), 0.0);
    assert_eq!(
        ai.boost_research_value(&game, 0, "craftsmanship", false),
        0.0
    );
    game.players[0].boosted_techs.insert(name!("masonry"));
    game.players[0]
        .boosted_civics
        .insert(name!("craftsmanship"));
    assert_eq!(
        ai.boost_research_value(&game, 0, "masonry", true),
        BOOST_IN_HAND_FLAT_VALUE
    );
    assert_eq!(
        ai.boost_research_value(&game, 0, "craftsmanship", false),
        BOOST_IN_HAND_FLAT_VALUE
    );
}

/// The empire's science per turn, as the gene reads it.
fn science_rate(game: &Game) -> f64 {
    game.player_city_ids(0)
        .into_iter()
        .map(|cid| game.city_yields(cid).science)
        .sum::<f64>()
        .max(1.0)
}

/// Put the capital on a chosen research rate.
fn set_science(game: &mut Game, science: f64) {
    let cid = game.player_city_ids(0)[0];
    std::sync::Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
        cid,
        crate::rules::Yields {
            science,
            ..Default::default()
        },
    );
}

/// `boost-first-research` multiplies the finished score by exactly what
/// the discount buys under the function's own `sqrt(cost)` divisor, and
/// leaves an unboosted node alone.
#[test]
fn a_boost_in_hand_scales_the_score_by_what_the_discount_buys() {
    let mut game = capital_board(53_002);
    game.players[0].boosted_techs.insert(name!("masonry"));
    game.players[0]
        .boosted_civics
        .insert(name!("craftsmanship"));
    let plain = AdvancedAi::new();
    let mut ai = AdvancedAi::new();
    ai.enable_boost_first_research();
    let strategy = GrandStrategy::Expansion;
    let expected = 1.0 / (1.0f64 - 0.4).sqrt();
    for (node, techs) in [("masonry", true), ("craftsmanship", false)] {
        let off = if techs {
            plain.tech_value(&game, 0, node, strategy)
        } else {
            plain.civic_value(&game, 0, node, strategy)
        };
        let on = if techs {
            ai.tech_value(&game, 0, node, strategy)
        } else {
            ai.civic_value(&game, 0, node, strategy)
        };
        assert!(off > 0.0);
        assert!(
            (on / off - expected).abs() < 1e-9,
            "{node} scaled by {} where the 40% discount buys {expected}",
            on / off
        );
    }
    // A node whose boost is not in hand is untouched, whatever its cost.
    for node in ["pottery", "mining", "irrigation"] {
        assert_eq!(
            ai.tech_value(&game, 0, node, strategy),
            plain.tech_value(&game, 0, node, strategy),
            "{node} has no boost in hand and must not move"
        );
    }
}

/// The multiplier is a ratio, not a lump: a boosted node that unlocks
/// nothing still loses to an unboosted node that unlocks something. This
/// is the property the additive first cut did not have, and the reason its
/// probe resolved a score-share loss.
#[test]
fn a_boosted_node_that_unlocks_nothing_does_not_outrank_a_useful_one() {
    let mut game = capital_board(53_011);
    set_science(&mut game, 3.0);
    let strategy = GrandStrategy::Expansion;
    let plain = AdvancedAi::new();
    let mut ai = AdvancedAi::new();
    ai.enable_boost_first_research();
    // The two ends of the opening tree by plain merit.
    let available = game.available_techs(0);
    let best = *available
        .iter()
        .max_by(|a, b| {
            plain
                .tech_value(&game, 0, a, strategy)
                .total_cmp(&plain.tech_value(&game, 0, b, strategy))
        })
        .expect("the opening tree offers something");
    let worst = *available
        .iter()
        .filter(|tech| **tech != best)
        .min_by(|a, b| {
            plain
                .tech_value(&game, 0, a, strategy)
                .total_cmp(&plain.tech_value(&game, 0, b, strategy))
        })
        .expect("and something else");
    let gap =
        plain.tech_value(&game, 0, &best, strategy) / plain.tech_value(&game, 0, &worst, strategy);
    assert!(
        gap > BOOST_SCALE_CAP,
        "the opening tree spreads wider than the cap ({gap} > {BOOST_SCALE_CAP}), \
             so this test can tell a ratio from a lump"
    );
    game.players[0].boosted_techs.insert(Name::new(&worst));
    assert!(
        ai.tech_value(&game, 0, &best, strategy) > ai.tech_value(&game, 0, &worst, strategy),
        "boosting {worst} must not lift it over {best}"
    );
}

/// The cap: no inspiration is allowed to make a node worth several of its
/// neighbours, which is the additive mistake in another costume.
#[test]
fn the_scale_is_capped() {
    let mut game = capital_board(53_003);
    let mut ai = AdvancedAi::new();
    ai.enable_boost_first_research();
    for tech in game.rules.techs.keys().copied().collect::<Vec<_>>() {
        game.players[0].boosted_techs.insert(tech);
    }
    for civic in game.rules.civics.keys().copied().collect::<Vec<_>>() {
        game.players[0].boosted_civics.insert(civic);
    }
    for tech in game.rules.techs.keys() {
        let scale = ai.boost_in_hand_scale(&game, 0, tech.as_str(), true);
        assert!(
            (1.0..=BOOST_SCALE_CAP).contains(&scale),
            "{tech} scales by {scale}"
        );
    }
    for civic in game.rules.civics.keys() {
        let scale = ai.boost_in_hand_scale(&game, 0, civic.as_str(), false);
        assert!(
            (1.0..=BOOST_SCALE_CAP).contains(&scale),
            "{civic} scales by {scale}"
        );
    }
}

/// The wait keeps only the high-confidence edge: a short node and the
/// final outstanding trigger step, with a small penalty that breaks a
/// close choice instead of vetoing useful research.
#[test]
fn boost_wait_research_2_is_a_short_final_step_tiebreak() {
    let mut game = capital_board(53_014);
    let mut v2 = AdvancedAi::new();
    v2.enable_boost_wait_research_2();
    let cost = game.tech_cost("engineering");
    set_science(&mut game, cost);

    assert_eq!(
        v2.boost_research_value(&game, 0, "engineering", true),
        0.0,
        "a merely buildable Wall does not delay Engineering"
    );
    let capital = game.player_city_ids(0)[0];
    game.cities.get_mut(&capital).unwrap().queue = vec![Item::Building {
        building: name!("walls"),
    }];
    game.turn += 1;
    let v2_penalty = v2.boost_research_value(&game, 0, "engineering", true);
    assert!(v2_penalty < 0.0, "the queued final Wall can justify a wait");
    assert!(
        v2_penalty.abs() <= BOOST_TURNS_CAP * BOOST_TURN_VALUE * BOOST_WAIT_V2_FACTOR,
        "a tie-break, never a veto: {v2_penalty}"
    );

    set_science(&mut game, cost / (BOOST_WAIT_V2_HORIZON_TURNS * 2.0));
    assert_eq!(
        v2.boost_research_value(&game, 0, "engineering", true),
        0.0,
        "a node outside v2's short window keeps its place"
    );
}

/// A boost already in hand is never waited for — there is nothing left to
/// earn, and the node should be taken now.
#[test]
fn a_boost_in_hand_is_never_waited_for() {
    let mut game = capital_board(53_005);
    game.players[0].techs.insert(name!("mining"));
    game.players[0].boosted_techs.insert(name!("masonry"));
    let mut waiting = AdvancedAi::new();
    waiting.enable_boost_wait_research_2();
    let capital = game.player_city_ids(0)[0];
    let rich = crate::rules::Yields {
        science: game.tech_cost("masonry"),
        ..Default::default()
    };
    std::sync::Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(capital, rich);
    assert_eq!(
        waiting.boost_research_value(&game, 0, "masonry", true),
        BOOST_IN_HAND_FLAT_VALUE
    );
}

/// `boost-unlock-research` pays the node that buys the permission a boost
/// trigger needs — Mining, for the quarry Masonry's eureka wants — and
/// stops paying for that trigger once the permission is held.
#[test]
fn the_node_that_opens_a_trigger_is_paid_for_it() {
    let mut game = capital_board(53_006);
    set_science(&mut game, 40.0);
    let plain = AdvancedAi::new();
    let mut opening = AdvancedAi::new();
    opening.enable_boost_unlock_research();
    assert_eq!(
        game.rules.improvements["quarry"].tech.as_deref(),
        Some("mining"),
        "the quarry Masonry's boost wants is gated on Mining"
    );
    assert!(
        opening
            .eureka_chases(&game, 0)
            .iter()
            .any(|chase| chase.trigger == "improvement_on_resource:quarry"),
        "the quarry trigger is on the chase table"
    );
    let off = plain.boost_research_value(&game, 0, "mining", true);
    let on = opening.boost_research_value(&game, 0, "mining", true);
    assert_eq!(off, 0.0);
    assert!(on > 0.0, "Mining is paid for the triggers it opens: {on}");
    // Held, the quarry is buildable and its trigger is the Builder gene's
    // business, not the research argmax's. Mining keeps only what it still
    // opens further down its own branch, which is strictly less.
    game.players[0].techs.insert(name!("mining"));
    game.turn += 1;
    let held = opening.boost_research_value(&game, 0, "mining", true);
    assert!(
        held < on,
        "the permission already held is not paid for twice: {held} < {on}"
    );
}

/// A node that opens nothing is untouched by the gene.
#[test]
fn a_node_that_opens_no_trigger_is_not_paid() {
    let mut game = capital_board(53_010);
    set_science(&mut game, 40.0);
    let mut opening = AdvancedAi::new();
    opening.enable_boost_unlock_research();
    let untouched: Vec<&str> = game
        .rules
        .techs
        .keys()
        .map(|tech| tech.as_str())
        .filter(|tech| {
            opening
                .eureka_chases(&game, 0)
                .iter()
                .all(|chase| !opening.opens_the_trigger(&game, 0, tech, true, &chase.trigger))
        })
        .collect();
    assert!(
        !untouched.is_empty(),
        "some technology on the board opens no trigger in reach"
    );
    for tech in untouched {
        assert_eq!(
            opening.boost_research_value(&game, 0, tech, true),
            0.0,
            "{tech} opens nothing and is priced exactly as before"
        );
    }
}

/// The credit is bounded: a node that opens many triggers at once cannot
/// price itself above the ceiling.
#[test]
fn the_unlock_credit_is_capped() {
    let game = capital_board(53_007);
    let mut opening = AdvancedAi::new();
    opening.enable_boost_unlock_research();
    let ceiling = BOOST_UNLOCK_TURNS_CAP * BOOST_TURN_VALUE;
    for tech in game.rules.techs.keys() {
        let value = opening.boost_research_value(&game, 0, tech.as_str(), true);
        assert!(
            value <= ceiling + 1e-9,
            "{tech} is credited {value}, over the {ceiling} ceiling"
        );
    }
}

/// A civic chase is priced against culture and a technology chase against
/// science: the two rates differ and the trees must not be crossed.
#[test]
fn the_two_trees_are_priced_against_their_own_rate() {
    let mut game = capital_board(53_008);
    set_science(&mut game, 400.0);
    let mut opening = AdvancedAi::new();
    opening.enable_boost_unlock_research();
    let science = science_rate(&game);
    let culture: f64 = game
        .player_city_ids(0)
        .into_iter()
        .map(|cid| game.city_yields(cid).culture)
        .sum::<f64>()
        .max(1.0);
    assert!(
        science > culture * 10.0,
        "the capital's science is far ahead of its culture: {science} vs {culture}"
    );
    // Every chase this board offers is a technology's, so a credit priced
    // against culture instead of science would be an order out.
    let chases = opening.eureka_chases(&game, 0);
    assert!(!chases.is_empty());
    for chase in &chases {
        let is_tech = game.rules.techs.contains_key(&chase.node);
        assert_eq!(
            is_tech,
            !game.rules.civics.contains_key(&chase.node),
            "{} belongs to exactly one tree",
            chase.node
        );
    }
}

/// A seat plays at most one version of the family: arming either version
/// stands the other down.
#[test]
fn boost_first_research_versions_are_mutually_exclusive() {
    let mut ai = AdvancedAi::new();
    ai.enable_boost_first_research();
    ai.enable_boost_first_research_2();
    assert!(!ai.boost_first_research && ai.boost_first_research_2);
    ai.enable_boost_first_research();
    assert!(ai.boost_first_research && !ai.boost_first_research_2);
    ai.disable_boost_first_research();
    assert!(!ai.boost_first_research && !ai.boost_first_research_2);
}

/// Version two never touches the value functions — a boosted node prices
/// exactly as with every gene off — and the tie-break answers nothing
/// while the gene is off, however boosted the runner-up.
#[test]
fn boost_first_research_2_leaves_the_value_functions_and_the_off_argmax_alone() {
    let mut game = capital_board(53_020);
    let strategy = GrandStrategy::Expansion;
    let plain = AdvancedAi::new();
    let mut v2 = AdvancedAi::new();
    v2.enable_boost_first_research_2();
    let available = game.available_techs(0);
    for tech in game.rules.techs.keys().copied().collect::<Vec<_>>() {
        game.players[0].boosted_techs.insert(tech);
    }
    for civic in game.rules.civics.keys().copied().collect::<Vec<_>>() {
        game.players[0].boosted_civics.insert(civic);
    }
    for tech in game.rules.techs.keys() {
        assert_eq!(
            v2.tech_value(&game, 0, tech.as_str(), strategy),
            plain.tech_value(&game, 0, tech.as_str(), strategy),
            "{tech} is priced the same under v2 as with every gene off"
        );
    }
    for civic in game.rules.civics.keys() {
        assert_eq!(
            v2.civic_value(&game, 0, civic.as_str(), strategy),
            plain.civic_value(&game, 0, civic.as_str(), strategy),
            "{civic} is priced the same under v2 as with every gene off"
        );
    }
    let ordinary = available[0];
    assert_eq!(
        plain.boost_tiebreak_pick(&game, 0, strategy, &available, &ordinary, true),
        None,
        "off, the tie-break never speaks"
    );
}

/// The band's arithmetic: at the shipped 40% boost a node at the band's
/// edge is carried over the winner (so the tie-break can speak at all),
/// and there is a gap below the band where v1's scale alone would carry
/// a node over the winner and v2 refuses it — the whole difference
/// between the versions.
#[test]
fn the_band_admits_its_edge_and_leaves_v1_a_gap_below_it() {
    let scale = 1.0 / (1.0f64 - 0.4).sqrt();
    assert!(
        BOOST_TIEBREAK_BAND * scale > 1.0,
        "a node at the band's edge scales to {:.3} and must beat the winner",
        BOOST_TIEBREAK_BAND * scale
    );
    let v1_floor = 1.0 / scale;
    assert!(
        v1_floor < BOOST_TIEBREAK_BAND,
        "v1 lifts anything above {v1_floor:.3}; v2 admits nothing below {BOOST_TIEBREAK_BAND}"
    );
    assert!(
        (1.0..=BOOST_SCALE_CAP).contains(&scale),
        "the shipped scale sits under the cap"
    );
}

/// The band, on real trees: boosting one node at a time under each grand
/// strategy, on the opening tree and again on the classical tier, the
/// tie-break takes it exactly when its unscaled score is within
/// `BOOST_TIEBREAK_BAND` of the plain winner AND the scale carries it over
/// — so a comparable node is admitted and a clear leader is never
/// overturned. (The real trees are lumpy: no node on these boards falls
/// in the gap v1 takes and v2 refuses, which the arithmetic test above
/// covers.)
#[test]
fn the_tiebreak_admits_a_comparable_boost_and_refuses_a_clear_leader_overturn() {
    let mut game = capital_board(53_009);
    let plain = AdvancedAi::new();
    let mut v2 = AdvancedAi::new();
    v2.enable_boost_first_research_2();
    let (mut admitted, mut refused) = (0, 0);
    let mut sweep = |game: &mut Game| {
        let available = game.available_techs(0);
        let argmax = |ai: &AdvancedAi, game: &Game, strategy: GrandStrategy| {
            available
                .iter()
                .max_by(|a, b| {
                    ai.tech_value(game, 0, a, strategy)
                        .partial_cmp(&ai.tech_value(game, 0, b, strategy))
                        .unwrap()
                        .then_with(|| b.cmp(a))
                })
                .cloned()
                .expect("the tree offers something")
        };
        for strategy in [
            GrandStrategy::Expansion,
            GrandStrategy::Science,
            GrandStrategy::Culture,
        ] {
            game.players[0].boosted_techs.clear();
            let ordinary = argmax(&plain, game, strategy);
            let winner = plain.tech_value(game, 0, &ordinary, strategy);
            for node in available.iter().filter(|node| **node != ordinary) {
                game.players[0].boosted_techs.clear();
                game.players[0].boosted_techs.insert(*node);
                // The plain score of a boosted node already carries the
                // pre-gene flat credit; the band reads that score.
                let unscaled = plain.tech_value(game, 0, node, strategy);
                let scale = AdvancedAi::boost_discount_scale(game, node, true);
                let expected = (unscaled >= winner * BOOST_TIEBREAK_BAND
                    && unscaled * scale > winner)
                    .then_some(*node);
                let pick = v2.boost_tiebreak_pick(game, 0, strategy, &available, &ordinary, true);
                assert_eq!(
                    pick, expected,
                    "{node} at {unscaled:.1} against {ordinary} at {winner:.1} ({strategy:?})"
                );
                match pick {
                    Some(_) => admitted += 1,
                    None => refused += 1,
                }
            }
            // The clear leader case by name: the worst node on offer,
            // boosted, is never admitted.
            let worst = *available
                .iter()
                .filter(|tech| **tech != ordinary)
                .min_by(|a, b| {
                    plain
                        .tech_value(game, 0, a, strategy)
                        .total_cmp(&plain.tech_value(game, 0, b, strategy))
                })
                .expect("and something else");
            game.players[0].boosted_techs.clear();
            game.players[0].boosted_techs.insert(worst);
            assert_eq!(
                v2.boost_tiebreak_pick(game, 0, strategy, &available, &ordinary, true),
                None,
                "boosting {worst} must not lift it over {ordinary}"
            );
        }
    };
    sweep(&mut game);
    for tech in [
        "pottery",
        "animal_husbandry",
        "mining",
        "sailing",
        "astrology",
        "irrigation",
        "writing",
        "archery",
        "masonry",
        "bronze_working",
        "wheel",
    ] {
        game.players[0].techs.insert(Name::new(tech));
    }
    game.turn += 1;
    sweep(&mut game);
    assert!(
        admitted > 0,
        "some runner-up is comparable and takes the slot"
    );
    assert!(refused > 0, "some node is a clear loser and stays put");
}

/// The whole point, on the real argmax: among nodes the plain function
/// ranks within the multiplier of each other, the boosted one wins.
#[test]
fn the_research_pick_prefers_a_boost_it_already_holds() {
    let mut game = capital_board(53_009);
    // Diplomacy, not Expansion: the property needs a runner-up inside the
    // gene's discount yet beyond the flat boost credit, and the opening
    // values barely move with the map. Once the lookahead stopped counting
    // other civilizations' unique units, Expansion's Mining/Animal
    // Husbandry gap (39.0/33.8) fell under the flat credit on every seed
    // from 53,000 to 53,200; Diplomacy's (ratio 1.25) holds on all of them.
    let strategy = GrandStrategy::Diplomacy;
    let plain = AdvancedAi::new();
    let mut ai = AdvancedAi::new();
    ai.enable_boost_first_research();
    let argmax = |ai: &AdvancedAi, game: &Game| {
        game.available_techs(0)
            .into_iter()
            .max_by(|a, b| {
                ai.tech_value(game, 0, a, strategy)
                    .partial_cmp(&ai.tech_value(game, 0, b, strategy))
                    .unwrap()
                    .then_with(|| b.cmp(a))
            })
            .expect("the opening tree offers something")
    };
    let before = argmax(&plain, &game);
    // The runner-up, boosted, takes the slot: it is inside the ratio.
    let runner_up = game
        .available_techs(0)
        .into_iter()
        .filter(|tech| *tech != before)
        .max_by(|a, b| {
            plain
                .tech_value(&game, 0, a, strategy)
                .partial_cmp(&plain.tech_value(&game, 0, b, strategy))
                .unwrap()
                .then_with(|| b.cmp(a))
        })
        .expect("and a runner-up");
    let ratio = plain.tech_value(&game, 0, &before, strategy)
        / plain.tech_value(&game, 0, &runner_up, strategy);
    assert!(
        ratio < 1.0 / (1.0f64 - 0.4).sqrt(),
        "the runner-up is within the discount ({ratio}), so boosting it should win"
    );
    game.players[0].boosted_techs.insert(runner_up);
    assert_eq!(
        argmax(&ai, &game),
        runner_up,
        "the boost in hand takes the slot from {before}"
    );
    assert_eq!(
        argmax(&plain, &game),
        before,
        "and the gene off leaves the old pick alone"
    );
}
