//! Position features chosen so that the decisions an agent ranks are
//! visible in them.
//!
//! Several scalar learned experiments in this codebase failed at the boundary
//! between state prediction and action ranking. `evolve::features` is
//! twenty-five *empire
//! aggregates* — cities, population, owned tiles, techs, civics, military
//! power, unit count, three yields, Gold, score, mirrored for the leading
//! rival, plus turn fraction. Repositioning a unit changes none of them, so
//! `PolicyAi`'s computed gain is exactly zero on 96% of the candidates it
//! clones; the trained value net never flips a lane; and swapping that net
//! in as a production search's terminal value moved one game in 240,
//! because a win probability regressed on those numbers is a re-weighting
//! of the score share already in them.
//!
//! The precondition nobody had checked is measurable: **does the feature
//! vector change at all when the action is taken?** Measured over 37,900
//! candidate actions across three four-player games, `evolve::features`
//! moves for 44.5% of them, and for 2.1% of unit moves. The vector below
//! moves for 86.1%, and for 69.2% of unit moves.
//!
//! | action kind | n | `evolve::features` | this |
//! |---|---|---|---|
//! | `move` | 6,842 | 2.1% | **69.2%** |
//! | `produce` | 11,869 | 11.6% | **91.5%** |
//! | `fortify` | 1,170 | 0.0% | **100%** |
//! | `ranged` | 57 | 0.0% | **100%** |
//! | `city_strike` | 18 | 0.0% | **100%** |
//! | `research` | 316 | 0.0% | **100%** |
//! | total | 37,900 | 44.5% | **86.1%** |
//!
//! Three cautions, so this is not oversold.
//!
//! 1. **Visibility is necessary, not sufficient.** A feature that moves
//!    under an action does not thereby rank actions correctly. What the
//!    measurements above establish is that the previous set made correct
//!    ranking *impossible*, which is a different and stronger claim than
//!    "it was inaccurate".
//! 2. **The live controller does not use this.** The evaluator-only
//!    `policy_wide` arm can load a 34-wide net, and its first trained run was a
//!    large regression because visibility did not make the state-value delta
//!    causal. No 34-wide artifact ships. This representation is research
//!    infrastructure, not a strength improvement.
//! 3. **Policy-card and diplomacy actions remain invisible** — `slot_policy`
//!    and `unslot_policy` are still 0%, because nothing here encodes which
//!    cards occupy which slots. That is the next gap, not a solved one.
//!
//! The extra terms are deliberately cheap scalars rather than a spatial
//! tensor. `obs_tensor` already renders 25 fog-honest planes and nothing in
//! Rust can consume them, so a plane-based evaluator is blocked on
//! inference machinery that does not exist; this is not.
use crate::evolve::features;
use crate::game::Game;

/// Width of [`decision_features`]: the 25 empire aggregates plus 9 terms
/// that respond to unit position, unit condition, city fabric and current
/// research.
pub const WIDTH: usize = 34;

/// Index of the adjacent-enemy count within [`decision_features`].
pub const ADJACENT_ENEMIES: usize = 29;
/// Index of the mean gap to the nearest enemy.
pub const MEAN_ENEMY_GAP: usize = 30;

/// The two terms that make a unit move visible — and the two an argmax
/// over a correlational value net exploits.
///
/// Measured over 468 committed decisions, `PolicyAi` scoring with a net
/// trained on these features raised the adjacent-enemy count 78× more than
/// the average legal candidate, closed distance where the field opened it,
/// and lost material doing so. In games the scripted agent wins, contact
/// is a *symptom* of a strong empire pressing an attack; maximising it is
/// not the same as pressing an attack.
pub const CONTACT_TERMS: [usize; 2] = [ADJACENT_ENEMIES, MEAN_ENEMY_GAP];

/// Hex distance on the offset grid the engine stores positions in.
fn hex_distance(a: (i32, i32), b: (i32, i32)) -> i32 {
    let (dx, dy) = (a.0 - b.0, a.1 - b.1);
    (dx.abs() + dy.abs() + (dx + dy).abs()) / 2
}

/// `evolve::features` extended with terms an action can move.
///
/// Ordering is stable and append-only, like `action_space::KINDS`:
/// reordering invalidates anything trained on it.
pub fn decision_features(g: &Game, pid: usize) -> Vec<f32> {
    let mut f = features(g, pid);
    let units = g.player_unit_ids(pid);

    // Condition, not just count. A ranged attack that damages without
    // killing leaves `unit count` and `military_power` alone.
    let mut own_material = 0.0f32;
    let mut wounded = 0.0f32;
    let mut fortified = 0.0f32;
    for id in &units {
        let unit = &g.units[id];
        own_material += unit.hp as f32 / 100.0;
        if unit.hp < 100 {
            wounded += 1.0;
        }
        if unit.fortified {
            fortified += 1.0;
        }
    }
    f.push(own_material / 20.0);
    f.push(wounded / 10.0);
    f.push(fortified / 10.0);

    let enemy_material: f32 = g
        .units
        .values()
        .filter(|unit| unit.owner != pid)
        .map(|unit| unit.hp as f32 / 100.0)
        .sum();
    f.push(enemy_material / 40.0);

    // Contact. These two are what make an ordinary move visible: stepping
    // toward or away from an enemy changes the mean gap, and closing
    // changes the adjacency count.
    let enemy_positions: Vec<(i32, i32)> = g
        .units
        .values()
        .filter(|unit| unit.owner != pid)
        .map(|unit| (unit.pos.0, unit.pos.1))
        .collect();
    let mut adjacent = 0.0f32;
    let mut gap_total = 0.0f32;
    let mut gap_count = 0.0f32;
    for id in &units {
        let pos = g.units[id].pos;
        let mut nearest = i32::MAX;
        for enemy in &enemy_positions {
            let distance = hex_distance((pos.0, pos.1), *enemy);
            nearest = nearest.min(distance);
            if distance <= 1 {
                adjacent += 1.0;
            }
        }
        if nearest != i32::MAX {
            gap_total += nearest as f32;
            gap_count += 1.0;
        }
    }
    f.push(adjacent / 10.0);
    f.push(if gap_count > 0.0 {
        gap_total / gap_count / 20.0
    } else {
        0.0
    });

    // City fabric and what is being built. Queue *cost* rather than queue
    // length, because a Produce that swaps one item for another leaves the
    // length untouched — which is most of why `produce` scored 11.6%.
    let mut buildings = 0.0f32;
    let mut districts = 0.0f32;
    let mut queued_cost = 0.0f64;
    for id in &g.player_city_ids(pid) {
        let city = &g.cities[id];
        buildings += city.buildings.len() as f32;
        districts += city.districts.len() as f32;
        for item in &city.queue {
            queued_cost += g.item_cost_for(pid, item);
        }
    }
    f.push(buildings / 20.0);
    f.push(districts / 10.0);
    f.push(queued_cost as f32 / 500.0);

    debug_assert_eq!(f.len(), WIDTH);
    f
}

#[cfg(test)]
mod tests {
    use super::{decision_features, WIDTH};
    use crate::action_space::{kind_name, legal_encoded};
    use crate::ai::{Ai, BasicAi};
    use crate::evolve::features;
    use crate::game::{Action, Game};
    use std::collections::BTreeMap;

    #[test]
    fn width_matches_the_declared_constant() {
        let g = Game::new(4, 28, 18, 900, 60, 2);
        assert_eq!(decision_features(&g, 0).len(), WIDTH);
        assert!(decision_features(&g, 0)
            .iter()
            .all(|value| value.is_finite()));
    }

    /// The extension must preserve the original vector as a prefix, so a
    /// model trained on the 25 can be compared against one trained on the
    /// 34 without the comparison confounding two changes at once.
    #[test]
    fn the_original_features_are_a_prefix() {
        let g = Game::new(4, 28, 18, 901, 60, 2);
        let base = features(&g, 0);
        let extended = decision_features(&g, 0);
        assert_eq!(&extended[..base.len()], &base[..]);
    }

    /// The claim this module exists for, measured rather than asserted: the
    /// decisions the learned components were failing to rank must actually
    /// move these numbers. Thresholds are set well below the measured
    /// values so ordinary rules churn does not fail the suite, but far
    /// above what `evolve::features` achieves — moves are 2.1% there.
    #[test]
    fn the_decisions_that_were_invisible_are_visible() {
        let mut base: BTreeMap<&'static str, (usize, usize)> = BTreeMap::new();
        let mut ext: BTreeMap<&'static str, (usize, usize)> = BTreeMap::new();
        for seed in 0..2u64 {
            let mut g = Game::new(4, 28, 18, seed + 900, 70, 2);
            let mut ais = BasicAi::fleet(&g);
            while g.winner.is_none() && g.turn <= g.max_turns {
                let pid = g.current;
                if pid == 0 {
                    let base_before = features(&g, 0);
                    let ext_before = decision_features(&g, 0);
                    for action in legal_encoded(&g, 0).actions {
                        if matches!(action, Action::EndTurn) {
                            continue;
                        }
                        let kind = kind_name(&action);
                        let mut sim = g.clone();
                        if sim.apply(0, &action).is_err() {
                            continue;
                        }
                        let b = base.entry(kind).or_default();
                        b.0 += 1;
                        b.1 += usize::from(features(&sim, 0) != base_before);
                        let e = ext.entry(kind).or_default();
                        e.0 += 1;
                        e.1 += usize::from(decision_features(&sim, 0) != ext_before);
                    }
                }
                ais[pid].take_turn(&mut g, pid);
                if g.winner.is_none() && g.current == pid {
                    let _ = g.apply(pid, &Action::EndTurn);
                }
            }
        }
        let share = |table: &BTreeMap<&'static str, (usize, usize)>, kind: &str| {
            table
                .get(kind)
                .filter(|(n, _)| *n >= 50)
                .map(|(n, hits)| *hits as f64 / *n as f64)
        };
        if let Some(moves) = share(&ext, "move") {
            assert!(
                moves > 0.4,
                "unit moves visible in only {:.1}% of cases",
                100.0 * moves
            );
            let before = share(&base, "move").unwrap_or(0.0);
            assert!(
                moves > before * 5.0,
                "extension barely improved moves: {:.1}% against {:.1}%",
                100.0 * moves,
                100.0 * before
            );
        }
        if let Some(produce) = share(&ext, "produce") {
            assert!(
                produce > 0.6,
                "production visible in only {:.1}% of cases",
                100.0 * produce
            );
        }
        let total = |table: &BTreeMap<&'static str, (usize, usize)>| {
            let (n, hits) = table
                .values()
                .fold((0, 0), |(n, hits), (kn, kh)| (n + kn, hits + kh));
            hits as f64 / n.max(1) as f64
        };
        assert!(
            total(&ext) > total(&base) + 0.2,
            "overall visibility {:.1}% against {:.1}%",
            100.0 * total(&ext),
            100.0 * total(&base)
        );
    }
}
