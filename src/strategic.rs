//! StrategicAi: rollout-based victory routing on top of AdvancedAi.
//!
//! Every `review_every` turns the agent simulates staying adaptive and committing
//! to each enabled victory lane for `horizon` rounds (rivals remain AdvancedAi
//! opponents). It commits only when a targeted policy beats its adaptive parent
//! by a real margin — macro search applied to victory routing, generalizing the
//! war-decision rollouts `NeuralAi` proved. Positions are judged by the trained
//! value net when `evolved/valuenet.json` exists, otherwise by score share. Public
//! victory threats interrupt the periodic search before they can end the game,
//! while irreversible Prophet investment and duel victory geometry supply
//! priors that a short economic rollout cannot discover in time. The learned
//! estimate is deliberately regularized toward score share because the
//! counterfactual rollout endpoints remain out of distribution for ordinary
//! self-play trajectories.
use crate::ai::{run_game, AdvancedAi, Ai, PlanReport, VictoryTarget, Weights};
use crate::evolve::features;
use crate::game::{Action, Game, Item};
use crate::obs_tensor::obs_tensor;
use crate::valuenet::ValueNet;

const TARGET_COMMITMENT_MARGIN: f64 = 0.01;
const VALUE_NET_WEIGHT: f64 = 0.25;
/// Improvement a rival doctrine must show over the one in force. Set from
/// the measured spread between doctrine rollouts rather than copied from
/// the lane margin: the two axes move the projected value by different
/// amounts, and a margin larger than the spread is a search that can never
/// choose.
const DOCTRINE_COMMITMENT_MARGIN: f64 = 0.002;
/// Rounds projected between separation checks under an adaptive horizon.
/// Small enough to stop soon after the branches part, large enough that
/// the check itself is not most of the work.
const ADAPTIVE_CHUNK: u32 = 10;
pub const FIRST_REVIEW_TURN: u32 = 30;

/// One position on the exact distribution consumed by the Strategic value
/// evaluator, paired with the eventual result of continuing that rollout.
#[derive(Clone, Debug, PartialEq)]
pub struct CounterfactualValueSample {
    pub features: Vec<f32>,
    /// Fog-honest endpoint tensor, populated only when the exporter asks for
    /// a spatial sample. It is captured before the branch is continued to
    /// its terminal label, exactly where Strategic evaluates the rollout.
    pub planes: Vec<f32>,
    pub globals: Vec<f32>,
    pub global_names: Vec<String>,
    pub won: bool,
    pub turn_fraction: f32,
    /// `None` is the adaptive parent; `Some` is a committed victory lane.
    pub target: Option<VictoryTarget>,
}

/// A named play style: a bounded perturbation of the strategy genome.
///
/// The rollout planner's one free variable has always been the victory
/// lane — seven branches, re-decided about three to ten times a game. A
/// lane says what the empire is playing *for*; a doctrine says how it
/// plays, which is the dimension a strong human adapts to the map and the
/// neighbours. Both are searched the same way, by rolling the position
/// forward and reading the result, so neither depends on the learned
/// evaluator being any good.
///
/// The perturbations are deliberately coarse and few. This is a search
/// axis, not an optimizer: the genome is already tuned offline by
/// `evolve`, and the question here is only whether *this* position wants a
/// wider, tighter or more aggressive version of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Doctrine {
    /// The evolved genome exactly as tuned. Wins every tie, so a doctrine
    /// is adopted only on evidence.
    Incumbent,
    /// More cities, sooner, closer together, with the builders to work
    /// them.
    Expand,
    /// Fewer, better cities: stop settling early and spend on
    /// infrastructure and wonders instead.
    Consolidate,
    /// A larger standing army and a lower bar for using it.
    Militarize,
}

impl Doctrine {
    pub const ALL: [Doctrine; 4] = [
        Doctrine::Incumbent,
        Doctrine::Expand,
        Doctrine::Consolidate,
        Doctrine::Militarize,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Doctrine::Incumbent => "incumbent",
            Doctrine::Expand => "expand",
            Doctrine::Consolidate => "consolidate",
            Doctrine::Militarize => "militarize",
        }
    }

    /// Apply this doctrine to a genome, clamped to the same per-gene bounds
    /// evolution respects. Clamping matters: an unbounded multiple of an
    /// already-extreme evolved gene is not a play style, it is a broken
    /// agent, and the rollout would duly report that.
    pub fn apply(self, base: &Weights) -> Weights {
        let mut w = base.clone();
        match self {
            Doctrine::Incumbent => return w,
            Doctrine::Expand => {
                w.city_target *= 1.5;
                w.settler_stop_turn *= 1.3;
                w.settler_min_pop *= 0.7;
                w.min_city_dist *= 0.85;
                w.builder_per_city *= 1.2;
            }
            Doctrine::Consolidate => {
                w.city_target *= 0.7;
                w.settler_stop_turn *= 0.7;
                w.builder_per_city *= 1.4;
                w.wonder_min_bld *= 0.7;
                w.settle_dist *= 1.3;
            }
            Doctrine::Militarize => {
                w.mil_per_city *= 1.6;
                w.war_ratio *= 0.85;
                w.war_margin *= 0.7;
                w.war_min_turn *= 0.7;
                w.focus_fire *= 1.3;
                w.local_superiority *= 0.8;
            }
        }
        let mut genes = w.to_vec();
        for (gene, (low, high)) in genes.iter_mut().zip(Weights::bounds()) {
            *gene = gene.clamp(low, high);
        }
        Weights::from_vec(&genes)
    }
}

/// Which branch of `review` produced a lane, recorded so a run can say
/// whether the macro search ran at all.
///
/// Three priors deliberately answer before the economic rollouts, because a
/// bounded projection cannot discover an irreversible or already-decided
/// race in time. Each is cheap and each is correct in isolation — but a
/// prior that fires on every review turns the search agent into a scripted
/// one wearing its name, and nothing in a win rate distinguishes the two.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewPath {
    /// Two living majors and an open religious race: the lane is forced.
    DuelReligion,
    /// A rival is about to win; the counter-target is read off the
    /// victory screen rather than projected.
    UrgentCounter,
    /// Prophet investment already made and a slot still open.
    IrreversibleReligion,
    /// The economic rollouts actually ran and the evaluator was consulted.
    Rollouts,
}

/// How a run's reviews were resolved, aggregated over one agent's lifetime.
///
/// `rollouts` is the load-bearing number: it counts the reviews where the
/// value evaluator was consulted, so `0` says the run measured priors and
/// the scripted parent, whatever the entrant was called.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReviewCensus {
    pub duel_religion: u32,
    pub urgent_counter: u32,
    pub irreversible_religion: u32,
    pub rollouts: u32,
}

impl ReviewCensus {
    pub fn total(&self) -> u32 {
        self.duel_religion + self.urgent_counter + self.irreversible_religion + self.rollouts
    }

    /// Accumulate another agent's reviews into this total.
    pub fn merge(&mut self, other: ReviewCensus) {
        self.duel_religion += other.duel_religion;
        self.urgent_counter += other.urgent_counter;
        self.irreversible_religion += other.irreversible_religion;
        self.rollouts += other.rollouts;
    }

    fn record(&mut self, path: ReviewPath) {
        match path {
            ReviewPath::DuelReligion => self.duel_religion += 1,
            ReviewPath::UrgentCounter => self.urgent_counter += 1,
            ReviewPath::IrreversibleReligion => self.irreversible_religion += 1,
            ReviewPath::Rollouts => self.rollouts += 1,
        }
    }

    /// Share of reviews that reached the evaluator, or `None` when the agent
    /// never reviewed at all (a game too short to reach `FIRST_REVIEW_TURN`).
    pub fn search_exposure(&self) -> Option<f64> {
        match self.total() {
            0 => None,
            total => Some(self.rollouts as f64 / total as f64),
        }
    }
}

pub struct StrategicAi {
    inner: AdvancedAi,
    weights: Weights,
    net: Option<ValueNet>,
    census: ReviewCensus,
    pub review_every: u32,
    /// Rounds each branch is projected before it is judged.
    ///
    /// Longer is not monotonically better, and the ceiling is closer than
    /// it looks. A branch that reaches a decided game returns exactly 1.0
    /// or 0.0, so once every branch resolves inside the horizon they agree
    /// by construction rather than by judgement and the search is blind.
    /// Measured on four-player 200-turn games, the share of reviews where
    /// every branch is already decided runs 22% at horizon 40, **56% at
    /// 80**, and 89% at 120. `strategic_deep` uses 80 — it wins, but it is
    /// already over half saturated, and pushing further mostly buys
    /// agreement rather than discrimination. The threshold scales with how
    /// much game is left, so it bites sooner in short games and later in
    /// long ones.
    pub horizon: u32,
    next_review: u32,
    /// Search the doctrine axis as well as the lane. Off by default so
    /// `strategic` is bit-identical to its published behaviour; the
    /// `strategic_doctrine` entrant turns it on, which makes the two an
    /// exact paired A/B of the second axis alone.
    /// Project every branch together and stop as soon as they separate,
    /// instead of running each for a fixed count of rounds.
    ///
    /// A fixed horizon averages two regimes that want opposite things. A
    /// branch that reaches a decided game returns exactly 1.0 or 0.0, so
    /// once every branch resolves inside the horizon they agree by
    /// construction and the extra rounds bought agreement rather than
    /// discrimination — 56% of reviews are in that state at horizon 80 and
    /// 89% at 120. The reviews that do *not* saturate carry more the
    /// longer they run, with the spread between branches growing from
    /// 0.015 to 0.160 across the same range.
    ///
    /// Stepping the branches in lockstep and stopping at the first chunk
    /// where they differ by more than the commitment margin spends rounds
    /// only while they still buy something: short where the answer is
    /// early, long where it is genuinely close, and never deep enough to
    /// wash the difference out.
    pub adaptive_horizon: bool,
    pub doctrine_search: bool,
    /// Whether a prior-driven interrupt postpones the next periodic review.
    /// True reproduces the shipped behaviour exactly.
    pub defer_periodic_on_interrupt: bool,
    /// Project a rotating subset of lanes instead of all of them: the
    /// adaptive baseline, the lane in force, and one challenger that
    /// advances each review. Cuts a review from seven branches to about
    /// three, which buys frequency at negative compute cost.
    pub rotate_lanes: bool,
    rotation: usize,
    doctrine: Doctrine,
    doctrine_switches: u32,
}

impl Default for StrategicAi {
    fn default() -> Self {
        Self::new()
    }
}

impl StrategicAi {
    pub fn new() -> StrategicAi {
        Self::with_weights(Weights::default())
    }

    pub fn with_weights(weights: Weights) -> StrategicAi {
        let net = ValueNet::load_width("evolved", crate::evolve::FEATURE_WIDTH);
        Self::configured(weights, net)
    }

    /// Exact score-share control for paired value-evaluator experiments. It
    /// keeps the same weights, rollout horizon, and lane policy as Strategic,
    /// while refusing any model found in `evolved/`.
    pub fn score_only_with_weights(weights: Weights) -> StrategicAi {
        Self::configured(weights, None)
    }

    fn configured(weights: Weights, net: Option<ValueNet>) -> StrategicAi {
        StrategicAi {
            inner: AdvancedAi::with_weights(weights.clone()),
            weights,
            net,
            census: ReviewCensus::default(),
            adaptive_horizon: false,
            doctrine_search: false,
            defer_periodic_on_interrupt: true,
            rotate_lanes: false,
            rotation: 0,
            doctrine: Doctrine::Incumbent,
            doctrine_switches: 0,
            review_every: 40,
            horizon: 40,
            // The opening book plays itself; the first lane choice lands
            // once the empire exists enough for lanes to differ.
            next_review: FIRST_REVIEW_TURN,
        }
    }

    pub fn current_target(&self) -> Option<VictoryTarget> {
        self.inner.victory_target()
    }

    fn position_value(&self, g: &Game, pid: usize) -> f64 {
        if !g.players[pid].alive {
            return 0.0;
        }
        // Score share among living majors; 1/majors is parity.
        let mut own = 0.0;
        let mut total = 0.0;
        for p in &g.players {
            if p.is_minor || p.is_barbarian || !p.alive {
                continue;
            }
            let s = g.score(p.id).max(0) as f64;
            total += s;
            if p.id == pid {
                own = s;
            }
        }
        let score_share = if total <= 0.0 {
            0.5
        } else {
            own / total
        };
        if let Some(net) = &self.net {
            let learned = net.eval(&features(g, pid));
            if learned.is_finite() {
                score_share + VALUE_NET_WEIGHT * (learned - score_share)
            } else {
                score_share
            }
        } else {
            score_share
        }
    }

    fn target_enabled(g: &Game, target: VictoryTarget) -> bool {
        match target {
            VictoryTarget::Science => g.victory_conditions.science,
            VictoryTarget::Culture => g.victory_conditions.culture,
            VictoryTarget::Religion => g.victory_conditions.religious,
            VictoryTarget::Diplomacy => g.victory_conditions.diplomatic,
            VictoryTarget::Domination => g.victory_conditions.domination,
            VictoryTarget::Score => g.victory_conditions.score,
        }
    }

    /// Prophet slots are an irreversible global race. Once the opening book
    /// has invested in Astrology, a Holy Site, or Prophet points, a 30-turn
    /// score projection must not throw that option away while a slot remains.
    fn viable_religious_commitment(g: &Game, pid: usize) -> bool {
        let player = &g.players[pid];
        if !g.victory_conditions.religious || player.religion.is_some() {
            return false;
        }
        if player.prophet_pending {
            return true;
        }
        let claimed = g.religions_founded()
            + g.players
                .iter()
                .filter(|candidate| candidate.prophet_pending)
                .count();
        if claimed >= g.max_religions() {
            return false;
        }
        let cities = g.player_city_ids(pid);
        let holy_site = cities.iter().any(|city| {
            g.cities[city].districts.contains_key("holy_site")
                || matches!(
                    g.cities[city].queue.first(),
                    Some(Item::District { district, .. }) if district == "holy_site"
                )
        });
        holy_site
            || player.techs.contains("astrology")
            || player.research.as_deref() == Some("astrology")
            || player.gpp.get("prophet").copied().unwrap_or(0.0) > 0.0
    }

    /// A rival founding first does not close the religious race when this
    /// empire can still claim another prophet slot and place a Holy Site.
    fn religious_option_open(g: &Game, pid: usize) -> bool {
        let player = &g.players[pid];
        if !g.victory_conditions.religious || player.religion.is_some() {
            return false;
        }
        let claimed = g.religions_founded()
            + g.players
                .iter()
                .filter(|candidate| candidate.prophet_pending)
                .count();
        let cities = g.player_city_ids(pid);
        claimed < g.max_religions()
            && cities.len() >= 2
            && cities.iter().any(|city| {
                g.cities[city].districts.contains_key("holy_site")
                    || matches!(
                        g.cities[city].queue.first(),
                        Some(Item::District { district, .. }) if district == "holy_site"
                    )
                    || !g.district_sites(*city, "holy_site").is_empty()
            })
    }

    /// In a duel, Religious Victory needs only one foreign conversion. The
    /// prophet race is therefore a must-contest game-ending objective, not an
    /// optional yield specialization. Multiplayer keeps the normal search.
    fn duel_religious_race(g: &Game, pid: usize) -> bool {
        if !g.victory_conditions.religious {
            return false;
        }
        let living = g
            .players
            .iter()
            .filter(|player| player.alive && !player.is_minor && !player.is_barbarian)
            .count();
        if living != 2 {
            return false;
        }
        if g.players[pid].religion.is_some() || g.players[pid].prophet_pending {
            return true;
        }
        let claimed = g.religions_founded()
            + g.players
                .iter()
                .filter(|candidate| candidate.prophet_pending)
                .count();
        claimed < g.max_religions()
    }

    /// Public victory-screen progress distilled to the same 0..100 scale for
    /// every lane. This deliberately scores only concrete endgame progress;
    /// the rollouts remain responsible for comparing ordinary development.
    fn victory_progress(g: &Game, pid: usize, target: VictoryTarget) -> i32 {
        let player = &g.players[pid];
        let starting_majors: Vec<usize> = g
            .players
            .iter()
            .filter(|candidate| !candidate.is_minor && !candidate.is_barbarian)
            .map(|candidate| candidate.id)
            .collect();
        let living_majors: Vec<usize> = starting_majors
            .iter()
            .copied()
            .filter(|candidate| g.players[*candidate].alive)
            .collect();
        match target {
            VictoryTarget::Science => {
                if player.science_projects.contains("exoplanet_expedition") {
                    // Match AdvancedAi's urgency model: the final launch is
                    // an irreversible victory-clock commitment, so macro
                    // search must interrupt immediately instead of spending
                    // another review window on ordinary economic rollouts.
                    78 + (22.0 * player.exoplanet_distance / 50.0).clamp(0.0, 22.0) as i32
                } else if player.science_projects.contains("launch_mars_colony") {
                    65
                } else if player.science_projects.contains("launch_moon_landing") {
                    45
                } else if player.science_projects.contains("launch_earth_satellite") {
                    25
                } else {
                    0
                }
            }
            VictoryTarget::Culture => {
                let target = living_majors
                    .iter()
                    .filter(|other| **other != pid)
                    .map(|other| g.domestic_tourists(*other))
                    .max()
                    .unwrap_or(1)
                    .max(1);
                (100 * g.foreign_tourists(pid) / target).clamp(0, 100) as i32
            }
            VictoryTarget::Religion => player.religion.as_ref().map_or(0, |religion| {
                let converted = living_majors
                    .iter()
                    .filter(|other| {
                        let cities = g.player_city_ids(**other);
                        let following = cities
                            .iter()
                            .filter(|city| {
                                g.city_religion(&g.cities[city]) == Some(religion.as_str())
                            })
                            .count();
                        !cities.is_empty() && following * 2 > cities.len()
                    })
                    .count();
                (100 * converted / living_majors.len().max(1)) as i32
            }),
            VictoryTarget::Diplomacy => (player.dvp * 5).clamp(0, 100) as i32,
            VictoryTarget::Domination => {
                let foreign_capitals = starting_majors
                    .iter()
                    .filter(|owner| **owner != pid)
                    .count();
                let controlled = g
                    .cities
                    .values()
                    .filter(|city| {
                        city.is_capital && city.original_owner != pid && city.owner == pid
                    })
                    .count();
                (100 * controlled)
                    .checked_div(foreign_capitals)
                    .unwrap_or(0) as i32
            }
            VictoryTarget::Score => {
                let leading = living_majors
                    .iter()
                    .map(|candidate| g.score(*candidate))
                    .max();
                if g.max_turns > 0
                    && g.turn.saturating_mul(4) >= g.max_turns.saturating_mul(3)
                    && leading == Some(g.score(pid))
                {
                    (40 + 60 * g.turn.min(g.max_turns) / g.max_turns) as i32
                } else {
                    0
                }
            }
        }
    }

    /// A short economic rollout must not choose a prosperous losing line.
    /// Interrupt it when public race state says a rival can end the game
    /// before the next review. Religious progress advances in whole-civ jumps,
    /// so it warns with two holdouts left and becomes unconditional at match
    /// point; continuous races use the same 78% / 15-point urgency margin as
    /// AdvancedAi's adaptive planner.
    fn urgent_counter(&self, g: &Game, pid: usize) -> Option<(usize, VictoryTarget)> {
        let own_progress = VictoryTarget::ALL
            .into_iter()
            .filter(|target| Self::target_enabled(g, *target))
            .map(|target| Self::victory_progress(g, pid, target))
            .max()
            .unwrap_or(0);
        let mut threat: Option<(i32, usize, VictoryTarget)> = None;
        for rival in g.players.iter().filter(|player| {
            player.id != pid && player.alive && !player.is_minor && !player.is_barbarian
        }) {
            for target in VictoryTarget::ALL {
                if !Self::target_enabled(g, target) {
                    continue;
                }
                let progress = Self::victory_progress(g, rival.id, target);
                let candidate = (progress, usize::MAX - rival.id, target);
                if threat.as_ref().is_none_or(|best| {
                    candidate.0 > best.0 || (candidate.0 == best.0 && candidate.1 > best.1)
                }) {
                    threat = Some(candidate);
                }
            }
        }
        let (progress, inverted_rival, target) = threat?;
        let rival = usize::MAX - inverted_rival;
        if target == VictoryTarget::Religion {
            let living = g
                .players
                .iter()
                .filter(|player| player.alive && !player.is_minor && !player.is_barbarian)
                .count()
                .max(1) as i32;
            let match_point = 100 * living.saturating_sub(1) / living;
            let early_warning = (100 * living.saturating_sub(2) / living)
                .max(50)
                .min(match_point);
            if progress < early_warning || (progress < match_point && progress < own_progress + 15)
            {
                return None;
            }
            let counter = if g.players[pid].religion.is_some() {
                VictoryTarget::Religion
            } else if Self::viable_religious_commitment(g, pid)
                || Self::religious_option_open(g, pid)
            {
                VictoryTarget::Religion
            } else {
                VictoryTarget::Domination
            };
            return Some((rival, counter));
        }
        if progress < 78 || progress < own_progress + 15 {
            return None;
        }
        let counter = match target {
            VictoryTarget::Culture => VictoryTarget::Culture,
            VictoryTarget::Diplomacy => VictoryTarget::Diplomacy,
            VictoryTarget::Science | VictoryTarget::Domination | VictoryTarget::Score => {
                VictoryTarget::Domination
            }
            VictoryTarget::Religion => unreachable!(),
        };
        Some((rival, counter))
    }

    fn urgent_counter_target(&self, g: &Game, pid: usize) -> Option<VictoryTarget> {
        self.urgent_counter(g, pid).map(|(_, target)| target)
    }

    /// Advance the exact branch policy used by the macro planner and retain
    /// its stateful agents so an offline labeler can continue from the same
    /// endpoint rather than restarting their plans.
    fn rollout_state(
        &self,
        g: &Game,
        pid: usize,
        target: Option<VictoryTarget>,
    ) -> (Game, Vec<Box<dyn Ai>>) {
        self.rollout_state_weighted(g, pid, target, &self.weights)
    }

    /// `rollout_state` with an explicit genome for the searching seat, so a
    /// doctrine can be projected the same way a lane is. Rivals keep the
    /// stock agent either way: the counterfactual asks what *this* empire
    /// should do, not what the table would look like if everyone changed.
    fn rollout_state_weighted(
        &self,
        g: &Game,
        pid: usize,
        target: Option<VictoryTarget>,
        weights: &Weights,
    ) -> (Game, Vec<Box<dyn Ai>>) {
        let mut sim = g.clone();
        let mut ais: Vec<Box<dyn Ai>> = sim
            .players
            .iter()
            .map(|p| {
                if p.id == pid {
                    if let Some(target) = target {
                        Box::new(AdvancedAi::with_weights_and_target(weights.clone(), target))
                            as Box<dyn Ai>
                    } else {
                        Box::new(AdvancedAi::with_weights(weights.clone())) as Box<dyn Ai>
                    }
                } else {
                    // The counterfactual must preserve the opponent class the
                    // strategic layer is trying to beat. BasicAi understates
                    // victory pressure (especially religion), so a locally
                    // attractive Science rollout can be globally losing.
                    Box::new(AdvancedAi::new()) as Box<dyn Ai>
                }
            })
            .collect();
        let stop = sim.turn + self.horizon;
        while sim.winner.is_none() && sim.turn < stop {
            let p = sim.current;
            ais[p].take_turn(&mut sim, p);
            if sim.winner.is_none() && sim.current == p {
                let _ = sim.apply(p, &Action::EndTurn);
            }
        }
        (sim, ais)
    }

    /// Projected value of staying adaptive (`None`) or committing to `target`
    /// for `horizon` rounds.
    fn rollout(&self, g: &Game, pid: usize, target: Option<VictoryTarget>) -> f64 {
        self.rollout_weighted(g, pid, target, &self.weights)
    }

    fn rollout_weighted(
        &self,
        g: &Game,
        pid: usize,
        target: Option<VictoryTarget>,
        weights: &Weights,
    ) -> f64 {
        let (sim, _) = self.rollout_state_weighted(g, pid, target, weights);
        match sim.winner {
            Some(w) if w == pid => 1.0,
            Some(_) => 0.0,
            None => self.position_value(&sim, pid),
        }
    }

    /// Second axis of the same search: with the lane fixed, project each
    /// doctrine and keep the best.
    ///
    /// Coordinate descent rather than a product: four more rollouts a
    /// review instead of seven times four. The lane is chosen first because
    /// it is the coarser decision — what an empire is playing for changes
    /// which doctrine suits it far more than the reverse.
    ///
    /// Every doctrine is derived from the *evolved* genome, never from the
    /// one currently in play, so repeated reviews cannot ratchet a gene to
    /// its bound. Switching requires clearing the incumbent by the same
    /// margin a lane must clear its adaptive parent, so a doctrine is
    /// adopted on evidence and otherwise the tuned genome stands.
    pub fn review_doctrine(&self, g: &Game, pid: usize, target: Option<VictoryTarget>) -> Doctrine {
        let values = self.doctrine_values(g, pid, target);
        let current = values
            .iter()
            .find_map(|(doctrine, value)| (*doctrine == self.doctrine).then_some(*value))
            .expect("the doctrine in force is always projected");
        let best = values
            .iter()
            .copied()
            .fold(None, |best: Option<(Doctrine, f64)>, candidate| {
                match best.is_none_or(|best| candidate.1 > best.1) {
                    true => Some(candidate),
                    false => best,
                }
            })
            .expect("Doctrine::ALL is not empty");
        match best.1 > current + DOCTRINE_COMMITMENT_MARGIN {
            true => best.0,
            false => self.doctrine,
        }
    }

    /// Projected value of every doctrine under a fixed lane, in
    /// `Doctrine::ALL` order. Exposed because the spread between these
    /// numbers is what decides whether the axis can act at all: if it is
    /// smaller than the commitment margin, the search runs and can never
    /// choose anything, which no win rate would reveal.
    pub fn doctrine_values(
        &self,
        g: &Game,
        pid: usize,
        target: Option<VictoryTarget>,
    ) -> Vec<(Doctrine, f64)> {
        Doctrine::ALL
            .into_iter()
            .map(|doctrine| {
                (
                    doctrine,
                    self.rollout_weighted(g, pid, target, &doctrine.apply(&self.weights)),
                )
            })
            .collect()
    }

    /// The play style currently in force.
    pub fn doctrine(&self) -> Doctrine {
        self.doctrine
    }

    /// How many times the doctrine search changed the genome in play.
    pub fn doctrine_switches(&self) -> u32 {
        self.doctrine_switches
    }

    /// Export unresolved rollout endpoints with true terminal outcomes.
    ///
    /// The ordinary on-policy snapshots used by early value models are not
    /// the positions this agent ranks: it ranks the endpoints of adaptive and
    /// lane-committed counterfactuals. This method reuses the planner's exact
    /// branch policy, excludes branches where the evaluator already has an
    /// exact result (a winner or a dead candidate), and continues each retained
    /// branch with the same stateful agents until the game ends. Cheap tactical
    /// and irreversible priors return before rollouts in `review`, so those
    /// roots deliberately produce no samples here either.
    pub fn counterfactual_value_samples(
        &self,
        g: &Game,
        pid: usize,
    ) -> Vec<CounterfactualValueSample> {
        self.counterfactual_value_samples_with_observation(g, pid, false)
    }

    /// Like [`Self::counterfactual_value_samples`], with an optional
    /// fog-honest board/global observation of every rollout endpoint. Keeping
    /// this opt-in preserves the inexpensive scalar export used for broad
    /// calibration runs while allowing a contextual evaluator to train on the
    /// exact distribution Strategic actually ranks.
    pub fn counterfactual_value_samples_with_observation(
        &self,
        g: &Game,
        pid: usize,
        include_observation: bool,
    ) -> Vec<CounterfactualValueSample> {
        if Self::duel_religious_race(g, pid)
            || self.urgent_counter_target(g, pid).is_some()
            || Self::viable_religious_commitment(g, pid)
        {
            return Vec::new();
        }

        let mut targets = vec![None];
        targets.extend(
            VictoryTarget::ALL
                .into_iter()
                .filter(|target| Self::target_enabled(g, *target))
                .map(Some),
        );
        let mut samples = Vec::with_capacity(targets.len());
        for target in targets {
            let (mut endpoint, mut ais) = self.rollout_state(g, pid, target);
            if endpoint.winner.is_some() || !endpoint.players[pid].alive {
                continue;
            }
            let endpoint_features = features(&endpoint, pid);
            let turn_fraction = endpoint.turn as f32 / endpoint.max_turns.max(1) as f32;
            let observation = include_observation.then(|| obs_tensor(&endpoint, pid));
            run_game(&mut endpoint, &mut ais);
            let Some(winner) = endpoint.winner else {
                continue;
            };
            samples.push(CounterfactualValueSample {
                features: endpoint_features,
                planes: observation
                    .as_ref()
                    .map_or_else(Vec::new, |tensor| tensor.data.clone()),
                globals: observation
                    .as_ref()
                    .map_or_else(Vec::new, |tensor| tensor.global.clone()),
                global_names: observation
                    .as_ref()
                    .map_or_else(Vec::new, |tensor| tensor.global_names.clone()),
                won: winner == pid,
                turn_fraction,
                target,
            });
        }
        samples
    }

    fn choose_rollout_target(
        &self,
        values: &[(f64, Option<VictoryTarget>)],
    ) -> Option<VictoryTarget> {
        let adaptive = values
            .iter()
            .find_map(|(value, target)| target.is_none().then_some(*value))
            .expect("adaptive rollout is always present");
        let mut best_target = None;
        for candidate in values
            .iter()
            .copied()
            .filter(|(_, target)| target.is_some())
        {
            if best_target.is_none_or(|best: (f64, Option<VictoryTarget>)| candidate.0 > best.0) {
                best_target = Some(candidate);
            }
        }
        best_target
            .filter(|(value, _)| *value > adaptive + TARGET_COMMITMENT_MARGIN)
            .and_then(|(_, target)| target)
    }

    /// Every branch's value, projected in lockstep and stopped as soon as
    /// they separate.
    ///
    /// Returns the same `(value, target)` pairs a fixed-horizon review
    /// produces, so `choose_rollout_target` is unchanged. Determinism is
    /// preserved: the branches are stepped in a fixed order, the stopping
    /// test reads only their values, and no clock or RNG is consulted.
    fn lockstep_values(
        &self,
        g: &Game,
        pid: usize,
        targets: &[Option<VictoryTarget>],
    ) -> Vec<(f64, Option<VictoryTarget>)> {
        self.lockstep_values_projected(g, pid, targets).0
    }

    /// The same search, also reporting how many rounds it actually spent.
    /// A treatment has to be shown to fire before it is worth evaluating,
    /// and the rounds it declines to project are the only direct evidence
    /// that this one does anything at all.
    fn lockstep_values_projected(
        &self,
        g: &Game,
        pid: usize,
        targets: &[Option<VictoryTarget>],
    ) -> (Vec<(f64, Option<VictoryTarget>)>, u32) {
        let mut branches: Vec<(Game, Vec<Box<dyn Ai>>)> = targets
            .iter()
            .map(|target| self.branch_state(g, pid, *target))
            .collect();
        let deadline = g.turn + self.horizon;
        let mut projected = 0;
        loop {
            let chunk = ADAPTIVE_CHUNK.min(self.horizon.saturating_sub(projected));
            if chunk == 0 {
                break;
            }
            for (sim, ais) in branches.iter_mut() {
                let stop = sim.turn + chunk;
                while sim.winner.is_none() && sim.turn < stop && sim.turn < deadline {
                    let current = sim.current;
                    ais[current].take_turn(sim, current);
                    if sim.winner.is_none() && sim.current == current {
                        let _ = sim.apply(current, &Action::EndTurn);
                    }
                }
            }
            projected += chunk;
            let values: Vec<f64> = branches
                .iter()
                .map(|(sim, _)| self.branch_value(sim, pid))
                .collect();
            let low = values.iter().copied().fold(f64::INFINITY, f64::min);
            let high = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            // Separated enough to decide, or every branch has resolved and
            // further rounds cannot change anything.
            if high - low > TARGET_COMMITMENT_MARGIN
                || branches.iter().all(|(sim, _)| sim.winner.is_some())
            {
                break;
            }
        }
        (
            branches
                .iter()
                .zip(targets)
                .map(|((sim, _), target)| (self.branch_value(sim, pid), *target))
                .collect(),
            projected,
        )
    }

    fn branch_state(
        &self,
        g: &Game,
        pid: usize,
        target: Option<VictoryTarget>,
    ) -> (Game, Vec<Box<dyn Ai>>) {
        let sim = g.clone();
        let ais: Vec<Box<dyn Ai>> = sim
            .players
            .iter()
            .map(|p| {
                if p.id == pid {
                    match target {
                        Some(target) => Box::new(AdvancedAi::with_weights_and_target(
                            self.weights.clone(),
                            target,
                        )) as Box<dyn Ai>,
                        None => {
                            Box::new(AdvancedAi::with_weights(self.weights.clone())) as Box<dyn Ai>
                        }
                    }
                } else {
                    Box::new(AdvancedAi::new()) as Box<dyn Ai>
                }
            })
            .collect();
        (sim, ais)
    }

    fn branch_value(&self, sim: &Game, pid: usize) -> f64 {
        match sim.winner {
            Some(winner) if winner == pid => 1.0,
            Some(_) => 0.0,
            None => self.position_value(sim, pid),
        }
    }

    /// Compare the adaptive parent with every enabled victory lane. Deterministic:
    /// rollouts are seed-free clones and ties keep declaration order; an explicit
    /// lane must clear the adaptive value by `TARGET_COMMITMENT_MARGIN`.
    pub fn review(&self, g: &Game, pid: usize) -> Option<VictoryTarget> {
        self.review_detailed(g, pid).0
    }

    /// `review`, plus which branch answered.
    ///
    /// Callers that record evidence need the second half: a lane chosen by a
    /// prior and a lane chosen by forty rounds of projection are the same
    /// value with entirely different provenance, and only the second one is
    /// evidence about the search or the evaluator.
    pub fn review_detailed(&self, g: &Game, pid: usize) -> (Option<VictoryTarget>, ReviewPath) {
        if Self::duel_religious_race(g, pid) {
            return (Some(VictoryTarget::Religion), ReviewPath::DuelReligion);
        }
        if let Some(counter) = self.urgent_counter_target(g, pid) {
            return (Some(counter), ReviewPath::UrgentCounter);
        }
        if Self::viable_religious_commitment(g, pid) {
            return (
                Some(VictoryTarget::Religion),
                ReviewPath::IrreversibleReligion,
            );
        }
        let mut branches: Vec<Option<VictoryTarget>> = vec![None];
        branches.extend(self.lane_candidates(g).into_iter().map(Some));
        let values = if self.adaptive_horizon {
            self.lockstep_values(g, pid, &branches)
        } else {
            branches
                .into_iter()
                .map(|target| (self.rollout(g, pid, target), target))
                .collect()
        };
        (self.choose_rollout_target(&values), ReviewPath::Rollouts)
    }

    /// Lanes this review will project, always alongside the adaptive
    /// baseline.
    ///
    /// Without rotation this is every enabled lane, which is what the agent
    /// has always done. With it, the set is the lane currently in force plus
    /// one challenger that advances every review. Keeping the incumbent in
    /// the set matters: `choose_rollout_target` only returns a lane it
    /// actually projected, so omitting the lane in force would drop it on
    /// the first review that failed to re-nominate it, and the agent would
    /// thrash between whichever two lanes the cursor happened to expose.
    pub fn lane_candidates(&self, g: &Game) -> Vec<VictoryTarget> {
        let enabled: Vec<VictoryTarget> = VictoryTarget::ALL
            .into_iter()
            .filter(|target| Self::target_enabled(g, *target))
            .collect();
        if !self.rotate_lanes || enabled.len() <= 2 {
            return enabled;
        }
        let mut out = Vec::with_capacity(2);
        if let Some(current) = self.inner.victory_target() {
            if enabled.contains(&current) {
                out.push(current);
            }
        }
        let challenger = enabled[self.rotation % enabled.len()];
        if !out.contains(&challenger) {
            out.push(challenger);
        }
        out
    }

    /// Advance the rotation cursor. Test-only: in play the cursor moves
    /// once per projected review inside `take_turn`, and exposing that as a
    /// public step would invite a caller to desynchronise it from the
    /// reviews it is meant to count.
    #[cfg(test)]
    pub(crate) fn advance_rotation_for_test(&mut self) {
        self.rotation = self.rotation.wrapping_add(1);
    }

    /// How this agent's reviews were resolved so far.
    pub fn review_census(&self) -> ReviewCensus {
        self.census
    }
}

impl Ai for StrategicAi {
    fn take_turn(&mut self, g: &mut Game, pid: usize) {
        let major = !g.players[pid].is_minor && !g.players[pid].is_barbarian;
        let counter = major.then(|| self.urgent_counter(g, pid)).flatten();
        let interrupted = counter.as_ref().is_some_and(|(rival, target)| {
            self.inner.victory_target() != Some(*target)
                || self.inner.forced_target_player() != Some(*rival)
        });
        let due = g.turn >= self.next_review;
        if major && g.winner.is_none() && (due || interrupted) {
            // An interrupt is a cheap prior read off the victory screen, not
            // the periodic six-lane search. Letting it push `next_review`
            // forward lets a free decision consume the budget of an
            // expensive one: a counter at turn 35 postpones to turn 75 the
            // projection that was due at 35. `defer_periodic_on_interrupt`
            // keeps the shipped behaviour; turning it off lets the periodic
            // search hold its own schedule.
            if due || self.defer_periodic_on_interrupt {
                self.next_review = g.turn + self.review_every;
            }
            // Public victory threats are cheap to inspect every turn and may
            // end the game before the next expensive six-lane review. Reuse
            // the already-computed counter rather than running rollouts.
            let (target, path) = match counter {
                Some((_, target)) => (Some(target), ReviewPath::UrgentCounter),
                None => self.review_detailed(g, pid),
            };
            self.census.record(path);
            // Advance the cursor on every projected review, not only when
            // the challenger was adopted, so each lane comes up for
            // examination on a fixed cycle rather than a lucky one.
            if path == ReviewPath::Rollouts {
                self.rotation = self.rotation.wrapping_add(1);
            }
            // Only search the second axis on a review that reached the
            // first one. A prior short-circuits because the lane is already
            // forced or the game is nearly decided; spending four more
            // rollouts there buys a play style for a position that has
            // stopped asking the question.
            if self.doctrine_search && path == ReviewPath::Rollouts {
                let doctrine = self.review_doctrine(g, pid, target);
                if doctrine != self.doctrine {
                    self.doctrine = doctrine;
                    self.doctrine_switches += 1;
                    self.inner.reweight(doctrine.apply(&self.weights));
                }
            }
            match (target, counter) {
                (Some(target), Some((rival, _)))
                    if self.inner.victory_target() != Some(target)
                        || self.inner.forced_target_player() != Some(rival) =>
                {
                    self.inner.retarget_against(target, rival);
                }
                (Some(target), None) if self.inner.victory_target() != Some(target) => {
                    self.inner.retarget(target);
                }
                (None, _) if self.inner.victory_target().is_some() => self.inner.adapt(),
                _ => {}
            }
        }
        self.inner.take_turn(g, pid);
    }

    fn strategy_label(&self) -> Option<&'static str> {
        self.inner.strategy_label()
    }

    fn plan_report(&self) -> Option<PlanReport> {
        self.inner.plan_report()
    }

    fn review_census(&self) -> Option<ReviewCensus> {
        Some(self.census)
    }
}

#[cfg(test)]
mod tests {
    use super::{Doctrine, StrategicAi};
    use crate::ai::{run_game, Ai, BasicAi, VictoryTarget, Weights};
    use crate::game::{Action, Game};
    use crate::valuenet::ValueNet;

    fn found_capitals(game: &mut Game, players: usize) {
        for pid in 0..players {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.current = pid;
            game.apply(pid, &Action::FoundCity { unit: settler })
                .unwrap();
        }
        game.current = 0;
    }

    #[test]
    fn default_macro_search_looks_forty_rounds_ahead() {
        assert_eq!(StrategicAi::new().horizon, 40);
    }

    #[test]
    fn score_only_control_refuses_a_runtime_value_model() {
        assert!(StrategicAi::score_only_with_weights(Default::default())
            .net
            .is_none());
    }

    #[test]
    fn counterfactual_samples_cover_the_evaluated_endpoint_distribution() {
        // Three players avoid the duel-religion prior, which intentionally
        // bypasses value rollouts. A one-round horizon keeps the seven exact
        // branch continuations cheap while still exercising terminal labels.
        let game = Game::new(3, 20, 14, 200, 12, 0);
        let mut strategic = StrategicAi::score_only_with_weights(Default::default());
        strategic.horizon = 1;

        let samples = strategic.counterfactual_value_samples_with_observation(&game, 0, true);
        assert_eq!(samples.len(), 1 + VictoryTarget::ALL.len());
        assert_eq!(
            samples,
            strategic.counterfactual_value_samples_with_observation(&game, 0, true),
            "branch endpoints and their terminal labels must be deterministic"
        );
        assert!(samples.iter().all(|sample| {
            sample.planes.len() == 25 * 20 * 14
                && sample.globals.len() == sample.global_names.len()
                && !sample.global_names.is_empty()
        }));
        assert_eq!(samples[0].target, None);
        assert_eq!(
            samples
                .iter()
                .skip(1)
                .filter_map(|sample| sample.target)
                .collect::<Vec<_>>(),
            VictoryTarget::ALL
        );
        assert!(samples.iter().all(|sample| {
            sample.features.len() == 25
                && sample.features.iter().all(|value| value.is_finite())
                && sample.turn_fraction > 0.0
        }));
    }

    #[test]
    fn learned_values_are_regularized_toward_score_share() {
        let game = Game::new(4, 24, 16, 20, 180, 0);
        let total: i64 = game
            .players
            .iter()
            .filter(|player| !player.is_minor && !player.is_barbarian)
            .map(|player| game.score(player.id).max(0))
            .sum();
        let score_share = if total > 0 {
            game.score(0).max(0) as f64 / total as f64
        } else {
            0.5
        };
        let mut strategic = StrategicAi::new();
        strategic.net = None;
        assert!((strategic.position_value(&game, 0) - score_share).abs() < 1e-12);
        strategic.net = Some(ValueNet {
            sizes: vec![25, 64, 32, 1],
            weights: vec![
                vec![vec![0.0; 64]; 25],
                vec![vec![0.0; 32]; 64],
                vec![vec![0.0; 1]; 32],
            ],
            biases: vec![vec![0.0; 64], vec![0.0; 32], vec![0.0; 1]],
        });

        let expected = score_share + 0.25 * (0.5 - score_share);
        assert!((strategic.position_value(&game, 0) - expected).abs() < 1e-12);
    }

    /// Rotation must still examine every lane, just on a cycle. A subset
    /// that permanently excluded a lane would not be a cheaper search, it
    /// would be a smaller game.
    #[test]
    fn rotation_reaches_every_enabled_lane_on_a_cycle() {
        let g = Game::new(4, 28, 18, 41000, 200, 2);
        let enabled: Vec<VictoryTarget> = VictoryTarget::ALL
            .into_iter()
            .filter(|target| {
                let mut probe = StrategicAi::new();
                probe.rotate_lanes = false;
                probe.lane_candidates(&g).contains(target)
            })
            .collect();
        assert!(enabled.len() > 2, "need a real lane set to rotate over");
        let mut seen: Vec<VictoryTarget> = Vec::new();
        let mut rotating = StrategicAi::new();
        rotating.rotate_lanes = true;
        for _ in 0..enabled.len() * 2 {
            let candidates = rotating.lane_candidates(&g);
            assert!(
                candidates.len() <= 2,
                "a rotating review projected {} lanes",
                candidates.len()
            );
            for target in candidates {
                if !seen.contains(&target) {
                    seen.push(target);
                }
            }
            rotating.advance_rotation_for_test();
        }
        for target in &enabled {
            assert!(
                seen.contains(target),
                "{target:?} never came up in two full cycles"
            );
        }
    }

    /// Without rotation the candidate set must be exactly what it always
    /// was, so the default agent is untouched.
    #[test]
    fn rotation_is_off_by_default_and_projects_every_lane() {
        let g = Game::new(4, 28, 18, 41000, 200, 2);
        let plain = StrategicAi::new();
        assert!(!plain.rotate_lanes);
        let all: Vec<VictoryTarget> = VictoryTarget::ALL
            .into_iter()
            .filter(|target| plain.lane_candidates(&g).contains(target))
            .collect();
        assert_eq!(plain.lane_candidates(&g), all);
        assert!(all.len() > 2);
    }

    /// Halving the review period must actually buy reviews. This is the
    /// mechanism behind the cadence result: if the counter never moved, the
    /// dose would be nominal and the measured win difference would have to
    /// come from somewhere else.
    #[test]
    fn a_shorter_review_period_reaches_the_search_more_often() {
        let census = |review_every: u32| {
            let mut g = Game::new(4, 28, 18, 41000, 160, 2);
            let mut strategic = StrategicAi::new();
            strategic.review_every = review_every;
            strategic.horizon = 4;
            let mut others = BasicAi::fleet(&g);
            while g.winner.is_none() && g.turn <= g.max_turns {
                let pid = g.current;
                if pid == 0 {
                    strategic.take_turn(&mut g, pid);
                } else {
                    others[pid].take_turn(&mut g, pid);
                }
                if g.winner.is_none() && g.current == pid {
                    let _ = g.apply(pid, &Action::EndTurn);
                }
            }
            strategic.review_census()
        };
        let sparse = census(40);
        let dense = census(10);
        assert!(
            dense.rollouts > sparse.rollouts,
            "cadence 10 reached the rollouts {} times against 40's {}",
            dense.rollouts,
            sparse.rollouts
        );
    }

    /// A prior-driven interrupt must not postpone the periodic search when
    /// the agent has opted out of deferring. The shipped default still
    /// defers, so this is a behaviour switch and not a silent change.
    #[test]
    fn an_interrupt_need_not_postpone_the_periodic_search() {
        assert!(
            StrategicAi::new().defer_periodic_on_interrupt,
            "the shipped default must be unchanged"
        );
        let census = |defer: bool| {
            let mut g = Game::new(4, 28, 18, 41003, 170, 2);
            let mut strategic = StrategicAi::new();
            strategic.defer_periodic_on_interrupt = defer;
            strategic.horizon = 4;
            let mut others = BasicAi::fleet(&g);
            while g.winner.is_none() && g.turn <= g.max_turns {
                let pid = g.current;
                if pid == 0 {
                    strategic.take_turn(&mut g, pid);
                } else {
                    others[pid].take_turn(&mut g, pid);
                }
                if g.winner.is_none() && g.current == pid {
                    let _ = g.apply(pid, &Action::EndTurn);
                }
            }
            strategic.review_census()
        };
        let deferring = census(true);
        let holding = census(false);
        assert!(
            holding.total() >= deferring.total(),
            "holding the schedule reviewed {} times against {}",
            holding.total(),
            deferring.total()
        );
    }

    /// Every doctrine must stay inside the bounds evolution respects, so a
    /// multiple of an already-extreme evolved gene cannot produce a genome
    /// that is not a play style at all.
    #[test]
    fn doctrines_stay_inside_the_evolution_bounds() {
        let extreme = Weights::from_vec(
            &Weights::bounds()
                .iter()
                .map(|(_, high)| *high)
                .collect::<Vec<f64>>(),
        );
        for base in [Weights::default(), extreme] {
            for doctrine in Doctrine::ALL {
                let genes = doctrine.apply(&base).to_vec();
                for (index, (gene, (low, high))) in
                    genes.iter().zip(Weights::bounds()).enumerate()
                {
                    assert!(
                        *gene >= low && *gene <= high,
                        "{} gene {index} = {gene} escaped [{low}, {high}]",
                        doctrine.name()
                    );
                }
            }
        }
    }

    /// The incumbent must be the evolved genome untouched, so a tie leaves
    /// the tuned agent exactly as it was.
    #[test]
    fn the_incumbent_doctrine_changes_nothing() {
        let base = Weights::default();
        assert_eq!(Doctrine::Incumbent.apply(&base).to_vec(), base.to_vec());
        for doctrine in [Doctrine::Expand, Doctrine::Consolidate, Doctrine::Militarize] {
            assert_ne!(
                doctrine.apply(&base).to_vec(),
                base.to_vec(),
                "{} is indistinguishable from the incumbent",
                doctrine.name()
            );
        }
    }

    /// The default agent must be untouched by this feature: `strategic` is
    /// a published entrant and the A/B is only meaningful if the control
    /// did not move.
    #[test]
    fn doctrine_search_is_off_by_default() {
        let plain = StrategicAi::new();
        assert!(!plain.doctrine_search);
        assert_eq!(plain.doctrine(), Doctrine::Incumbent);
        assert_eq!(plain.doctrine_switches(), 0);
    }

    /// With the axis on, a real game must reach the doctrine search and the
    /// agent must still finish a legal game. Repeating the same seed must
    /// give the same doctrine: the rollouts are seed-free clones, so the
    /// second axis has to be as deterministic as the first.
    #[test]
    fn doctrine_search_runs_and_is_deterministic() {
        let play = || {
            let mut g = Game::new(4, 28, 18, 31, 130, 2);
            let mut strategic = StrategicAi::new();
            strategic.doctrine_search = true;
            strategic.horizon = 4;
            let mut others = BasicAi::fleet(&g);
            while g.winner.is_none() && g.turn <= g.max_turns {
                let pid = g.current;
                if pid == 0 {
                    strategic.take_turn(&mut g, pid);
                } else {
                    others[pid].take_turn(&mut g, pid);
                }
                if g.winner.is_none() && g.current == pid {
                    let _ = g.apply(pid, &Action::EndTurn);
                }
            }
            (
                strategic.review_census(),
                strategic.doctrine(),
                strategic.doctrine_switches(),
            )
        };
        let first = play();
        assert!(
            first.0.rollouts > 0,
            "the lane search never ran, so the doctrine axis was never reached: {:?}",
            first.0
        );
        assert_eq!(first, play(), "the doctrine axis must be deterministic");
    }

    /// A two-player game with religion enabled is answered by the duel
    /// prior, so the economic rollouts — and the value evaluator with them —
    /// never run. Every published duel number for this agent measures a
    /// forced religious lane rather than macro search, which is exactly what
    /// the census exists to say out loud.
    #[test]
    fn a_duel_never_reaches_the_rollouts() {
        let mut g = Game::new(2, 20, 14, 21, 200, 0);
        assert!(g.victory_conditions.religious);
        let mut strategic = StrategicAi::new();
        strategic.horizon = 4;
        let mut ais = BasicAi::fleet(&g);
        for _ in 0..70 {
            if g.winner.is_some() {
                break;
            }
            for pid in 0..g.players.len() {
                if g.winner.is_some() {
                    break;
                }
                if pid == 0 {
                    strategic.take_turn(&mut g, pid);
                } else {
                    ais[pid].take_turn(&mut g, pid);
                }
            }
        }
        let census = strategic.review_census();
        assert!(census.total() > 0, "the agent must have reviewed at all");
        assert_eq!(
            census.rollouts, 0,
            "a duel reached the rollouts {} times: {census:?}",
            census.rollouts
        );
        assert_eq!(census.search_exposure(), Some(0.0));
        assert_eq!(strategic.current_target(), Some(VictoryTarget::Religion));
    }

    /// With religion switched off the duel prior cannot fire, so the same
    /// two-player position does reach the search. This is the control that
    /// keeps the test above a statement about the prior rather than about
    /// two-player games.
    #[test]
    fn a_duel_without_a_religious_race_does_reach_the_rollouts() {
        let mut g = Game::new(2, 20, 14, 21, 200, 0);
        g.victory_conditions.religious = false;
        let mut strategic = StrategicAi::new();
        strategic.horizon = 3;
        let mut ais = BasicAi::fleet(&g);
        for _ in 0..40 {
            if g.winner.is_some() {
                break;
            }
            for pid in 0..g.players.len() {
                if g.winner.is_some() {
                    break;
                }
                if pid == 0 {
                    strategic.take_turn(&mut g, pid);
                } else {
                    ais[pid].take_turn(&mut g, pid);
                }
            }
        }
        let census = strategic.review_census();
        assert!(
            census.rollouts > 0,
            "the search never ran without a religious race: {census:?}"
        );
        assert_eq!(census.duel_religion, 0);
    }

    /// An agent with no search reports none, so an evaluator can tell
    /// "searched zero times" from "has nothing to search".
    #[test]
    fn a_scripted_agent_reports_no_census() {
        assert_eq!(BasicAi::new().review_census(), None);
        assert_eq!(
            crate::ai::AdvancedAi::new().review_census(),
            None,
            "the scripted parent has no macro search of its own"
        );
        // Reached through the trait, where an evaluator sees it: the
        // inherent method returns the census itself.
        assert!(Ai::review_census(&StrategicAi::new()).is_some());
    }

    /// The review must pick a lane deterministically and commit it to the
    /// wrapped agent; a tiny horizon keeps the six rollouts fast.
    #[test]
    fn review_selects_and_commits_a_victory_lane() {
        let mut g = Game::new(2, 20, 14, 21, 120, 0);
        let mut ais = BasicAi::fleet(&g);
        for _ in 0..12 {
            for pid in 0..g.players.len() {
                if g.winner.is_some() {
                    break;
                }
                ais[pid].take_turn(&mut g, pid);
            }
        }
        let mut strategic = StrategicAi::new();
        strategic.horizon = 4;
        strategic.next_review = 0;
        let first = strategic.review(&g, 0);
        assert_eq!(first, strategic.review(&g, 0), "review must be deterministic");
        assert_eq!(strategic.current_target(), None);
        strategic.take_turn(&mut g, 0);
        assert_eq!(strategic.current_target(), first);
    }

    #[test]
    fn religious_match_point_interrupts_the_economic_rollouts() {
        let mut game = Game::new_full(2, 24, 16, 22, 180, 0, false);
        found_capitals(&mut game, 2);
        game.players[0].religion = Some("Home Faith".to_string());
        game.players[1].religion = Some("Rival Faith".to_string());
        for (owner, religion) in [(0, "Home Faith"), (1, "Rival Faith")] {
            let capital = game.player_city_ids(owner)[0];
            game.cities
                .get_mut(&capital)
                .unwrap()
                .pressure
                .insert(religion.to_string(), 1_000.0);
        }

        let strategic = StrategicAi::new();
        assert_eq!(
            strategic.urgent_counter_target(&game, 0),
            Some(VictoryTarget::Religion)
        );
        assert_eq!(strategic.review(&game, 0), Some(VictoryTarget::Religion));

        game.players[0].religion = None;
        assert_eq!(
            strategic.urgent_counter_target(&game, 0),
            Some(VictoryTarget::Domination)
        );
    }

    #[test]
    fn strategic_review_preserves_a_viable_prophet_investment() {
        let mut game = Game::new_full(3, 24, 16, 24, 180, 0, false);
        found_capitals(&mut game, 3);
        game.players[0].research = Some("astrology".to_string());

        assert!(StrategicAi::viable_religious_commitment(&game, 0));
        assert_eq!(
            StrategicAi::new().review(&game, 0),
            Some(VictoryTarget::Religion)
        );

        game.victory_conditions.religious = false;
        assert!(!StrategicAi::viable_religious_commitment(&game, 0));
    }

    #[test]
    fn duel_treats_the_prophet_race_as_a_mandatory_objective() {
        let mut game = Game::new_full(2, 24, 16, 26, 180, 0, false);
        found_capitals(&mut game, 2);
        let strategic = StrategicAi::new();

        assert!(StrategicAi::duel_religious_race(&game, 0));
        assert_eq!(strategic.review(&game, 0), Some(VictoryTarget::Religion));

        game.victory_conditions.religious = false;
        assert!(!StrategicAi::duel_religious_race(&game, 0));
    }

    /// A treatment has to be shown to fire before it is worth evaluating.
    /// #380 spent a 240-game run to discover its treatment was inert, with
    /// every diagnostic identical to the digit. This asserts the adaptive
    /// horizon actually declines rounds in ordinary positions, so that a
    /// pre-registered evaluation is measuring something that happens.
    #[test]
    fn the_adaptive_horizon_stops_early_in_ordinary_games() {
        let mut checked = 0;
        let mut fired = 0;
        for seed in [31u64, 57, 83] {
            // Three players: a duel is answered by the religion prior and
            // never reaches the rollouts this measures.
            let mut g = Game::new(3, 20, 14, seed, 200, 0);
            let mut strategic = StrategicAi::with_weights(Default::default());
            strategic.adaptive_horizon = true;
            // The promoted deep budget, which is the one the saturation
            // measurement was taken at and the only one this can help: at
            // horizon 40 the median spread between branches is 0.0045,
            // under the 0.01 commitment margin, so the branches never
            // separate and the search runs its full count regardless.
            strategic.horizon = 80;
            let mut ais = BasicAi::fleet(&g);
            for _ in 0..40 {
                if g.winner.is_some() {
                    break;
                }
                for pid in 0..g.players.len() {
                    if g.winner.is_some() {
                        break;
                    }
                    ais[pid].take_turn(&mut g, pid);
                }
            }
            if g.winner.is_some() {
                continue;
            }
            let mut targets: Vec<Option<VictoryTarget>> = vec![None];
            targets.extend(strategic.lane_candidates(&g).into_iter().map(Some));
            let (values, projected) = strategic.lockstep_values_projected(&g, 0, &targets);
            assert_eq!(values.len(), targets.len());
            assert!(values.iter().all(|(value, _)| value.is_finite()));
            checked += 1;
            if projected < strategic.horizon {
                fired += 1;
            }
        }
        assert!(checked > 0, "no ordinary mid-game position was reached");
        assert!(
            fired > 0,
            "the adaptive horizon projected every round in all {checked} positions, \
             so it is inert and an evaluation would measure nothing"
        );
    }

    #[test]
    fn imminent_victory_interrupts_before_the_periodic_review() {
        let mut game = Game::new_full(2, 24, 16, 25, 180, 0, false);
        found_capitals(&mut game, 2);
        game.players[0].religion = Some("Home Faith".to_string());
        game.players[1].religion = Some("Rival Faith".to_string());
        for (owner, religion) in [(0, "Home Faith"), (1, "Rival Faith")] {
            let capital = game.player_city_ids(owner)[0];
            game.cities
                .get_mut(&capital)
                .unwrap()
                .pressure
                .insert(religion.to_string(), 1_000.0);
        }

        let mut strategic = StrategicAi::new();
        strategic.inner.retarget(VictoryTarget::Science);
        strategic.next_review = game.turn + 100;
        strategic.take_turn(&mut game, 0);

        assert_eq!(strategic.current_target(), Some(VictoryTarget::Religion));
        assert!(strategic.next_review < game.turn + 100);
    }

    #[test]
    fn imminent_space_race_routes_to_denial() {
        let mut game = Game::new_full(3, 24, 16, 23, 300, 0, false);
        found_capitals(&mut game, 3);
        game.players[2].science_projects.extend([
            "launch_earth_satellite".to_string(),
            "launch_moon_landing".to_string(),
            "launch_mars_colony".to_string(),
            "exoplanet_expedition".to_string(),
        ]);
        game.players[2].exoplanet_distance = 42.0;

        let mut strategic = StrategicAi::new();
        assert_eq!(
            strategic.urgent_counter(&game, 0),
            Some((2, VictoryTarget::Domination))
        );
        strategic.next_review = game.turn + 100;
        strategic.take_turn(&mut game, 0);
        assert_eq!(strategic.current_target(), Some(VictoryTarget::Domination));
        assert_eq!(
            strategic.inner.current_plan().and_then(|plan| plan.target_player),
            Some(2),
            "an urgent counter-campaign must pursue the civilization about to win"
        );
    }

    #[test]
    fn urgent_denial_retargets_when_a_different_rival_takes_the_lead() {
        let mut game = Game::new_full(3, 24, 16, 24, 300, 0, false);
        found_capitals(&mut game, 3);
        for rival in [1, 2] {
            game.players[rival]
                .science_projects
                .insert("exoplanet_expedition".to_string());
        }
        game.players[1].exoplanet_distance = 40.0;
        game.players[2].exoplanet_distance = 45.0;

        let mut strategic = StrategicAi::new();
        strategic.next_review = game.turn + 100;
        strategic.take_turn(&mut game, 0);
        assert_eq!(strategic.inner.forced_target_player(), Some(2));

        // The counter-lane remains Domination, but player 1 is now closer to
        // the science victory. Refreshing only on lane changes would leave
        // the campaign pinned to player 2.
        game.players[1].exoplanet_distance = 50.0;
        assert_eq!(
            strategic.urgent_counter(&game, 0),
            Some((1, VictoryTarget::Domination))
        );
        game.current = 0;
        strategic.take_turn(&mut game, 0);

        assert_eq!(strategic.current_target(), Some(VictoryTarget::Domination));
        assert_eq!(strategic.inner.forced_target_player(), Some(1));
        assert_eq!(
            strategic.inner.current_plan().and_then(|plan| plan.target_player),
            Some(1)
        );
    }

    #[test]
    fn final_space_launch_interrupts_before_the_first_light_year() {
        let mut game = Game::new_full(3, 24, 16, 23_001, 300, 0, false);
        found_capitals(&mut game, 3);
        game.players[2]
            .science_projects
            .insert("exoplanet_expedition".to_string());

        assert_eq!(
            StrategicAi::victory_progress(&game, 2, VictoryTarget::Science),
            78
        );
        assert_eq!(
            StrategicAi::new().urgent_counter_target(&game, 0),
            Some(VictoryTarget::Domination),
            "the macro planner must interrupt before its next expensive review"
        );
    }

    #[test]
    fn explicit_lane_must_clear_the_adaptive_rollout() {
        let strategic = StrategicAi::new();
        assert_eq!(
            strategic
                .choose_rollout_target(&[(0.30, None), (0.305, Some(VictoryTarget::Domination)),]),
            None
        );
        assert_eq!(
            strategic.choose_rollout_target(&[(0.30, None), (0.32, Some(VictoryTarget::Science)),]),
            Some(VictoryTarget::Science)
        );
    }

    #[test]
    fn periodic_review_can_return_a_targeted_agent_to_adaptive_planning() {
        let mut game = Game::new_full(3, 24, 16, 31, 300, 0, false);
        found_capitals(&mut game, 3);
        game.victory_conditions.culture = false;
        game.victory_conditions.religious = false;
        game.victory_conditions.diplomatic = false;
        game.victory_conditions.domination = false;
        game.victory_conditions.score = false;
        let mut strategic = StrategicAi::new();
        strategic.inner.retarget(VictoryTarget::Science);
        strategic.horizon = 0;
        strategic.next_review = 0;
        assert_eq!(strategic.current_target(), Some(VictoryTarget::Science));
        strategic.take_turn(&mut game, 0);
        assert_eq!(strategic.current_target(), None);
    }

    #[test]
    fn review_never_selects_a_disabled_lane() {
        let mut game = Game::new_full(3, 24, 16, 30, 300, 0, false);
        found_capitals(&mut game, 3);
        game.victory_conditions.culture = false;
        game.victory_conditions.religious = false;
        game.victory_conditions.diplomatic = false;
        game.victory_conditions.domination = false;
        game.victory_conditions.score = false;
        let mut strategic = StrategicAi::new();
        strategic.horizon = 0;

        assert_eq!(strategic.review(&game, 0), None);
    }

    /// Full smoke game: a strategic seat finishes a real game without
    /// upsetting the loop.
    #[test]
    fn strategic_seat_completes_a_game() {
        let mut g = Game::new(2, 20, 14, 3, 80, 0);
        let mut strategic = StrategicAi::new();
        strategic.horizon = 6;
        strategic.review_every = 25;
        // One AI per player seat, barbarians included.
        let mut ais: Vec<Box<dyn Ai>> = g
            .players
            .iter()
            .map(|p| {
                if p.id == 0 {
                    Box::new(std::mem::take(&mut strategic)) as Box<dyn Ai>
                } else {
                    Box::new(BasicAi::new()) as Box<dyn Ai>
                }
            })
            .collect();
        run_game(&mut g, &mut ais);
        assert!(g.winner.is_some());
    }
}
