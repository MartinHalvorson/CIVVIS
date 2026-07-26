//! PolicyAi: the learned net chooses the actions.
//!
//! Where `NeuralAi` consults the value net for one decision (war), this
//! agent uses it as the policy itself: each turn it repeatedly scores the
//! legal action set by applying each candidate to a clone and evaluating the
//! resulting position, then commits the best improvement. That is one-ply
//! net-guided search over the real action space — the first rung of a
//! learned policy (AI_GAPS item 1) and the loop a trained policy head would
//! later replace.
//!
//! Without `evolved/valuenet.json` there is nothing learned to consult, so
//! the agent falls back to the scripted `AdvancedAi` rather than playing
//! randomly.
use crate::action_space::{kind_name, legal_encoded};
use crate::ai::{AdvancedAi, Ai, PlanReport, Weights};
use crate::decision_features::{decision_features, WIDTH as DECISION_WIDTH};
use crate::evolve::features;
use crate::game::{Action, Game};
use crate::valuenet::ValueNet;

pub struct PolicyAi {
    fallback: AdvancedAi,
    net: Option<ValueNet>,
    /// Most candidate actions scored per decision.
    pub width: usize,
    /// Most committed actions per turn before ending it.
    pub depth: usize,
    /// Minimum value gain required to commit an action.
    pub margin: f64,
    /// Score candidates with `decision_features` and a net trained on it,
    /// instead of `evolve::features` and a 25-wide net.
    ///
    /// **Measured at 14.2% against `advanced` — an Elo-equivalent of
    /// −313.** Turning this on is a large regression, and the reason is
    /// the most useful thing this agent has produced.
    ///
    /// With the 25-wide vector the computed gain is exactly zero on 96% of
    /// candidates, so the agent declines to act and falls through to the
    /// scripted layer; that is why it measured at parity. The wider vector
    /// lets it distinguish candidates, so it commits — and its ranking is
    /// much worse than the scripted doctrine it displaces. The blindness
    /// was not the reason it failed to win. It was the reason it failed to
    /// lose.
    ///
    /// The obvious mechanism — that greedy maximisation walks off the
    /// training distribution — was tested and is **wrong**. Scoring the
    /// same net against outcomes on states this agent reaches gives BCE
    /// 0.3898 against 0.3720 on states the expert reaches, with *better*
    /// calibration (ECE 0.064 against 0.103). The estimate is fine where
    /// the agent goes.
    ///
    /// What is actually wrong is subtler and more general. The net is fit
    /// to outcomes, so it encodes **correlation**, and the argmax over
    /// sibling actions optimises whichever correlate is cheapest to move.
    /// Measured over 468 committed decisions, the chosen action raises the
    /// adjacent-enemy count by 0.137 where the average legal candidate
    /// raises it by 0.002 — seventy-eight times the field — while own
    /// HP-weighted material falls. In games that `advanced` wins, units
    /// stand in contact because a strong empire is pressing an attack;
    /// contact is a *symptom* of strength. Maximising a symptom drives
    /// units into fights they lose.
    ///
    /// A state-value function cannot fix this by being more accurate,
    /// because it is accurate. Ranking actions needs a counterfactual
    /// signal — play the action out and observe the consequence, which is
    /// what a rollout does and what a one-ply value delta does not, and is
    /// why the macro search is the search in this codebase that wins
    /// games. The learned route is action-conditioned value (Q or
    /// advantage) trained on returns for actions actually taken, not a
    /// state-value regression read greedily.
    ///
    /// This is the whole point of the wider vector: the 25 aggregates are
    /// unchanged by 96% of the candidates this agent clones, so its
    /// computed gain is exactly zero and it declines to act. The 34-wide
    /// vector moves for 69% of unit moves. Whether that turns into better
    /// *ranking* is a separate question from whether it can rank at all,
    /// and is what an evaluation of `policy_wide` measures.
    pub wide: bool,
    /// Restrict the net to action kinds where a one-ply value delta is
    /// meaningful. Multi-turn commitments (production, research, purchases)
    /// look near-free to a one-ply evaluator, which is how an unrestricted
    /// policy empties its treasury; those stay with the scripted layer.
    pub tactical_only: bool,
}

/// Action kinds whose whole effect lands this turn, so the resulting
/// position honestly reflects the choice.
pub const TACTICAL_KINDS: [&str; 12] = [
    "move", "move_to", "attack", "ranged", "air_strike", "air_patrol",
    "air_rebase", "fortify", "pillage", "city_strike", "encampment_strike",
    "condemn_heretic",
];

/// Action kinds the evaluator is structurally blind to.
///
/// `evolve::features` is twenty-five empire aggregates — cities, population,
/// owned tiles, techs, civics, military power, unit count, three yields,
/// Gold and score, mirrored for the leading rival, plus turn fraction.
/// Repositioning a unit changes none of them, so the value of the position
/// after the action is *bit-identical* to the value before it and the
/// computed gain is exactly zero.
///
/// Measured over four 120-turn four-player games (11,347 candidate
/// evaluations): `fortify` moved the value on 0 of 1,445 candidates,
/// `ranged` 0 of 152, `city_strike` 0 of 44, and `move` on 2.0% of 9,481 —
/// the exceptions being moves that capture a civilian or claim a tile.
/// Only `attack` was always visible, at 225 of 225.
///
/// These three kinds are excluded a priori rather than by that sample:
/// none of them can change unit count, ownership, yields, Gold, research or
/// score, so no game state exists in which they move a feature.
/// `moves_no_feature` pins that as a property.
pub const UNOBSERVABLE_KINDS: [&str; 3] = ["fortify", "air_patrol", "air_rebase"];

impl Default for PolicyAi {
    fn default() -> Self {
        Self::new()
    }
}

impl PolicyAi {
    pub fn new() -> PolicyAi {
        Self::with_weights(Weights::default())
    }

    pub fn with_weights(weights: Weights) -> PolicyAi {
        PolicyAi {
            fallback: AdvancedAi::with_weights(weights),
            net: ValueNet::load_width("evolved", crate::evolve::FEATURE_WIDTH),
            wide: false,
            width: 48,
            depth: 10,
            margin: 1e-4,
            tactical_only: true,
        }
    }

    pub fn has_net(&self) -> bool {
        self.net.is_some()
    }

    /// Load the wider net and score with the wider vector.
    pub fn with_decision_features(mut self) -> PolicyAi {
        self.net = ValueNet::load_width("evolved", DECISION_WIDTH);
        self.wide = true;
        self
    }

    fn value(&self, g: &Game, pid: usize) -> f64 {
        match &self.net {
            Some(net) => {
                let x = if self.wide {
                    decision_features(g, pid)
                } else {
                    features(g, pid)
                };
                net.eval(&x)
            }
            None => 0.0,
        }
    }

    /// Candidate actions worth spending a clone on. EndTurn is excluded (it
    /// is the loop's exit, not a move) and the set is capped at `width` so a
    /// turn with hundreds of legal actions stays affordable.
    ///
    /// Kinds the evaluator cannot see are dropped before the cap rather than
    /// scored and discarded. They can never clear `margin` — their gain is
    /// exactly zero — so removing them cannot change which action is chosen,
    /// but leaving them in spends both a clone each and a share of the
    /// `width` budget that visible candidates need. On a busy turn the
    /// stride below is what decides which candidates survive, so a blind
    /// candidate does not merely cost time: it displaces an attack.
    fn candidates(&self, g: &Game, pid: usize) -> Vec<Action> {
        let encoded = legal_encoded(g, pid);
        let mut out: Vec<Action> = encoded
            .actions
            .into_iter()
            .filter(|a| !matches!(a, Action::EndTurn))
            .filter(|a| !UNOBSERVABLE_KINDS.contains(&kind_name(a)))
            .filter(|a| !self.tactical_only || TACTICAL_KINDS.contains(&kind_name(a)))
            .collect();
        if out.len() > self.width {
            // Deterministic stride keeps a spread across action kinds rather
            // than the first N, which would be all unit moves.
            let stride = out.len() / self.width + 1;
            out = out.into_iter().step_by(stride).collect();
        }
        out
    }

    /// One net-guided decision: returns the best improving action, if any.
    pub fn best_action(&self, g: &Game, pid: usize) -> Option<(Action, f64)> {
        self.net.as_ref()?;
        let base = self.value(g, pid);
        let mut best: Option<(Action, f64)> = None;
        for action in self.candidates(g, pid) {
            let mut sim = g.clone();
            if sim.apply(pid, &action).is_err() {
                continue;
            }
            let gain = match sim.winner {
                Some(w) if w == pid => 1.0,
                Some(_) => -1.0,
                None => self.value(&sim, pid) - base,
            };
            let better = best
                .as_ref()
                .map(|(ba, bg)| {
                    gain > *bg || (gain == *bg && kind_name(&action) < kind_name(ba))
                })
                .unwrap_or(true);
            if better {
                best = Some((action, gain));
            }
        }
        best.filter(|(_, gain)| *gain > self.margin)
    }
}

impl Ai for PolicyAi {
    fn take_turn(&mut self, g: &mut Game, pid: usize) {
        if self.net.is_none()
            || g.players[pid].is_minor
            || g.players[pid].is_barbarian
            || g.winner.is_some()
        {
            self.fallback.take_turn(g, pid);
            return;
        }
        for _ in 0..self.depth {
            let Some((action, _)) = self.best_action(g, pid) else {
                break;
            };
            if g.apply(pid, &action).is_err() {
                break;
            }
            if g.winner.is_some() || g.current != pid {
                return;
            }
        }
        // The scripted agent still runs the routine empire management the
        // one-ply net cannot see the value of (multi-turn builds, research),
        // then ends the turn.
        self.fallback.take_turn(g, pid);
    }

    fn strategy_label(&self) -> Option<&'static str> {
        self.fallback.strategy_label()
    }

    fn plan_report(&self) -> Option<PlanReport> {
        self.fallback.plan_report()
    }
}

#[cfg(test)]
mod tests {
    use super::{PolicyAi, TACTICAL_KINDS, UNOBSERVABLE_KINDS};
    use crate::action_space::{kind_name, legal_encoded};
    use crate::ai::{run_game, Ai, BasicAi};
    use crate::evolve::features;
    use crate::game::{Action, Game};

    /// Play a real four-player game and hand every legal tactical candidate
    /// to `probe` at the start of each of player 0's turns.
    fn walk_tactical_candidates(seeds: u64, turns: u32, mut probe: impl FnMut(&Game, &Action)) {
        for seed in 0..seeds {
            let mut g = Game::new(4, 28, 18, seed + 900, turns, 2);
            let mut ais = BasicAi::fleet(&g);
            while g.winner.is_none() && g.turn <= g.max_turns {
                let pid = g.current;
                if pid == 0 {
                    for action in legal_encoded(&g, 0).actions {
                        if matches!(action, Action::EndTurn)
                            || !TACTICAL_KINDS.contains(&kind_name(&action))
                        {
                            continue;
                        }
                        probe(&g, &action);
                    }
                }
                ais[pid].take_turn(&mut g, pid);
                if g.winner.is_none() && g.current == pid {
                    let _ = g.apply(pid, &Action::EndTurn);
                }
            }
        }
    }

    /// The excluded kinds must be provably invisible, not merely unobserved.
    /// None of them can change unit count, ownership, yields, Gold, research
    /// or score, so the twenty-five empire features are bit-identical after
    /// the action and the computed gain is exactly zero. If a rules change
    /// ever gives one of them a feature-visible effect, this fails and the
    /// exclusion must be revisited rather than the assertion relaxed.
    #[test]
    fn excluded_kinds_cannot_move_a_single_feature() {
        let mut checked = 0usize;
        walk_tactical_candidates(3, 110, |g, action| {
            if !UNOBSERVABLE_KINDS.contains(&kind_name(action)) {
                return;
            }
            let before = features(g, 0);
            let mut sim = g.clone();
            if sim.apply(0, action).is_err() {
                return;
            }
            checked += 1;
            assert_eq!(
                before,
                features(&sim, 0),
                "{} changed a feature: {action:?}",
                kind_name(action)
            );
        });
        assert!(checked > 100, "only {checked} candidates exercised");
    }

    /// Dropping them cannot change the chosen action, because a zero gain
    /// never clears the commitment margin. This is the safety half of the
    /// exclusion: the speedup is only worth having if play is unchanged.
    #[test]
    fn an_excluded_candidate_could_never_have_been_committed() {
        let policy = PolicyAi::new();
        assert!(policy.margin > 0.0, "a zero gain must be below the margin");
        for kind in UNOBSERVABLE_KINDS {
            assert!(
                TACTICAL_KINDS.contains(&kind),
                "{kind} is excluded from a set it was never in"
            );
        }
    }

    /// With no trained net on disk the agent must still play a full legal
    /// game by falling back to the scripted agent.
    #[test]
    fn falls_back_without_a_trained_net() {
        let mut g = Game::new(2, 20, 14, 5, 60, 0);
        let policy = PolicyAi::new();
        let scripted = !policy.has_net();
        let mut ais: Vec<Box<dyn Ai>> = g
            .players
            .iter()
            .map(|p| {
                if p.id == 0 {
                    Box::new(PolicyAi::new()) as Box<dyn Ai>
                } else {
                    Box::new(BasicAi::new()) as Box<dyn Ai>
                }
            })
            .collect();
        run_game(&mut g, &mut ais);
        assert!(g.winner.is_some());
        assert!(scripted || policy.has_net());
    }

    /// Candidate generation must stay bounded and never offer EndTurn.
    #[test]
    fn candidates_are_bounded_and_exclude_end_turn() {
        let g = Game::new(4, 28, 18, 9, 80, 2);
        let mut policy = PolicyAi::new();
        policy.width = 12;
        let candidates = policy.candidates(&g, 0);
        assert!(candidates.len() <= 12, "got {}", candidates.len());
        assert!(!candidates
            .iter()
            .any(|a| matches!(a, crate::game::Action::EndTurn)));
    }
}
