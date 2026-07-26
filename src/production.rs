//! ProductionSearchAi: rollout search over what a city builds.
//!
//! **This agent is measurably worse than the scripted governor it replaces,
//! and exists as a recorded negative result.** Over 120 mirrored
//! four-player maps it lost 9 map directions to 21 against `advanced`
//! (sign p=0.0428, 45.0% of games). Do not reach for it as a strength
//! improvement; read the paragraph below before rebuilding the idea.
//!
//! The likely reason is worth more than the agent. Rollout search over
//! victory lanes works because a lane's effect is visible in score share
//! within the projection: committing to Science changes what an empire
//! does immediately. A *building* is the opposite — its payoff compounds
//! over the century after it completes, and this rollout stops 10 rounds
//! after the slowest candidate finishes, capped at 40. Inside that window
//! a cheap unit that adds score now beats infrastructure that pays later,
//! so the search systematically overrides the governor toward the myopic
//! choice, and the governor's long-horizon sequencing — reserved
//! Spaceports, district families, wonder timing — is what gets discarded.
//!
//! That is the same failure that made `PolicyAi` inert, one level up: not
//! an evaluator blind to the action, but an evaluator whose *horizon* is
//! shorter than the decision's payoff. A future attempt needs a terminal
//! value that credits unfinished compounding — a trained value net, or
//! continuing the branch to a real result the way
//! `counterfactual_value_samples` does — rather than a longer fixed
//! horizon, which only moves the cliff.
//!
//! `StrategicAi` is the one learned-or-searched component in this codebase
//! that measurably wins games, and the reason is that a rollout is an
//! evaluator matched to the decision it judges: it answers "what happens if
//! I commit to this?" by committing to it. But it searches a single
//! decision type — the victory lane — about one and a half times per seat
//! per game. Everything else, including the choice that compounds most in
//! Civ, is a hand-written heuristic run greedily.
//!
//! This applies the same machinery to production. The scripted governor
//! skips any city whose queue is non-empty, so an agent can decide a build
//! first and let the governor handle everything else untouched.
//!
//! Two constraints shaped the design, both learned the hard way elsewhere
//! in this codebase:
//!
//! 1. **The horizon must outlast the build.** `PolicyAi` scores unit moves
//!    with empire-level features that a move cannot change, so its computed
//!    gain is exactly zero on 96% of candidates. A production rollout that
//!    ends before the item finishes has the identical defect: every branch
//!    returns the same position and the search is blind. The horizon here
//!    is therefore derived from the actual remaining cost of the candidates
//!    rather than fixed.
//! 2. **The budget must be bounded per game, not per decision.** Cities
//!    empty their queues constantly; searching every one costs tens of
//!    thousands of simulated rounds per game against the lane search's few
//!    hundred. Searches are rate-limited by turn and to one city, which
//!    keeps the whole feature in the same cost class as the search that is
//!    already known to pay for itself.
use crate::ai::{AdvancedAi, Ai, PlanReport, VictoryTarget, Weights};
use crate::game::{Action, Game, Item};

/// Turns between production searches. One search every fifteen turns puts a
/// 200-turn game at about thirteen, which is the same order as the lane
/// search's reviews.
const DEFAULT_SEARCH_EVERY: u32 = 15;
/// Rounds of payoff to simulate after the slowest candidate completes.
const PAYOFF_WINDOW: u32 = 10;
/// Upper bound on the horizon, so one very expensive candidate cannot turn
/// a bounded search into a whole-game projection.
const MAX_HORIZON: u32 = 40;
/// A candidate must beat the governor's own pick by this much to displace
/// it, on the same 0..1 score-share scale the lane search uses.
const COMMITMENT_MARGIN: f64 = 0.002;

pub struct ProductionSearchAi {
    inner: AdvancedAi,
    weights: Weights,
    /// Most candidate items projected per decision.
    pub width: usize,
    /// Turns between searches.
    pub search_every: u32,
    next_search: u32,
    searches: u32,
    overrides: u32,
}

impl Default for ProductionSearchAi {
    fn default() -> Self {
        Self::new()
    }
}

impl ProductionSearchAi {
    pub fn new() -> ProductionSearchAi {
        Self::with_weights(Weights::default())
    }

    pub fn with_weights(weights: Weights) -> ProductionSearchAi {
        ProductionSearchAi {
            inner: AdvancedAi::with_weights(weights.clone()),
            weights,
            width: 5,
            search_every: DEFAULT_SEARCH_EVERY,
            next_search: 0,
            searches: 0,
            overrides: 0,
        }
    }

    /// How many decisions the search has taken, and how often it displaced
    /// the governor's pick. A search that never overrides is inert, and
    /// nothing in a win rate would say so.
    pub fn search_census(&self) -> (u32, u32) {
        (self.searches, self.overrides)
    }

    /// The city to spend this turn's search on: the one whose empty queue
    /// commits the most production, since that is where a wrong build costs
    /// the most. Ties break on city id so the choice is deterministic.
    fn search_target(&self, g: &Game, pid: usize) -> Option<u32> {
        g.player_city_ids(pid)
            .into_iter()
            .filter(|cid| g.cities[cid].queue.is_empty())
            .max_by(|left, right| {
                let production = |cid: &u32| g.city_yields(*cid).production;
                production(left)
                    .partial_cmp(&production(right))
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(left.cmp(right))
            })
    }

    /// Legal builds for one city, capped at `width` in enumeration order.
    fn candidates(&self, g: &Game, pid: usize, city: u32) -> Vec<Item> {
        let mut out: Vec<Item> = g
            .legal_actions(pid)
            .into_iter()
            .filter_map(|action| match action {
                Action::Produce { city: c, item } if c == city => Some(item),
                _ => None,
            })
            .collect();
        out.truncate(self.width);
        out
    }

    /// Rounds to simulate: enough for every candidate to finish and then
    /// show its payoff. A rollout that ends mid-build scores every branch
    /// identically, which is the failure this exists to avoid.
    fn horizon(&self, g: &Game, pid: usize, city: u32, candidates: &[Item]) -> u32 {
        let production = g.city_yields(city).production.max(1.0);
        let slowest = candidates
            .iter()
            .map(|item| {
                let cost = g.item_cost_for(pid, item).max(0.0);
                (cost / production).ceil() as u32
            })
            .max()
            .unwrap_or(0);
        (slowest + PAYOFF_WINDOW).min(MAX_HORIZON)
    }

    /// Project one committed build and judge the resulting position by
    /// score share among living majors, the same terminal value the lane
    /// search uses when no winner has emerged.
    fn rollout(&self, g: &Game, pid: usize, city: u32, item: &Item, horizon: u32) -> Option<f64> {
        let mut sim = g.clone();
        sim.apply(
            pid,
            &Action::Produce {
                city,
                item: item.clone(),
            },
        )
        .ok()?;
        let mut ais: Vec<Box<dyn Ai>> = sim
            .players
            .iter()
            .map(|p| {
                if p.id == pid {
                    Box::new(AdvancedAi::with_weights(self.weights.clone())) as Box<dyn Ai>
                } else {
                    Box::new(AdvancedAi::new()) as Box<dyn Ai>
                }
            })
            .collect();
        let stop = sim.turn + horizon;
        while sim.winner.is_none() && sim.turn < stop {
            let current = sim.current;
            ais[current].take_turn(&mut sim, current);
            if sim.winner.is_none() && sim.current == current {
                let _ = sim.apply(current, &Action::EndTurn);
            }
        }
        Some(match sim.winner {
            Some(winner) if winner == pid => 1.0,
            Some(_) => 0.0,
            None => score_share(&sim, pid),
        })
    }

    /// Projected value of every candidate build, for diagnostics. The
    /// spread between these is what decides whether the search can act at
    /// all: if every branch returns the same number the horizon is too
    /// short for the build to land, and no win rate would say so.
    pub fn candidate_values(&self, g: &Game, pid: usize, city: u32) -> Vec<(Item, f64)> {
        let candidates = self.candidates(g, pid, city);
        let horizon = self.horizon(g, pid, city, &candidates);
        candidates
            .into_iter()
            .filter_map(|item| {
                self.rollout(g, pid, city, &item, horizon)
                    .map(|value| (item, value))
            })
            .collect()
    }

    /// Choose this city's build by projection, or `None` to leave it to the
    /// governor.
    fn search(&self, g: &Game, pid: usize, city: u32) -> Option<(Item, f64, f64)> {
        let candidates = self.candidates(g, pid, city);
        if candidates.len() < 2 {
            return None;
        }
        let horizon = self.horizon(g, pid, city, &candidates);
        let mut best: Option<(Item, f64)> = None;
        for item in candidates {
            let Some(value) = self.rollout(g, pid, city, &item, horizon) else {
                continue;
            };
            if best.as_ref().is_none_or(|(_, top)| value > *top) {
                best = Some((item, value));
            }
        }
        let (item, value) = best?;
        // What the governor would have built, projected the same way, so
        // the comparison is like for like rather than search against a
        // number from a different scale.
        let baseline = self.governor_pick(g, pid, city)?;
        let baseline_value = self.rollout(g, pid, city, &baseline, horizon)?;
        Some((item, value, baseline_value))
    }

    /// The item the scripted governor would choose, obtained by letting it
    /// choose on a clone. Cloning is how every other search in this
    /// codebase asks a counterfactual question, and it avoids duplicating
    /// several hundred lines of governor logic that would then drift.
    fn governor_pick(&self, g: &Game, pid: usize, city: u32) -> Option<Item> {
        let mut sim = g.clone();
        let mut governor = AdvancedAi::with_weights(self.weights.clone());
        governor.take_turn(&mut sim, pid);
        sim.cities.get(&city).and_then(|c| c.queue.first().cloned())
    }
}

/// Share of Civilization score among living majors; `1 / majors` is parity.
fn score_share(g: &Game, pid: usize) -> f64 {
    if !g.players[pid].alive {
        return 0.0;
    }
    let mut own = 0.0;
    let mut total = 0.0;
    for player in &g.players {
        if player.is_minor || player.is_barbarian || !player.alive {
            continue;
        }
        let score = g.score(player.id).max(0) as f64;
        total += score;
        if player.id == pid {
            own = score;
        }
    }
    if total <= 0.0 {
        0.5
    } else {
        own / total
    }
}

impl Ai for ProductionSearchAi {
    fn take_turn(&mut self, g: &mut Game, pid: usize) {
        let major = !g.players[pid].is_minor && !g.players[pid].is_barbarian;
        if major && g.winner.is_none() && g.turn >= self.next_search {
            if let Some(city) = self.search_target(g, pid) {
                self.next_search = g.turn + self.search_every;
                self.searches += 1;
                if let Some((item, value, baseline)) = self.search(g, pid, city) {
                    if value > baseline + COMMITMENT_MARGIN
                        && g.apply(pid, &Action::Produce { city, item }).is_ok()
                    {
                        self.overrides += 1;
                    }
                }
            }
        }
        // The governor skips any city with a non-empty queue, so a build
        // committed above survives and everything else is untouched.
        self.inner.take_turn(g, pid);
    }

    fn strategy_label(&self) -> Option<&'static str> {
        self.inner.strategy_label()
    }

    fn plan_report(&self) -> Option<PlanReport> {
        self.inner.plan_report()
    }
}

/// Explicit victory targeting, for parity with the other agents.
impl ProductionSearchAi {
    pub fn targeting(weights: Weights, target: VictoryTarget) -> ProductionSearchAi {
        let mut ai = Self::with_weights(weights);
        ai.inner.retarget(target);
        ai
    }
}

#[cfg(test)]
mod tests {
    use super::ProductionSearchAi;
    use crate::ai::{Ai, BasicAi};
    use crate::game::{Action, Game};

    fn play(seed: u64, turns: u32) -> (ProductionSearchAi, Game) {
        let mut g = Game::new(4, 28, 18, seed, turns, 2);
        let mut agent = ProductionSearchAi::new();
        let mut others = BasicAi::fleet(&g);
        while g.winner.is_none() && g.turn <= g.max_turns {
            let pid = g.current;
            if pid == 0 {
                agent.take_turn(&mut g, pid);
            } else {
                others[pid].take_turn(&mut g, pid);
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &Action::EndTurn);
            }
        }
        (agent, g)
    }

    /// The agent must play a legal game to completion, and must actually
    /// reach its search rather than skipping every turn.
    #[test]
    fn it_searches_and_finishes_a_game() {
        let (agent, game) = play(41_000, 90);
        let (searches, _) = agent.search_census();
        assert!(searches > 0, "the production search never ran");
        assert!(game.turn > 1);
    }

    /// A search that never displaces the governor is inert, and no win rate
    /// would reveal it. This is the same guard the lane search needed.
    #[test]
    fn the_search_sometimes_overrides_the_governor() {
        let mut total_searches = 0;
        let mut total_overrides = 0;
        for seed in 0..3u64 {
            let (agent, _) = play(41_000 + seed, 90);
            let (searches, overrides) = agent.search_census();
            total_searches += searches;
            total_overrides += overrides;
        }
        assert!(total_searches > 0);
        assert!(
            total_overrides > 0,
            "{total_overrides} overrides in {total_searches} searches: the \
             search ran but never changed a build, so it cannot help"
        );
    }

    /// Rollouts are seed-free clones, so the same position must produce the
    /// same decisions every time.
    #[test]
    fn the_search_is_deterministic() {
        let (first, _) = play(41_002, 70);
        let (second, _) = play(41_002, 70);
        assert_eq!(first.search_census(), second.search_census());
    }
}
