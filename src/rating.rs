//! Contextual, information-weighted player ratings.
//!
//! The league rates strategies with Glicko-2 over a full pairwise
//! decomposition of each game's finishing order (`league.rs`). Measured
//! against the exhibition's own match history that system forecasts no
//! better than a coin flip, for two reasons this module fixes:
//!
//! 1. **Most of a finishing order is noise.** In a six-player game the
//!    winner is decided by play; ranks 4th through 6th are decided by an
//!    engine score that barely separates strategies. Weighting all fifteen
//!    pairwise comparisons equally buries the one informative comparison
//!    under fourteen coin flips. [`fit_stage_weights`] measures how much
//!    information each placement stage actually carries, and the model
//!    weights the stages accordingly.
//! 2. **A seat is not just a strategy.** Which civilization a seat drew
//!    moves the result at least as much as which strategy plays it, and
//!    seating that assigns civs by rating confounds the two permanently.
//!    Here a seat's strength is `skill[player] + edge[civ]`, so a rating
//!    means "how strong is this player, net of what it drew", and the civ
//!    edge is a shared quantity every game helps estimate.
//!
//! Both effects are Gaussian beliefs updated by an exact Kalman step
//! against a Laplace approximation of the Plackett-Luce stage likelihood,
//! which splits each surprise between the player and the civ in proportion
//! to how uncertain each one is. A settled civ edge stays put while a fresh
//! player absorbs the news, which is the behaviour you want and which a
//! separate per-civ rating table cannot produce.
//!
//! Nothing here asks to be believed: [`backtest`] replays a match history
//! through this model and the systems it replaces, scoring every forecast
//! before the result is revealed.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Elo points per natural logit — `400 / ln(10)`, the constant that makes a
/// 400-point lead a 10:1 favourite. Glicko-2's internal scale is the same
/// number, so ratings here are directly comparable to `league.rs`.
pub const ELO_PER_LOGIT: f64 = 173.717_792_761_797_6;
/// Where an unrated player starts, matching the league's scale.
pub const BASE_ELO: f64 = 1500.0;
/// Starting uncertainty for an unrated player, in Elo points.
pub const BASE_RD: f64 = 350.0;
/// A civ edge is a much smaller effect than the spread of player skill, and
/// every game in the league observes one, so it starts far more confident.
pub const CIV_PRIOR_RD: f64 = 120.0;

fn elo_to_logit(elo: f64) -> f64 {
    (elo - BASE_ELO) / ELO_PER_LOGIT
}

fn rd_to_logit(rd: f64) -> f64 {
    rd / ELO_PER_LOGIT
}

/// A Gaussian belief about one latent strength, held in logit units.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Belief {
    /// Posterior mean, in logits above [`BASE_ELO`].
    pub mu: f64,
    /// Posterior standard deviation, in logits.
    pub sigma: f64,
}

impl Belief {
    /// A belief stated the way a leaderboard states it.
    pub fn from_elo(elo: f64, rd: f64) -> Belief {
        Belief {
            mu: elo_to_logit(elo),
            sigma: rd_to_logit(rd),
        }
    }

    /// The familiar 1500-centred rating.
    pub fn elo(&self) -> f64 {
        BASE_ELO + self.mu * ELO_PER_LOGIT
    }

    /// The familiar rating deviation.
    pub fn rd(&self) -> f64 {
        self.sigma * ELO_PER_LOGIT
    }

    /// Skill this player is 97.5% likely to exceed — what selection should
    /// use, so nothing is promoted or culled on an unsettled number.
    pub fn conservative_elo(&self) -> f64 {
        self.elo() - 1.96 * self.rd()
    }

    fn variance(&self) -> f64 {
        self.sigma * self.sigma
    }
}

impl Default for Belief {
    fn default() -> Belief {
        Belief::from_elo(BASE_ELO, BASE_RD)
    }
}

/// One seat of one finished game.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Seat {
    /// Whoever played the seat: a league strategy name, or a human account.
    pub player: String,
    /// The civilization the seat drew. Empty means "no context recorded",
    /// which the model treats as a single shared no-op context.
    pub civ: String,
    /// Finishing rank, 0 = won. Equal ranks are a tie.
    pub rank: u32,
}

/// One finished game, as the rating system needs to see it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MatchRecord {
    pub seats: Vec<Seat>,
    /// Round or sequence number, for ordering a replay. Not used in rating.
    pub round: u32,
    /// Turn the game ended on, and how. Recorded for auditing only.
    pub turn: u32,
    pub victory: String,
}

impl MatchRecord {
    /// Seat indices grouped into tied blocks, best block first.
    fn ranked_groups(&self) -> Vec<Vec<usize>> {
        let mut order: Vec<usize> = (0..self.seats.len()).collect();
        order.sort_by_key(|i| (self.seats[*i].rank, *i));
        let mut groups: Vec<Vec<usize>> = Vec::new();
        for i in order {
            match groups.last_mut() {
                Some(last) if self.seats[last[0]].rank == self.seats[i].rank => last.push(i),
                _ => groups.push(vec![i]),
            }
        }
        groups
    }
}

/// Tuning for [`ContextualRating`]. Every field has a defensible default;
/// `civvis rating --backtest --sweep` is how they were chosen.
#[derive(Clone, Debug, PartialEq)]
pub struct RatingCfg {
    /// Relative credit for placement stage `k` (0 = who won). Geometric with
    /// ratio [`RatingCfg::stage_decay`] unless set explicitly.
    pub stage_decay: f64,
    /// Irreducible per-observation noise: how much a single game's result is
    /// luck even between perfectly known players. Without it a long history
    /// would drive every deviation to zero and the ratings would stop moving.
    pub beta: f64,
    /// Uncertainty added per game played, so a rating can follow a player
    /// whose strength actually changes (a genome being bred, code shipping
    /// under a live exhibition).
    pub drift: f64,
    /// Deviation is never allowed outside these bounds, in Elo points.
    pub min_rd: f64,
    pub max_rd: f64,
    /// Starting deviation for a civ context.
    pub civ_prior_rd: f64,
    /// Players whose mean rating is pinned, so the scale survives a roster
    /// that churns completely. Empty means the scale floats.
    pub anchors: BTreeSet<String>,
    /// Rating the anchor set is held at.
    pub anchor_elo: f64,
}

impl Default for RatingCfg {
    fn default() -> RatingCfg {
        RatingCfg {
            stage_decay: 0.5,
            beta: 0.9,
            drift: 0.02,
            min_rd: 25.0,
            max_rd: BASE_RD,
            civ_prior_rd: CIV_PRIOR_RD,
            anchors: BTreeSet::new(),
            anchor_elo: BASE_ELO,
        }
    }
}

impl RatingCfg {
    /// Credit for each of `stages` placement decisions, summing to one.
    pub fn stage_weights(&self, stages: usize) -> Vec<f64> {
        if stages == 0 {
            return Vec::new();
        }
        let decay = self.stage_decay.clamp(0.0, 1.0);
        let mut w: Vec<f64> = (0..stages).map(|k| decay.powi(k as i32)).collect();
        let total: f64 = w.iter().sum();
        if total > 0.0 {
            for x in &mut w {
                *x /= total;
            }
        } else {
            // A zero decay means "rate only who won".
            w[0] = 1.0;
        }
        w
    }
}

/// Record of how often a player has been seen, alongside its belief.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub games: u32,
    pub wins: u32,
}

/// Evidence one game contributes, before it is applied: a mean shift and a
/// precision gain per entity. Both are additive, which is what lets every
/// stage of a game be scored against the same prior and then committed at
/// once.
#[derive(Default)]
struct Evidence {
    players: BTreeMap<String, (f64, f64)>,
    civs: BTreeMap<String, (f64, f64)>,
}

/// The rating system: player skill plus a shared per-civilization edge.
#[derive(Clone, Debug)]
pub struct ContextualRating {
    players: BTreeMap<String, Belief>,
    civs: BTreeMap<String, Belief>,
    records: BTreeMap<String, Record>,
    civ_records: BTreeMap<String, Record>,
    cfg: RatingCfg,
}

impl Default for ContextualRating {
    fn default() -> ContextualRating {
        ContextualRating::new(RatingCfg::default())
    }
}

/// Shrink a strength difference toward zero in proportion to how uncertain
/// the field is. This is Glicko's `g(phi)`, and it is what keeps a forecast
/// between two barely-rated players near even money instead of confidently
/// wrong.
fn attenuation(variance: f64) -> f64 {
    1.0 / (1.0 + 3.0 * variance / (std::f64::consts::PI * std::f64::consts::PI)).sqrt()
}

fn softmax(logits: &[f64]) -> Vec<f64> {
    let max = logits.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let exp: Vec<f64> = logits.iter().map(|x| (x - max).exp()).collect();
    let total: f64 = exp.iter().sum();
    if total <= 0.0 || !total.is_finite() {
        return vec![1.0 / logits.len() as f64; logits.len()];
    }
    exp.into_iter().map(|x| x / total).collect()
}

impl ContextualRating {
    pub fn new(cfg: RatingCfg) -> ContextualRating {
        ContextualRating {
            players: BTreeMap::new(),
            civs: BTreeMap::new(),
            records: BTreeMap::new(),
            civ_records: BTreeMap::new(),
            cfg,
        }
    }

    pub fn cfg(&self) -> &RatingCfg {
        &self.cfg
    }

    /// Belief about a player, defaulting to unrated.
    pub fn player(&self, name: &str) -> Belief {
        self.players.get(name).copied().unwrap_or_default()
    }

    /// Belief about a civilization's edge. Unlike a player this is centred
    /// on zero: it is a correction, not a rating.
    pub fn civ_edge(&self, civ: &str) -> Belief {
        self.civs.get(civ).copied().unwrap_or(Belief {
            mu: 0.0,
            sigma: rd_to_logit(self.cfg.civ_prior_rd),
        })
    }

    pub fn record(&self, name: &str) -> Record {
        self.records.get(name).cloned().unwrap_or_default()
    }

    pub fn players(&self) -> impl Iterator<Item = (&String, &Belief)> {
        self.players.iter()
    }

    pub fn civs(&self) -> impl Iterator<Item = (&String, &Belief)> {
        self.civs.iter()
    }

    /// Expected strength of a seat and how uncertain that expectation is.
    fn seat_belief(&self, seat: &Seat) -> Belief {
        let p = self.player(&seat.player);
        if seat.civ.is_empty() {
            return p;
        }
        let c = self.civ_edge(&seat.civ);
        Belief {
            mu: p.mu + c.mu,
            sigma: (p.variance() + c.variance()).sqrt(),
        }
    }

    /// Each seat's probability of winning the game. These sum to one, so
    /// they can be checked against who actually won.
    pub fn win_probabilities(&self, seats: &[Seat]) -> Vec<f64> {
        let beliefs: Vec<Belief> = seats.iter().map(|s| self.seat_belief(s)).collect();
        self.stage_probabilities(&beliefs, &(0..seats.len()).collect::<Vec<_>>())
    }

    /// Probability that `a` finishes above `b`, for auditing against the
    /// pairwise metrics the league already publishes.
    pub fn pair_probability(&self, a: &Seat, b: &Seat) -> f64 {
        let (ba, bb) = (self.seat_belief(a), self.seat_belief(b));
        let g = attenuation(ba.variance() + bb.variance() + self.cfg.beta * self.cfg.beta);
        1.0 / (1.0 + (-g * (ba.mu - bb.mu)).exp())
    }

    /// Plackett-Luce stage probabilities over the seats still unplaced,
    /// attenuated by the field's uncertainty.
    fn stage_probabilities(&self, beliefs: &[Belief], remaining: &[usize]) -> Vec<f64> {
        let n = remaining.len() as f64;
        let mean_var = remaining
            .iter()
            .map(|i| beliefs[*i].variance())
            .sum::<f64>()
            / n.max(1.0);
        let g = attenuation(mean_var + self.cfg.beta * self.cfg.beta);
        let mean_mu = remaining.iter().map(|i| beliefs[*i].mu).sum::<f64>() / n.max(1.0);
        let logits: Vec<f64> = remaining
            .iter()
            .map(|i| g * (beliefs[*i].mu - mean_mu))
            .collect();
        softmax(&logits)
    }

    /// Fold one finished game into the ratings.
    ///
    /// Every stage is scored against the beliefs held *before* the game, and
    /// the resulting evidence is applied in one step. That makes the update
    /// independent of the order seats are visited in — a tie moves nobody,
    /// and two games in the same period commute.
    pub fn observe(&mut self, m: &MatchRecord) {
        if m.seats.len() < 2 {
            return;
        }
        let groups = m.ranked_groups();
        let stages = groups.len().saturating_sub(1);
        let weights = self.cfg.stage_weights(stages.max(1));
        let beliefs: Vec<Belief> = m.seats.iter().map(|s| self.seat_belief(s)).collect();
        let mut evidence = Evidence::default();
        let mut remaining: Vec<usize> = groups.iter().flatten().copied().collect();

        for (k, group) in groups.iter().enumerate() {
            if remaining.len() < 2 {
                break;
            }
            let weight = weights.get(k).copied().unwrap_or(0.0);
            if weight > 0.0 {
                self.collect_stage(m, &beliefs, &remaining, group, weight, &mut evidence);
            }
            remaining.retain(|i| !group.contains(i));
        }
        self.apply_evidence(&evidence);

        self.recentre();
        for (i, seat) in m.seats.iter().enumerate() {
            let won = groups
                .first()
                .map(|g| g.contains(&i) && g.len() == 1)
                .unwrap_or(false);
            let rec = self.records.entry(seat.player.clone()).or_default();
            rec.games += 1;
            rec.wins += u32::from(won);
            if !seat.civ.is_empty() {
                let rec = self.civ_records.entry(seat.civ.clone()).or_default();
                rec.games += 1;
                rec.wins += u32::from(won);
            }
            // Strength can drift between games; let the belief follow it.
            let drift = self.cfg.drift;
            let entry = self.players.entry(seat.player.clone()).or_default();
            entry.sigma = (entry.variance() + drift * drift).sqrt();
            self.clamp_player(&seat.player);
        }
    }

    /// One Plackett-Luce stage: the seats in `winners` shared the best
    /// finish among `remaining`. A tie contributes its share of a win to
    /// each member rather than being broken by seat order.
    fn collect_stage(
        &self,
        m: &MatchRecord,
        beliefs: &[Belief],
        remaining: &[usize],
        winners: &[usize],
        weight: f64,
        out: &mut Evidence,
    ) {
        let probs = self.stage_probabilities(beliefs, remaining);
        let n = remaining.len() as f64;
        let mean_var = remaining.iter().map(|i| beliefs[*i].variance()).sum::<f64>() / n.max(1.0);
        let g = attenuation(mean_var + self.cfg.beta * self.cfg.beta);
        let share = 1.0 / winners.len() as f64;

        for (slot, &seat_idx) in remaining.iter().enumerate() {
            let p = probs[slot].clamp(1e-9, 1.0 - 1e-9);
            let actual = if winners.contains(&seat_idx) { share } else { 0.0 };
            // Laplace approximation of this stage's likelihood as a Gaussian
            // observation of the seat's strength: offset `grad / info`, with
            // variance `1 / info`. The stage weight scales its information.
            let curvature = g * g * p * (1.0 - p);
            if curvature * weight <= 1e-12 {
                continue;
            }
            let innovation = g * (actual - p) / curvature;
            let noise = 1.0 / (curvature * weight) + self.cfg.beta * self.cfg.beta;
            self.accumulate(&m.seats[seat_idx], innovation, noise, out);
        }
    }

    /// Exact Kalman gain for an observation of the sum `player + civ`: each
    /// side moves in proportion to its own variance, so a settled civ edge
    /// stays put while an unsettled player absorbs the surprise. Recorded as
    /// a mean shift and a precision gain, both additive across stages.
    fn accumulate(&self, seat: &Seat, innovation: f64, noise: f64, out: &mut Evidence) {
        let pv = self.player(&seat.player).variance();
        let cv = if seat.civ.is_empty() {
            0.0
        } else {
            self.civ_edge(&seat.civ).variance()
        };
        let s = pv + cv + noise;
        if s <= 0.0 || !s.is_finite() || !innovation.is_finite() {
            return;
        }
        // 1/var' - 1/var for the Kalman posterior var(1 - var/s) reduces to
        // 1/(s - var), which stays well-conditioned as var shrinks.
        let entry = out.players.entry(seat.player.clone()).or_default();
        entry.0 += (pv / s) * innovation;
        entry.1 += 1.0 / (cv + noise).max(1e-12);
        // A zero-variance context is switched off, not merely certain: the
        // backtest uses that to run this model without any civ term at all.
        if !seat.civ.is_empty() && cv > 0.0 {
            let entry = out.civs.entry(seat.civ.clone()).or_default();
            entry.0 += (cv / s) * innovation;
            entry.1 += 1.0 / (pv + noise).max(1e-12);
        }
    }

    fn apply_evidence(&mut self, evidence: &Evidence) {
        for (name, (shift, precision)) in &evidence.players {
            let prior = self.player(name);
            let entry = self.players.entry(name.clone()).or_insert(prior);
            entry.mu += shift;
            entry.sigma = (1.0 / (1.0 / prior.variance() + precision)).max(0.0).sqrt();
            self.clamp_player(name);
        }
        let max = rd_to_logit(self.cfg.civ_prior_rd);
        let min = rd_to_logit(self.cfg.min_rd).min(max);
        for (name, (shift, precision)) in &evidence.civs {
            let prior = self.civ_edge(name);
            let entry = self.civs.entry(name.clone()).or_insert(prior);
            entry.mu += shift;
            entry.sigma = (1.0 / (1.0 / prior.variance() + precision))
                .max(0.0)
                .sqrt()
                .clamp(min, max);
        }
    }

    fn clamp_player(&mut self, name: &str) {
        let min = rd_to_logit(self.cfg.min_rd);
        let max = rd_to_logit(self.cfg.max_rd);
        if let Some(b) = self.players.get_mut(name) {
            b.sigma = b.sigma.clamp(min, max);
        }
    }

    /// Only sums of `player + civ` are observable, so the split between them
    /// is fixed by convention: civ edges average zero, and if anchors exist
    /// their mean rating is pinned. Both are gauge choices — they never
    /// change a forecast, only which half of it is called "skill".
    fn recentre(&mut self) {
        if !self.civs.is_empty() {
            let mean = self.civs.values().map(|b| b.mu).sum::<f64>() / self.civs.len() as f64;
            if mean.abs() > 1e-15 {
                for b in self.civs.values_mut() {
                    b.mu -= mean;
                }
                for b in self.players.values_mut() {
                    b.mu += mean;
                }
            }
        }
        if self.cfg.anchors.is_empty() {
            return;
        }
        let anchored: Vec<f64> = self
            .cfg
            .anchors
            .iter()
            .filter_map(|a| self.players.get(a).map(|b| b.mu))
            .collect();
        if anchored.is_empty() {
            return;
        }
        let mean = anchored.iter().sum::<f64>() / anchored.len() as f64;
        let target = elo_to_logit(self.cfg.anchor_elo);
        let shift = target - mean;
        if shift.abs() > 1e-15 {
            for b in self.players.values_mut() {
                b.mu += shift;
            }
        }
    }

    /// Ranked table, strongest first, using the conservative bound so a
    /// newcomer cannot top the board on one lucky game.
    pub fn standings(&self) -> String {
        let mut rows: Vec<(&String, &Belief)> = self.players.iter().collect();
        rows.sort_by(|a, b| {
            b.1.conservative_elo()
                .partial_cmp(&a.1.conservative_elo())
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.0.cmp(b.0))
        });
        let mut out = String::from(
            "player                      elo     RD    conf95   games  wins  winrate\n",
        );
        for (name, b) in rows {
            let r = self.record(name);
            let _ = writeln!(
                out,
                "  {:<22} {:7.1} ±{:5.1} {:8.1} {:>6} {:>5} {:>7.1}%",
                name,
                b.elo(),
                b.rd(),
                b.conservative_elo(),
                r.games,
                r.wins,
                100.0 * r.wins as f64 / r.games.max(1) as f64
            );
        }
        if !self.civs.is_empty() {
            out.push_str("\ncivilization edge (Elo points, averages zero by construction)\n");
            let mut civs: Vec<(&String, &Belief)> = self.civs.iter().collect();
            civs.sort_by(|a, b| {
                b.1.mu
                    .partial_cmp(&a.1.mu)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            for (civ, b) in civs {
                let r = self.civ_records.get(civ).cloned().unwrap_or_default();
                let _ = writeln!(
                    out,
                    "  {:<22} {:+7.1} ±{:5.1} {:>14} seats {:>5.1}% wins",
                    civ,
                    b.mu * ELO_PER_LOGIT,
                    b.rd(),
                    r.games,
                    100.0 * r.wins as f64 / r.games.max(1) as f64
                );
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Baselines, so "better" is a measurement rather than a claim.
// ---------------------------------------------------------------------------

/// What every candidate rating system must do: forecast before it is told.
pub trait RatingModel {
    fn name(&self) -> &str;
    /// Each seat's chance of winning the game. Must sum to one.
    fn forecast(&self, seats: &[Seat]) -> Vec<f64>;
    /// Chance seat `a` finishes above seat `b`.
    fn pair(&self, a: &Seat, b: &Seat) -> f64;
    fn observe(&mut self, m: &MatchRecord);
}

/// The floor: every seat equally likely. Any system that cannot beat this is
/// not measuring anything.
#[derive(Default)]
pub struct UniformModel;

impl RatingModel for UniformModel {
    fn name(&self) -> &str {
        "uniform (no information)"
    }
    fn forecast(&self, seats: &[Seat]) -> Vec<f64> {
        vec![1.0 / seats.len() as f64; seats.len()]
    }
    fn pair(&self, _a: &Seat, _b: &Seat) -> f64 {
        0.5
    }
    fn observe(&mut self, _m: &MatchRecord) {}
}

/// Plain Elo on player identity with the full pairwise decomposition, the
/// system `elo.rs` runs for one-shot tournaments.
pub struct EloModel {
    ratings: BTreeMap<String, f64>,
    k: f64,
}

impl Default for EloModel {
    fn default() -> EloModel {
        EloModel {
            ratings: BTreeMap::new(),
            k: 24.0,
        }
    }
}

impl EloModel {
    fn rating(&self, name: &str) -> f64 {
        self.ratings.get(name).copied().unwrap_or(BASE_ELO)
    }
    fn expect(a: f64, b: f64) -> f64 {
        1.0 / (1.0 + 10f64.powf((b - a) / 400.0))
    }
}

impl RatingModel for EloModel {
    fn name(&self) -> &str {
        "elo (pairwise placements)"
    }
    fn forecast(&self, seats: &[Seat]) -> Vec<f64> {
        let logits: Vec<f64> = seats
            .iter()
            .map(|s| elo_to_logit(self.rating(&s.player)))
            .collect();
        softmax(&logits)
    }
    fn pair(&self, a: &Seat, b: &Seat) -> f64 {
        EloModel::expect(self.rating(&a.player), self.rating(&b.player))
    }
    fn observe(&mut self, m: &MatchRecord) {
        let n = m.seats.len();
        if n < 2 {
            return;
        }
        let order = m.ranked_groups().concat();
        let mut delta: BTreeMap<String, f64> = BTreeMap::new();
        for i in 0..n {
            for j in (i + 1)..n {
                let (a, b) = (&m.seats[order[i]], &m.seats[order[j]]);
                if a.player == b.player {
                    continue;
                }
                let score = match m.seats[order[i]].rank.cmp(&m.seats[order[j]].rank) {
                    std::cmp::Ordering::Less => 1.0,
                    std::cmp::Ordering::Equal => 0.5,
                    std::cmp::Ordering::Greater => 0.0,
                };
                let e = EloModel::expect(self.rating(&a.player), self.rating(&b.player));
                let gain = self.k / (n as f64 - 1.0) * (score - e);
                *delta.entry(a.player.clone()).or_insert(0.0) += gain;
                *delta.entry(b.player.clone()).or_insert(0.0) -= gain;
            }
        }
        for (name, d) in delta {
            let r = self.ratings.entry(name).or_insert(BASE_ELO);
            *r += d;
        }
    }
}

/// Glicko-2 over the same pairwise decomposition, one game per rating
/// period — the system the league runs today when it rates a live game.
/// Written out here rather than reused so the baseline cannot silently
/// change under the comparison; `glicko2_matches_glickman_paper_example`
/// pins it to the published worked example.
pub struct Glicko2Model {
    state: BTreeMap<String, (f64, f64, f64)>,
    tau: f64,
}

impl Default for Glicko2Model {
    fn default() -> Glicko2Model {
        Glicko2Model {
            state: BTreeMap::new(),
            tau: 0.5,
        }
    }
}

impl Glicko2Model {
    fn get(&self, name: &str) -> (f64, f64, f64) {
        self.state
            .get(name)
            .copied()
            .unwrap_or((BASE_ELO, BASE_RD, 0.06))
    }
    fn g(phi: f64) -> f64 {
        1.0 / (1.0 + 3.0 * phi * phi / (std::f64::consts::PI * std::f64::consts::PI)).sqrt()
    }
    fn e(mu: f64, mu_j: f64, phi_j: f64) -> f64 {
        1.0 / (1.0 + (-Glicko2Model::g(phi_j) * (mu - mu_j)).exp())
    }
    /// Glicko-2 update for one player against `results` of
    /// `(opponent, score, weight)`.
    fn rate(p: (f64, f64, f64), results: &[((f64, f64, f64), f64, f64)], tau: f64) -> (f64, f64, f64) {
        let (mu, phi, sigma) = (
            (p.0 - BASE_ELO) / ELO_PER_LOGIT,
            p.1 / ELO_PER_LOGIT,
            p.2,
        );
        let mut v_inv = 0.0;
        let mut delta_sum = 0.0;
        for (o, s, w) in results {
            let mu_j = (o.0 - BASE_ELO) / ELO_PER_LOGIT;
            let phi_j = o.1 / ELO_PER_LOGIT;
            let g = Glicko2Model::g(phi_j);
            let e = Glicko2Model::e(mu, mu_j, phi_j);
            v_inv += w * g * g * e * (1.0 - e);
            delta_sum += w * g * (s - e);
        }
        if v_inv <= 0.0 {
            return p;
        }
        let v = 1.0 / v_inv;
        let delta = v * delta_sum;
        let a = (sigma * sigma).ln();
        let f = |x: f64| {
            let ex = x.exp();
            (ex * (delta * delta - phi * phi - v - ex)) / (2.0 * (phi * phi + v + ex).powi(2))
                - (x - a) / (tau * tau)
        };
        let mut lo = a;
        let mut hi = if delta * delta > phi * phi + v {
            (delta * delta - phi * phi - v).ln()
        } else {
            let mut k = 1.0;
            while f(a - k * tau) < 0.0 && k < 100.0 {
                k += 1.0;
            }
            a - k * tau
        };
        let (mut f_lo, mut f_hi) = (f(lo), f(hi));
        for _ in 0..200 {
            if (hi - lo).abs() <= 1e-6 {
                break;
            }
            let c = lo + (lo - hi) * f_lo / (f_hi - f_lo);
            let f_c = f(c);
            if f_c * f_hi <= 0.0 {
                lo = hi;
                f_lo = f_hi;
            } else {
                f_lo /= 2.0;
            }
            hi = c;
            f_hi = f_c;
        }
        let sigma_new = (lo / 2.0).exp();
        let phi_star = (phi * phi + sigma_new * sigma_new).sqrt();
        let phi_new = 1.0 / (1.0 / (phi_star * phi_star) + 1.0 / v).sqrt();
        let mu_new = mu + phi_new * phi_new * delta_sum;
        (
            BASE_ELO + mu_new * ELO_PER_LOGIT,
            phi_new * ELO_PER_LOGIT,
            sigma_new,
        )
    }
}

impl RatingModel for Glicko2Model {
    fn name(&self) -> &str {
        "glicko-2 (league today)"
    }
    fn forecast(&self, seats: &[Seat]) -> Vec<f64> {
        let logits: Vec<f64> = seats
            .iter()
            .map(|s| {
                let (r, d, _) = self.get(&s.player);
                Glicko2Model::g(d / ELO_PER_LOGIT) * (r - BASE_ELO) / ELO_PER_LOGIT
            })
            .collect();
        softmax(&logits)
    }
    fn pair(&self, a: &Seat, b: &Seat) -> f64 {
        let (ra, da, _) = self.get(&a.player);
        let (rb, db, _) = self.get(&b.player);
        let phi = (da * da + db * db).sqrt() / ELO_PER_LOGIT;
        1.0 / (1.0 + (-Glicko2Model::g(phi) * (ra - rb) / ELO_PER_LOGIT).exp())
    }
    fn observe(&mut self, m: &MatchRecord) {
        let n = m.seats.len();
        if n < 2 {
            return;
        }
        let mut results: BTreeMap<String, Vec<((f64, f64, f64), f64, f64)>> = BTreeMap::new();
        for i in 0..n {
            for j in (i + 1)..n {
                let (a, b) = (&m.seats[i], &m.seats[j]);
                if a.player == b.player {
                    continue;
                }
                let score = match a.rank.cmp(&b.rank) {
                    std::cmp::Ordering::Less => 1.0,
                    std::cmp::Ordering::Equal => 0.5,
                    std::cmp::Ordering::Greater => 0.0,
                };
                let w = 1.0 / (n as f64 - 1.0);
                let (ga, gb) = (self.get(&a.player), self.get(&b.player));
                results
                    .entry(a.player.clone())
                    .or_default()
                    .push((gb, score, w));
                results
                    .entry(b.player.clone())
                    .or_default()
                    .push((ga, 1.0 - score, w));
            }
        }
        let updated: Vec<(String, (f64, f64, f64))> = results
            .iter()
            .map(|(name, rs)| {
                (
                    name.clone(),
                    Glicko2Model::rate(self.get(name), rs, self.tau),
                )
            })
            .collect();
        for (name, s) in updated {
            self.state.insert(name, s);
        }
    }
}

impl RatingModel for ContextualRating {
    fn name(&self) -> &str {
        "contextual (skill + civ, staged)"
    }
    fn forecast(&self, seats: &[Seat]) -> Vec<f64> {
        self.win_probabilities(seats)
    }
    fn pair(&self, a: &Seat, b: &Seat) -> f64 {
        self.pair_probability(a, b)
    }
    fn observe(&mut self, m: &MatchRecord) {
        ContextualRating::observe(self, m)
    }
}

// ---------------------------------------------------------------------------
// Honest evaluation
// ---------------------------------------------------------------------------

/// Forecast quality over a replay. Every number is out of sample: the model
/// predicted, then was told.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    pub games: u64,
    /// Negative log likelihood of the seat that actually won. Compare
    /// against `ln(players per game)`.
    pub win_log_loss: f64,
    /// How often the highest-rated seat was the one that won.
    pub win_accuracy: f64,
    /// Mean squared error of the win forecast.
    pub win_brier: f64,
    /// Pairwise scores, the metric the league publishes today. Compare
    /// against 0.6931 and 0.2500.
    pub pair_log_loss: f64,
    pub pair_brier: f64,
    pub pair_count: u64,
    /// `ln(seats) - win_log_loss`: nats of real information per game.
    pub information: f64,
    /// What knowing nothing scores on this same history. Table sizes vary,
    /// so this is not a constant and has to be measured alongside.
    pub uniform_log_loss: f64,
    /// What guessing at random scores, i.e. mean `1 / seats`.
    pub uniform_accuracy: f64,
}

#[derive(Default)]
struct MetricSums {
    games: u64,
    win_ll: f64,
    win_hits: f64,
    win_brier: f64,
    uniform_ll: f64,
    uniform_hits: f64,
    pair_ll: f64,
    pair_brier: f64,
    pairs: u64,
}

impl MetricSums {
    fn finish(&self) -> Metrics {
        let g = self.games.max(1) as f64;
        let p = self.pairs.max(1) as f64;
        Metrics {
            games: self.games,
            win_log_loss: self.win_ll / g,
            win_accuracy: self.win_hits / g,
            win_brier: self.win_brier / g,
            pair_log_loss: self.pair_ll / p,
            pair_brier: self.pair_brier / p,
            pair_count: self.pairs,
            information: (self.uniform_ll - self.win_ll) / g,
            uniform_log_loss: self.uniform_ll / g,
            uniform_accuracy: self.uniform_hits / g,
        }
    }
}

fn score_game(sums: &mut MetricSums, model: &dyn RatingModel, m: &MatchRecord) {
    // A history stores seats in finishing order, so scoring them in that
    // order would let a model that breaks ties by position look clairvoyant.
    // Canonicalise first: the outcome then lives only in `rank`.
    let mut seats = m.seats.clone();
    seats.sort_by(|a, b| {
        a.player
            .cmp(&b.player)
            .then(a.civ.cmp(&b.civ))
            .then(a.rank.cmp(&b.rank))
    });
    let seats = &seats;
    let n = seats.len();
    if n < 2 {
        return;
    }
    let probs = model.forecast(seats);
    let total: f64 = probs.iter().sum();
    let probs: Vec<f64> = if total > 0.0 {
        probs.iter().map(|p| p / total).collect()
    } else {
        vec![1.0 / n as f64; n]
    };
    let best = seats.iter().map(|s| s.rank).min().unwrap_or(0);
    let winners: Vec<usize> = (0..n).filter(|i| seats[*i].rank == best).collect();
    // A tie for first is scored as the mass the model put on any of them.
    let mass: f64 = winners.iter().map(|i| probs[*i]).sum::<f64>() / winners.len() as f64;
    sums.games += 1;
    sums.win_ll -= mass.clamp(1e-12, 1.0).ln();
    sums.uniform_ll += (n as f64).ln();
    sums.uniform_hits += winners.len() as f64 / n as f64;
    for (i, p) in probs.iter().enumerate() {
        let actual = if winners.contains(&i) {
            1.0 / winners.len() as f64
        } else {
            0.0
        };
        sums.win_brier += (p - actual) * (p - actual);
    }
    let top = (0..n).max_by(|a, b| {
        probs[*a]
            .partial_cmp(&probs[*b])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if let Some(top) = top {
        if winners.contains(&top) {
            sums.win_hits += 1.0;
        }
    }
    for i in 0..n {
        for j in (i + 1)..n {
            if seats[i].player == seats[j].player {
                continue;
            }
            let actual = match seats[i].rank.cmp(&seats[j].rank) {
                std::cmp::Ordering::Less => 1.0,
                std::cmp::Ordering::Equal => 0.5,
                std::cmp::Ordering::Greater => 0.0,
            };
            let p = model.pair(&seats[i], &seats[j]).clamp(1e-12, 1.0 - 1e-12);
            sums.pairs += 1;
            sums.pair_brier += (p - actual) * (p - actual);
            sums.pair_ll -= actual * p.ln() + (1.0 - actual) * (1.0 - p).ln();
        }
    }
}

/// Replay `history` through `model`, scoring only the tail so every model
/// gets the same chance to learn from the same warm-up.
pub fn evaluate(model: &mut dyn RatingModel, history: &[MatchRecord], burn_in: f64) -> Metrics {
    let start = ((history.len() as f64) * burn_in.clamp(0.0, 0.95)) as usize;
    let mut sums = MetricSums::default();
    for (i, m) in history.iter().enumerate() {
        if i >= start {
            score_game(&mut sums, model, m);
        }
        model.observe(m);
    }
    sums.finish()
}

/// How much information each placement stage carries, measured rather than
/// assumed. Stage `k` is "which of the seats not yet placed finished next";
/// the value is `ln(remaining) - log loss`, in nats. A stage at or below
/// zero is noise and should carry no weight in an update.
pub fn fit_stage_weights(history: &[MatchRecord], burn_in: f64) -> Vec<f64> {
    let mut model = ContextualRating::new(RatingCfg {
        // Measure with flat weights so the answer is not assumed by the prior.
        stage_decay: 1.0,
        ..RatingCfg::default()
    });
    let start = ((history.len() as f64) * burn_in.clamp(0.0, 0.95)) as usize;
    let mut info: Vec<(f64, u64)> = Vec::new();
    for (i, m) in history.iter().enumerate() {
        if i >= start {
            let beliefs: Vec<Belief> = m.seats.iter().map(|s| model.seat_belief(s)).collect();
            let groups = m.ranked_groups();
            let mut remaining: Vec<usize> = groups.iter().flatten().copied().collect();
            for (k, group) in groups.iter().enumerate() {
                if remaining.len() < 2 {
                    break;
                }
                let probs = model.stage_probabilities(&beliefs, &remaining);
                let mass: f64 = remaining
                    .iter()
                    .enumerate()
                    .filter(|(_, idx)| group.contains(idx))
                    .map(|(slot, _)| probs[slot])
                    .sum::<f64>()
                    / group.len() as f64;
                let uniform = (remaining.len() as f64).ln() - (group.len() as f64).ln();
                if info.len() <= k {
                    info.resize(k + 1, (0.0, 0));
                }
                info[k].0 += uniform + mass.clamp(1e-12, 1.0).ln();
                info[k].1 += 1;
                remaining.retain(|i| !group.contains(i));
            }
        }
        model.observe(m);
    }
    info.into_iter()
        .map(|(sum, n)| if n == 0 { 0.0 } else { sum / n as f64 })
        .collect()
}

/// Result of one model's replay, ready to print.
pub struct BacktestRow {
    pub name: String,
    pub metrics: Metrics,
}

/// Replay one history through every candidate system.
pub fn backtest(history: &[MatchRecord], burn_in: f64, cfg: &RatingCfg) -> Vec<BacktestRow> {
    let mut models: Vec<Box<dyn RatingModel>> = vec![
        Box::new(UniformModel),
        Box::new(EloModel::default()),
        Box::new(Glicko2Model::default()),
        Box::new(ContextualRating::new(RatingCfg {
            civ_prior_rd: 0.0,
            ..cfg.clone()
        })),
        Box::new(ContextualRating::new(cfg.clone())),
    ];
    let names = [
        "uniform (no information)",
        "elo (pairwise placements)",
        "glicko-2 (league today)",
        "staged, no civ context",
        "staged + civ context",
    ];
    models
        .iter_mut()
        .zip(names)
        .map(|(model, name)| BacktestRow {
            name: name.to_string(),
            metrics: evaluate(model.as_mut(), history, burn_in),
        })
        .collect()
}

/// Format a backtest for a terminal.
pub fn backtest_report(rows: &[BacktestRow], seats: f64) -> String {
    let mut out = String::new();
    let first = rows.first().map(|r| r.metrics.clone()).unwrap_or_default();
    let _ = writeln!(
        out,
        "forecast quality, scored before each result was revealed \
         ({} games, {seats:.1} seats on average)",
        first.games
    );
    let _ = writeln!(
        out,
        "  winner LL:  negative log likelihood of the seat that won; \
         {:.4} = knowing nothing",
        first.uniform_log_loss
    );
    let _ = writeln!(
        out,
        "  info/game:  nats the system knew that guessing did not; \
         at or below zero it knows nothing"
    );
    let _ = writeln!(
        out,
        "  pair LL:    the metric the league publishes today; 0.6931 = knowing nothing\n"
    );
    let _ = writeln!(
        out,
        "{:<30}{:>12}{:>10}{:>12}{:>12}{:>11}",
        "rating system", "winner LL", "accuracy", "info/game", "pair LL", "pair Brier"
    );
    for row in rows {
        let m = &row.metrics;
        let _ = writeln!(
            out,
            "{:<30}{:>12.4}{:>9.1}%{:>12.4}{:>12.4}{:>11.4}",
            row.name,
            m.win_log_loss,
            100.0 * m.win_accuracy,
            m.information,
            m.pair_log_loss,
            m.pair_brier
        );
    }
    let _ = writeln!(
        out,
        "{:<30}{:>12.4}{:>9.1}%{:>12.4}{:>12.4}{:>11.4}   <- guessing",
        "(random guess)",
        first.uniform_log_loss,
        100.0 * first.uniform_accuracy,
        0.0,
        std::f64::consts::LN_2,
        0.25
    );
    out
}

// ---------------------------------------------------------------------------
// Reading a league's history
// ---------------------------------------------------------------------------

/// Parse a league `matches.csv`. The placement column is
/// `name@Civ|name@Civ|...` in finishing order; anything after a second `@`
/// (a leader, say) is kept with the civ so richer histories still load.
pub fn parse_matches_csv(text: &str) -> Vec<MatchRecord> {
    let mut out = Vec::new();
    for line in text.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.splitn(5, ',').collect();
        if cols.len() < 5 {
            continue;
        }
        let seats: Vec<Seat> = cols[4]
            .split('|')
            .enumerate()
            .filter(|(_, s)| !s.trim().is_empty())
            .map(|(rank, entry)| {
                let (player, civ) = entry.split_once('@').unwrap_or((entry, ""));
                Seat {
                    player: player.trim().to_string(),
                    civ: civ.trim().to_string(),
                    rank: rank as u32,
                }
            })
            .collect();
        if seats.len() < 2 {
            continue;
        }
        out.push(MatchRecord {
            seats,
            round: cols[0].parse().unwrap_or(0),
            turn: cols[2].parse().unwrap_or(0),
            victory: cols[3].to_string(),
        });
    }
    out
}

/// Load the match history of a league directory.
pub fn load_history(dir: &str) -> std::io::Result<Vec<MatchRecord>> {
    let path = Path::new(dir).join("matches.csv");
    let text = std::fs::read_to_string(&path)?;
    Ok(parse_matches_csv(&text))
}

/// Rate a whole history from scratch and return the finished system.
pub fn rate_history(history: &[MatchRecord], cfg: &RatingCfg) -> ContextualRating {
    let mut rating = ContextualRating::new(cfg.clone());
    for m in history {
        rating.observe(m);
    }
    rating
}

// ---------------------------------------------------------------------------
// Seating: the other half of the fix
// ---------------------------------------------------------------------------

/// Assign `players` to `civs` for game number `game`, rotating so that over
/// a full cycle every player draws every civ the same number of times.
///
/// Seating the best-rated strategy on each civ, which is what the live
/// exhibition did, makes strategy and civilization the same variable: in the
/// observed history one strategy played Rome in 200 consecutive games. No
/// rating system can separate skill from draw under that design, so this
/// rotation is a precondition for the ratings above meaning anything. It is
/// a Latin square: deterministic, reproducible, and balanced by construction.
pub fn rotate_seating(players: usize, civs: usize, game: u64) -> Vec<usize> {
    if players == 0 || civs == 0 {
        return Vec::new();
    }
    let n = players.min(civs);
    let offset = (game % n as u64) as usize;
    (0..n).map(|civ| (civ + offset) % n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seat(player: &str, civ: &str, rank: u32) -> Seat {
        Seat {
            player: player.into(),
            civ: civ.into(),
            rank,
        }
    }

    fn game(order: &[(&str, &str)]) -> MatchRecord {
        MatchRecord {
            seats: order
                .iter()
                .enumerate()
                .map(|(i, (p, c))| seat(p, c, i as u32))
                .collect(),
            ..MatchRecord::default()
        }
    }

    #[test]
    fn elo_and_logit_scales_round_trip() {
        let b = Belief::from_elo(1800.0, 90.0);
        assert!((b.elo() - 1800.0).abs() < 1e-9);
        assert!((b.rd() - 90.0).abs() < 1e-9);
        // 400 points is the definition of a ten-to-one favourite.
        let strong = Belief::from_elo(1900.0, 0.0);
        let weak = Belief::from_elo(1500.0, 0.0);
        let odds = (strong.mu - weak.mu).exp();
        assert!((odds - 10.0).abs() < 1e-6, "400 points should be 10:1");
    }

    #[test]
    fn stage_weights_decay_and_sum_to_one() {
        let cfg = RatingCfg::default();
        let w = cfg.stage_weights(5);
        assert_eq!(w.len(), 5);
        assert!((w.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        for pair in w.windows(2) {
            assert!(pair[0] > pair[1], "later stages must count for less");
        }
        // Decay 0 rates only who won.
        let only_winner = RatingCfg {
            stage_decay: 0.0,
            ..RatingCfg::default()
        }
        .stage_weights(4);
        assert!((only_winner[0] - 1.0).abs() < 1e-12);
        assert!(only_winner[1..].iter().all(|w| *w == 0.0));
    }

    #[test]
    fn winning_raises_a_rating_and_losing_lowers_it() {
        let mut r = ContextualRating::default();
        r.observe(&game(&[("a", "Rome"), ("b", "Egypt")]));
        assert!(r.player("a").elo() > BASE_ELO);
        assert!(r.player("b").elo() < BASE_ELO);
        assert_eq!(r.record("a").wins, 1);
        assert_eq!(r.record("b").wins, 0);
    }

    #[test]
    fn a_tie_moves_nobody_and_counts_nobody_a_winner() {
        let mut r = ContextualRating::default();
        r.observe(&MatchRecord {
            seats: vec![seat("a", "Rome", 0), seat("b", "Egypt", 0)],
            ..MatchRecord::default()
        });
        assert!((r.player("a").elo() - r.player("b").elo()).abs() < 1e-9);
        assert_eq!(r.record("a").wins, 0);
        assert_eq!(r.record("b").wins, 0);
    }

    #[test]
    fn confidence_grows_with_evidence_but_never_to_certainty() {
        let mut r = ContextualRating::default();
        for i in 0..200 {
            let m = if i % 2 == 0 {
                game(&[("a", "Rome"), ("b", "Egypt")])
            } else {
                game(&[("a", "Egypt"), ("b", "Rome")])
            };
            r.observe(&m);
        }
        let rd = r.player("a").rd();
        assert!(rd < BASE_RD, "200 games must settle a rating");
        assert!(
            rd >= RatingCfg::default().min_rd - 1e-9,
            "a rating never becomes certain: {rd}"
        );
    }

    #[test]
    fn forecasts_are_probabilities_that_sum_to_one() {
        let mut r = ContextualRating::default();
        r.observe(&game(&[("a", "Rome"), ("b", "Egypt"), ("c", "Greece")]));
        let seats = vec![
            seat("a", "Rome", 0),
            seat("b", "Egypt", 1),
            seat("c", "Greece", 2),
        ];
        let p = r.win_probabilities(&seats);
        assert_eq!(p.len(), 3);
        assert!((p.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        assert!(p.iter().all(|x| *x > 0.0 && *x < 1.0));
        // The winner should be the favourite after that evidence.
        assert!(p[0] > p[1] && p[1] > p[2]);
    }

    #[test]
    fn the_civ_edge_is_learned_and_averages_zero() {
        // Four equally skilled players; Rome wins whoever holds it.
        let mut r = ContextualRating::default();
        let players = ["a", "b", "c", "d"];
        let civs = ["Rome", "Egypt", "Greece", "China"];
        for g in 0..400u64 {
            let assign = rotate_seating(4, 4, g);
            // Rome wins. The order behind it rotates on its own cycle, so
            // over the run nothing but the civilization separates the four
            // players — exactly the balanced design a live league needs.
            let mut order: Vec<usize> = (0..4).collect();
            order.sort_by_key(|i| (*i as u64 + g / 4) % 4);
            order.sort_by_key(|i| u32::from(civs[assign[*i]] != "Rome"));
            let seats: Vec<Seat> = order
                .iter()
                .enumerate()
                .map(|(rank, i)| seat(players[*i], civs[assign[*i]], rank as u32))
                .collect();
            r.observe(&MatchRecord {
                seats,
                ..MatchRecord::default()
            });
        }
        let edges: Vec<f64> = civs.iter().map(|c| r.civ_edge(c).mu).collect();
        let mean: f64 = edges.iter().sum::<f64>() / edges.len() as f64;
        assert!(mean.abs() < 1e-6, "civ edges must average zero: {mean}");
        let rome = r.civ_edge("Rome").mu;
        assert!(
            edges.iter().all(|e| rome >= *e),
            "Rome must carry the highest edge"
        );
        // With balanced seating the players themselves stay indistinguishable.
        let spread = players
            .iter()
            .map(|p| r.player(p).elo())
            .fold((f64::MAX, f64::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
        assert!(
            spread.1 - spread.0 < 60.0,
            "equal players should not separate: {spread:?}"
        );
    }

    #[test]
    fn a_settled_context_lets_a_newcomer_absorb_the_surprise() {
        // Establish the civ edges with a long balanced history...
        let mut r = ContextualRating::default();
        for g in 0..200u64 {
            let assign = rotate_seating(2, 2, g);
            let civs = ["Rome", "Egypt"];
            let mut seats = vec![
                seat("a", civs[assign[0]], 0),
                seat("b", civs[assign[1]], 0),
            ];
            seats.sort_by_key(|s| u32::from(s.civ != "Rome"));
            for (rank, s) in seats.iter_mut().enumerate() {
                s.rank = rank as u32;
            }
            r.observe(&MatchRecord {
                seats,
                ..MatchRecord::default()
            });
        }
        let civ_before = r.civ_edge("Rome").mu;
        let newcomer_before = r.player("newbie").elo();
        r.observe(&game(&[("newbie", "Rome"), ("a", "Egypt")]));
        let civ_moved = (r.civ_edge("Rome").mu - civ_before).abs();
        let player_moved = (r.player("newbie").elo() - newcomer_before).abs();
        assert!(
            player_moved > civ_moved * ELO_PER_LOGIT,
            "an unrated player must absorb more than a settled civ edge"
        );
    }

    #[test]
    fn anchors_pin_the_scale_while_the_roster_churns() {
        let cfg = RatingCfg {
            anchors: ["anchor".to_string()].into_iter().collect(),
            anchor_elo: 1500.0,
            ..RatingCfg::default()
        };
        let mut r = ContextualRating::new(cfg);
        for _ in 0..50 {
            r.observe(&game(&[("climber", "Rome"), ("anchor", "Egypt")]));
        }
        assert!(
            (r.player("anchor").elo() - 1500.0).abs() < 1e-6,
            "the anchor must stay put: {}",
            r.player("anchor").elo()
        );
        assert!(
            r.player("climber").elo() > 1600.0,
            "and everyone else moves relative to it"
        );
    }

    #[test]
    fn rotate_seating_is_a_balanced_latin_square() {
        let n = 6;
        let mut counts = vec![vec![0u32; n]; n];
        for g in 0..(n as u64 * 10) {
            let assign = rotate_seating(n, n, g);
            let mut seen = BTreeSet::new();
            for (player, civ) in assign.iter().enumerate() {
                assert!(seen.insert(*civ), "an assignment must be a permutation");
                counts[player][*civ] += 1;
            }
        }
        for row in &counts {
            assert!(
                row.iter().all(|c| *c == 10),
                "every player draws every civ equally: {row:?}"
            );
        }
    }

    #[test]
    fn glicko2_matches_glickman_paper_example() {
        // Glickman's worked example: a 1500/200 player beats 1400/30,
        // loses to 1550/100 and 1700/300, with tau = 0.5.
        let player = (1500.0, 200.0, 0.06);
        let results = [
            ((1400.0, 30.0, 0.06), 1.0, 1.0),
            ((1550.0, 100.0, 0.06), 0.0, 1.0),
            ((1700.0, 300.0, 0.06), 0.0, 1.0),
        ];
        let (rating, rd, vol) = Glicko2Model::rate(player, &results, 0.5);
        assert!((rating - 1464.06).abs() < 0.1, "rating was {rating}");
        assert!((rd - 151.52).abs() < 0.1, "rd was {rd}");
        assert!((vol - 0.05999).abs() < 1e-4, "volatility was {vol}");
    }

    #[test]
    fn parsing_a_matches_csv_recovers_seats_and_order() {
        let csv = "round,seed,turns,victory,placements\n\
                   61,571192301,198,religious,alpha@Egypt|beta@Rome|gamma@Greece\n";
        let history = parse_matches_csv(csv);
        assert_eq!(history.len(), 1);
        let m = &history[0];
        assert_eq!(m.round, 61);
        assert_eq!(m.turn, 198);
        assert_eq!(m.victory, "religious");
        assert_eq!(m.seats.len(), 3);
        assert_eq!(m.seats[0].player, "alpha");
        assert_eq!(m.seats[0].civ, "Egypt");
        assert_eq!(m.seats[0].rank, 0);
        assert_eq!(m.seats[2].rank, 2);
    }

    /// The headline claim, on synthetic data whose truth we control: with
    /// tail placements made pure noise, staged weighting must beat the
    /// pairwise decomposition the league uses today.
    #[test]
    fn staged_weighting_beats_pairwise_when_the_tail_is_noise() {
        let mut history = Vec::new();
        let strength = [3.0f64, 2.0, 1.0, 0.0, -1.0, -2.0];
        let mut state = 12345u64;
        let mut rand = move || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((state >> 33) as f64) / ((1u64 << 31) as f64)
        };
        for _ in 0..600 {
            // The winner is drawn by strength; everyone else is shuffled.
            let weights: Vec<f64> = strength.iter().map(|s| s.exp()).collect();
            let total: f64 = weights.iter().sum();
            let mut pick = rand() * total;
            let mut winner = 0;
            for (i, w) in weights.iter().enumerate() {
                pick -= w;
                if pick <= 0.0 {
                    winner = i;
                    break;
                }
            }
            let mut rest: Vec<usize> = (0..6).filter(|i| *i != winner).collect();
            for i in (1..rest.len()).rev() {
                let j = (rand() * (i + 1) as f64) as usize;
                rest.swap(i, j.min(i));
            }
            let mut seats = vec![Seat {
                player: format!("p{winner}"),
                civ: String::new(),
                rank: 0,
            }];
            for (k, idx) in rest.iter().enumerate() {
                seats.push(Seat {
                    player: format!("p{idx}"),
                    civ: String::new(),
                    rank: (k + 1) as u32,
                });
            }
            history.push(MatchRecord {
                seats,
                ..MatchRecord::default()
            });
        }
        let staged = evaluate(&mut ContextualRating::default(), &history, 0.3);
        let pairwise = evaluate(&mut Glicko2Model::default(), &history, 0.3);
        let uniform = evaluate(&mut UniformModel, &history, 0.3);
        assert!(
            staged.win_log_loss < pairwise.win_log_loss,
            "staged {:.4} should beat pairwise glicko {:.4}",
            staged.win_log_loss,
            pairwise.win_log_loss
        );
        assert!(
            staged.win_log_loss < uniform.win_log_loss,
            "staged {:.4} should beat knowing nothing {:.4}",
            staged.win_log_loss,
            uniform.win_log_loss
        );
        assert!(
            staged.information > 0.15,
            "should recover real information: {:.4} nats",
            staged.information
        );
    }

    #[test]
    fn measured_stage_information_falls_off_toward_the_tail() {
        let mut history = Vec::new();
        let mut state = 99u64;
        let mut rand = move || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((state >> 33) as f64) / ((1u64 << 31) as f64)
        };
        for _ in 0..600 {
            // p0 nearly always wins; the rest of the order is a shuffle.
            let winner = if rand() < 0.8 { 0 } else { 1 + (rand() * 4.0) as usize };
            let winner = winner.min(4);
            let mut rest: Vec<usize> = (0..5).filter(|i| *i != winner).collect();
            for i in (1..rest.len()).rev() {
                let j = (rand() * (i + 1) as f64) as usize;
                rest.swap(i, j.min(i));
            }
            let mut seats = vec![Seat {
                player: format!("p{winner}"),
                civ: String::new(),
                rank: 0,
            }];
            for (k, idx) in rest.iter().enumerate() {
                seats.push(Seat {
                    player: format!("p{idx}"),
                    civ: String::new(),
                    rank: (k + 1) as u32,
                });
            }
            history.push(MatchRecord {
                seats,
                ..MatchRecord::default()
            });
        }
        let info = fit_stage_weights(&history, 0.3);
        assert!(info.len() >= 4);
        assert!(
            info[0] > 0.2,
            "who won must be informative: {:.4} nats",
            info[0]
        );
        assert!(
            info[0] > info[3],
            "the tail must carry less than the head: {info:?}"
        );
    }

    #[test]
    fn rating_a_history_is_deterministic() {
        let history = vec![
            game(&[("a", "Rome"), ("b", "Egypt"), ("c", "Greece")]),
            game(&[("c", "Rome"), ("a", "Egypt"), ("b", "Greece")]),
            game(&[("b", "Rome"), ("c", "Egypt"), ("a", "Greece")]),
        ];
        let cfg = RatingCfg::default();
        let a = rate_history(&history, &cfg);
        let b = rate_history(&history, &cfg);
        for name in ["a", "b", "c"] {
            assert_eq!(a.player(name), b.player(name));
        }
        assert_eq!(a.standings(), b.standings());
    }
}
