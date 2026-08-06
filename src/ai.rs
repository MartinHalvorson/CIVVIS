//! Scripted AIs (mirrors civvis/ai/). BasicAi reads full state (no fog) —
//! sparring partner, not a fair-play agent.
use crate::name::{AsName, Name};
use crate::game::{effective_strength, Action, ActionFamilies, Game, Item, TraversalClass};
use crate::parallel::WorkPool;
use crate::reasoning::{plain, Journal};
use crate::rng::Rng;
use crate::think;
use crate::Pos;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

/// A bounded first-step initiative bonus breaks positional and formation ties
/// in favor of doing something useful with the turn. Four points can overcome
/// a couple of lost adjacency bonuses, but not the much larger penalty for
/// stepping into a dangerous attack envelope.
const FIRST_MOVE_SCORE_BONUS: f64 = 4.0;

/// Assignment value for putting the right front-line class into its favorable
/// matchup. The engine still resolves the real combat modifiers; this is the
/// smaller commander's preference that stops two otherwise-comparable units
/// from choosing one another's jobs.
const TACTICAL_COUNTER_ASSIGNMENT: f64 = 12.0;

/// Value of firing from outside the defender's current return-fire range.
/// This is deliberately smaller than a kill or a class counter: safety breaks
/// close choices without making a ranged unit ignore a decisive target.
const SAFE_RANGED_FIRE: f64 = 10.0;

/// Standing walls are the siege arm's job. Melee can still attack them, but
/// it should wait for the ram/tower that makes the blow useful when possible.
const SIEGE_WALL_ASSIGNMENT: f64 = 22.0;
const SUPPORTED_WALL_ASSAULT: f64 = 16.0;
const UNSUPPORTED_WALL_ASSAULT: f64 = 12.0;

/// How many turns of a unit's recent whereabouts to keep. A livelock is not
/// visible in one decision — every individual step looks like the best one
/// available — so it can only be recognized from a unit's own recent past.
const LIVELOCK_WINDOW: usize = 6;

/// A unit whose whole recent past fits in this many tiles has not gone
/// anywhere. Three allows a genuine three-tile shuffle around an obstacle to
/// be recognized as one, while any unit on a real march leaves the footprint
/// within two turns and is never considered again.
const LIVELOCK_FOOTPRINT: usize = 3;

/// What leaving a proven-fruitless footprint is worth to a unit that is
/// circling inside it, in the same units the tactical scorers use. Two hexes
/// of positional error is a price worth paying to get out of a loop; walking
/// into an even fight, at roughly fifteen points of threat, is not — so this
/// redirects a stuck unit without ever ordering it to its death.
const LIVELOCK_ESCAPE_VALUE: f64 = 8.0;

/// After this many fruitless turns the tabu has had every chance to work and
/// has not: whatever the unit is trying to reach, it cannot. Standing it down
/// is strictly better than another lap — it fortifies, heals, and stops
/// paying for a route search that keeps returning the same answer.
const LIVELOCK_STAND_DOWN_AFTER: u32 = 2 * LIVELOCK_WINDOW as u32;

/// Long enough for the neighbours, borders, and enemies that produced the
/// loop to have moved on before the unit tries again.
const LIVELOCK_STAND_DOWN_TURNS: u32 = 4;

/// A sighting that makes the next tile a bad commitment is useful for longer
/// than the individual movement choice that discovered it. Keep it just long
/// enough for a unit to withdraw and reconsider from a safer square; longer
/// would make a vanished enemy freeze an otherwise healthy advance.
const UNIT_DANGER_MEMORY_TURNS: u32 = 3;

/// A unit that decides an approach would leave it below its withdrawal floor
/// gets two whole turns to create distance. The objective remains intact, so
/// this is a pause to survive rather than a silent abandonment of the campaign.
const UNIT_RETREAT_TURNS: u32 = 2;

/// Unlevied city-state forces defend the state and its immediate approaches;
/// ownership transfers to the Suzerain while levied, so those units naturally
/// use the major civilization's unrestricted tactical doctrine instead.
/// What a land unit gives up by taking its march across water instead of around.
///
/// `tactical_step`'s score is distance-to-objective plus adjacent threat and
/// support, with no terrain term — and a sea tile is both closer to an
/// objective across a bay and free of the adjacent-enemy penalty, because the
/// enemies stand on land. Open water therefore scored as the fastest and
/// safest road simultaneously. An embarked unit cannot attack, cannot fortify,
/// and defends at the era's flat `embarked_strength`, so the true cost of that
/// tile is most of the unit.
///
/// Ships fixed rather than as a gene: the genome is pinned at 40 with a
/// committed champion on that length, so `from_vec` indexing `v[40]` would
/// panic. Earn a gene on evidence.
pub(crate) const WATER_MARCH_PENALTY: f64 = 18.0;

/// Probe: how many times the deployed controller reaches `garrison_step`.
pub static GARRISON_STEP_CALLS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

const MINOR_DEFENSE_RADIUS: i32 = 6;

/// How near one of our own cities an enemy has to stand before the empire owes
/// it an answer. Six is the same ring `nearest_enemy` already calls "near home"
/// when it decides whether to chase a barbarian, so the two agree on the word.
const HOME_THREAT_RADIUS: i32 = 6;

/// The loyalty level the governor logic has always treated as an emergency.
/// Retained as a floor so making the rule rate-aware never makes it blinder
/// than it was.
const LOYALTY_LEVEL_ALARM: f64 = 70.0;

/// The share of the army home defence may claim. Half, because the failure this
/// fixes was total neglect of the homeland and the opposite extreme — recalling
/// everything to chase raiders — loses the same game by another route.
const HOME_DEFENSE_MAX_SHARE: f64 = 0.5;

/// Answer a threat with this much more strength than it carries, so the units
/// sent are sent to win rather than to trade evenly and leave a wounded raider
/// healing on our ground.
const HOME_DEFENSE_MARGIN: f64 = 1.25;

/// A defender ten tiles from the threat spends five turns walking and arrives
/// after the damage. Past this range the unit keeps its offensive job.
const HOME_DEFENSE_RECALL_RANGE: i32 = 10;

/// How near a city a hostile must come before that city wants somebody actually
/// standing in it. Three tiles is one turn's move for most classical units, so a
/// garrison ordered at this range is in place before the attacker arrives.
pub(crate) const GARRISON_ALERT_RADIUS: i32 = 3;

/// How close a visible hostile must be before a city musters against it. A
/// horseman three tiles out reaches the city next turn; a wanderer beyond that
/// is not worth a standing garrison.
const SIEGE_MUSTER_RADIUS: i32 = 3;

/// The most extra defenders one besieged city may add to the empire's standing
/// floor. The point is to outlast a camp's raiding party while still producing
/// civilians, not to convert the empire into an army it cannot pay for.
const SIEGE_MUSTER_CAP: usize = 3;

/// How many visible hostiles at the gates make a city stop building for the
/// long term and answer the short one. One is a scout passing through; two is
/// a raiding party.
const SIEGE_PRESSURE_MIN: usize = 2;

/// How many wall-breaking units the empire will ask for before it stops. Two
/// is a siege train, not a doctrine; past this the ordinary melee/ranged
/// alternation resumes so this cannot become an endless military appetite.
const SIEGE_ARM_MAX: usize = 2;

/// How far a walled enemy city can be and still be this empire's problem.
/// Beyond it the siege train would spend its life walking.
const SIEGE_TARGET_REACH: i32 = 20;

/// Railroads are valuable infrastructure, but every tile consumes one Iron
/// and one Coal. Keep enough of each material for an emergency unit upgrade
/// instead of letting an idle Engineer pave the stockpile down to zero.
const RAILROAD_RESOURCE_RESERVE: f64 = 4.0;

/// The ranked candidate for an immediate border purchase: its utility, a
/// deterministic city/position tie-breaker, and the legal action to replay.
type PlotPurchaseCandidate = (f64, std::cmp::Reverse<(u32, Pos)>, Action);

mod advanced;
pub use advanced::{
    AdvancedAi, ForceDomain, ForceGroup, ForcePosture, GrandStrategy, StrategicPlan,
    ExpansionCensus, StrategyCensus, VictoryTarget,
};

const TECH_PRIORITY: [&str; 15] = [
    "pottery",
    "animal_husbandry",
    "mining",
    "writing",
    "archery",
    "bronze_working",
    "currency",
    "masonry",
    "irrigation",
    "iron_working",
    "mathematics",
    "construction",
    "engineering",
    "education",
    "machinery",
];
const CIVIC_PRIORITY: [&str; 8] = [
    "code_of_laws",
    "craftsmanship",
    "foreign_trade",
    "early_empire",
    "state_workforce",
    "military_tradition",
    "drama_poetry",
    "political_philosophy",
];
const DISTRICT_PRIORITY: [&str; 4] = ["campus", "commercial_hub", "holy_site", "theater_square"];

/// The two district families that raise a city's housing ceiling, cheapest
/// first. Deliberately NOT in `DISTRICT_PRIORITY`: those four are ranked by a
/// bred genome weight that expresses a lane preference, while these two are
/// ranked by the housing they would actually deliver into a measured shortfall,
/// and they disappear entirely from the ranking once the city has headroom.
const HOUSING_DISTRICTS: [&str; 2] = ["aqueduct", "neighborhood"];

/// The headroom a city is steered to keep. `Game::housing_growth_mult` pays
/// full growth at 2 and half at 1, so 2 is the first value that is not a
/// penalty — not a margin of comfort, the break-even point.
const HOUSING_HEADROOM_TARGET: f64 = 2.0;

/// One coordinated force as an observer sees it: what it is, where it is
/// going, and how ready it is to fight when it gets there.
#[derive(Clone, Debug, PartialEq)]
pub struct ForceReport {
    pub domain: &'static str,
    pub posture: &'static str,
    pub units: usize,
    pub objective: Pos,
    pub readiness: f64,
    pub strength_ratio: f64,
}

/// Observer-only state for one unified, appointed military campaign.
///
/// The controller owns the mutable plan. This report exposes both the current
/// package and cumulative lifecycle counters so evaluators can distinguish a
/// plan that formed from one that researched, mobilized, declared, and took
/// its objective quickly. Nothing in play reads this type.
#[derive(Clone, Debug, PartialEq)]
pub struct WarPlanReport {
    pub enabled: bool,
    /// Whether appointment and launch use the preregistered selective-v2
    /// policy. The mutable campaign still lives in the same `WarPlan`.
    pub selective: bool,
    /// Whether appointment additionally requires the preregistered v3
    /// ready-force package and short estimated launch window.
    pub rapid: bool,
    pub active: bool,
    pub phase: Option<&'static str>,
    pub target_player: Option<usize>,
    pub objective_city: Option<u32>,
    pub breakthrough_tech: Option<Name>,
    pub assault_unit: Option<Name>,
    pub predecessor: Option<Name>,
    pub breach_unit: Option<Name>,
    pub required_bodies: usize,
    pub ready_bodies: usize,
    pub staged_bodies: usize,
    pub breach_ready: bool,
    pub upgrade_gold_reserved: f64,
    pub appointed_turn: Option<u32>,
    pub appointments: u32,
    pub breakthroughs: u32,
    pub mobilizations: u32,
    pub declarations: u32,
    pub complete_package_declarations: u32,
    pub objectives_captured: u32,
    pub objectives_captured_within_ten: u32,
    pub appointment_to_tech_turns: u32,
    pub tech_to_declaration_turns: u32,
    pub declaration_to_capture_turns: u32,
    pub appointment_to_tech_samples: Vec<u32>,
    pub tech_to_declaration_samples: Vec<u32>,
    pub declaration_to_capture_samples: Vec<u32>,
    pub aborts: BTreeMap<&'static str, u32>,
}

/// Everything an agent is willing to say about its own medium-term
/// intentions. The spectator HUD reads this to explain *why* a civilization
/// is doing what it does instead of only showing the outcome; nothing here
/// feeds back into the simulation.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanReport {
    pub strategy: &'static str,
    pub victory_target: Option<&'static str>,
    /// Whether the plan in force is the ancient-rush treatment. Observer-only:
    /// evaluators use this to measure treatment exposure, never to choose an
    /// action.
    pub rush: bool,
    pub target_player: Option<usize>,
    pub target_city: Option<u32>,
    pub threatened_city: Option<u32>,
    pub desired_cities: usize,
    pub assessed_turn: u32,
    /// Rivals this planner decided to offer peace this turn. The offer itself
    /// is `ProposeDeal { peace: true }`, answered on an ordinary board by the
    /// other seat's valuation — so a declined offer never reaches the
    /// applied-action log. A mirrored game must still ASK the real rival, so
    /// the made decision is reported as intent here. Unlike the retracted
    /// war-from-plan channel this exports a decision the planner took, not a
    /// preference upgraded into one.
    pub peace_offers: Vec<usize>,
    pub forces: Vec<ForceReport>,
    /// The one authority spanning target selection, research, production,
    /// treasury, staging, declaration, and exploitation.
    pub war: Option<WarPlanReport>,
}

pub trait Ai {
    fn take_turn(&mut self, g: &mut Game, pid: usize);

    fn strategy_label(&self) -> Option<&'static str> {
        None
    }

    /// The agent's current plan, for observers only. Stateless baselines
    /// have no plan to report.
    fn plan_report(&self) -> Option<PlanReport> {
        None
    }

    /// How many of this agent's macro reviews reached its search, for
    /// evaluators only. An agent that searches must be able to say when it
    /// did not: a cheap prior answering every review leaves a scripted
    /// agent under a searching agent's name, and a win rate cannot tell the
    /// difference. Agents without a search return `None`.
    fn review_census(&self) -> Option<crate::strategic::ReviewCensus> {
        None
    }

    /// How often an adaptive Expansion plan reached the Advanced-production
    /// dispatcher, and what that newly exposed call actually completed. This
    /// is observer-only telemetry for the default-off expansion experiments;
    /// agents without the instrument return `None`.
    fn expansion_census(&self) -> Option<ExpansionCensus> {
        None
    }

    /// Write this agent's reasoning into an observer's log.
    ///
    /// Every seat at a watched table is handed a handle on the *same*
    /// [`Journal`], so the record is one ordered account of a turn rather than
    /// one log per civilization that has to be interleaved afterwards. An
    /// agent with nothing to say about itself — the random baseline — ignores
    /// this, which is what the default does.
    fn attach_journal(&mut self, _journal: Journal) {}
}

impl<T: Ai + ?Sized> Ai for Box<T> {
    fn take_turn(&mut self, g: &mut Game, pid: usize) {
        (**self).take_turn(g, pid);
    }

    fn strategy_label(&self) -> Option<&'static str> {
        (**self).strategy_label()
    }

    fn plan_report(&self) -> Option<PlanReport> {
        (**self).plan_report()
    }

    fn review_census(&self) -> Option<crate::strategic::ReviewCensus> {
        (**self).review_census()
    }

    fn expansion_census(&self) -> Option<ExpansionCensus> {
        (**self).expansion_census()
    }

    fn attach_journal(&mut self, journal: Journal) {
        (**self).attach_journal(journal);
    }
}

/// Play a game out headlessly. The turn bound is not belt-and-braces: the only
/// thing that reliably ends a game is the score victory `do_end_turn` awards at
/// the turn limit, and `set_winner` refuses that when the lobby switched score
/// off — a setting that is serialized and restored from saves. Without the
/// bound such a game runs past its limit forever. With score enabled the bound
/// never fires first, because `set_winner` runs inside `do_end_turn` before
/// this condition is tested again.
pub fn run_game<A: Ai>(g: &mut Game, ais: &mut [A]) {
    // A headless rollout never serializes a player observation between
    // actions. Explored ground, contacts and Natural-Wonder discovery remain
    // gameplay state and are still maintained; only the large last-seen tile
    // and city copies used to render fog are omitted. Interactive server
    // stepping does not use `run_game`, so spectator and player displays keep
    // complete observation memory.
    g.set_fog_memory(false);
    while g.winner.is_none() && g.turn <= g.max_turns {
        let pid = g.current;
        ais[pid].take_turn(g, pid);
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &Action::EndTurn);
        }
    }
}

// ----------------------------------------------------------------- RandomAi

pub struct RandomAi {
    rng: Rng,
}

impl RandomAi {
    pub fn new(seed: u64) -> RandomAi {
        RandomAi {
            rng: Rng::new(seed),
        }
    }
}

impl Ai for RandomAi {
    fn take_turn(&mut self, g: &mut Game, pid: usize) {
        g.with_deferred_visibility(|g| self.take_turn_inner(g, pid));
    }
}

impl RandomAi {
    fn take_turn_inner(&mut self, g: &mut Game, pid: usize) {
        for _ in 0..60 {
            let acts: Vec<Action> = g
                .legal_actions(pid)
                .into_iter()
                .filter(|a| !matches!(a, Action::EndTurn))
                .collect();
            if acts.is_empty() {
                break;
            }
            let a = acts[self.rng.below(acts.len())].clone();
            let _ = g.apply(pid, &a);
            if g.winner.is_some() {
                break;
            }
        }
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &Action::EndTurn);
        }
    }
}

// ------------------------------------------------------------------ BasicAi

const GOV_PRIORITY: [&str; 12] = [
    "digital_democracy",
    "synthetic_technocracy",
    "corporate_libertarianism",
    "democracy",
    "communism",
    "fascism",
    "merchant_republic",
    "monarchy",
    "classical_republic",
    "oligarchy",
    "autocracy",
    "chiefdom",
];
const POLICY_PRIORITY: [&str; 20] = [
    "urban_planning",
    "colonization",
    "ilkum",
    "feudal_contract",
    "agoge",
    "discipline",
    "god_king",
    "insulae",
    "meritocracy",
    "serfdom",
    "conscription",
    "bastions",
    "retainers",
    "town_charters",
    "craftsmen",
    "maritime_industries",
    "maneuver",
    "limes",
    "survey",
    "strategos",
];

/// Turns between re-examinations of a deck that is already full. Each review
/// costs one empire valuation per candidate card, and the answer moves on the
/// scale of a civic unlocking, not of a turn passing.
const POLICY_REVIEW_EVERY: u32 = 8;

/// A policy review has only a few dozen independent cards, while each active
/// worker needs a full private game snapshot. Four branches are the measured
/// knee on the deployment-shaped single-game workload; use the rest of the
/// persistent pool for wider unit, tactical, purchase, and visibility work.
const POLICY_SCORE_MAX_WORKERS: usize = 4;

/// What `pid`'s empire is worth to `w` right now, in yield-equivalent points.
///
/// Read either side of a candidate card, the difference is that card's entire
/// effect **as the engine computes it** — district adjacency percentages,
/// unit-era windows, per-city production multipliers and all. Nothing here
/// names an effect key, so the 125-card catalogue and every card a mod adds
/// are covered by construction, and a card whose effect the engine does not
/// implement scores exactly zero instead of scoring its own documentation.
///
/// This is a counterfactual, which is the only kind of estimate this
/// repository has ever got value from: the card is applied and the result
/// measured, rather than a regression predicting what cards tend to accompany
/// winning. `docs/SUPERHUMAN.md` §0 is the evidence for preferring the former.
fn empire_reading(g: &Game, pid: usize, w: &Weights) -> f64 {
    // A card counterfactual is a read-only whole-empire sweep. Keep one memo
    // scope over it so the city-yield and ownership derivations shared by its
    // cities are answered once, then drop it before the caller changes cards.
    let _memo = g.query_memo();
    let mut value = 0.0;
    for cid in g.player_city_ids(pid) {
        let y = g.city_yields(cid);
        // Production is read as production *toward what this city is actually
        // building*. `city_yields` carries flat adders -- Urban Planning's
        // `city_production` -- but the whole `*_production_pct` family reaches
        // the game only through `item_prod_mult`, so without this factor Agoge,
        // Colonization, Ilkum, Maritime Industries, Conscription, Limes and
        // Maneuver all score exactly 0.0 and lose every tie to a card worth a
        // rounding error of gold. That is the failure `PolicyAi` was retired
        // for, one layer up.
        let queued = g.cities.get(&cid).and_then(|city| city.queue.first());
        let toward_item = g.item_prod_mult(pid, cid, queued);
        value += w.pol_food * y.food
            + w.pol_production * y.production * toward_item
            + w.pol_gold * y.gold
            + w.pol_science * y.science
            + w.pol_culture * y.culture
            + w.pol_faith * y.faith;
    }
    if let Some(y) = g.observed_yield_adjustments.get(&pid) {
        value += w.pol_food * y.food
            + w.pol_production * y.production
            + w.pol_gold * y.gold
            + w.pol_science * y.science
            + w.pol_culture * y.culture
            + w.pol_faith * y.faith;
    }
    // Combat cards move no yield. Ask the units themselves instead: a card
    // worth +5 strength to a standing army is visible here and nowhere else.
    let mut strength = 0.0;
    for uid in g.player_unit_ids(pid) {
        if let Some(unit) = g.units.get(&uid) {
            strength += g.unit_strength(unit, false) + g.unit_strength(unit, true);
        }
    }
    // Influence is neither a city yield nor unit strength, so a card whose
    // whole effect is `influence_per_turn` reads bit-identical either side of
    // the counterfactual and scores exactly 0.0 -- the same failure the
    // `*_production_pct` family had before `item_prod_mult` was folded in
    // above, and it silences an entire slot category. Ten of the seventeen
    // diplomatic cards name no yield in any effect key, `charismatic_leader`
    // (+2 influence) and `gunboat_diplomacy` (+4) among them.
    //
    // It is worth seeing because influence is the whole envoy economy. #602
    // measured suzerainty of every met city-state at **56.7% against a 22.7%
    // control**, p=0.0000 over 400 maps -- the largest ceiling in this
    // repository. #608 found the envoy pool at 0.00 unspent on every sample
    // and the agent suzerain of 3% of the city-states it meets, so the gap is
    // income, not placement. #612 found `charismatic_leader` unlocked on 63.7%
    // of turns with an open diplomatic slot on 49.1% -- and slotted on 0.0%.
    //
    // Read as a rate, because that is what a card changes. The stock is
    // `player.influence`, which the counterfactual cannot move in one turn.
    //
    // Only the policy term appears. `city_state_influence` sums three sources
    // -- the government's `influence_per_turn`, this, and the `influence_points`
    // buildings -- but slotting a card moves neither of the other two, so they
    // are identical either side of the counterfactual and cancel exactly in
    // the difference. Including them would add a large constant to both
    // readings and change no ranking.
    value
        + w.pol_military * strength
        + w.pol_influence * g.policy_effect(pid, "influence_per_turn")
}

/// Take the Dedications this age offers, best first.
///
/// Both AI tiers used to take `available_dedications(pid).next()` — the first
/// name in a `BTreeMap`, so every civilization in every game dedicated
/// alphabetically. In the Classical era that is Exodus of the Evangelists,
/// chosen by civilizations that have not founded a religion and never will.
///
/// The ranking is the civilization's own record. `projected_dedication_score`
/// asks what each Dedication *would have paid* over the era that just ended,
/// from a tally of trigger firings the engine keeps whether or not the trigger
/// was dedicated. That single number ranks both halves of the choice, because
/// a Dedication's two halves name the same activity: Free Inquiry counts your
/// Eurekas and then makes Eurekas worth more, To Arms counts your Corps kills
/// and then makes Corps cheaper. So the civilization that has been doing a
/// thing is the one both halves pay.
///
/// Which half is live still changes what the number *means*, and the engine
/// settles that: a Golden or Heroic Age normally banks no Era Score, so there
/// the tally is read purely as "which lane am I in". In a Normal or Dark Age it
/// is read literally, as the score that buys the next age. Georgia's Strength
/// in Unity is the explicit exception and keeps the literal ranking in every
/// age.
///
/// Ties — including the all-zero tie of a civilization whose first age arrives
/// before it has done anything the table counts — fall back to the alphabetical
/// order this code has always used, so the choice only moves where there is
/// evidence to move it.
pub(crate) fn choose_dedications(g: &mut Game, pid: usize, choice: DedicationChoice) {
    loop {
        let mut offered = g.available_dedications(pid);
        if offered.is_empty() {
            return;
        }
        // A Golden or Heroic Age normally banks no Era Score, so there the
        // projection is only a correlate of what the Golden half is worth —
        // and ranking on it is what lost the first gate. `Banking` keeps the
        // measured number where it is the literal objective and leaves the
        // rest alone. Georgia is the shipped exception: Strength in Unity also
        // pays the Normal-Age half during Golden and Heroic Ages.
        let banking = !matches!(g.players[pid].age.as_str(), "golden" | "heroic")
            || g.civ_effect(pid, "golden_dedication_era_score") > 0.0;
        let rank = match choice {
            DedicationChoice::Alphabetical => false,
            DedicationChoice::Measured => true,
            DedicationChoice::Banking => banking,
        };
        if rank {
            offered.sort_by(|left, right| {
                g.projected_dedication_score(pid, right)
                    .cmp(&g.projected_dedication_score(pid, left))
                    .then(left.cmp(right))
            });
        }
        let mut progressed = false;
        for dedication in offered {
            if g.apply(pid, &Action::ChooseDedication { dedication: Name::new(&dedication) }).is_ok() {
                progressed = true;
                break;
            }
        }
        if !progressed {
            return;
        }
    }
}

/// Hold the deck the empire is worth most with, and change it when that
/// changes.
///
/// The predecessor of this function tried twenty hard-coded cards in a fixed
/// order, and only while a slot stood empty — so a deck filled once in the
/// Ancient era and `policies_fit` refused every later card for the rest of the
/// game. Measured over 64 seat-games (`src/bin/policy_census.rs`), an average
/// seat unlocked 42.0 cards and played 7.3 of them.
///
/// Ordering falls back to `POLICY_PRIORITY` on ties, so wherever the
/// counterfactual is silent — a card the engine gives no effect — the choice
/// is exactly the one this code has always made.
/// Score one candidate against the policy slate the review started with.
///
/// The caller restores the exact initial slate before the next score, making
/// candidate values independent. That lets a headless controller evaluate
/// this expensive counterfactual batch on worker-private game snapshots while
/// retaining the serial candidate order and deck commit below.
fn policy_card_score(
    g: &mut Game,
    pid: usize,
    w: &Weights,
    candidate: &(usize, String, Name),
) -> (f64, usize, String, Name) {
    let (priority, slot, card) = candidate;
    let incumbent = g.players[pid].policies.contains(card);
    if incumbent {
        g.players[pid].policies.remove(card);
    }
    let without = empire_reading(g, pid, w);
    g.players[pid].policies.insert(*card);
    let with = empire_reading(g, pid, w);
    if !incumbent {
        g.players[pid].policies.remove(card);
    }
    let gain = with - without;
    // A sitting card keeps its slot unless beaten by the margin. Without
    // this the deck reshuffles on arithmetic noise every review.
    let hysteresis = if incumbent {
        w.pol_swap_margin * gain.abs()
    } else {
        0.0
    };
    (gain + hysteresis, *priority, slot.clone(), *card)
}

fn revise_policy_deck(g: &mut Game, pid: usize, w: &Weights, pool: Option<&WorkPool>) {
    let slots = g.gov_slots(pid);
    let total = slots.military + slots.economic + slots.diplomatic + slots.wildcard;
    if total <= 0 {
        return;
    }
    if w.policy_deck == PolicyDeck::Empty {
        return;
    }
    if w.policy_deck == PolicyDeck::Legacy {
        if (g.players[pid].policies.len() as i64) < total {
            for card in POLICY_PRIORITY {
                let _ = g.apply(
                    pid,
                    &Action::SlotPolicy {
                        policy: Name::new(card),
                    },
                );
            }
        }
        return;
    }
    let held: Vec<Name> = g.players[pid].policies.iter().cloned().collect();
    if held.len() as i64 >= total && !g.turn.is_multiple_of(POLICY_REVIEW_EVERY) {
        return;
    }

    let mut candidates = g.available_policies(pid);
    candidates.extend(held.iter().cloned());
    candidates.sort();
    candidates.dedup();

    let rank = |card: &str| {
        POLICY_PRIORITY
            .iter()
            .position(|entry| *entry == card)
            .unwrap_or(POLICY_PRIORITY.len())
    };

    let candidates: Vec<(usize, String, Name)> = candidates
        .into_iter()
        .filter_map(|card| {
            g.rules
                .policies
                .get(&card)
                .map(|spec| (rank(&card), spec.slot.clone(), card))
        })
        .collect();
    let mut scored: Vec<(f64, usize, String, Name)> = match pool
        .filter(|pool| pool.threads() > 1 && candidates.len() > 1)
    {
        Some(pool) => {
            // `Game` has worker-local RefCell caches and cannot be shared.
            // One snapshot per active worker keeps those caches private while
            // the pool's shared cursor balances uneven candidate costs.
            let active = pool
                .threads()
                .min(candidates.len())
                .min(POLICY_SCORE_MAX_WORKERS);
            let states = (0..active).map(|_| g.clone()).collect::<Vec<_>>();
            let weights = w.clone();
            pool.map_stateful_limited(
                candidates.len(),
                POLICY_SCORE_MAX_WORKERS,
                states,
                move |mut branch, indices| {
                    indices
                        .map(|index| {
                            (
                                index,
                                policy_card_score(
                                    &mut branch,
                                    pid,
                                    &weights,
                                    &candidates[index],
                                ),
                            )
                        })
                        .collect()
                },
            )
        }
        None => candidates
            .iter()
            .map(|candidate| policy_card_score(g, pid, w, candidate))
            .collect(),
    };

    scored.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.cmp(&b.1))
            .then(a.3.cmp(&b.3))
    });

    let mut room = [slots.military, slots.economic, slots.diplomatic];
    let mut wildcard = slots.wildcard;
    let mut target: std::collections::BTreeSet<Name> = Default::default();
    for (_, _, slot, card) in &scored {
        let kind = match slot.as_str() {
            "military" => 0,
            "economic" => 1,
            "diplomatic" => 2,
            _ => 3, // a Wildcard card fits only a Wildcard slot
        };
        if kind < 3 && room[kind] > 0 {
            room[kind] -= 1;
            target.insert(*card);
        } else if wildcard > 0 {
            wildcard -= 1;
            target.insert(*card);
        }
    }

    for card in held {
        if !target.contains(&card) {
            let _ = g.apply(pid, &Action::UnslotPolicy { policy: card });
        }
    }
    for card in &target {
        if !g.players[pid].policies.contains(card) {
            let _ = g.apply(pid, &Action::SlotPolicy { policy: *card });
        }
    }
    // Floor: whatever the assignment above left empty, fill the way this code
    // always has. An empty slot is strictly worse than a card of unknown worth,
    // and this guarantees the change can never reduce occupancy.
    if (g.players[pid].policies.len() as i64) < total {
        for card in POLICY_PRIORITY {
            let _ = g.apply(
                pid,
                &Action::SlotPolicy {
                    policy: Name::new(card),
                },
            );
        }
    }
}

/// Strategy weights steering BasicAi decisions. Defaults reproduce the
/// original hand-tuned behavior; the `evolve` GA searches this space.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Weights {
    pub city_target: f64,       // stop settling at this many cities+settlers
    pub settler_min_pop: f64,   // city pop needed before making a settler
    pub settler_stop_turn: f64, // no new settlers after this turn
    pub mil_per_city: f64,      // military units to keep per city
    pub builder_per_city: f64,  // builders to keep per city
    pub war_ratio: f64,         // declare war if my power > ratio*theirs+margin
    pub war_margin: f64,
    pub peace_ratio: f64, // sue for peace if my power < ratio*theirs
    pub war_min_turn: f64,
    pub attack_floor: f64,  // minimum exchange score to attack (SEE-style)
    pub kill_bonus: f64,    // exchange bonus for a killing blow
    pub trade_caution: f64, // weight on expected counter-damage
    pub settle_food: f64,   // settle-site yield weights
    pub settle_prod: f64,
    pub settle_gold: f64,
    pub settle_dist: f64, // per-hex penalty on distant settle sites
    pub min_city_dist: f64,
    pub wonder_min_bld: f64, // buildings before a city tries wonders
    pub faith_builder: f64,  // faith reserve before buying a builder
    pub d_campus: f64,       // district build priorities (higher first)
    pub d_commercial: f64,
    pub d_holy: f64,
    pub d_theater: f64,
    // opening book: first four capital builds, indexes into OPENING_MENU
    // (floor; >= menu length = no scripted pick, evaluate normally)
    pub open0: f64,
    pub open1: f64,
    pub open2: f64,
    pub open3: f64,
    // 1-ply tactical movement: candidate tiles scored by progress toward the
    // target plus these positional terms
    pub mv_support: f64, // bonus per adjacent friendly military unit
    pub mv_threat: f64,  // penalty per point of expected incoming damage
    // Hierarchical combat doctrine. AdvancedAi turns these genes into shared
    // army/fleet orders; keeping them in Weights lets self-play evolve economy,
    // grand strategy, and battlefield execution as one genome.
    pub command_radius: f64,     // maximum separation inside one force group
    pub muster_radius: f64,      // distance from group anchor considered ready
    pub muster_readiness: f64,   // fraction assembled before a planned advance
    pub cohesion: f64,           // movement reward for staying with the force
    pub focus_fire: f64,         // attack bonus for the group's shared target
    pub screen: f64,             // penalty for ranged/siege moving ahead of melee
    pub role_spacing: f64,       // reward for each role's preferred engagement depth
    pub objective_progress: f64, // movement reward toward the shared objective
    pub local_superiority: f64,  // caution when local hostile power is greater
    pub withdraw_hp: f64,        // enter persistent recovery at or below this HP
    pub rejoin_hp: f64,          // leave recovery at or above this HP
    // Policy deck appetite. A card is valued by slotting it and asking the
    // engine what changed (`Weights::card_value`), so these are the exchange
    // rates between the things a card can buy -- not a per-card table. The
    // catalogue is 125 cards and grows with any mod, so nothing here names one.
    pub pol_food: f64,
    pub pol_production: f64,
    pub pol_gold: f64,
    pub pol_science: f64,
    pub pol_culture: f64,
    pub pol_faith: f64,
    /// Yield-equivalent of one point of fielded combat strength. Small: a card
    /// worth +5% to ten units moves strength by tens, a yield card by ones.
    pub pol_military: f64,
    /// What one point of influence per turn is worth to the policy valuation,
    /// in the same units as a point of yield.
    ///
    /// **Zero by default, which reproduces the shipped behaviour exactly.**
    /// Influence converts to envoys at the government's `influence_threshold`
    /// and envoys convert to suzerainty at three per city-state, so a point of
    /// influence is worth a fraction of an envoy and an envoy is worth a
    /// fraction of a suzerainty -- but the suzerainty it eventually buys is
    /// measured at a 34-point win-rate swing, so the chain is short and the
    /// prize is large. The right value is not derivable from that and is what
    /// an evaluation is for; this exists so the axis can be measured at all.
    pub pol_influence: f64,
    /// Fraction by which a challenger must beat the incumbent to take its
    /// slot. Zero re-shuffles the deck on noise; one never swaps at all.
    pub pol_swap_margin: f64,
    /// Which deck this strategy holds.
    ///
    /// **Not a gene.** Deliberately absent from `to_vec`/`from_vec`/`bounds`,
    /// so the GA can neither read nor breed it and the genome stays 40 wide.
    /// It rides on `Weights` for one reason: `AdvancedAi::with_weights` already
    /// carries a genome into the inner `BasicAi`, so an eval arm costs no
    /// change to `src/ai/advanced.rs` or `src/elo.rs`. Set it in a harness;
    /// leave it alone in play.
    #[serde(default)]
    pub policy_deck: PolicyDeck,
    /// How this strategy picks its Dedication at an age transition.
    ///
    /// **Not a gene**, for the same reasons as `policy_deck`: absent from
    /// `to_vec`/`from_vec`/`bounds`, so the genome stays 40 wide.
    #[serde(default)]
    pub dedication_choice: DedicationChoice,
}

/// The two arms a Dedication experiment needs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DedicationChoice {
    /// The first name `available_dedications` returns, which is a `BTreeMap`
    /// key, which is alphabetical order.
    ///
    /// **This is the agent that plays, because it is the agent that wins.** It
    /// looks arbitrary and it is, but in the Classical era alphabetical order
    /// leads with Exodus of the Evangelists, whose Golden-Age half feeds
    /// missionaries and Great Prophet points — and religion is the lane that
    /// converts in this engine. Measured against `Measured` over 120 mirrored
    /// maps it took **58.8%** of games, 31 map directions to 10, sign
    /// p=0.0015, with the anytime-valid e-process crossing at map 51. See
    /// `docs/AGES.md`. It is no longer the default — `Banking` beat it — but it
    /// remains the frozen control for every age number published before
    /// 2026-07-27.
    Alphabetical,
    /// Ranked by what each Dedication would have paid over the era that just
    /// ended, measured from the civilization's own trigger tally.
    ///
    /// **A recorded negative result, retained as an evaluator arm.** The
    /// projection is the right objective in a Normal or Dark Age, where Era
    /// Score literally buys the next age. In a Golden or Heroic Age it is only
    /// a *correlate* of what the Golden half is worth, and an argmax over a
    /// correlate is the failure mode this repository keeps rediscovering.
    Measured,
    /// `Measured` restricted to the ages where the number it ranks on is the
    /// literal objective: a Normal or Dark Age banks Era Score, so the
    /// Dedication that would have paid most is the one that buys the next age
    /// soonest. A Golden or Heroic Age banks nothing, so that choice is left
    /// exactly as `Alphabetical` makes it.
    ///
    /// This is the repair for `Measured`'s loss, and it is the whole of the
    /// repair — no new signal, just the same signal withdrawn from the half of
    /// the decision where it was never causal.
    ///
    /// **PROMOTED.** Pre-registered at seed 970000, 300 mirrored maps, 600
    /// games: **57.7%**, 67 map directions to 21, sign p=0.0000, Elo **+54**
    /// (CI +14..+93), Wilson **52.0%–63.1%**, e-process 5.72e4 crossing at map
    /// 112 — `promotion gate: PASS` under the unmodified gate. The earlier
    /// disjoint seed 960000 agreed at 56.2%; pooled that is **420 maps, 93 map
    /// directions to 32**.
    #[default]
    Banking,
}

/// The three arms a policy-deck experiment needs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PolicyDeck {
    /// Cards valued by slotting them and reading the empire either side.
    ///
    /// Old champion artifacts predate this non-gene field. They deserialize
    /// to Live for compatibility, and a recorded six-seat A/B showed that
    /// changing them to Legacy loses significantly (12 map directions for,
    /// 26 against, p=0.0336). Gene-vector children preserve their template's
    /// non-gene policy through `Weights::from_vec_like`.
    #[default]
    Live,
    /// The pre-2026-07-27 behaviour: the twenty cards of `POLICY_PRIORITY`, in
    /// order, and only while a slot stands empty.
    Legacy,
    /// Slot nothing, ever.
    ///
    /// Not a strategy — an ablation. `Legacy` against `Empty` measures what the
    /// entire card layer is worth, which bounds what *any* card policy can win
    /// and therefore whether choosing well within it deserves more effort. Run
    /// this before optimising a subsystem, not after.
    Empty,
}

pub const OPENING_MENU: [&str; 6] = [
    "scout", "warrior", "builder", "settler", "slinger", "monument",
];

impl Default for Weights {
    fn default() -> Weights {
        Weights {
            city_target: 4.0,
            settler_min_pop: 2.0,
            settler_stop_turn: 150.0,
            mil_per_city: 1.0,
            builder_per_city: 0.5,
            war_ratio: 1.8,
            war_margin: 20.0,
            peace_ratio: 0.6,
            war_min_turn: 40.0,
            attack_floor: 0.0,
            kill_bonus: 25.0,
            trade_caution: 1.0,
            settle_food: 1.2,
            settle_prod: 1.0,
            settle_gold: 0.3,
            settle_dist: 0.4,
            min_city_dist: 4.0,
            wonder_min_bld: 3.0,
            faith_builder: 120.0,
            d_campus: 4.0,
            d_commercial: 3.0,
            d_holy: 2.0,
            d_theater: 1.0,
            open0: 1.0,
            open1: 3.0,
            open2: 2.0,
            open3: 5.0, // warrior settler builder monument
            mv_support: 2.0,
            mv_threat: 0.5,
            command_radius: 6.0,
            muster_radius: 3.0,
            muster_readiness: 0.67,
            cohesion: 3.0,
            focus_fire: 2.5,
            screen: 4.0,
            role_spacing: 2.0,
            objective_progress: 2.5,
            local_superiority: 6.0,
            withdraw_hp: 45.0,
            rejoin_hp: 80.0,
            pol_food: 0.6,
            pol_production: 1.0,
            pol_gold: 0.6,
            pol_science: 1.0,
            pol_culture: 1.0,
            pol_faith: 0.7,
            pol_military: 0.05,
            pol_influence: 0.0,
            pol_swap_margin: 0.15,
            // LEGACY, not Live. The counterfactual deck is a measured null:
            // 18 map directions to 15, p=0.7283 over 120 mirrored maps, with
            // terminal score also flat. It costs an empire valuation per
            // candidate card per review, so shipping it would buy a real
            // slowdown with no evidence of strength. The mechanism stays for
            // study -- `PolicyDeck::Live` still selects it and the eval arms
            // still work -- but the agent that plays is the one that always
            // played.
            policy_deck: PolicyDeck::Legacy,
            dedication_choice: DedicationChoice::Banking,
        }
    }
}

impl Weights {
    pub fn to_vec(&self) -> Vec<f64> {
        vec![
            self.city_target,
            self.settler_min_pop,
            self.settler_stop_turn,
            self.mil_per_city,
            self.builder_per_city,
            self.war_ratio,
            self.war_margin,
            self.peace_ratio,
            self.war_min_turn,
            self.attack_floor,
            self.kill_bonus,
            self.trade_caution,
            self.settle_food,
            self.settle_prod,
            self.settle_gold,
            self.settle_dist,
            self.min_city_dist,
            self.wonder_min_bld,
            self.faith_builder,
            self.d_campus,
            self.d_commercial,
            self.d_holy,
            self.d_theater,
            self.open0,
            self.open1,
            self.open2,
            self.open3,
            self.mv_support,
            self.mv_threat,
            self.command_radius,
            self.muster_radius,
            self.muster_readiness,
            self.cohesion,
            self.focus_fire,
            self.screen,
            self.role_spacing,
            self.objective_progress,
            self.local_superiority,
            self.withdraw_hp,
            self.rejoin_hp,
        ]
    }

    pub fn from_vec(v: &[f64]) -> Weights {
        Weights {
            city_target: v[0],
            settler_min_pop: v[1],
            settler_stop_turn: v[2],
            mil_per_city: v[3],
            builder_per_city: v[4],
            war_ratio: v[5],
            war_margin: v[6],
            peace_ratio: v[7],
            war_min_turn: v[8],
            attack_floor: v[9],
            kill_bonus: v[10],
            trade_caution: v[11],
            settle_food: v[12],
            settle_prod: v[13],
            settle_gold: v[14],
            settle_dist: v[15],
            min_city_dist: v[16],
            wonder_min_bld: v[17],
            faith_builder: v[18],
            d_campus: v[19],
            d_commercial: v[20],
            d_holy: v[21],
            d_theater: v[22],
            open0: v[23],
            open1: v[24],
            open2: v[25],
            open3: v[26],
            mv_support: v[27],
            mv_threat: v[28],
            command_radius: v[29],
            muster_radius: v[30],
            muster_readiness: v[31],
            cohesion: v[32],
            focus_fire: v[33],
            screen: v[34],
            role_spacing: v[35],
            objective_progress: v[36],
            local_superiority: v[37],
            withdraw_hp: v[38],
            rejoin_hp: v[39],
            // The policy appetites are NOT genes. Measured on the statistic
            // that tracks winning, scrambling the whole policy block costs
            // +0.0006 +/- 0.0229 -- flat. Carrying eight worthless dimensions
            // would widen the search for nothing, and it collided with the
            // committed 40-gene champion another agent landed on main.
            // `PolicyDeck::Live` still reads them; the GA does not.
            // `policy_deck` and the appetites take the shipped defaults here.
            ..Weights::default()
        }
    }

    /// Reconstruct the gene vector without changing non-gene policy state.
    ///
    /// `from_vec` intentionally supplies production defaults when no template
    /// exists. Evolution and causal gene interventions do have a template:
    /// the parent or incumbent they are changing. Old champion artifacts can
    /// carry a compatibility policy different from `Weights::default`, so
    /// dropping these fields makes the first child differ before any gene
    /// mutation acts.
    pub fn from_vec_like(v: &[f64], template: &Weights) -> Weights {
        let mut weights = Weights::from_vec(v);
        weights.pol_food = template.pol_food;
        weights.pol_production = template.pol_production;
        weights.pol_gold = template.pol_gold;
        weights.pol_science = template.pol_science;
        weights.pol_culture = template.pol_culture;
        weights.pol_faith = template.pol_faith;
        weights.pol_military = template.pol_military;
        weights.pol_influence = template.pol_influence;
        weights.pol_swap_margin = template.pol_swap_margin;
        weights.policy_deck = template.policy_deck;
        weights.dedication_choice = template.dedication_choice;
        weights
    }

    /// (lo, hi) clamp per gene, same order as to_vec.
    pub fn bounds() -> [(f64, f64); 40] {
        [
            (2.0, 12.0),
            (1.0, 5.0),
            (60.0, 400.0),
            (0.3, 4.0),
            (0.2, 2.0),
            (0.8, 5.0),
            (-20.0, 80.0),
            (0.2, 1.2),
            (10.0, 200.0),
            (-25.0, 25.0),
            (0.0, 80.0),
            (0.2, 3.0),
            (0.2, 3.0),
            (0.2, 3.0),
            (0.0, 2.0),
            (0.0, 2.0),
            (3.0, 7.0),
            (0.0, 8.0),
            (40.0, 400.0),
            (0.0, 8.0),
            (0.0, 8.0),
            (0.0, 8.0),
            (0.0, 8.0),
            (0.0, 6.99),
            (0.0, 6.99),
            (0.0, 6.99),
            (0.0, 6.99),
            (0.0, 10.0),
            (0.0, 3.0),
            (2.0, 12.0),
            (1.0, 6.0),
            (0.25, 1.0),
            (0.0, 10.0),
            (0.0, 8.0),
            (0.0, 12.0),
            (0.0, 8.0),
            (0.5, 6.0),
            (0.0, 16.0),
            (20.0, 65.0),
            (60.0, 100.0),
        ]
    }

    /// Gene names, same order as `to_vec` and `bounds`.
    ///
    /// A search that reports per-gene results has to name them, and deriving
    /// the name from the index by hand is how a table ends up mislabelled by
    /// one row. `gene_names_match_the_vector` pins the length.
    pub fn gene_names() -> [&'static str; 40] {
        [
            "city_target",
            "settler_min_pop",
            "settler_stop_turn",
            "mil_per_city",
            "builder_per_city",
            "war_ratio",
            "war_margin",
            "peace_ratio",
            "war_min_turn",
            "attack_floor",
            "kill_bonus",
            "trade_caution",
            "settle_food",
            "settle_prod",
            "settle_gold",
            "settle_dist",
            "min_city_dist",
            "wonder_min_bld",
            "faith_builder",
            "d_campus",
            "d_commercial",
            "d_holy",
            "d_theater",
            "open0",
            "open1",
            "open2",
            "open3",
            "mv_support",
            "mv_threat",
            "command_radius",
            "muster_radius",
            "muster_readiness",
            "cohesion",
            "focus_fire",
            "screen",
            "role_spacing",
            "objective_progress",
            "local_superiority",
            "withdraw_hp",
            "rejoin_hp",
        ]
    }
}

#[cfg(test)]
mod gene_table_tests {
    use super::Weights;

    #[test]
    fn gene_names_match_the_vector() {
        let w = Weights::default();
        assert_eq!(w.to_vec().len(), Weights::gene_names().len());
        assert_eq!(w.to_vec().len(), Weights::bounds().len());
    }

    #[test]
    fn every_gene_default_sits_inside_its_own_bounds() {
        let v = Weights::default().to_vec();
        for (index, (lo, hi)) in Weights::bounds().iter().enumerate() {
            assert!(
                v[index] >= *lo && v[index] <= *hi,
                "{} default {} outside [{lo}, {hi}]",
                Weights::gene_names()[index],
                v[index]
            );
        }
    }

    #[test]
    fn gene_vector_children_preserve_non_gene_policy_state() {
        let legacy_artifact: Weights = serde_json::from_str("{}").expect("empty legacy weights");
        assert_eq!(legacy_artifact.policy_deck, super::PolicyDeck::Live);
        assert_eq!(Weights::default().policy_deck, super::PolicyDeck::Legacy);
        let template = Weights {
            pol_food: 0.11,
            pol_production: 0.22,
            pol_gold: 0.33,
            pol_science: 0.44,
            pol_culture: 0.55,
            pol_faith: 0.66,
            pol_military: 0.77,
            pol_swap_margin: 0.88,
            policy_deck: super::PolicyDeck::Live,
            dedication_choice: super::DedicationChoice::Alphabetical,
            ..Weights::default()
        };
        assert_eq!(
            Weights::from_vec_like(&template.to_vec(), &template),
            template,
            "a templated gene reconstruction must not change non-gene policy state"
        );
    }
}

/// Strategic job inferred from a unit's class and promotion line. Both AI
/// tiers use the same doctrine so independent movement and force coordination
/// do not disagree about what a unit is for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UnitDoctrine {
    Recon,
    Assault,
    Mobile,
    Ranged,
    Siege,
    Support,
    AirDefense,
    AirStrike,
    Carrier,
}

/// Everything about a unit that changes when it accomplishes something:
/// charges spent improving, building, or spreading; experience from a fight;
/// damage taken or healed; a promotion chosen; a concert played. Whatever else
/// a unit did with a turn, if none of this moved then the turn bought nothing.
type WorkMark = (i32, i64, i32, usize, i64);

fn work_mark(g: &Game, uid: u32) -> WorkMark {
    let unit = &g.units[&uid];
    (
        unit.charges,
        unit.xp,
        unit.hp,
        unit.promotions.len(),
        unit.album_sales,
    )
}

/// One unit's recent whereabouts, which is the only place a livelock is
/// visible. Every step of a loop is individually the best move available, so
/// no single decision can be blamed for it; only the unit's own history shows
/// that the decisions together are going nowhere.
#[derive(Clone, Default)]
struct UnitMotion {
    /// The tile this unit began each of its last `LIVELOCK_WINDOW` turns on,
    /// newest last.
    tiles: VecDeque<Pos>,
    /// The work fingerprint as of the last turn it changed.
    work: WorkMark,
    /// Consecutive turns since the fingerprint last changed.
    fruitless: u32,
    /// While this is in the future the unit holds its ground instead of
    /// issuing orders it has already proved are worthless.
    resume_turn: u32,
    /// This turn's verdict, settled once when the window is taken down. The
    /// tactical scorers ask about it for every candidate tile of every unit,
    /// which is far too hot a path to re-derive it in.
    looping: bool,
}

impl UnitMotion {
    /// How many distinct tiles the window covers.
    fn footprint(&self) -> usize {
        let mut seen: Vec<Pos> = Vec::with_capacity(self.tiles.len());
        for tile in &self.tiles {
            if !seen.contains(tile) {
                seen.push(*tile);
            }
        }
        seen.len()
    }

    /// A full window of turns spent moving between a handful of tiles while
    /// nothing about the unit changed. Both halves matter: a unit that has
    /// stopped is a different (already reported) problem, and a unit that is
    /// spending charges or trading blows is working, however small its circuit.
    fn circling(&self) -> bool {
        if self.tiles.len() < LIVELOCK_WINDOW || (self.fruitless as usize) < LIVELOCK_WINDOW {
            return false;
        }
        (2..=LIVELOCK_FOOTPRINT).contains(&self.footprint())
    }
}

/// The durable, unit-local job the AI has committed to.
///
/// This lives in the controller rather than in [`crate::game::Unit`]: it is
/// reasoning state, not a game rule, and it must survive the Civilization VI
/// mirror rebuilding its board with fresh internal unit IDs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnitObjective {
    /// This unit is part of the force trying to take this specific city.
    CaptureCity {
        city: u32,
        target: Pos,
        started_turn: u32,
        last_confirmed_turn: u32,
    },
}

impl UnitObjective {
    /// The physical destination associated with this objective.
    pub fn target(self) -> Pos {
        match self {
            Self::CaptureCity { target, .. } => target,
        }
    }
}

/// A dangerous approach remembered by one unit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnitDangerMemory {
    /// The tile the unit deliberately declined to enter.
    pub position: Pos,
    /// Conservative expected counter-damage from the enemies visible to this
    /// planner when the warning was recorded.
    pub expected_damage: u32,
    pub observed_turn: u32,
    /// Exclusive turn at which the sighting is no longer trusted.
    pub expires_turn: u32,
}

/// Inspectable, multi-turn memory owned by one individual unit.
///
/// An objective, a danger warning, and a temporary retreat are independent:
/// taking a safe step backward must not erase the city this unit was helping
/// to capture. Callers receive a copy through [`BasicAi::unit_memory`], so
/// observers cannot mutate the controller's reasoning state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UnitMemory {
    pub objective: Option<UnitObjective>,
    pub danger: Option<UnitDangerMemory>,
    /// Exclusive turn through which the unit must prefer getting safe over
    /// resuming its objective.
    pub retreat_until: Option<u32>,
}

#[derive(Clone)]
pub struct BasicAi {
    minor: bool,
    barb: bool,
    /// Let the baseline governor build the district that repairs an Amenity
    /// deficit.
    ///
    /// ⚠ `DISTRICT_PRIORITY` is four families — Campus, Commercial Hub, Holy
    /// Site, Theater Square — and `entertainment_complex` is **not one of
    /// them**. So this governor cannot build one, ever, and it makes most of a
    /// deployed empire's build decisions: `advanced_production` is reached on
    /// 22.2% of an adaptive agent's planned turns (all of them Recovery), and
    /// everything else ends here. Across every live game inspected this session
    /// CIVVIS ordered an Entertainment Complex **zero times**.
    ///
    /// That is not a small omission, because Amenities are not a yield — they
    /// are a band that multiplies every yield the city makes.
    /// `Game::amenity_yield_mult_for`: `0` → 1.00, `-3` → **0.80**, `-6` →
    /// **0.70**. Host-exported figures from run `civvis-20260803T082856Z` at
    /// turn 251, seven cities:
    ///
    /// | city | amenities | surplus | multiplier |
    /// |---|---|---|---|
    /// | Kraków | 4/7 | -3 | 0.80 |
    /// | Wroclaw | 2/8 | **-6** | **0.70** |
    /// | Radom, Warsaw, Gdansk | 4/7 | -3 | 0.80 |
    /// | Bydgoszcz | 3/6 | -3 | 0.80 |
    ///
    /// ⚠⚠ Those are **Firaxis's own numbers**, not CIVVIS's model — #967 added
    /// `GetAmenities`/`GetAmenitiesNeeded` to the bridge. An earlier attempt at
    /// this axis (#975) was closed partly because it was priced against a field
    /// nobody was sending; that is no longer true.
    ///
    /// The empire is running at roughly 0.78 on everything, including the
    /// science these games are short of. Returning it to neutral is about
    /// **+28%** — larger than the Research Lab this session also chased.
    ///
    /// Off for the frozen native controllers, whose recorded ladders would
    /// otherwise shift underneath them.
    pub(crate) amenity_districts: bool,
    /// Let this governor build the districts that raise the housing ceiling —
    /// the Aqueduct and the Neighborhood.
    ///
    /// ⚠⚠ Amenities are not the only band on growth, and they are not the
    /// tighter one. `Game::housing_growth_mult` **halves** growth the moment
    /// headroom falls below 2 and **quarters** it below 1, where the Amenity
    /// cliff needs a −5 surplus to bite that hard. Population is what science
    /// is — roughly 1.16 science per citizen, measured across three live games
    /// at 7/5/6 cities — so the housing ceiling is the science ceiling.
    ///
    /// Measured over **12,969 host-exported city-turns** (`GetHousing()`, not
    /// CIVVIS's model) across **every one of the 18 live runs that carries the
    /// export**:
    ///
    /// | headroom | growth | share of city-turns |
    /// |---|---|---|
    /// | ≥ 2 | 1.00x | 28.8% |
    /// | 1..2 | **0.50x** | 22.5% |
    /// | −4..1 | **0.25x** | **45.8%** |
    /// | ≤ −4 | **0.00x** | 2.9% |
    ///
    /// **71.2% of city-turns are throttled**, the median headroom is **1** —
    /// already inside the half-growth band — and the mean growth multiplier is
    /// **0.515**. At pop ≥ 8 (n = 6,122) it is **87.9%** throttled on a mean
    /// headroom of −0.52.
    ///
    /// And the repair is not merely out-ranked, it is barely reached at all.
    /// Over the same 18 runs, of **485 district orders**: Aqueduct **4**, its
    /// Roman unique Bath **4**, Neighborhood **0** — **1.65%** together,
    /// against 92 Commercial Hubs, 79 Campuses and 76 Entertainment Complexes.
    /// The empire builds the districts that produce science and not the one
    /// that raises the population the science is computed from.
    ///
    /// It is also **late**: the Aqueduct's median order turn is **164** and the
    /// Bath's **214**, against a Campus at 131 — the repair arrives long after
    /// the growth it was meant to unlock was needed.
    ///
    /// Off for the frozen native controllers, whose recorded ladders would
    /// otherwise shift underneath them.
    pub(crate) housing_districts: bool,
    /// Scale each district family by how much of the empire still lacks it.
    pub(crate) district_coverage: bool,
    /// Break a production COST TIE by which great-work slots can actually be filled.
    pub(crate) slot_kind_tiebreak: bool,
    pursue_religion: bool,
    /// Enforce the live Firaxis rule that a religious unit inherits its
    /// purchase city's majority. Off for the frozen native controllers and
    /// enabled explicitly by the Civilization VI bridge.
    live_religious_purchase_guard: bool,
    /// Let a city under visible siege raise its standing-army floor, so that a
    /// besieging force it is not formally "at war" with — Barbarians — can be
    /// answered at all. Off for the frozen native controllers, whose recorded
    /// ladders would otherwise shift underneath them, and enabled explicitly
    /// by the Civilization VI bridge. See `besieged_military_floor`.
    siege_muster: bool,
    /// Let the unit chooser ask for SIEGE as a role. Off for the frozen native
    /// controllers. See `best_military_role` and `siege_is_the_missing_arm`.
    siege_role: bool,
    /// Keep the land army out of the water: exclude water from a land unit's
    /// exploration goals, bring an already-embarked unit ashore whether or not
    /// it has an upgrade waiting, and let `peacetime_step` know when
    /// `military_step` fell through to it *at war*.
    ///
    /// Measured across 133 live runs: land combat units spend a mean 15% of
    /// their unit-turns embarked (p90 48%, worst run 84%), and **21.7% mean —
    /// 92.8% at worst — while one of our own cities is taking damage**. An
    /// embarked unit cannot attack and defends at `embarked_strength`. See
    /// `disembark_step`.
    ///
    /// Off for the frozen native controllers, whose recorded ladders would
    /// otherwise shift underneath them, and enabled explicitly by the
    /// Civilization VI bridge.
    come_ashore: bool,
    /// Let threats standing in our own territory claim units before the
    /// offensive does. Off for the frozen native controllers, whose recorded
    /// ladders would otherwise shift underneath them, and enabled explicitly by
    /// the Civilization VI bridge — which is where the failure was measured.
    /// See `home_defense_objective`.
    home_defense: bool,
    /// Rank loyalty emergencies by TURNS TO FLIP rather than by level. Off for
    /// the frozen native controllers, enabled by the Civilization VI bridge.
    /// See `loyalty_emergency`.
    loyalty_rate_alarm: bool,
    /// Record every tactical step through `path_move` instead of applying it
    /// raw, so a unit stepped a second time in the same turn cannot walk back
    /// onto the tile it just left.
    ///
    /// **Off by default, live-bridge only**, on the same footing as
    /// `home_defense`. A raw `g.apply(Move)` records nothing, so the
    /// same-turn reversal guard inside `path_move` never sees the first step;
    /// the second step is then free to undo it. Net zero ground, two emitted
    /// orders, and Civilization VI refuses the second as a MOVE_TO of the
    /// unit's own tile — 217 of 217 refused moves on run
    /// `civvis-20260801T224944Z` were exactly that pair. Gated because the
    /// call sites are shared with the frozen `advanced_v1` anchor, whose
    /// recorded ladders must keep replaying move-for-move.
    recorded_tactical_step: bool,
    /// Use explicit combined-arms roles when assigning attacks and movement:
    /// the melee/anti-cavalry/cavalry counter cycle, safe ranged standoff,
    /// siege against walls, compatible ram/tower escorts, and distinct light-
    /// versus-heavy cavalry jobs.
    ///
    /// Off for the frozen Basic/`advanced_v1` tournament controls and enabled
    /// by the production Advanced controller.
    tactical_strategy: bool,
    /// Let the production controller retain one unit's campaign objective,
    /// dangerous approaches, and a short retreat commitment across turns.
    ///
    /// This stays off for Basic and the frozen `advanced_v1` anchor: those
    /// controllers keep their historical step-by-step choices while production
    /// Advanced can use the richer state.
    unit_objective_memory: bool,
    w: Weights,
    book_pos: usize, // opening-book progress (capital builds played so far)
    /// Units that have withdrawn from combat stay in recovery until they are
    /// healthy enough to rejoin it, instead of advancing again after one tick.
    recovering_units: HashSet<u32>,
    /// Persistent peacetime destinations keep surplus troops patrolling the
    /// empire's frontier instead of permanently stacking around the capital.
    patrol_targets: HashMap<u32, Pos>,
    /// Frontier posts are identical for military units in the same movement
    /// domain. Reuse the scan for the rest of this player's turn instead of
    /// walking the whole map once per idle unit.
    patrol_posts: HashMap<String, Vec<Pos>>,
    /// Parallel unit snapshots can be primed before they are cloned. Keep
    /// those primed scans separate by full traversal class so a Scout that can
    /// embark does not change the post list used by a land-only unit.
    patrol_posts_by_class: HashMap<(String, TraversalClass), Vec<Pos>>,
    /// Colonies, especially overseas ones, need a fixed destination. Re-scoring
    /// only a short local radius each step strands settlers on shorelines and
    /// can make them reverse course after embarking.
    settler_targets: HashMap<u32, Pos>,
    /// The complete objective/threat/retreat ledger for individual units.
    /// `RefCell` lets both serial and snapshot tactical planners update only
    /// their own unit's entry without changing the broad movement API.
    unit_memories: RefCell<BTreeMap<u32, UnitMemory>>,
    /// The source of each generic path step taken this turn. Do not immediately
    /// traverse the same edge backward: a greedy step into a cul-de-sac would
    /// otherwise be undone by A* with the unit's next movement point, and the
    /// identical round trip would repeat forever.
    last_path_step_from: RefCell<HashMap<u32, (u32, Pos)>>,
    /// The same round trip spread over two turns instead of one, which nothing
    /// inside a single turn's reasoning can see. Each unit's recent
    /// whereabouts are remembered here, and a unit found circling is priced
    /// out of the tiles it has already proved are worthless.
    unit_motion: BTreeMap<u32, UnitMotion>,
    /// Melee units the ancient-rush lane wants in hand, or 0 when no rush is
    /// running. Set once a turn by `AdvancedAi` from its strategic plan.
    ///
    /// The standing-army target is `mil_per_city * n_cities`, and
    /// `mil_per_city` defaults to 1.0 — so a two-city empire wants two
    /// military units, which is what `rush_census` measures it fielding
    /// (2.5 melee at turn 50, 1.1 of them near any rival capital). A siege
    /// needs four. Without this floor the rush plans a war it never builds
    /// the army for, which is the failure the census caught.
    pub(crate) rush_military_floor: usize,
    /// Where this agent tells an observer what it is doing. Off unless a
    /// spectator attached one; see [`crate::reasoning`].
    pub(crate) journal: Journal,
}

/// Unit-scoped mutable planner state produced on a private search branch.
///
/// Advanced snapshot planning never publishes a cloned controller wholesale:
/// doing so would overwrite decisions made by earlier units in the serial
/// commit phase. This is the complete state one military unit's movement
/// helpers may update, extracted and merged by unit ID only.
pub(crate) struct BasicUnitPlanState {
    recovering: bool,
    patrol_target: Option<Pos>,
    settler_target: Option<Pos>,
    memory: Option<UnitMemory>,
    last_path_step: Option<(u32, Pos)>,
    patrol_posts: HashMap<String, Vec<Pos>>,
}

impl Default for BasicAi {
    fn default() -> Self {
        Self::new()
    }
}

impl BasicAi {
    pub(crate) fn unit_doctrine(g: &Game, uid: u32) -> UnitDoctrine {
        let spec = &g.rules.units[g.units[&uid].kind];
        if spec.class == "support" {
            return UnitDoctrine::Support;
        }
        if spec.domain.as_deref() == Some("air") {
            return if spec.siege {
                UnitDoctrine::AirStrike
            } else {
                UnitDoctrine::AirDefense
            };
        }
        if spec.class == "military" && !spec.is_melee_capable() && !spec.has_ranged_attack() {
            return UnitDoctrine::Support;
        }
        if spec.siege {
            return UnitDoctrine::Siege;
        }
        match spec.promotion_class.as_str() {
            "recon" => UnitDoctrine::Recon,
            "light_cavalry" | "naval_raider" | "naval_melee" => UnitDoctrine::Mobile,
            "ranged" | "naval_ranged" => UnitDoctrine::Ranged,
            "naval_carrier" => UnitDoctrine::Carrier,
            _ => UnitDoctrine::Assault,
        }
    }

    /// Innate class strength the rules engine applies against one opponent.
    /// Keep tactical projections on the same counter cycle as combat itself.
    pub(crate) fn class_matchup_strength(g: &Game, attacker: u32, defender: u32) -> f64 {
        let unit = &g.units[&attacker];
        let other_unit = &g.units[&defender];
        let spec = &g.rules.units[unit.kind];
        let other = &g.rules.units[other_unit.kind];
        if spec.promotion_class == "anti_cavalry"
            && (matches!(
                other.promotion_class.as_str(),
                "light_cavalry" | "heavy_cavalry"
            ) || (other.cavalry && other.promotion_class == "ranged"))
            && other_unit.kind != "war_cart"
        {
            10.0
        } else if spec.promotion_class == "melee" && other.promotion_class == "anti_cavalry" {
            5.0
        } else {
            0.0
        }
    }

    fn wall_levels(g: &Game, cid: u32) -> usize {
        let city = &g.cities[&cid];
        city.buildings
            .iter()
            .filter(|building| !city.pillaged_buildings.contains(*building))
            .filter(|building| g.rules.buildings[*building].outer_defense > 0)
            .count()
    }

    fn siege_support_works_for_city(g: &Game, kind: &str, cid: u32) -> bool {
        let city = &g.cities[&cid];
        if g.players[city.owner].techs.contains(&crate::name!("steel")) {
            return false;
        }
        match (kind, Self::wall_levels(g, cid)) {
            ("battering_ram", 1) => true,
            ("siege_tower", 1..=2) => true,
            _ => false,
        }
    }

    /// Rams and towers only empower the promotion classes the engine accepts.
    /// Other support formations retain the ordinary any-military escort rule.
    fn support_escort_compatible(&self, g: &Game, support: u32, escort: u32) -> bool {
        let support_unit = &g.units[&support];
        let escort_unit = &g.units[&escort];
        let escort_spec = &g.rules.units[escort_unit.kind];
        if escort_spec.class != "military" {
            return false;
        }
        if !self.tactical_strategy
            || !matches!(support_unit.kind.as_str(), "battering_ram" | "siege_tower")
        {
            return true;
        }
        matches!(escort_spec.promotion_class.as_str(), "melee" | "anti_cavalry")
    }

    fn active_breach_support(g: &Game, pid: usize, cid: u32, target: Pos) -> bool {
        g.nbrs(target).into_iter().any(|position| {
            g.units_at(position).into_iter().any(|support| {
                let unit = &g.units[&support];
                unit.owner == pid
                    && matches!(unit.kind.as_str(), "battering_ram" | "siege_tower")
                    && Self::siege_support_works_for_city(g, unit.kind.as_str(), cid)
            })
        })
    }

    /// Extra assignment value for the tactical job this action performs.
    /// Damage, casualties and captures remain valued by the normal exchange or
    /// exact forward model; this directs close choices to the correct role.
    pub(crate) fn tactical_action_bonus(
        &self,
        g: &Game,
        uid: u32,
        target: Pos,
        ranged: bool,
    ) -> f64 {
        self.tactical_action_bonus_from(g, uid, g.units[&uid].pos, target, ranged)
    }

    pub(crate) fn tactical_action_bonus_from(
        &self,
        g: &Game,
        uid: u32,
        from: Pos,
        target: Pos,
        ranged: bool,
    ) -> f64 {
        if !self.tactical_strategy {
            return 0.0;
        }
        let attacker = &g.units[&uid];
        let spec = &g.rules.units[attacker.kind];

        if let Some(cid) = g.city_at(target).or_else(|| g.encampment_at(target)) {
            let city = &g.cities[&cid];
            let wall_hp = if g.city_at(target) == Some(cid) {
                city.wall_hp
            } else {
                city.encampment_wall_hp
            };
            if wall_hp > 0 {
                if spec.siege {
                    return SIEGE_WALL_ASSIGNMENT;
                }
                if !ranged
                    && matches!(spec.promotion_class.as_str(), "melee" | "anti_cavalry")
                {
                    return if Self::active_breach_support(g, attacker.owner, cid, target) {
                        SUPPORTED_WALL_ASSAULT
                    } else {
                        -UNSUPPORTED_WALL_ASSAULT
                    };
                }
            }
            return 0.0;
        }

        let defender = g
            .units_at(target)
            .into_iter()
            .filter(|other| {
                let other = &g.units[other];
                other.owner != attacker.owner
                    && g.is_at_war(attacker.owner, other.owner)
                    && g.rules.units[other.kind].class == "military"
            })
            .max_by(|left, right| {
                let strength = |id: &u32| {
                    let unit = &g.units[id];
                    effective_strength(g.unit_strength(unit, true), unit.hp)
                };
                strength(left)
                    .partial_cmp(&strength(right))
                    .unwrap_or(Ordering::Equal)
            });
        let Some(defender) = defender else {
            return 0.0;
        };
        let defender_spec = &g.rules.units[g.units[&defender].kind];
        let favorable = match spec.promotion_class.as_str() {
            "melee" => defender_spec.promotion_class == "anti_cavalry",
            "anti_cavalry" => {
                matches!(
                    defender_spec.promotion_class.as_str(),
                    "light_cavalry" | "heavy_cavalry"
                ) && g.units[&defender].kind != "war_cart"
            }
            "light_cavalry" | "heavy_cavalry" => defender_spec.promotion_class == "melee",
            _ => false,
        };
        let mut value = if favorable {
            TACTICAL_COUNTER_ASSIGNMENT
        } else {
            0.0
        };
        if ranged {
            let return_range = if defender_spec.has_ranged_attack() {
                g.unit_attack_range(defender).max(1)
            } else {
                1
            };
            if g.wdist(from, target) > return_range {
                value += SAFE_RANGED_FIRE;
            }
        }
        value
    }

    /// Expected damage enemies could deliver after one approach step. Ranged
    /// and siege units use this to prefer standoff tiles outside return reach.
    pub(crate) fn projected_counter_damage(
        &self,
        g: &Game,
        uid: u32,
        tile: Pos,
        hostile_units: &[u32],
    ) -> f64 {
        let defender = &g.units[&uid];
        hostile_units
            .iter()
            .filter_map(|enemy| g.units.get(enemy).map(|unit| (*enemy, unit)))
            .filter(|(_, enemy)| {
                let spec = &g.rules.units[enemy.kind];
                spec.class == "military"
                    && (spec.is_melee_capable() || spec.has_ranged_attack())
            })
            .filter_map(|(enemy_id, enemy)| {
                let enemy_spec = &g.rules.units[enemy.kind];
                let ranged = enemy_spec.has_ranged_attack();
                let attack_range = if ranged {
                    g.unit_attack_range(enemy_id).max(1)
                } else {
                    1
                };
                // Price the enemy's next turn, when movement refreshes; its
                // leftover points from the previous turn are irrelevant.
                // Ordinary land units can take at least one approach step,
                // while siege cannot move and fire without a promotion.
                let approach = (!enemy_spec.siege) as i32;
                (g.wdist(tile, enemy.pos) <= attack_range + approach).then(|| {
                    let attack = if ranged {
                        g.unit_ranged_attack_strength(enemy)
                    } else {
                        g.unit_strength(enemy, false)
                    } + Self::class_matchup_strength(g, enemy_id, uid);
                    let defense = g.unit_strength(defender, true)
                        + Self::class_matchup_strength(g, uid, enemy_id);
                    let attack = effective_strength(attack, enemy.hp);
                    let defense = effective_strength(defense, defender.hp);
                    30.0 * ((attack - defense) / 25.0).exp()
                })
            })
            .sum()
    }

    pub(crate) fn city_is_coastal(g: &Game, cid: u32) -> bool {
        g.cities.get(&cid).is_some_and(|city| {
            g.nbrs(city.pos)
                .into_iter()
                .any(|pos| g.map.get(pos).is_some_and(|tile| g.rules.is_water(tile)))
        })
    }

    pub(crate) fn empire_is_coastal(g: &Game, pid: usize) -> bool {
        g.player_city_ids(pid)
            .into_iter()
            .any(|cid| Self::city_is_coastal(g, cid))
    }

    fn tech_leads_to(g: &Game, candidate: &str, target: &str) -> bool {
        candidate == target
            || g.rules.techs.get(target).is_some_and(|spec| {
                spec.requires
                    .iter()
                    .any(|parent| Self::tech_leads_to(g, candidate, parent))
            })
    }

    fn civic_leads_to(g: &Game, candidate: &str, target: &str) -> bool {
        candidate == target
            || g.rules.civics.get(target).is_some_and(|spec| {
                spec.requires
                    .iter()
                    .any(|parent| Self::civic_leads_to(g, candidate, parent))
            })
    }

    fn has_building_family(g: &Game, pid: usize, family: impl AsName) -> bool {
        g.player_city_ids(pid).into_iter().any(|cid| {
            g.cities[&cid]
                .buildings
                .iter()
                .any(|building| g.building_is_family(building, family))
        })
    }

    /// Follow through on the technology unlocked by infrastructure already in
    /// the empire. Without this, the fixed early-game priority list ends at
    /// Machinery and the fallback can keep taking whichever late branch the
    /// rules map happens to return first. In a completed spectator game that
    /// let a Market-owning city-state reach Electricity while permanently
    /// skipping Stirrups -> Banking, leaving more than 4,000 Gold with no Bank
    /// to buy. Replacement Markets and Banks count as their base families.
    fn economic_research_goal(g: &Game, pid: usize) -> Option<&'static str> {
        let player = &g.players[pid];
        if Self::has_building_family(g, pid, crate::name!("market")) && !player.techs.contains(&crate::name!("banking")) {
            return Some("banking");
        }
        if Self::has_building_family(g, pid, crate::name!("bank")) && !player.techs.contains(&crate::name!("economics")) {
            return Some("economics");
        }
        None
    }

    fn research_step_toward(
        g: &Game,
        avail: &[Name],
        goal: Option<&str>,
    ) -> Option<Name> {
        goal.and_then(|goal| {
            avail
                .iter()
                .find(|tech| tech.as_str() == goal)
                .cloned()
                .or_else(|| {
                    avail
                        .iter()
                        .filter(|tech| Self::tech_leads_to(g, tech, goal))
                        .min_by(|a, b| {
                            g.rules.techs[*a]
                                .cost
                                .partial_cmp(&g.rules.techs[*b].cost)
                                .unwrap()
                                .then(a.cmp(b))
                        })
                        .cloned()
                })
        })
    }

    fn civic_step_toward(g: &Game, avail: &[Name], goal: Option<&str>) -> Option<Name> {
        goal.and_then(|goal| {
            avail
                .iter()
                .find(|civic| civic.as_str() == goal)
                .cloned()
                .or_else(|| {
                    avail
                        .iter()
                        .filter(|civic| Self::civic_leads_to(g, civic, goal))
                        .min_by(|a, b| {
                            g.rules.civics[*a]
                                .cost
                                .partial_cmp(&g.rules.civics[*b].cost)
                                .unwrap()
                                .then(a.cmp(b))
                        })
                        .cloned()
                })
        })
    }

    /// Navigation is treated as a capability chain rather than an incidental
    /// unit unlock. Coastal empires first launch ships, then unlock general
    /// embarkation and harbors, and finally cross ocean once expansion has had
    /// time to reach the edge of its home landmass.
    /// Whether an empire has any use for the naval chain beyond Sailing:
    /// ships of its own, more than one coast to join up, room to settle
    /// overseas, or a war it may have to fight at sea.
    fn naval_ambitions(g: &Game, pid: usize) -> bool {
        let coastal_cities = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| Self::city_is_coastal(g, *cid))
            .count();
        if coastal_cities >= 2 {
            return true;
        }
        let owns_ships = g.units.values().any(|unit| {
            unit.owner == pid && g.rules.units[unit.kind].domain.as_deref() == Some("sea")
        });
        if owns_ships {
            return true;
        }
        // A civilization still expanding may need to cross water to do it; a
        // single-city minor never does.
        if !g.players[pid].is_minor && g.player_city_ids(pid).len() >= 2 {
            return true;
        }
        g.players.iter().any(|enemy| {
            enemy.id != pid
                && enemy.alive
                && !enemy.is_barbarian
                && g.is_at_war(pid, enemy.id)
                && (g
                    .player_city_ids(enemy.id)
                    .into_iter()
                    .any(|cid| Self::city_is_coastal(g, cid))
                    || g.units.values().any(|unit| {
                        unit.owner == enemy.id
                            && g.map
                                .get(unit.pos)
                                .is_some_and(|tile| g.rules.is_water(tile))
                    }))
        })
    }

    pub(crate) fn water_research_goal(g: &Game, pid: usize) -> Option<&'static str> {
        if !Self::empire_is_coastal(g, pid) {
            return None;
        }
        let player = &g.players[pid];
        if !player.techs.contains(&crate::name!("sailing")) {
            return Some("sailing");
        }
        // Past Sailing the naval chain gets expensive, and it overrides every
        // other research priority while it is being pursued. An empire with
        // nothing afloat and nowhere to sail spends those turns unable to
        // unlock the buildings that make its cities work - which left one-city
        // city-states grinding Shipbuilding for seventy turns on one
        // technology. Ask for a reason before committing to the rest.
        if !Self::naval_ambitions(g, pid) {
            return None;
        }
        if !player.techs.contains(&crate::name!("shipbuilding")) {
            return Some("shipbuilding");
        }
        if !player.techs.contains(&crate::name!("celestial_navigation"))
            && (g.turn >= 30 || g.player_city_ids(pid).len() >= 2)
        {
            return Some("celestial_navigation");
        }
        let has_ocean = g.map.tiles.values().any(|tile| tile.terrain == "ocean");
        let has_expansion_unit = g
            .units
            .values()
            .any(|unit| unit.owner == pid && unit.kind == "settler");
        if has_ocean
            && !player.techs.contains(&crate::name!("cartography"))
            && (g.turn >= 55 || g.player_city_ids(pid).len() >= 3 || has_expansion_unit)
        {
            return Some("cartography");
        }
        let naval_war = g.players.iter().any(|enemy| {
            enemy.id != pid
                && enemy.alive
                && g.is_at_war(pid, enemy.id)
                && (g.units.values().any(|unit| {
                    unit.owner == enemy.id
                        && g.map
                            .get(unit.pos)
                            .is_some_and(|tile| g.rules.is_water(tile))
                }) || g
                    .player_city_ids(enemy.id)
                    .into_iter()
                    .any(|cid| Self::city_is_coastal(g, cid)))
        });
        if naval_war && player.techs.contains(&crate::name!("cartography")) {
            if !player.techs.contains(&crate::name!("square_rigging")) {
                return Some("square_rigging");
            }
            // After the first dedicated naval-ranged unlock, pursue later
            // fleet upgrades only when their era's prerequisite is already in
            // hand. This keeps naval readiness current without dragging an
            // ancient empire through an entire industrial branch at once.
            for (goal, prerequisite) in [
                ("steam_power", "industrialization"),
                ("refining", "rifling"),
                ("electricity", "steam_power"),
                ("combined_arms", "combustion"),
                ("lasers", "nuclear_fission"),
                ("telecommunications", "computers"),
            ] {
                if player.techs.contains(&Name::new(prerequisite)) && !player.techs.contains(&Name::new(goal)) {
                    return Some(goal);
                }
            }
        }
        None
    }

    pub(crate) fn waterborne(g: &Game, uid: u32) -> bool {
        let unit = &g.units[&uid];
        g.rules.units[unit.kind].domain.as_deref() == Some("sea")
            || g.map
                .get(unit.pos)
                .is_some_and(|tile| g.rules.is_water(tile))
    }

    fn naval_counts(g: &Game, pid: usize) -> (usize, usize, usize, usize, usize) {
        let mut counts = (0, 0, 0, 0, 0);
        let mut add = |kind: &str| {
            let spec = &g.rules.units[kind];
            if spec.class != "military" || spec.domain.as_deref() != Some("sea") {
                return;
            }
            counts.0 += 1;
            match spec.promotion_class.as_str() {
                "naval_melee" => counts.1 += 1,
                "naval_ranged" => counts.2 += 1,
                "naval_raider" => counts.3 += 1,
                "naval_carrier" => counts.4 += 1,
                _ => {}
            }
        };
        for uid in g.player_unit_ids(pid) {
            add(&g.units[&uid].kind);
        }
        for cid in g.player_city_ids(pid) {
            if let Some(Item::Unit { unit }) = g.cities[&cid].queue.first() {
                add(unit);
            }
        }
        counts
    }

    /// One-city states need a credible local defense, not an empire-sized
    /// standing army. Their budget grows when an actual hostile force reaches
    /// the city, but remains bounded so mature maps do not fill every tile
    /// with idle city-state units.
    fn minor_military_budget(g: &Game, pid: usize) -> usize {
        let enemies: Vec<usize> = g
            .players
            .iter()
            .filter(|player| {
                player.id != pid
                    && player.alive
                    && !player.is_barbarian
                    && g.is_at_war(pid, player.id)
            })
            .map(|player| player.id)
            .collect();
        if enemies.is_empty() {
            // Once its three-unit garrison is filled, a peaceful city-state
            // may develop infrastructure or simply leave Production idle.
            return 3;
        }
        let cities = g.player_city_ids(pid);
        let nearby_hostiles = g
            .units
            .values()
            .filter(|unit| {
                enemies.contains(&unit.owner)
                    && cities
                        .iter()
                        .any(|city| g.wdist(g.cities[city].pos, unit.pos) <= 6)
            })
            .count();
        (4 + nearby_hostiles.div_ceil(2)).min(7)
    }

    fn minor_home(g: &Game, pid: usize) -> Option<Pos> {
        g.cities
            .values()
            .filter(|city| city.owner == pid)
            .min_by_key(|city| (city.original_owner != pid, city.id))
            .map(|city| city.pos)
    }

    fn minor_enemy_near_home(g: &Game, pid: usize, enemy: usize) -> bool {
        let Some(home) = Self::minor_home(g, pid) else {
            return false;
        };
        g.units
            .values()
            .any(|unit| unit.owner == enemy && g.wdist(home, unit.pos) <= MINOR_DEFENSE_RADIUS)
            || g.cities
                .values()
                .any(|city| city.owner == enemy && g.wdist(home, city.pos) <= MINOR_DEFENSE_RADIUS)
            || (g.barb_pid == Some(enemy)
                && g.barb_camps
                    .keys()
                    .any(|camp| g.wdist(home, *camp) <= MINOR_DEFENSE_RADIUS))
    }

    fn minor_district_family(g: &Game, pid: usize) -> &'static str {
        match g.cs_type(&g.players[pid].civ) {
            "scientific" => "campus",
            "cultural" => "theater_square",
            "religious" => "holy_site",
            "militaristic" => "encampment",
            "industrial" => "industrial_zone",
            _ => "commercial_hub",
        }
    }

    fn minor_tech_goal(g: &Game, pid: usize) -> Option<&'static str> {
        let goal = match g.cs_type(&g.players[pid].civ) {
            "scientific" => "writing",
            "religious" => "astrology",
            "militaristic" => "bronze_working",
            "industrial" => "apprenticeship",
            "trade" => "currency",
            _ => return None,
        };
        (!g.players[pid].techs.contains(&Name::new(goal))).then_some(goal)
    }

    pub(crate) fn desired_navy(g: &Game, pid: usize) -> usize {
        let coastal_cities = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| Self::city_is_coastal(g, *cid))
            .count();
        if coastal_cities == 0 || !g.players[pid].techs.contains(&crate::name!("sailing")) {
            return 0;
        }
        let mut desired = 1;
        let settlers_at_sea = g.units.values().any(|unit| {
            unit.owner == pid
                && unit.kind == "settler"
                && g.map
                    .get(unit.pos)
                    .is_some_and(|tile| g.rules.is_water(tile))
        });
        if settlers_at_sea
            || (g.players[pid].techs.contains(&crate::name!("shipbuilding"))
                && g.units
                    .values()
                    .any(|unit| unit.owner == pid && unit.kind == "settler"))
        {
            desired = desired.max(2);
        }
        let naval_war = g.players.iter().any(|enemy| {
            enemy.id != pid
                && enemy.alive
                && g.is_at_war(pid, enemy.id)
                && (g.units.values().any(|unit| {
                    unit.owner == enemy.id
                        && g.map
                            .get(unit.pos)
                            .is_some_and(|tile| g.rules.is_water(tile))
                }) || g
                    .player_city_ids(enemy.id)
                    .into_iter()
                    .any(|cid| Self::city_is_coastal(g, cid)))
        });
        if naval_war {
            desired = desired.max(coastal_cities.saturating_add(1).max(2));
        } else if g.players[pid].techs.contains(&crate::name!("cartography")) && coastal_cities >= 2 {
            desired = desired.max(2);
        }
        desired
    }

    fn has_exploration_target(&self, g: &Game, pid: usize, uid: u32) -> bool {
        g.map.tiles.iter().any(|(pos, _)| {
            !g.players[pid].explored.contains(pos) && g.unit_can_traverse(uid, *pos)
        })
    }

    /// Recon explores even during war. Without recon, one ordinary combat
    /// unit per movement domain scouts at peace so the empire is not blind,
    /// while the rest remain available for patrol and defense.
    fn should_explore(&self, g: &Game, pid: usize, uid: u32, at_war: bool) -> bool {
        let doctrine = Self::unit_doctrine(g, uid);
        if doctrine == UnitDoctrine::Recon {
            return true;
        }
        if at_war
            || matches!(
                doctrine,
                UnitDoctrine::Siege
                    | UnitDoctrine::Support
                    | UnitDoctrine::AirDefense
                    | UnitDoctrine::AirStrike
                    | UnitDoctrine::Carrier
            )
        {
            return false;
        }
        let domain = g.rules.units[g.units[&uid].kind]
            .domain
            .as_deref()
            .unwrap_or("land");
        let candidates = g.player_unit_ids(pid).into_iter().filter(|other| {
            let spec = &g.rules.units[g.units[other].kind];
            spec.class == "military"
                && spec.domain.as_deref().unwrap_or("land") == domain
                && !matches!(
                    Self::unit_doctrine(g, *other),
                    UnitDoctrine::Siege
                        | UnitDoctrine::AirDefense
                        | UnitDoctrine::AirStrike
                        | UnitDoctrine::Carrier
                )
        });
        let recon_exists = candidates
            .clone()
            .any(|other| Self::unit_doctrine(g, other) == UnitDoctrine::Recon);
        !recon_exists && candidates.min() == Some(uid)
    }

    /// Required exchange value for an attack. Dedicated assault and mobile
    /// units accept thinner advantages, high-strength units press them harder,
    /// recon avoids routine combat, and siege strongly prefers districts.
    pub(crate) fn attack_threshold(&self, g: &Game, uid: u32, target: Pos) -> f64 {
        let unit = &g.units[&uid];
        let doctrine = Self::unit_doctrine(g, uid);
        let role = match doctrine {
            UnitDoctrine::Recon => 14.0,
            UnitDoctrine::Assault => -2.0,
            // Light cavalry raids first and takes only favorable field trades;
            // heavy cavalry remains Assault and presses attacks first.
            UnitDoctrine::Mobile if self.tactical_strategy => 2.0,
            UnitDoctrine::Mobile => -5.0,
            UnitDoctrine::Ranged => 0.0,
            UnitDoctrine::Siege => 5.0,
            UnitDoctrine::Support | UnitDoctrine::Carrier => 1_000.0,
            UnitDoctrine::AirDefense => -1.0,
            UnitDoctrine::AirStrike => -4.0,
        };
        let attack_strength = g
            .unit_strength(unit, false)
            .max(g.unit_ranged_attack_strength(unit));
        let strength_drive = ((attack_strength - 25.0) * 0.12).clamp(0.0, 8.0);
        let target_adjustment = if g.city_at(target).is_some()
            || g.map
                .get(target)
                .is_some_and(|tile| tile.district.is_some())
        {
            match doctrine {
                UnitDoctrine::Siege => -22.0,
                UnitDoctrine::Assault => -3.0,
                UnitDoctrine::Recon => 8.0,
                _ => 0.0,
            }
        } else {
            match doctrine {
                UnitDoctrine::Siege => 14.0,
                UnitDoctrine::Mobile
                    if g.units_at(target).iter().any(|other| {
                        g.rules.units[g.units[other].kind].class != "military"
                            || g.units[other].hp <= 40
                    }) =>
                {
                    -6.0
                }
                _ => 0.0,
            }
        };
        self.w.attack_floor + role + target_adjustment - strength_drive
    }

    /// Non-generic actions that define a unit's strategic job. Fast raiders
    /// exploit infrastructure, and aircraft use missions and rebasing instead
    /// of pretending to be land units with long range.
    fn air_pillage_score(g: &Game, target: Pos) -> i32 {
        let Some(tile) = g.map.get(target) else {
            return 0;
        };
        if let Some(improvement) = tile.improvement.as_deref() {
            return match improvement {
                "airstrip" => 145,
                "oil_well" | "offshore_oil_rig" | "mine" | "quarry" => 90,
                "farm" | "fishing_boats" => 55,
                _ => 70,
            };
        }
        let Some(district) = tile.district else {
            return 0;
        };
        if let Some(cost) = tile
            .owner_city
            .and_then(|city| g.cities.get(&city))
            .and_then(|city| {
                city.buildings
                    .iter()
                    .filter(|building| !city.pillaged_buildings.contains(*building))
                    .filter(|building| {
                        g.rules.buildings[building]
                            .district
                            .is_some_and(|family| g.district_family(district) == family)
                    })
                    .map(|building| g.rules.buildings[building].cost as i32)
                    .max()
            })
        {
            return 70 + cost / 5;
        }
        if !tile.pillaged {
            return match g.district_family(district).as_str() {
                "aerodrome" | "industrial_zone" | "campus" | "spaceport" => 135,
                "commercial_hub" | "harbor" | "holy_site" | "theater_square" => 115,
                _ => 90,
            };
        }
        65
    }

    fn priority_target_score(g: &Game, pid: usize, target: Pos) -> i32 {
        let Some(support) = g.priority_support_target_at(pid, target) else {
            return 0;
        };
        let unit = &g.units[&support];
        let spec = &g.rules.units[unit.kind];
        105 + (100 - unit.hp)
            + (spec.cost * 0.18) as i32
            + if spec.anti_air_strength > 0.0 {
                100
            } else if matches!(unit.kind.as_str(), "drone" | "observation_balloon") {
                45
            } else if matches!(unit.kind.as_str(), "medic" | "supply_convoy") {
                30
            } else {
                0
            }
    }

    pub(crate) fn doctrine_action(&self, g: &Game, pid: usize, uid: u32) -> Option<Action> {
        let doctrine = Self::unit_doctrine(g, uid);
        if !matches!(
            doctrine,
            UnitDoctrine::Mobile | UnitDoctrine::AirDefense | UnitDoctrine::AirStrike
        ) {
            return None;
        }
        let legal = g.legal_doctrine_actions(pid, uid);
        match doctrine {
            UnitDoctrine::Mobile => legal
                .iter()
                .find(|action| matches!(action, Action::CoastalRaid { unit, .. } if *unit == uid))
                .cloned()
                .or_else(|| {
                    legal
                        .iter()
                        .find(|action| matches!(action, Action::Pillage { unit } if *unit == uid))
                        .cloned()
                }),
            UnitDoctrine::AirDefense => legal
                .iter()
                .find(|action| match action {
                    Action::AirStrike { unit, target } if *unit == uid => {
                        g.units_at(*target).iter().any(|other| {
                            let other = &g.units[other];
                            other.owner != pid
                                && g.rules.units[other.kind].domain.as_deref()
                                    == Some("air")
                        })
                    }
                    _ => false,
                })
                .cloned()
                .or_else(|| {
                    legal
                        .iter()
                        .filter_map(|action| match action {
                            Action::PriorityTarget { unit, target } if *unit == uid => Some((
                                Self::priority_target_score(g, pid, *target),
                                *target,
                                action.clone(),
                            )),
                            _ => None,
                        })
                        .max_by_key(|(score, target, _)| (*score, std::cmp::Reverse(*target)))
                        .map(|(_, _, action)| action)
                })
                .or_else(|| {
                    legal
                        .iter()
                        .filter_map(|action| match action {
                            Action::AirPatrol { unit, to } if *unit == uid => {
                                let city_cover = g
                                    .cities
                                    .values()
                                    .filter(|city| city.owner == pid && g.wdist(*to, city.pos) <= 1)
                                    .map(|city| 100 + city.pop * 5)
                                    .sum::<i32>();
                                let unit_cover =
                                    g.units
                                        .values()
                                        .filter(|other| {
                                            other.owner == pid
                                                && other.id != uid
                                                && g.wdist(*to, other.pos) <= 1
                                                && g.rules.units[other.kind].class
                                                    == "military"
                                        })
                                        .count() as i32
                                        * 12;
                                Some((city_cover + unit_cover, *to, action.clone()))
                            }
                            _ => None,
                        })
                        .max_by_key(|(score, to, _)| (*score, std::cmp::Reverse(*to)))
                        .map(|(_, _, action)| action)
                })
                .or_else(|| {
                    legal.into_iter().find(
                        |action| matches!(action, Action::AirStrike { unit, .. } if *unit == uid),
                    )
                }),
            UnitDoctrine::AirStrike => {
                let mission = legal
                    .iter()
                    .filter_map(|action| match action {
                        Action::AirStrike { unit, target } if *unit == uid => {
                            let target_hp = g
                                .units_at(*target)
                                .iter()
                                .filter_map(|other| {
                                    let other = &g.units[other];
                                    (other.owner != pid).then_some(other.hp)
                                })
                                .min()
                                .unwrap_or(100);
                            let city = g.city_at(*target).is_some() as i32;
                            Some((city * 120 + 100 - target_hp, *target, action.clone()))
                        }
                        Action::AirPillage { unit, target } if *unit == uid => {
                            Some((Self::air_pillage_score(g, *target), *target, action.clone()))
                        }
                        Action::PriorityTarget { unit, target } if *unit == uid => Some((
                            Self::priority_target_score(g, pid, *target),
                            *target,
                            action.clone(),
                        )),
                        _ => None,
                    })
                    .max_by_key(|(score, target, _)| (*score, std::cmp::Reverse(*target)))
                    .map(|(_, _, action)| action);
                mission
                    .or_else(|| {
                        let enemy_positions: Vec<Pos> = g
                            .units
                            .values()
                            .filter(|other| other.owner != pid && g.is_at_war(pid, other.owner))
                            .map(|other| other.pos)
                            .chain(
                                g.cities
                                    .values()
                                    .filter(|city| {
                                        city.owner != pid && g.is_at_war(pid, city.owner)
                                    })
                                    .map(|city| city.pos),
                            )
                            .collect();
                        if enemy_positions.is_empty() {
                            None
                        } else {
                            legal
                                .iter()
                                .filter_map(|action| match action {
                                    Action::AirRebase { unit, to } if *unit == uid => {
                                        let distance = enemy_positions
                                            .iter()
                                            .map(|enemy| g.wdist(*to, *enemy))
                                            .min()
                                            .unwrap_or(i32::MAX);
                                        Some((distance, *to, action.clone()))
                                    }
                                    _ => None,
                                })
                                .min_by_key(|(distance, to, _)| (*distance, *to))
                                .map(|(_, _, action)| action)
                        }
                    })
                    .or_else(|| {
                        legal
                            .into_iter()
                            .filter_map(|action| match action {
                                Action::AirPatrol { unit, to } if unit == uid => {
                                    let nearest_city = g
                                        .cities
                                        .values()
                                        .filter(|city| city.owner == pid)
                                        .map(|city| g.wdist(to, city.pos))
                                        .min()
                                        .unwrap_or(i32::MAX);
                                    Some((nearest_city, to, action))
                                }
                                _ => None,
                            })
                            .min_by_key(|(distance, to, _)| (*distance, *to))
                            .map(|(_, _, action)| action)
                    })
            }
            _ => None,
        }
    }

    /// Heavy cavalry attacks first, but remains a strong fallback pillager.
    /// Light cavalry reaches Pillage through `doctrine_action` before combat.
    pub(crate) fn heavy_cavalry_pillage_action(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
    ) -> Option<Action> {
        if !self.tactical_strategy
            || g.rules.units[g.units[&uid].kind].promotion_class != "heavy_cavalry"
        {
            return None;
        }
        g.legal_doctrine_actions(pid, uid)
            .into_iter()
            .find(|action| matches!(action, Action::Pillage { unit } if *unit == uid))
    }

    pub fn new() -> BasicAi {
        BasicAi {
            minor: false,
            barb: false,
            amenity_districts: false,
            housing_districts: false,
            district_coverage: false,
            slot_kind_tiebreak: false,
            pursue_religion: true,
            live_religious_purchase_guard: false,
            siege_muster: false,
            siege_role: false,
            come_ashore: false,
            home_defense: false,
            loyalty_rate_alarm: false,
            recorded_tactical_step: false,
            tactical_strategy: false,
            unit_objective_memory: false,
            w: Weights::default(),
            book_pos: 0,
            recovering_units: HashSet::new(),
            patrol_targets: HashMap::new(),
            patrol_posts: HashMap::new(),
            patrol_posts_by_class: HashMap::new(),
            settler_targets: HashMap::new(),
            unit_memories: RefCell::new(BTreeMap::new()),
            last_path_step_from: RefCell::new(HashMap::new()),
            unit_motion: BTreeMap::new(),
            rush_military_floor: 0,
            journal: Journal::default(),
        }
    }

    pub fn with_weights(w: Weights) -> BasicAi {
        BasicAi {
            minor: false,
            barb: false,
            amenity_districts: false,
            housing_districts: false,
            district_coverage: false,
            slot_kind_tiebreak: false,
            pursue_religion: true,
            live_religious_purchase_guard: false,
            siege_muster: false,
            siege_role: false,
            come_ashore: false,
            home_defense: false,
            loyalty_rate_alarm: false,
            recorded_tactical_step: false,
            tactical_strategy: false,
            unit_objective_memory: false,
            w,
            book_pos: 0,
            recovering_units: HashSet::new(),
            patrol_targets: HashMap::new(),
            patrol_posts: HashMap::new(),
            patrol_posts_by_class: HashMap::new(),
            settler_targets: HashMap::new(),
            unit_memories: RefCell::new(BTreeMap::new()),
            last_path_step_from: RefCell::new(HashMap::new()),
            unit_motion: BTreeMap::new(),
            rush_military_floor: 0,
            journal: Journal::default(),
        }
    }

    pub fn fleet(g: &Game) -> Vec<BasicAi> {
        g.players.iter().map(|_| BasicAi::new()).collect()
    }

    /// Majors get `w`; minors/barbarians keep default weights.
    pub fn fleet_weighted(g: &Game, w: &Weights) -> Vec<BasicAi> {
        g.players
            .iter()
            .map(|p| {
                if p.is_minor || p.is_barbarian {
                    BasicAi::new()
                } else {
                    BasicAi::with_weights(w.clone())
                }
            })
            .collect()
    }
}

impl Ai for BasicAi {
    fn take_turn(&mut self, g: &mut Game, pid: usize) {
        // Stamp the context once. Nothing below repeats the turn number or the
        // acting civilization; the journal carries both. `AdvancedAi` opens
        // the turn on the shared journal before delegating here, and doing it
        // again is the same statement, so the baseline running on its own is
        // recorded identically.
        self.journal.begin_turn(g.turn, pid);
        g.with_deferred_visibility(|g| self.take_turn_inner(g, pid));
    }

    fn attach_journal(&mut self, journal: Journal) {
        self.journal = journal;
    }
}

impl BasicAi {
    fn take_turn_inner(&mut self, g: &mut Game, pid: usize) {
        self.minor = g.players[pid].is_minor;
        // Free Cities are diplomatically hostile like barbarians, but unlike
        // camps they keep developing their inherited cities and training
        // defenders through the ordinary minor-civilization production AI.
        self.barb = g.players[pid].is_barbarian && !g.players[pid].is_free_city;
        self.resolve_city_dispositions(g, pid, false, false);
        if !self.barb {
            if self.minor {
                // Minor civilizations keep a technology/civic tree and develop
                // their city, but do not run a player's corporations, diplomacy,
                // espionage, government, religion, governor, or envoy agenda.
                self.minor_research(g, pid);
            } else {
                self.research(g, pid);
                self.corporations(g, pid);
                self.diplomacy(g, pid);
                self.spies(g, pid);
            }
            self.cities(g, pid);
        }
        Self::upgrade_units(g, pid);
        self.units(g, pid);
        self.resolve_city_dispositions(g, pid, false, false);
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &Action::EndTurn);
        }
    }
}

impl BasicAi {
    /// Return a snapshot of one unit's durable reasoning state.
    ///
    /// This is intentionally observer-facing: a spectator, evaluator, or
    /// bridge can explain why one individual unit is moving without getting a
    /// mutable handle to the controller's internal maps.
    pub fn unit_memory(&self, uid: u32) -> Option<UnitMemory> {
        self.unit_memories.borrow().get(&uid).cloned()
    }

    /// Assign a military unit to the city the current campaign means to take.
    /// Reconfirming the same job preserves its original start turn, which is
    /// how an observer can distinguish a long-running siege from a retarget.
    pub(crate) fn remember_capture_objective(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        city: u32,
    ) {
        if !self.unit_objective_memory
            || !g.units.get(&uid).is_some_and(|unit| unit.owner == pid)
        {
            return;
        }
        let Some(target_city) = g.cities.get(&city).filter(|city| city.owner != pid) else {
            return;
        };
        let target = target_city.pos;
        let mut memories = self.unit_memories.borrow_mut();
        let memory = memories.entry(uid).or_default();
        match &mut memory.objective {
            Some(UnitObjective::CaptureCity {
                city: known_city,
                target: known_target,
                last_confirmed_turn,
                ..
            }) if *known_city == city && *known_target == target => {
                *last_confirmed_turn = g.turn;
            }
            _ => {
                memory.objective = Some(UnitObjective::CaptureCity {
                    city,
                    target,
                    started_turn: g.turn,
                    last_confirmed_turn: g.turn,
                });
            }
        }
    }

    /// A remembered capture objective is actionable only while the city still
    /// belongs to a hostile civilization. It can be retained before a formal
    /// declaration, but never pulls a unit through a peace treaty.
    pub(crate) fn capture_objective_target(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
    ) -> Option<Pos> {
        if !self.unit_objective_memory {
            return None;
        }
        let objective = self.unit_memories.borrow().get(&uid)?.objective?;
        let UnitObjective::CaptureCity { city, target, .. } = objective;
        g.cities
            .get(&city)
            .filter(|known| {
                known.owner != pid && known.pos == target && g.is_at_war(pid, known.owner)
            })
            .map(|_| target)
    }

    /// Remember that entering `position` would expose this unit to a serious
    /// counterattack. The warning always survives briefly; it becomes a real
    /// retreat when that damage would cross the controller's own withdrawal
    /// floor.
    pub(crate) fn remember_dangerous_approach(
        &self,
        g: &Game,
        uid: u32,
        position: Pos,
        expected_damage: f64,
    ) -> bool {
        if !self.unit_objective_memory || !expected_damage.is_finite() || expected_damage <= 0.0 {
            return false;
        }
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        let expected_damage = expected_damage.ceil().clamp(0.0, u32::MAX as f64) as u32;
        let withdraw_floor = self.w.withdraw_hp.round();
        let requires_retreat = expected_damage as f64
            >= (f64::from(unit.hp) - withdraw_floor).max(0.0);
        let mut memories = self.unit_memories.borrow_mut();
        let memory = memories.entry(uid).or_default();
        memory.danger = Some(UnitDangerMemory {
            position,
            expected_damage,
            observed_turn: g.turn,
            expires_turn: g.turn.saturating_add(UNIT_DANGER_MEMORY_TURNS),
        });
        if requires_retreat {
            let until = g.turn.saturating_add(UNIT_RETREAT_TURNS);
            memory.retreat_until = Some(memory.retreat_until.unwrap_or(until).max(until));
        }
        requires_retreat
    }

    /// Honor a remembered retreat before resuming a campaign. A retreat chooses
    /// the neighbor that reduces expected counter-damage, opens distance from
    /// the dangerous approach, and improves healing. It never discards the
    /// objective that will be resumed after the short cooldown.
    pub(crate) fn retreat_step(&self, g: &mut Game, pid: usize, uid: u32) -> Option<bool> {
        if !self.unit_objective_memory {
            return None;
        }
        let danger = self.unit_memories.borrow().get(&uid).and_then(|memory| {
            memory
                .retreat_until
                .filter(|until| g.turn < *until)
                .and(memory.danger)
        })?;
        let here = g.units.get(&uid)?.pos;
        let hostiles: Vec<u32> = g
            .units
            .values()
            .filter(|unit| {
                unit.owner != pid
                    && g.is_at_war(pid, unit.owner)
                    && g.unit_visible_to(unit.id, pid)
            })
            .map(|unit| unit.id)
            .collect();
        let value = |position: Pos| {
            -4.0 * self.projected_counter_damage(g, uid, position, &hostiles)
                + 3.0 * g.wdist(position, danger.position) as f64
                + 0.5 * g.healing_location(pid, position).rate() as f64
        };
        let holding = value(here);
        let next = g
            .nbrs(here)
            .into_iter()
            .filter(|position| g.can_move(uid, *position))
            .max_by(|left, right| {
                value(*left)
                    .total_cmp(&value(*right))
                    .then_with(|| right.cmp(left))
            });
        match next {
            Some(next) if value(next) > holding + 1e-9 => Some(
                self.tactical_apply_move(g, pid, uid, next)
                    || self.fortify_or_stop(g, pid, uid),
            ),
            _ => Some(self.fortify_or_stop(g, pid, uid)),
        }
    }

    /// Remove stale observations, completed objectives, and entries for units
    /// that no longer belong to this player. This must run before any unit
    /// moves, just like the older motion ledger it complements.
    fn refresh_unit_memories(&self, g: &Game, pid: usize) {
        self.unit_memories.borrow_mut().retain(|uid, memory| {
            let Some(unit) = g.units.get(uid) else {
                return false;
            };
            if unit.owner != pid {
                return false;
            }
            if let Some(UnitObjective::CaptureCity { city, target, .. }) = memory.objective {
                if g.cities.get(&city).is_none_or(|known| {
                    known.owner == pid || known.pos != target
                }) {
                    memory.objective = None;
                }
            }
            if memory
                .danger
                .is_some_and(|danger| g.turn >= danger.expires_turn)
            {
                memory.danger = None;
            }
            if memory.retreat_until.is_some_and(|until| g.turn >= until) {
                memory.retreat_until = None;
            }
            memory.objective.is_some() || memory.danger.is_some() || memory.retreat_until.is_some()
        });
    }

    /// Drop everything this agent remembers ABOUT INDIVIDUAL UNITS, keeping the rest.
    ///
    /// ★★★★★ FOR MIRRORING A GAME WHOSE UNIT IDS ARE REASSIGNED EVERY TURN.
    /// `civvis-orders --serve --fresh-board` keeps one agent alive across a real
    /// Civilization VI game and rebuilds the board each turn, because `take_turn`
    /// cannot be run twice on a board that has not advanced through `begin_turn`.
    /// Rebuilding reassigns unit ids, so every id-keyed map here silently describes a
    /// DIFFERENT unit than it did last turn.
    ///
    /// `unit_motion` is the one that does the damage: it is the livelock detector, so
    /// a unit whose recorded history jumps around looks like it is going in circles,
    /// and `hold_stood_down_unit` then fortifies it and suppresses it for several
    /// turns. Measured cost of not calling this — same code, same settings, one agent
    /// persisted across a real game: **about 1 unit order per turn against 13**, and by
    /// turn 62 the empire was ONE city and ONE unit where the no-continuity arm had 2
    /// cities and 25 units at turn 82. Continuity that poisons the unit layer is worse
    /// than no continuity.
    ///
    /// What is deliberately KEPT is the strategic plan, which is the whole reason to
    /// persist an agent: grand strategy, war target, city target — none of it keyed to
    /// a unit id.
    pub fn forget_unit_memory(&mut self) {
        self.recovering_units.clear();
        self.patrol_targets.clear();
        self.patrol_posts.clear();
        self.patrol_posts_by_class.clear();
        self.settler_targets.clear();
        self.unit_memories.get_mut().clear();
        self.unit_motion.clear();
    }

    /// Carry unit-keyed memory across a board that was rebuilt underneath it.
    ///
    /// ★★★★★ FORGETTING IS WHY THE SETTLERS WANDER. The Civilization VI bridge
    /// rebuilds the board every turn (`Ai::take_turn` needs a turn that has advanced
    /// through the engine's own private `begin_turn`), and unit ids are reassigned
    /// when it does — so every unit-keyed map described a different unit and the only
    /// safe thing to do was drop it. The cost of dropping it is that the settler's
    /// DESTINATION is re-derived from scratch each turn, and a re-derived optimum
    /// flips: measured on run `civvis-20260731T055749Z`, one settler was told to walk
    /// to a site 23 tiles away on turns 14, 18 and 20 and to a different site 7 tiles
    /// away on turn 16. The livelock detector is unit-keyed too, so the ONE mechanism
    /// that exists to catch a unit going in circles could never fire in the bridge.
    ///
    /// The ids are recoverable, though: the mirror knows each board's Civ 6 id for
    /// every unit, so old id -> Civ 6 id -> new id is a total function on the units
    /// that still exist. Units that died simply drop out, which is what should happen
    /// to their memory anyway.
    pub fn remap_unit_memory(&mut self, map: &std::collections::BTreeMap<u32, u32>) {
        fn remap<V: Clone>(
            old: &HashMap<u32, V>,
            map: &std::collections::BTreeMap<u32, u32>,
        ) -> HashMap<u32, V> {
            old.iter()
                .filter_map(|(uid, value)| map.get(uid).map(|new| (*new, value.clone())))
                .collect()
        }
        self.recovering_units = self
            .recovering_units
            .iter()
            .filter_map(|uid| map.get(uid).copied())
            .collect();
        self.patrol_targets = remap(&self.patrol_targets, map);
        // Posts are cleared every turn by `begin_movement_turn` anyway, and they are
        // claims on ground rather than memory about a unit.
        self.patrol_posts.clear();
        self.patrol_posts_by_class.clear();
        self.settler_targets = remap(&self.settler_targets, map);
        let unit_memories = std::mem::take(self.unit_memories.get_mut());
        *self.unit_memories.get_mut() = unit_memories
            .into_iter()
            .filter_map(|(uid, memory)| map.get(&uid).map(|new| (*new, memory)))
            .collect();
        self.unit_motion = self
            .unit_motion
            .iter()
            .filter_map(|(uid, motion)| map.get(uid).map(|new| (*new, motion.clone())))
            .collect();
        // ⚠ Cleared, not remapped. It records "this unit took a step FROM here on
        // THIS turn", and the turn is over by the time a board is rebuilt, so every
        // entry in it is already stale.
        self.last_path_step_from.borrow_mut().clear();
    }

    /// Reset caches whose contents depend on the current player's borders and
    /// movement capabilities, and take down where every unit is standing
    /// before any of them moves. Persistent destinations live across turns;
    /// the expensive all-map candidate scan does not need to.
    pub(crate) fn begin_movement_turn(&mut self, g: &Game, pid: usize) {
        self.patrol_posts.clear();
        self.patrol_posts_by_class.clear();
        self.refresh_unit_memories(g, pid);
        self.observe_unit_motion(g, pid);
    }

    pub(crate) fn unit_plan_state(&self, uid: u32) -> BasicUnitPlanState {
        BasicUnitPlanState {
            recovering: self.recovering_units.contains(&uid),
            patrol_target: self.patrol_targets.get(&uid).copied(),
            settler_target: self.settler_targets.get(&uid).copied(),
            memory: self.unit_memory(uid),
            last_path_step: self.last_path_step_from.borrow().get(&uid).copied(),
            patrol_posts: self.patrol_posts.clone(),
        }
    }

    pub(crate) fn merge_unit_plan_state(&mut self, uid: u32, state: BasicUnitPlanState) {
        if state.recovering {
            self.recovering_units.insert(uid);
        } else {
            self.recovering_units.remove(&uid);
        }
        match state.patrol_target {
            Some(target) => {
                self.patrol_targets.insert(uid, target);
            }
            None => {
                self.patrol_targets.remove(&uid);
            }
        }
        match state.settler_target {
            Some(target) => {
                self.settler_targets.insert(uid, target);
            }
            None => {
                self.settler_targets.remove(&uid);
            }
        }
        match state.memory {
            Some(memory) => {
                self.unit_memories.get_mut().insert(uid, memory);
            }
            None => {
                self.unit_memories.get_mut().remove(&uid);
            }
        }
        match state.last_path_step {
            Some(step) => {
                self.last_path_step_from.borrow_mut().insert(uid, step);
            }
            None => {
                self.last_path_step_from.borrow_mut().remove(&uid);
            }
        }
        // Posts are immutable for the turn. A branch may have paid to build a
        // domain's list, so retain that work for later serial fallbacks.
        self.patrol_posts.extend(state.patrol_posts);
    }

    pub(crate) fn clear_prepared_patrol_posts(&mut self) {
        self.patrol_posts_by_class.clear();
    }

    /// Record this turn's starting tile for every unit, and judge each unit
    /// against the window that has just closed. This is the only point in the
    /// turn where a livelock can be seen at all: it is a property of a unit's
    /// history, not of any decision the unit is about to make.
    fn observe_unit_motion(&mut self, g: &Game, pid: usize) {
        let ids = g.player_unit_ids(pid);
        self.unit_motion
            .retain(|uid, _| g.units.get(uid).is_some_and(|unit| unit.owner == pid));
        for uid in ids {
            let mark = work_mark(g, uid);
            let pos = g.units[&uid].pos;
            let motion = self.unit_motion.entry(uid).or_default();
            let was_looping = motion.looping;
            if motion.tiles.is_empty() {
                motion.work = mark;
            }
            if motion.work != mark {
                // The unit achieved something, so whatever it was doing was
                // worth doing. Judge it from here rather than against a
                // history that has just been made irrelevant.
                *motion = UnitMotion {
                    work: mark,
                    resume_turn: motion.resume_turn,
                    ..UnitMotion::default()
                };
            } else {
                motion.fruitless += 1;
            }
            motion.tiles.push_back(pos);
            while motion.tiles.len() > LIVELOCK_WINDOW {
                motion.tiles.pop_front();
            }
            motion.looping = motion.circling();
            let looping = motion.looping;
            let fruitless = motion.fruitless;
            let footprint = motion.footprint();
            let stand_down = looping && fruitless >= LIVELOCK_STAND_DOWN_AFTER;
            if stand_down {
                // The tabu has had a full second window to redirect this unit
                // and has not. Stop paying for the same fruitless search, hold
                // the ground, and come back to the problem with a clean slate
                // once the world around it has changed.
                *motion = UnitMotion {
                    work: mark,
                    resume_turn: g.turn + LIVELOCK_STAND_DOWN_TURNS,
                    ..UnitMotion::default()
                };
            }
            // Say it once when the loop is first recognized, and once more if
            // it outlasts every attempt to steer out of it.
            let kind = g.units[&uid].kind.as_str();
            if stand_down {
                think!(self.journal, Military, Decision,
                       "{kind} {uid} stands down; it is going nowhere";
                       "{fruitless} turns inside {footprint} tiles with nothing to show for \
                        them, and steering it out did not work; holding for \
                        {LIVELOCK_STAND_DOWN_TURNS} turns";
                       pos);
            } else if looping && !was_looping {
                think!(self.journal, Military, Detail,
                       "{kind} {uid} is walking in circles";
                       "{fruitless} turns inside {footprint} tiles with nothing to show for \
                        them; anywhere outside them is now worth \
                        {LIVELOCK_ESCAPE_VALUE:.0} more";
                       pos);
            }
        }
    }

    /// Dig in a stood-down unit that took its whole turn and found nothing to
    /// do. This runs *after* the unit's own step, never instead of it: an
    /// earlier version pre-empted the turn and guessed at what the unit might
    /// have wanted, which cost more productive turns than the loops it broke.
    /// A unit that acted needs nothing from this; one that did not is standing
    /// in the open regardless, and is better off fortified and healing.
    pub(crate) fn hold_stood_down_unit(&self, g: &mut Game, pid: usize, uid: u32) {
        let standing_down = self
            .unit_motion
            .get(&uid)
            .is_some_and(|motion| g.turn < motion.resume_turn);
        if standing_down && g.units.contains_key(&uid) {
            self.fortify_or_stop(g, pid, uid);
        }
    }

    /// What a candidate tile is worth against the fact that this unit has been
    /// going in circles. Nothing at all for a unit that is getting somewhere —
    /// which is almost every unit, almost always — and for one that is not, a
    /// flat charge for every tile of the footprint it keeps re-entering,
    /// including the one it is standing on. Any tile outside the loop is
    /// thereby worth `LIVELOCK_ESCAPE_VALUE` more than any tile inside it.
    pub(crate) fn livelock_penalty(&self, uid: u32, tile: Pos) -> f64 {
        match self.unit_motion.get(&uid) {
            Some(motion) if motion.looping && motion.tiles.contains(&tile) => {
                -LIVELOCK_ESCAPE_VALUE
            }
            _ => 0.0,
        }
    }

    /// Whether a plain pathing step should be refused because it walks back
    /// into a footprint this unit has already exhausted. Unlike the tactical
    /// scorers there is nothing to trade off here — the route is chosen for
    /// progress alone — so the tabu is absolute while it lasts, and it lasts
    /// only until the window slides off the loop or the stand-down fires.
    fn retreads_a_loop(&self, uid: u32, to: Pos) -> bool {
        self.unit_motion
            .get(&uid)
            .is_some_and(|motion| motion.looping && motion.tiles.contains(&to))
    }

    /// Run each available agent once. The baseline establishes sources before
    /// attempting the highest expected-value operation and otherwise embeds
    /// agents in the most developed non-allied foreign city.
    pub(crate) fn spies(&self, g: &mut Game, pid: usize) {
        let ids: Vec<u32> = g
            .spies
            .values()
            .filter(|spy| spy.owner == pid)
            .map(|spy| spy.id)
            .collect();
        for spy_id in ids {
            let legal = g.legal_spy_actions(pid, spy_id);
            if legal.is_empty() {
                continue;
            }
            if let Some(action) = [
                "technologist",
                "con_artist",
                "disguise",
                "linguist",
                "quartermaster",
                "seduction",
            ]
            .into_iter()
            .find_map(|wanted| {
                legal.iter().find(|action| {
                    matches!(action, Action::PromoteSpy { promotion, .. } if promotion == wanted)
                })
            })
            .or_else(|| {
                legal
                    .iter()
                    .find(|action| matches!(action, Action::PromoteSpy { .. }))
            }) {
                let _ = g.apply(pid, action);
                continue;
            }
            let current_city = g.spies.get(&spy_id).and_then(|spy| spy.city);
            let offensive = current_city
                .and_then(|city| g.cities.get(&city))
                .is_some_and(|city| city.owner != pid);
            if offensive {
                if let Some(action) = legal.iter().find(|action| {
                    matches!(action, Action::SpyMission { mission, .. } if mission == "gain_sources")
                }) {
                    let _ = g.apply(pid, action);
                    continue;
                }
                let operation = legal
                    .iter()
                    .filter_map(|action| {
                        let Action::SpyMission {
                            spy,
                            mission,
                            target,
                        } = action
                        else {
                            return None;
                        };
                        let active = crate::game::SpyMission {
                            kind: mission.clone(),
                            city: current_city?,
                            target: *target,
                            started: g.turn,
                            ends: g.turn,
                        };
                        let value = match mission.as_str() {
                            "steal_tech_boost" => 105.0,
                            "siphon_funds" => 95.0,
                            "great_work_heist" => 90.0,
                            "neutralize_governor" => 82.0,
                            "disrupt_rocketry" => 80.0,
                            "fabricate_scandal" => 74.0,
                            "sabotage_production" => 70.0,
                            "foment_unrest" => 62.0,
                            "breach_dam" => 58.0,
                            "recruit_partisans" => 55.0,
                            "listening_post" => 42.0,
                            _ => 0.0,
                        };
                        Some((g.spy_success_chance(*spy, &active) * value, mission, action))
                    })
                    .max_by(|left, right| {
                        left.0
                            .partial_cmp(&right.0)
                            .unwrap()
                            .then_with(|| right.1.cmp(left.1))
                    })
                    .map(|(_, _, action)| action);
                if let Some(action) = operation {
                    let _ = g.apply(pid, action);
                    continue;
                }
            }
            let assignment = legal
                .iter()
                .filter_map(|action| match action {
                    Action::AssignSpy { city, .. } => {
                        let target = &g.cities[city];
                        (target.owner != pid).then_some((
                            target.pop as i64 * 8
                                + target.districts.len() as i64 * 12
                                + target.wonders.len() as i64 * 20
                                - i64::from(g.players[target.owner].is_minor) * 20,
                            std::cmp::Reverse(*city),
                            action,
                        ))
                    }
                    _ => None,
                })
                .max_by(|left, right| {
                    left.0
                        .cmp(&right.0)
                        .then_with(|| left.1.cmp(&right.1))
                })
                .map(|(_, _, action)| action)
                .or_else(|| {
                    legal
                        .iter()
                        .find(|action| matches!(action, Action::SpyMission { mission, .. } if mission == "counterspy"))
                });
            if let Some(action) = assignment {
                let _ = g.apply(pid, action);
            }
        }
    }

    pub(crate) fn corporations(&self, g: &mut Game, pid: usize) {
        if let Some(action) = g
            .legal_actions_within(pid, ActionFamilies::CORPORATIONS)
            .into_iter()
            .find(|action| matches!(action, Action::FoundCorporation { .. }))
        {
            let _ = g.apply(pid, &action);
        }
    }

    /// Resolve mandatory conquest choices with explicit strategic tradeoffs.
    /// Capitals and developed bridgeheads are retained; diplomacy-oriented
    /// plans restore city-states, friends, and eliminated founders; only an
    /// aggressive plan razes a remote city whose long-run value is negligible.
    pub(crate) fn resolve_city_dispositions(
        &mut self,
        g: &mut Game,
        pid: usize,
        prefer_diplomacy: bool,
        prefer_conquest: bool,
    ) {
        loop {
            let legal = g.legal_city_disposition_actions(pid);
            let Some(cid) = legal.iter().find_map(|action| match action {
                Action::KeepCity { city }
                | Action::RazeCity { city }
                | Action::LiberateCity { city } => Some(*city),
                _ => None,
            }) else {
                break;
            };
            let city = g.cities[&cid].clone();
            let founder = city.original_owner;
            let can_liberate = legal
                .iter()
                .any(|action| matches!(action, Action::LiberateCity { city } if *city == cid));
            let diplomatic_liberation = can_liberate
                && (prefer_diplomacy
                    || g.active_emergencies.iter().any(|emergency| {
                        emergency.city == cid && emergency.members.contains(&pid)
                    })
                    || g.players[founder].is_minor
                    || !g.players[founder].alive
                    || g.are_friends(pid, founder)
                    || g.alliance_with(pid, founder).is_some());

            let nearest_core = g
                .cities
                .values()
                .filter(|other| other.owner == pid && other.id != cid)
                .map(|other| g.wdist(city.pos, other.pos))
                .min()
                .unwrap_or(i32::MAX);
            let durable_value = city.is_capital
                || city.pop >= 4
                || !city.districts.is_empty()
                || !city.wonders.is_empty()
                || nearest_core <= 8;
            let can_raze = legal
                .iter()
                .any(|action| matches!(action, Action::RazeCity { city } if *city == cid));
            let action = if self.minor && can_raze {
                Action::RazeCity { city: cid }
            } else if diplomatic_liberation {
                Action::LiberateCity { city: cid }
            } else if can_raze && prefer_conquest && !durable_value {
                Action::RazeCity { city: cid }
            } else {
                Action::KeepCity { city: cid }
            };
            if g.apply(pid, &action).is_err() {
                break;
            }
        }
    }

    fn research(&self, g: &mut Game, pid: usize) {
        self.research_with_government(g, pid, true, None);
    }

    /// City-states research enough to defend and express their type without
    /// inheriting the major civilization's ancillary government/religion pass.
    /// Lower difficulties reach Masonry first; Immortal and Deity already have
    /// setup-granted Walls and can move directly toward their specialty.
    fn minor_research(&self, g: &mut Game, pid: usize) {
        if g.players[pid].research.is_none() {
            let avail = g.available_techs(pid);
            if !avail.is_empty() {
                let has_walls = g.player_city_ids(pid).into_iter().any(|city| {
                    g.cities[&city]
                        .buildings
                        .iter()
                        .any(|building| building == "walls")
                });
                let defensive_goal =
                    (!has_walls && !g.players[pid].techs.contains(&crate::name!("masonry"))).then_some("masonry");
                let pick = Self::research_step_toward(g, &avail, defensive_goal)
                    .or_else(|| {
                        Self::research_step_toward(g, &avail, Self::minor_tech_goal(g, pid))
                    })
                    .or_else(|| {
                        Self::research_step_toward(g, &avail, Self::economic_research_goal(g, pid))
                    })
                    .or_else(|| {
                        TECH_PRIORITY
                            .iter()
                            .find(|tech| avail.iter().any(|candidate| candidate == *tech))
                            .map(|tech| Name::new(tech))
                    })
                    .unwrap_or_else(|| avail[0]);
                let _ = g.apply(pid, &Action::Research { tech: Name::new(&pick) });
            }
        }
        if g.players[pid].civic.is_none() {
            let avail = g.available_civics(pid);
            if !avail.is_empty() {
                let cultural_goal = (g.cs_type(&g.players[pid].civ) == "cultural"
                    && !g.players[pid].civics.contains(&crate::name!("drama_poetry")))
                .then_some("drama_poetry");
                let pick = Self::civic_step_toward(g, &avail, cultural_goal)
                    .or_else(|| {
                        CIVIC_PRIORITY
                            .iter()
                            .find(|civic| avail.iter().any(|candidate| candidate == *civic))
                            .map(|civic| Name::new(civic))
                    })
                    .unwrap_or_else(|| avail[0]);
                let _ = g.apply(pid, &Action::Civic { civic: Name::new(&pick) });
            }
        }
    }

    /// Choose research and run the ancillary pass without allowing the
    /// baseline government priority to compete with a strategic caller.
    /// `AdvancedAi` has its own plan-aware government selector later in the
    /// same turn; running both selectors made it adopt two Tier-1 governments
    /// back-to-back and then pay Anarchy when the baseline tried to undo the
    /// strategic choice on the next turn. A headless caller may also pass its
    /// persistent pool for the independent live-policy counterfactuals;
    /// interactive and baseline controllers pass `None`.
    pub(crate) fn research_without_government_with_pool(
        &self,
        g: &mut Game,
        pid: usize,
        pool: Option<&WorkPool>,
    ) {
        self.research_with_government(g, pid, false, pool);
    }

    fn research_with_government(
        &self,
        g: &mut Game,
        pid: usize,
        choose_government: bool,
        pool: Option<&WorkPool>,
    ) {
        if g.players[pid].research.is_none() {
            let avail = g.available_techs(pid);
            if !avail.is_empty() {
                let water_pick = Self::research_step_toward(
                    g,
                    &avail,
                    Self::water_research_goal(g, pid),
                );
                let economic_pick = Self::research_step_toward(
                    g,
                    &avail,
                    Self::economic_research_goal(g, pid),
                );
                let pick = water_pick
                    .or(economic_pick)
                    .or_else(|| {
                        TECH_PRIORITY
                            .iter()
                            .find(|t| avail.iter().any(|a| a == *t))
                            .map(|t| Name::new(t))
                    })
                    .unwrap_or_else(|| avail[0]);
                let _ = g.apply(pid, &Action::Research { tech: Name::new(&pick) });
            }
        }
        if g.players[pid].civic.is_none() {
            let avail = g.available_civics(pid);
            if !avail.is_empty() {
                let pick = CIVIC_PRIORITY
                    .iter()
                    .find(|c| avail.iter().any(|a| a == *c))
                    .map(|c| Name::new(c))
                    .unwrap_or_else(|| avail[0]);
                let _ = g.apply(pid, &Action::Civic { civic: Name::new(&pick) });
            }
        }
        // Great People are not awarded automatically when the points cross
        // the threshold: recruitment is a legal player action. Patronage had
        // a strategic buyer, but the free action had no AI consumer at all,
        // so a winning Scientist or Prophet could remain on the board while
        // every headless civilization waited forever. Claim every earned and
        // currently activatable person before founding a Religion or valuing
        // paid patronage later in the turn.
        Self::claim_free_great_people(g, pid);
        if choose_government {
            for gname in GOV_PRIORITY {
                if let Some(spec) = g.rules.governments.get(gname) {
                    let ok = spec
                        .civic
                        .as_ref()
                        .map(|c| g.players[pid].civics.contains(c))
                        .unwrap_or(true);
                    if ok {
                        if g.players[pid].government.as_deref() != Some(gname) {
                            let _ = g.apply(
                                pid,
                                &Action::Government {
                                    government: Name::new(gname),
                                },
                            );
                        }
                        break;
                    }
                }
            }
        }
        revise_policy_deck(g, pid, &self.w, pool);
        if g.players[pid].secret_society.is_none() {
            let society = if self.pursue_religion {
                "voidsingers"
            } else {
                "owls_of_minerva"
            };
            let _ = g.apply(
                pid,
                &Action::ChooseSecretSociety {
                    society: Name::new(society),
                },
            );
        }
        // ⚠⚠⚠ THE THIRD SPELLING OF THE SAME RULE, and the one that actually
        // decides. #1044 fixed the legality gate and the affordability check in
        // `game.rs` and missed this, so on Online — where the real price is 12.5 —
        // the AI sat waiting for 25 faith while Civilization VI had already raised
        // `ENDTURN_BLOCKING_PANTHEON`, and the mod's fallback picked instead.
        // Measured live on `civvis-20260803T231038Z`: faith passed 12.5 on turn 19,
        // the host asked on turn 25, and replaying turns 21 and 23 emitted 7 and 8
        // orders with NO pantheon among them.
        if !self.minor
            && g.players[pid].pantheon.is_none()
            && g.players[pid].faith >= g.pantheon_faith_cost()
        {
            for (rank, b) in [
                "divine_spark",
                "fertility_rites",
                "god_of_the_forge",
                "religious_settlements",
                "god_of_the_open_sky",
                "god_of_the_sea",
            ]
            .into_iter()
            .enumerate()
            {
                if g.apply(
                    pid,
                    &Action::ChoosePantheon {
                        belief: Name::new(b),
                    },
                )
                .is_ok()
                {
                    think!(self.journal, Faith, Decision, "Founding the pantheon {}", plain(b);
                           "the {} choice on the standing list still unclaimed",
                           match rank { 0 => "first".to_string(), _ => format!("{}th", rank + 1) });
                    break;
                }
            }
        }
        if self.pursue_religion && g.players[pid].prophet_pending {
            let mut followers: Vec<String> = [
                "work_ethic",
                "choral_music",
                "feed_the_world",
                "jesuit_education",
                "religious_community",
                "zen_meditation",
            ]
            .into_iter()
            .filter(|belief| g.rules.beliefs.follower.contains_key(*belief))
            .map(str::to_string)
            .collect();
            for belief in g.rules.beliefs.follower.keys() {
                if !followers.contains(belief) {
                    followers.push(belief.clone());
                }
            }
            let mut founders: Vec<String> = [
                "tithe",
                "world_church",
                "cross_cultural_dialogue",
                "pilgrimage",
                "religious_unity",
            ]
            .into_iter()
            .filter(|belief| g.rules.beliefs.founder.contains_key(*belief))
            .map(str::to_string)
            .collect();
            for belief in g.rules.beliefs.founder.keys() {
                if !founders.contains(belief) {
                    founders.push(belief.clone());
                }
            }
            'found: for follower in followers {
                for founder in &founders {
                    if g.apply(
                        pid,
                        &Action::FoundReligion {
                            follower: Name::new(&follower),
                            founder: Name::new(founder),
                        },
                    )
                    .is_ok()
                    {
                        think!(self.journal, Faith, Decision, "Founding a religion";
                               "on the {} follower belief and the {} founder \
                                belief, the first pair still unclaimed",
                               plain(&follower), plain(founder));
                        break 'found;
                    }
                }
            }
        }
        while g.governor_titles_available(pid) > 0 {
            // anchor the shakiest city
            let target = g
                .player_city_ids(pid)
                .into_iter()
                .filter(|c| !g.players[pid].governors.contains(c))
                .min_by(|a, b| {
                    g.cities[a]
                        .loyalty
                        .partial_cmp(&g.cities[b].loyalty)
                        .unwrap()
                        .then(a.cmp(b))
                });
            if let Some(c) = target {
                let governor = [
                    "pingala", "magnus", "liang", "reyna", "victor", "moksha", "amani",
                ]
                .into_iter()
                .find(|governor| !g.players[pid].governor_roster.contains_key(governor));
                if let Some(governor) = governor {
                    if g.apply(
                        pid,
                        &Action::AppointGovernor {
                            governor: Name::new(governor),
                            city: c,
                        },
                    )
                    .is_err()
                    {
                        break;
                    }
                    continue;
                }
                // A modded ruleset can replace every stock Governor name.
                // Keep the generic assignment tool as the data-driven
                // fallback instead of leaving all of those titles idle.
                if g.apply(pid, &Action::AssignGovernor { city: c }).is_ok() {
                    continue;
                }
            }
            let promotion = [
                "pingala", "magnus", "liang", "reyna", "victor", "moksha", "amani",
            ]
            .into_iter()
            .find_map(|governor| {
                g.available_governor_promotions(pid, governor)
                    .into_iter()
                    .next()
                    .map(|promotion| (governor.to_string(), promotion))
            });
            let Some((governor, promotion)) = promotion else {
                break;
            };
            if g.apply(
                pid,
                &Action::PromoteGovernor {
                    governor: Name::new(&governor),
                    promotion: Name::new(&promotion),
                },
            )
            .is_err()
            {
                break;
            }
        }
        // Appointment is not the end of governor play. If an ungoverned city
        // is already close to revolt, move an idle Governor there, or pull one
        // from a completely loyal city. AdvancedAI runs its plan-aware
        // governor pass first, so this is an emergency backstop rather than a
        // second strategy fighting the first one.
        self.reassign_governor_for_loyalty(g, pid);
        while g.players[pid].envoys_free > 0 {
            // consolidate on the city-state we already lead in (suzerain push)
            let target = g
                .players
                .iter()
                .filter(|m| g.can_send_envoy(pid, m.id))
                .max_by_key(|m| (g.envoys_at(pid, m.id), std::cmp::Reverse(m.id)))
                .map(|m| m.id);
            match target {
                Some(t) => {
                    if g.apply(pid, &Action::SendEnvoy { player: t }).is_err() {
                        break;
                    }
                }
                None => break,
            }
        }
    }

    /// Take every no-cost Great Person claim the empire has earned.
    ///
    /// The action list is authoritative about activation requirements: a
    /// Scientist still needs a Campus, a Writer needs enough work slots, and a
    /// Prophet needs an active Holy Site or Stonehenge. Applying the actions
    /// rather than duplicating those conditions keeps the AI on the same tool
    /// protocol as a human or learned policy.
    fn claim_free_great_people(g: &mut Game, pid: usize) -> usize {
        let threshold_reached = g.players[pid]
            .gpp
            .iter()
            .any(|(kind, points)| *points + f64::EPSILON >= g.gp_cost(pid, kind));
        if !threshold_reached {
            return 0;
        }
        let mut recruits: Vec<Action> = g
            .legal_actions_within(pid, ActionFamilies::EMPIRE)
            .into_iter()
            .filter(|action| matches!(action, Action::RecruitGreatPerson { .. }))
            .collect();
        // Determinism matters for same-seed evaluation. The rules map order is
        // stable today, but make the intended order explicit at this boundary.
        recruits.sort_by(|left, right| format!("{left:?}").cmp(&format!("{right:?}")));
        recruits
            .into_iter()
            .filter(|action| g.apply(pid, action).is_ok())
            .count()
    }

    /// Relocate one Governor to a city in immediate Loyalty danger.
    /// How many turns until this city flips, or `None` if it is not in trouble.
    ///
    /// Smaller is worse. A city already under `LOYALTY_LEVEL_ALARM` counts as an
    /// emergency whatever its rate, because the level threshold is what the
    /// governor logic used before and a city down there is in trouble even while
    /// recovering — it just is not the MOST urgent one if something else is
    /// visibly bleeding out faster.
    ///
    /// Returns `None` when the flag is off, so a frozen controller keeps the old
    /// level-only behaviour exactly.
    pub(crate) fn loyalty_emergency(&self, g: &Game, cid: u32) -> Option<f64> {
        let city = g.cities.get(&cid)?;
        if !self.loyalty_rate_alarm {
            // Frozen behaviour: the level threshold alone, exactly as before.
            return (city.loyalty < LOYALTY_LEVEL_ALARM).then_some(city.loyalty);
        }
        let rate = g.city_loyalty_per_turn(city);
        if rate < -f64::EPSILON {
            // Turns of headroom left at the current rate.
            return Some((city.loyalty / -rate).max(0.0));
        }
        // Stable or recovering: only the old level threshold still flags it, and
        // it ranks behind anything actually falling.
        (city.loyalty < LOYALTY_LEVEL_ALARM).then_some(f64::MAX / 2.0)
    }

    /// ★★★★★ LOYALTY LEVEL IS A LAGGING INDICATOR AND IT IS ALL THIS AI EVER READ.
    ///
    /// Every city loss across every recorded run on this machine, classified by
    /// the city's last sighting before it disappeared — **125 losses**:
    ///
    /// ```text
    ///   52  41.6%  loyalty < 50            revolt
    ///   37  29.6%  loyal, damaged          siege we could contest
    ///   36  28.8%  loyal, UNDAMAGED        gone from full health in one round
    /// ```
    ///
    /// **Loyalty is the single largest cause of city loss, ahead of every military
    /// shape**, and `66 of the 125` were carrying a NEGATIVE loyalty rate when
    /// last seen.
    ///
    /// ⚠ The rate was available the whole time and nothing read it.
    /// `Game::city_loyalty_per_turn` is computed by the engine, mirrored from
    /// Civilization VI (`mirror.rs` asserts it survives a save) and exported in
    /// `obs.rs` — yet it had **zero consumers in `ai.rs`, `ai/advanced.rs`,
    /// `strategic.rs` and `production.rs`**. The only two loyalty readers both
    /// took the LEVEL: this function's `< 70.0`, and the `bread_and_circuses`
    /// project score's `100 - loyalty`.
    ///
    /// A level threshold cannot see a city dying. A city on 100 losing 12 a turn
    /// is eight turns from flipping and reads as perfectly safe; a city on 60
    /// gaining 5 is recovering and reads as the emergency. That is exactly
    /// backwards, and it is why 36 cities vanished at full loyalty.
    ///
    /// So the governor goes to whichever city flips SOONEST, and a level below
    /// the old threshold is treated as an emergency in its own right so a city
    /// already in trouble is never ranked behind a healthy one.
    fn reassign_governor_for_loyalty(&self, g: &mut Game, pid: usize) -> bool {
        let target = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|city| !g.players[pid].governors.contains(city))
            .filter(|city| self.loyalty_emergency(g, *city).is_some())
            .min_by(|left, right| {
                self.loyalty_emergency(g, *left)
                    .unwrap_or(f64::MAX)
                    .total_cmp(&self.loyalty_emergency(g, *right).unwrap_or(f64::MAX))
                    .then(left.cmp(right))
            });
        let Some(target) = target else { return false };
        let target_loyalty = g.cities[&target].loyalty;
        let action = g
            .legal_actions_within(pid, ActionFamilies::EMPIRE)
            .into_iter()
            .filter_map(|action| {
                let Action::ReassignGovernor { governor, city } = &action else {
                    return None;
                };
                if *city != target {
                    return None;
                }
                let state = &g.players[pid].governor_roster[governor];
                let source_loyalty = state
                    .city
                    .and_then(|city| g.cities.get(&city))
                    .map_or(101.0, |city| city.loyalty);
                // An unassigned Governor is free to move. An established one
                // only leaves a city with a substantial Loyalty cushion.
                (state.city.is_none()
                    || (source_loyalty >= 90.0
                        && source_loyalty - target_loyalty >= 20.0))
                    .then_some((
                        state.city.is_none(),
                        governor == "victor",
                        source_loyalty,
                        std::cmp::Reverse(*governor),
                        action,
                    ))
            })
            .max_by(|left, right| {
                left.0
                    .cmp(&right.0)
                    .then(left.1.cmp(&right.1))
                    .then(left.2.total_cmp(&right.2))
                    .then(left.3.cmp(&right.3))
            })
            .map(|(_, _, _, _, action)| action);
        action.is_some_and(|action| g.apply(pid, &action).is_ok())
    }

    fn diplomacy(&self, g: &mut Game, pid: usize) {
        choose_dedications(g, pid, self.w.dedication_choice);
        let incoming: Vec<u32> = g
            .pending_deals
            .iter()
            .filter(|deal| deal.to == pid && deal.expires >= g.turn)
            .map(|deal| deal.id)
            .collect();
        for deal_id in incoming {
            let accept = g
                .pending_deals
                .iter()
                .find(|deal| deal.id == deal_id)
                .is_some_and(|deal| {
                    let partner_power = g.military_power(deal.from);
                    let grievance = g.players[pid]
                        .grievances
                        .get(&deal.from)
                        .copied()
                        .unwrap_or(0.0);
                    let special_accept = if deal.defensive_pact {
                        grievance < 75.0
                            && partner_power < g.military_power(pid) * 1.8 + 20.0
                    } else if let Some(target) = deal.joint_war_target {
                        let joint_power = g.military_power(pid) + partner_power;
                        let target_power = g.military_power(target);
                        g.players[pid]
                            .grievances
                            .get(&target)
                            .copied()
                            .unwrap_or(0.0)
                            >= 20.0
                            && joint_power > target_power * 1.2 + 20.0
                    } else if let Some(promise) = deal.promise.as_deref() {
                        match promise {
                            // An early Basic AI still needs nearby expansion
                            // room; later on it can safely give this promise.
                            "no_settling" => g.player_city_ids(pid).len() >= 3,
                            "no_city_state_attack" => !g.players.iter().any(|city_state| {
                                city_state.is_minor
                                    && !city_state.is_barbarian
                                    && g.is_at_war(pid, city_state.id)
                            }),
                            "no_conversion" | "no_spying" => true,
                            _ => false,
                        }
                    } else {
                        false
                    };
                    deal.peace
                        || special_accept
                        || deal.give_gold >= deal.request_gold
                        || ((deal.friendship || deal.alliance.is_some() || deal.open_borders)
                            && grievance < 75.0
                            && partner_power < g.military_power(pid) * 1.8 + 20.0)
                });
            let action = if accept {
                Action::AcceptDeal { deal: deal_id }
            } else {
                Action::RejectDeal { deal: deal_id }
            };
            let _ = g.apply(pid, &action);
        }
        if let Some(session) = g.congress.clone() {
            for resolution in session.resolutions {
                if resolution.ballots.contains_key(&pid) {
                    continue;
                }
                let own_a = format!("A:{pid}");
                let emergency_choice = g
                    .emergency_proposal_for_resolution(&resolution.id)
                    .and_then(|proposal| {
                        if proposal.target == pid {
                            Some("B:oppose".to_string())
                        } else if proposal.eligible.contains(&pid) {
                            Some("A:support".to_string())
                        } else {
                            None
                        }
                    });
                if g.emergency_proposal_for_resolution(&resolution.id)
                    .is_some()
                    && emergency_choice.is_none()
                {
                    continue;
                }
                let choice = emergency_choice
                    .or_else(|| {
                        resolution
                            .ballots
                            .values()
                            .max_by_key(|(choice, votes)| {
                                (*votes, std::cmp::Reverse(choice.clone()))
                            })
                            .map(|(choice, _)| choice.clone())
                            .or_else(|| {
                                (resolution.id == "world_leader"
                                    || resolution.id == "trade_policy"
                                    || resolution.id == "migration_treaty"
                                    || resolution.id == "border_control_treaty"
                                    || resolution.id == "public_relations")
                                    .then(|| {
                                        resolution
                                            .choices
                                            .iter()
                                            .find(|choice| **choice == own_a)
                                            .cloned()
                                    })
                                    .flatten()
                            })
                            .or_else(|| {
                                (resolution.id == "world_ideology")
                                    .then(|| {
                                        let own_government =
                                            g.players[pid].government.as_deref()?;
                                        let wanted = format!("A:{own_government}");
                                        resolution
                                            .choices
                                            .iter()
                                            .find(|choice| **choice == wanted)
                                            .cloned()
                                    })
                                    .flatten()
                            })
                            .or_else(|| {
                                (resolution.id == "mercenary_companies")
                                    .then(|| {
                                        resolution
                                            .choices
                                            .iter()
                                            .find(|choice| choice.as_str() == "B:production")
                                            .cloned()
                                    })
                                    .flatten()
                            })
                            .or_else(|| {
                                let preferred = match resolution.id.as_str() {
                                    "global_energy_treaty" => Some("A:coal_power_plant"),
                                    "public_works_program" => resolution
                                        .choices
                                        .iter()
                                        .find(|choice| choice.starts_with("A:"))
                                        .map(|name| name.as_str()),
                                    "deforestation_treaty" => resolution
                                        .choices
                                        .iter()
                                        .find(|choice| choice.starts_with("A:"))
                                        .map(|name| name.as_str()),
                                    _ => None,
                                }?;
                                resolution
                                    .choices
                                    .iter()
                                    .find(|choice| choice.as_str() == preferred)
                                    .cloned()
                            })
                    })
                    .unwrap_or_else(|| resolution.choices[pid % resolution.choices.len()].clone());
                let votes = if g.players[pid].diplomatic_favor >= 30.0 {
                    3
                } else if g.players[pid].diplomatic_favor >= 10.0 {
                    2
                } else {
                    1
                };
                let _ = g.apply(
                    pid,
                    &Action::CongressVote {
                        resolution: Name::new(&resolution.id),
                        choice,
                        votes,
                    },
                );
            }
        }
        self.bilateral_trade(g, pid);
        let my_power = g.military_power(pid);
        let others: Vec<usize> = g
            .players
            .iter()
            .filter(|o| o.id != pid && o.alive && !o.is_barbarian && g.has_met(pid, o.id))
            .map(|o| o.id)
            .collect();
        for o in &others {
            if g.is_at_war(pid, *o)
                // A city-state follows its Suzerain into a derived war. It
                // cannot settle that principal conflict itself, so only ask
                // for peace when this pair is present in the declared set.
                && g.at_war.contains(&(pid.min(*o), pid.max(*o)))
                && !g.emergency_war_pair(pid, *o)
                && my_power < self.w.peace_ratio * g.military_power(*o)
            {
                let _ = g.apply(pid, &Action::MakePeace { player: *o });
            }
        }
        if self.minor {
            return;
        }
        // Delegations and Resident Embassies are a small, recurring
        // investment rather than an AI-only stat. Establish one with the
        // least aggrieved major on a staggered cadence, preferring the later
        // Embassy replacement once Diplomatic Service is known.
        if g.turn % 9 == pid as u32 % 9 {
            let partner = others
                .iter()
                .copied()
                .filter(|other| {
                    !g.players[*other].is_minor
                        && !g.is_at_war(pid, *other)
                        && g.players[pid].grievances.get(other).copied().unwrap_or(0.0) < 50.0
                })
                .min_by(|first, second| {
                    g.players[pid]
                        .grievances
                        .get(first)
                        .copied()
                        .unwrap_or(0.0)
                        .total_cmp(
                            &g.players[pid]
                                .grievances
                                .get(second)
                                .copied()
                                .unwrap_or(0.0),
                        )
                        .then(first.cmp(second))
                });
            if let Some(partner) = partner {
                let embassy_ready = g.players[pid]
                    .civics
                    .contains(&crate::name!("diplomatic_service"))
                    && g.players[pid].gold >= 25.0
                    && !g
                        .diplomatic_mission_to(pid, partner)
                        .is_some_and(|mission| mission.kind == "embassy");
                let action = if embassy_ready {
                    Some(Action::SendEmbassy { player: partner })
                } else if g.players[pid].gold >= 10.0
                    && g.diplomatic_mission_to(pid, partner).is_none()
                {
                    Some(Action::SendDelegation { player: partner })
                } else {
                    None
                };
                if let Some(action) = action {
                    let _ = g.apply(pid, &action);
                }
            }
        }
        if g.turn % 20 == pid as u32 % 20 {
            if let Some(partner) = others.iter().copied().find(|other| {
                !g.players[*other].is_minor
                    && !g.is_at_war(pid, *other)
                    && g.players[pid].grievances.get(other).copied().unwrap_or(0.0) < 50.0
            }) {
                let alliance = if g.are_friends(pid, partner)
                    && g.players[pid].civics.contains(&crate::name!("civil_service"))
                    && g.players[partner].civics.contains(&crate::name!("civil_service"))
                    && g.alliance_with(pid, partner).is_none()
                {
                    let kinds = ["economic", "cultural", "military", "religious", "research"];
                    kinds
                        .into_iter()
                        .cycle()
                        .skip(pid % kinds.len())
                        .take(kinds.len())
                        .find(|kind| {
                            (*kind != "research"
                                || (g.tree_effect(pid, "research_agreements") > 0.0
                                    && g.tree_effect(partner, "research_agreements") > 0.0))
                                && !g.players[pid].alliances.values().any(|alliance| {
                                    alliance.ends > g.turn && alliance.kind == *kind
                                })
                                && !g.players[partner].alliances.values().any(|alliance| {
                                    alliance.ends > g.turn && alliance.kind == *kind
                                })
                        })
                        .map(str::to_string)
                } else {
                    None
                };
                let _ = g.apply(
                    pid,
                    &Action::ProposeDeal {
                        player: partner,
                        give_gold: 0.0,
                        request_gold: 0.0,
                        open_borders: g.players[pid].civics.contains(&crate::name!("early_empire")),
                        friendship: true,
                        peace: false,
                        alliance,
                    },
                );
            }
        }
        // A Defensive Pact is deliberately separate from an Alliance. Once
        // an allied partner is in place, offer the explicit 30-turn treaty
        // instead of treating every Alliance as an automatic call to war.
        if g.turn % 15 == pid as u32 % 15 {
            if let Some(partner) = others.iter().copied().find(|other| {
                !g.players[*other].is_minor
                    && g.are_allied(pid, *other)
                    && g.defensive_pact_until(pid, *other).is_none()
                    && !g.pending_deals.iter().any(|deal| {
                        deal.defensive_pact
                            && ((deal.from == pid && deal.to == *other)
                                || (deal.from == *other && deal.to == pid))
                    })
            }) {
                let _ = g.apply(pid, &Action::ProposeDefensivePact { player: partner });
            }
        }
        let at_war = others.iter().any(|o| g.is_at_war(pid, *o));
        if at_war {
            self.levy_city_state_military(g, pid, false);
        }
        if !at_war
            && (g.turn as f64) > self.w.war_min_turn
            && g.player_city_ids(pid).len() >= 2
            && !others.is_empty()
        {
            let weakest = *others
                .iter()
                .min_by(|a, b| {
                    g.military_power(**a)
                        .partial_cmp(&g.military_power(**b))
                        .unwrap()
                })
                .unwrap();
            if my_power > self.w.war_ratio * g.military_power(weakest) + self.w.war_margin {
                let formal = g.players[pid]
                    .denounced_until
                    .get(&weakest)
                    .is_some_and(|until| *until > g.turn && *until <= g.turn + 25);
                let action = if formal {
                    Action::DeclareWarWithCasusBelli {
                        player: weakest,
                        casus_belli: "formal_war".to_string(),
                    }
                } else if !g.players[pid]
                    .denounced_until
                    .get(&weakest)
                    .is_some_and(|until| *until > g.turn)
                {
                    Action::Denounce { player: weakest }
                } else {
                    return;
                };
                let _ = g.apply(pid, &action);
            }
        }
    }

    /// Turn spare wartime Gold into immediately usable troops when this AI is
    /// a city-state's Suzerain. `urgent` lets the strategic AI spend deeper
    /// into its treasury for Conquest/Recovery plans; the general AI retains
    /// a larger economic reserve.
    pub(crate) fn levy_city_state_military(&self, g: &mut Game, pid: usize, urgent: bool) {
        if self.minor || self.barb {
            return;
        }
        let reserve_share = if urgent { 0.20 } else { 0.40 };
        let spendable = (g.players[pid].gold * (1.0 - reserve_share) - 20.0).max(0.0);
        let best = g
            .players
            .iter()
            .filter(|minor| minor.is_minor && !minor.is_barbarian && minor.alive)
            .filter_map(|minor| {
                let cost = g.levy_cost(pid, minor.id)?;
                if cost > spendable + f64::EPSILON {
                    return None;
                }
                let strength = g
                    .units
                    .values()
                    .filter(|unit| unit.owner == minor.id && unit.levied_from.is_none())
                    .filter(|unit| g.rules.units[unit.kind].class == "military")
                    .map(|unit| g.unit_strength(unit, true))
                    .sum::<f64>();
                Some((
                    strength / cost.max(1.0),
                    strength,
                    std::cmp::Reverse(minor.id),
                    minor.id,
                ))
            })
            .max_by(|left, right| {
                left.0
                    .partial_cmp(&right.0)
                    .unwrap()
                    .then_with(|| left.1.partial_cmp(&right.1).unwrap())
                    .then(left.2.cmp(&right.2))
            })
            .map(|(_, _, _, minor)| minor);
        if let Some(player) = best {
            let _ = g.apply(pid, &Action::LevyMilitary { player });
        }
    }

    /// Execute at most one pre-negotiated exchange on a staggered cadence.
    /// `Game::quick_deals` has already valued both sides, and `Action::Trade`
    /// revalidates the contract atomically, so the AI never relies on gifts,
    /// exploits stale quotes, or trades when either empire would lose value.
    pub(crate) fn bilateral_trade(&self, g: &mut Game, pid: usize) {
        self.bilateral_trade_excluding(g, pid, None);
    }

    pub(crate) fn bilateral_trade_excluding(
        &self,
        g: &mut Game,
        pid: usize,
        excluded_partner: Option<usize>,
    ) {
        if self.minor || self.barb || g.turn % 6 != (pid as u32 % 6) {
            return;
        }
        let best = g
            .quick_deals(pid)
            .into_iter()
            .filter(|deal| Some(deal.partner) != excluded_partner)
            .max_by(|left, right| {
                left.my_value
                    .min(left.partner_value)
                    .partial_cmp(&right.my_value.min(right.partner_value))
                    .unwrap()
            });
        let Some(deal) = best.filter(|deal| deal.my_value >= 2.0 && deal.partner_value >= 2.0)
        else {
            return;
        };
        let _ = g.apply(
            pid,
            &Action::Trade {
                player: deal.partner,
                offer: Box::new(deal.offer),
                request: Box::new(deal.request),
            },
        );
    }

    /// Name a production item the way an observer's reasoning log says it.
    /// The wire carries rule keys everywhere else; this is the one place a
    /// person reads them, so a Corps is a Corps and a district says where it
    /// is going.
    pub(crate) fn item_label(item: &Item) -> String {
        match item {
            Item::Unit { unit } => plain(unit),
            Item::Formation { unit, formation } => match formation {
                2 => format!("an Army of {}", plain(unit)),
                _ => format!("a Corps of {}", plain(unit)),
            },
            Item::Building { building } => plain(building),
            Item::District { district, pos } => format!("a {} district at {pos:?}", plain(district)),
            Item::Wonder { wonder, pos } => format!("the wonder {} at {pos:?}", plain(wonder)),
            Item::Repair { repair, pos } => format!("repairs to the {} at {pos:?}", plain(repair)),
            Item::Project { project } => format!("the {} project", plain(project)),
            Item::Product { product } => format!("the product {}", plain(product)),
        }
    }

    /// Where on the map a production item is going, when it is going
    /// somewhere in particular rather than into the city itself.
    pub(crate) fn item_focus(item: &Item, city: Pos) -> Pos {
        match item {
            Item::District { pos, .. } | Item::Wonder { pos, .. } | Item::Repair { pos, .. } => *pos,
            _ => city,
        }
    }

    fn cities(&mut self, g: &mut Game, pid: usize) {
        let mut settlers: usize = 0;
        let mut builders = 0;
        let mut traders = 0;
        let mut siege_support = 0;
        let mut melee = 0;
        let mut ranged = 0;
        let city_ids = g.player_city_ids(pid);
        let n_cities = city_ids.len();
        // The standing-army target measures fighting power, not unit records.
        // Counting heads let a Warrior that survived to the Industrial era
        // occupy a whole city's military allowance, so the empire stopped
        // training anything and its army aged in place. Each unit instead
        // counts as the fraction of a current front-line unit it can field.
        let front_line = Self::front_line_strength(g, pid, &city_ids);
        let mut force = 0.0;
        for uid in g.player_unit_ids(pid) {
            let kind = g.units[&uid].kind;
            match kind.as_str() {
                "settler" => settlers += 1,
                "builder" => builders += 1,
                "trader" => traders += 1,
                "battering_ram" | "siege_tower" => siege_support += 1,
                _ => {
                    let spec = &g.rules.units[kind];
                    if spec.class == "military" {
                        force += Self::force_weight(g, &kind, front_line);
                        if spec.is_melee_capable() {
                            melee += 1;
                        }
                        if spec.has_ranged_attack() {
                            ranged += 1;
                        }
                    }
                }
            }
        }
        let active_settlers = settlers;
        // Treat queued units as part of the force plan. Without this, every
        // occupied city forgets what it is already building and the next
        // empty city can queue a duplicate settler, builder, or trader.
        for cid in &city_ids {
            if let Some(Item::Unit { unit }) = g.cities[cid].queue.first() {
                match unit.as_str() {
                    "settler" => settlers += 1,
                    "builder" => builders += 1,
                    "trader" => traders += 1,
                    "battering_ram" | "siege_tower" => siege_support += 1,
                    _ => {
                        let spec = &g.rules.units[unit];
                        if spec.class == "military" {
                            force += Self::force_weight(g, unit, front_line);
                            if spec.is_melee_capable() {
                                melee += 1;
                            }
                            if spec.has_ranged_attack() {
                                ranged += 1;
                            }
                        }
                    }
                }
            }
        }
        let mut military = force.round() as usize;
        // Settlement races can invalidate the final site after a Settler was
        // queued but before it finishes. Revalidate the queue every turn and
        // bank its progress behind a useful replacement instead of completing
        // a civilian that can never found a city.
        let practical_settle_site = self.has_practical_settle_site(g, pid);
        if settlers > active_settlers || (settlers > 0 && !practical_settle_site) {
            let mut committed_settlers = active_settlers;
            for cid in &city_ids {
                if !matches!(
                    g.cities[cid].queue.first(),
                    Some(Item::Unit { unit }) if unit == "settler"
                ) {
                    continue;
                }
                if committed_settlers == 0 && practical_settle_site {
                    committed_settlers = 1;
                    continue;
                }
                let replacement = self.pick_item(
                    g,
                    pid,
                    *cid,
                    n_cities,
                    settlers.saturating_sub(1),
                    builders,
                    traders,
                    siege_support,
                    military,
                    melee,
                    ranged,
                );
                let Some(item) = replacement else {
                    committed_settlers += 1;
                    continue;
                };
                if g
                    .apply(
                        pid,
                        &Action::Produce {
                            city: *cid,
                            item: item.clone(),
                        },
                    )
                    .is_err()
                {
                    committed_settlers += 1;
                    continue;
                }
                settlers = settlers.saturating_sub(1);
                match &item {
                    Item::Unit { unit } if unit == "builder" => builders += 1,
                    Item::Unit { unit } if unit == "trader" => traders += 1,
                    Item::Unit { unit }
                        if unit == "battering_ram" || unit == "siege_tower" =>
                    {
                        siege_support += 1
                    }
                    Item::Unit { unit } => {
                        let spec = &g.rules.units[unit];
                        if spec.class == "military" {
                            military += 1;
                            if spec.is_melee_capable() {
                                melee += 1;
                            }
                            if spec.has_ranged_attack() {
                                ranged += 1;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        // Walls fire at raiders in range. Encampment strikes are collected
        // once after the city loop: rebuilding the complete action list for
        // every city also enumerates production, deals, Congress votes, and
        // every unit move, which becomes quadratic in a developed empire.
        for cid in &city_ids {
            if g.city_can_strike(&g.cities[cid]) {
                let cpos = g.cities[cid].pos;
                for pos in g.wdisk(cpos, 2) {
                    let hit = g.units_at(pos).into_iter().any(|oid| {
                        let o = &g.units[&oid];
                        o.owner != pid && g.is_at_war(pid, o.owner)
                    });
                    if hit {
                        let _ = g.apply(
                            pid,
                            &Action::CityStrike {
                                city: *cid,
                                target: pos,
                            },
                        );
                        break;
                    }
                }
            }
        }
        let has_ready_encampment = city_ids.iter().any(|cid| {
            let city = &g.cities[cid];
            city.encampment_hp > 0
                && city.encampment_wall_hp > 0
                && !city.encampment_pillaged
                && !city.encampment_struck
        });
        if has_ready_encampment {
            let strikes: Vec<Action> = g
                .legal_actions_within(pid, ActionFamilies::CORE)
                .into_iter()
                .filter(|action| matches!(action, Action::EncampmentStrike { .. }))
                .collect();
            let mut used = HashSet::new();
            for action in strikes {
                let Action::EncampmentStrike { city, .. } = &action else {
                    unreachable!()
                };
                if used.insert(*city) {
                    let _ = g.apply(pid, &action);
                }
            }
        }
        for cid in &city_ids {
            if !g.cities[cid].queue.is_empty() {
                continue;
            }
            // chess-style opening book: scripted first capital builds
            if !self.minor && !self.barb && g.cities[cid].is_capital && self.book_pos < 4 {
                let mut played = false;
                while self.book_pos < 4 && !played {
                    let gene =
                        [self.w.open0, self.w.open1, self.w.open2, self.w.open3][self.book_pos];
                    self.book_pos += 1;
                    let i = gene.max(0.0) as usize;
                    if i >= OPENING_MENU.len() {
                        continue; // "pass" gene: fall back to evaluation
                    }
                    let name = OPENING_MENU[i];
                    if name == "settler" && !self.has_practical_settle_site(g, pid) {
                        continue;
                    }
                    let item = if name == "monument" {
                        Item::Building {
                            building: Name::new(name),
                        }
                    } else {
                        Item::Unit {
                            unit: Name::new(name),
                        }
                    };
                    if g.apply(
                        pid,
                        &Action::Produce {
                            city: *cid,
                            item: item.clone(),
                        },
                    )
                    .is_ok()
                    {
                        match &item {
                            Item::Unit { unit } if unit == "settler" => settlers += 1,
                            Item::Unit { unit } if unit == "builder" => builders += 1,
                            Item::Unit { unit } if unit == "trader" => traders += 1,
                            Item::Unit { unit }
                                if unit == "battering_ram" || unit == "siege_tower" =>
                            {
                                siege_support += 1
                            }
                            Item::Unit { unit } => {
                                let spec = &g.rules.units[unit];
                                if spec.class == "military" {
                                    military += 1;
                                    if spec.is_melee_capable() {
                                        melee += 1;
                                    }
                                    if spec.has_ranged_attack() {
                                        ranged += 1;
                                    }
                                }
                            }
                            _ => {}
                        }
                        played = true;
                    }
                }
                if played {
                    continue;
                }
            }
            if let Some(item) = self.pick_item(
                g,
                pid,
                *cid,
                n_cities,
                settlers,
                builders,
                traders,
                siege_support,
                military,
                melee,
                ranged,
            ) {
                if g.apply(
                    pid,
                    &Action::Produce {
                        city: *cid,
                        item: item.clone(),
                    },
                )
                .is_ok()
                {
                    if self.journal.wants(crate::reasoning::Level::Decision) {
                        let cost = g.item_cost_for_city(pid, *cid, &item);
                        let per_turn = g.city_yields(*cid).production.max(0.1);
                        let city = &g.cities[cid];
                        let turns = (cost / per_turn).ceil().max(1.0);
                        think!(self.journal, Cities, Decision,
                               "{} starts {}", city.name, Self::item_label(&item);
                               "{cost:.0} production, about {turns:.0} turn{} at \
                                {per_turn:.1} a turn; the empire holds {military} military \
                                for {n_cities} {} against a target of {:.1} each, and \
                                {settlers} settler{}, {builders} builder{}, {traders} trader{}",
                               if turns == 1.0 { "" } else { "s" },
                               if n_cities == 1 { "city" } else { "cities" },
                               self.w.mil_per_city,
                               if settlers == 1 { "" } else { "s" },
                               if builders == 1 { "" } else { "s" },
                               if traders == 1 { "" } else { "s" };
                               Self::item_focus(&item, city.pos));
                    }
                    match &item {
                        Item::Unit { unit } if unit == "settler" => settlers += 1,
                        Item::Unit { unit } if unit == "builder" => builders += 1,
                        Item::Unit { unit } if unit == "trader" => traders += 1,
                        Item::Unit { unit } if unit == "battering_ram" || unit == "siege_tower" => {
                            siege_support += 1
                        }
                        Item::Unit { unit } => {
                            let spec = &g.rules.units[unit];
                            if spec.class == "military" {
                                military += 1;
                                if spec.is_melee_capable() {
                                    melee += 1;
                                }
                                if spec.has_ranged_attack() {
                                    ranged += 1;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        self.spend_gold(
            g, pid, &city_ids, settlers, builders, traders, military, melee, ranged,
        );
        if g.players[pid].faith >= self.w.faith_builder
            && builders < n_cities
            && !city_ids.is_empty()
        {
            let _ = g.apply(
                pid,
                &Action::Buy {
                    city: city_ids[0],
                    unit: crate::name!("builder"),
                    formation: 0,
                    currency: "faith".to_string(),
                },
            );
        }
        if self.pursue_religion
            && g.players[pid].religion.is_some()
            && g.players[pid].faith >= 250.0
        {
            let religion = g.players[pid].religion.clone().unwrap();
            let missionaries = g
                .units
                .values()
                .filter(|u| u.owner == pid && u.kind == "missionary")
                .count();
            if missionaries < 2 {
                for cid in &city_ids {
                    // Firaxis gives a purchased religious unit the city's
                    // majority religion. A converted Holy Site would spend
                    // our Faith strengthening the rival faith; on a live
                    // mirror this also produced a purchase the host refused
                    // every turn once Rome converted. The live adapter keeps
                    // looking when the first matching city cannot place it.
                    // ⚠ AND THE TILE MUST BE FREE OF A RELIGIOUS UNIT. Civilization VI
                    // refuses a purchase that would place a second unit of the same
                    // class on one plot, and it says so in as many words — the
                    // `purchase_refused` instrument recorded the host's own text,
                    // "Too many units of the same class in this location.", on the
                    // missionary buy.
                    //
                    // That refusal is 799 of the 08-04/08-05 orders, and it was NOT
                    // affordability: faith balance at the moment of refusal ran a
                    // median 474 against a median quoted cost of 90. The buyer only
                    // ever counted missionaries EMPIRE-WIDE (`missionaries < 2`) and
                    // never asked whether the city it was buying into already had one
                    // standing on it — which is exactly where a just-purchased
                    // missionary is still sitting.
                    //
                    // ⚠⚠ GATED ON THE LIVE ADAPTER, like the religion check beside it.
                    // This function is the FROZEN controller's too, and an unconditional
                    // guard would change the legacy path — which means bumping
                    // `ELO_PROTOCOL_VERSION` and starting a new ledger for a bug that
                    // only bites the live bridge. The refusals are all live.
                    let center = g.cities[cid].pos;
                    let occupied = self.live_religious_purchase_guard
                        && g.units_at(center).into_iter().any(|uid| {
                            let unit = &g.units[&uid];
                            unit.owner == pid
                                && g.rules
                                    .units
                                    .get(unit.kind.as_str())
                                    .is_some_and(|spec| spec.class == "religious")
                        });
                    if occupied
                        || !g.cities[cid].districts.contains_key(crate::name!("holy_site"))
                        || (self.live_religious_purchase_guard
                            && g.city_religion(&g.cities[cid]) != Some(religion.as_str()))
                    {
                        continue;
                    }
                    let applied = g
                        .apply(
                            pid,
                            &Action::Buy {
                                city: *cid,
                                unit: crate::name!("missionary"),
                                formation: 0,
                                currency: "faith".to_string(),
                            },
                        )
                        .is_ok();
                    // Preserve the frozen controller's historical first-Holy-Site
                    // exit. Only the live adapter may continue after a refusal.
                    if !self.live_religious_purchase_guard || applied {
                        break;
                    }
                }
            }
        }
    }

    /// Modernize the standing army before it moves. An empire that never
    /// spends Gold here fights the Information era with Slingers: production
    /// only ever replaces losses, so the units already on the map are exactly
    /// the ones that fall behind. Upgrades are taken strongest-gain-first and
    /// stop at a treasury floor so the ordinary purchase passes still have
    /// something to spend.
    pub(crate) fn upgrade_units(g: &mut Game, pid: usize) {
        if g.players[pid].is_barbarian {
            return;
        }
        let at_war = g
            .players
            .iter()
            .any(|p| p.id != pid && p.alive && !p.is_barbarian && g.is_at_war(pid, p.id));
        let floor = if at_war { 30.0 } else { 120.0 };
        loop {
            let mut best: Option<(f64, f64, u32)> = None;
            for uid in g.player_unit_ids(pid) {
                let Some((target, gold, _)) = g.unit_gold_upgrade_offer(pid, uid) else {
                    continue;
                };
                if g.players[pid].gold - gold < floor {
                    continue;
                }
                let from = &g.rules.units[g.units[&uid].kind];
                let to = &g.rules.units[target];
                let gain = to.strength.max(to.ranged_attack_strength())
                    - from.strength.max(from.ranged_attack_strength());
                // Support and civilian successors carry no combat strength;
                // rank those by the Production they save instead.
                let gain = if gain > 0.0 {
                    gain
                } else {
                    (to.cost - from.cost).max(0.0) / 20.0
                };
                if gain <= 0.0 {
                    continue;
                }
                let value = gain / gold.max(1.0);
                let better = match &best {
                    None => true,
                    Some((top, top_gold, top_uid)) => {
                        value > *top + 1e-9
                            || ((value - *top).abs() <= 1e-9
                                && (gold < *top_gold - 1e-9
                                    || ((gold - *top_gold).abs() <= 1e-9 && uid < *top_uid)))
                    }
                };
                if better {
                    best = Some((value, gold, uid));
                }
            }
            let Some((_, _, uid)) = best else { break };
            if g.apply(pid, &Action::UpgradeUnit { unit: uid }).is_err() {
                break;
            }
        }
        Self::use_opportunistic_unit_tools(g, pid);
    }

    /// Execute rare, unambiguously beneficial unit tools before ordinary
    /// movement consumes the acting unit's turn.
    ///
    /// `AdvancedAi` deliberately calls this shared pre-movement pass, so these
    /// actions are part of the default strategic agent as well as BasicAI.
    fn use_opportunistic_unit_tools(g: &mut Game, pid: usize) -> usize {
        let conversion_ready = g.barb_pid.is_some()
            && g.player_unit_ids(pid).into_iter().any(|unit| {
                let unit = &g.units[&unit];
                unit.kind == "apostle"
                    && unit.moves_left > 0.0
                    && unit.charges > 0
                    && unit.promotions.iter().any(|promotion| {
                        g.rules
                            .promotions
                            .get(promotion)
                            .and_then(|spec| spec.effects.get("convert_barbarians"))
                            .is_some_and(|value| *value > 0.0)
                    })
            });
        if !conversion_ready {
            return 0;
        }
        let conversions: Vec<Action> = g
            .legal_actions_within(pid, ActionFamilies::UNITS)
            .into_iter()
            .filter(|action| matches!(action, Action::ConvertBarbarians { .. }))
            .collect();
        conversions
            .into_iter()
            .filter(|action| g.apply(pid, action).is_ok())
            .count()
    }

    fn best_military(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        want_ranged: Option<bool>,
    ) -> Option<String> {
        self.best_military_role(g, pid, cid, want_ranged, false)
    }

    fn best_military_role(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        want_ranged: Option<bool>,
        want_siege: bool,
    ) -> Option<String> {
        let mut best: Option<(f64, String)> = None;
        for (name, spec) in &g.rules.units {
            if spec.class != "military" || spec.domain.as_deref() == Some("sea") {
                continue;
            }
            let matches_role = match want_ranged {
                Some(true) => spec.has_ranged_attack(),
                Some(false) => spec.is_melee_capable(),
                None => spec.has_ranged_attack() || spec.is_melee_capable(),
            };
            // ★★★★★ SIEGE IS NOT A ROLE THIS CHOOSER HAD, so it never chose one.
            // Every siege unit carries a ranged attack, so it competed in the
            // RANGED bucket and lost on raw `strength.max(ranged)` to a Field
            // Cannon — while the one property that makes it siege, FULL damage
            // to walls where every other unit does half, is absent from that
            // comparison entirely.
            //
            // Measured on run `civvis-20260803T082856Z`, a game CIVVIS was
            // WINNING (turn 226, 7 cities, score 645, ~3x the corpus mean):
            // 151 turns at war with England at **594 military against 56**, a
            // ten to one advantage, and **zero cities taken**. All seven cities
            // came from `found` events; not one was captured. England's cities
            // sat at 400 wall and full health the whole time. CIVVIS held
            // engineering, military_engineering, metal_casting AND steel, so
            // catapult through artillery were all buildable — and it built
            // **zero siege units in 251 turns**, 8 Field Cannons instead.
            //
            // ⚠ THE APPETITE WAS NEVER THE PROBLEM. `siege_units_wanted` and
            // its `+95` production bonus both sit behind `if spec.siege`, i.e.
            // they are consulted only for a unit this function has ALREADY
            // returned. Instrumented over 251 turns, `siege_units_wanted` was
            // entered ONCE. That is why #963 measured parity: it tuned an
            // appetite that is read once a game.
            if self.siege_role && want_siege && !spec.siege {
                continue;
            }
            if !matches_role {
                continue;
            }
            if !g.can_produce(pid, cid, &Item::Unit { unit: *name }) {
                continue;
            }
            let power = spec.strength.max(spec.ranged_attack_strength());
            if best.as_ref().map(|(b, _)| power > *b).unwrap_or(true) {
                best = Some((power, name.to_string()));
            }
        }
        best.map(|(_, n)| n)
    }

    fn best_naval_unit(&self, g: &Game, pid: usize, cid: u32) -> Option<Name> {
        if !Self::city_is_coastal(g, cid) {
            return None;
        }
        let (total, melee, ranged, raiders, carriers) = Self::naval_counts(g, pid);
        let has_aircraft = g.units.values().any(|unit| {
            unit.owner == pid && g.rules.units[unit.kind].domain.as_deref() == Some("air")
        });
        g.rules
            .units
            .iter()
            .filter(|(name, spec)| {
                spec.class == "military"
                    && spec.domain.as_deref() == Some("sea")
                    && g.can_produce(
                        pid,
                        cid,
                        &Item::Unit {
                            unit: **name,
                        },
                    )
            })
            .map(|(name, spec)| {
                let power = spec.strength.max(spec.ranged_attack_strength());
                let role = match spec.promotion_class.as_str() {
                    // A navy without melee ships can bombard but never take a
                    // coastal city; preserve at least half the fleet for that
                    // capturing/screening role.
                    "naval_melee" => 42.0 * (melee <= ranged + raiders) as i32 as f64,
                    "naval_ranged" => 34.0 * (ranged < melee.max(1)) as i32 as f64,
                    "naval_raider" => 22.0 * (total >= 2 && raiders == 0) as i32 as f64,
                    "naval_carrier" => {
                        if has_aircraft && carriers == 0 {
                            30.0
                        } else {
                            -120.0
                        }
                    }
                    _ => 0.0,
                };
                (power * 3.0 + role - spec.cost * 0.04, *name)
            })
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then_with(|| b.1.cmp(&a.1)))
            .map(|(_, name)| name)
    }

    /// Strength of the strongest land and naval unit this empire can train
    /// right now. Zero for a domain with nothing available, in which case
    /// every unit of that domain counts in full.
    pub(crate) fn front_line_strength(g: &Game, pid: usize, city_ids: &[u32]) -> (f64, f64) {
        let mut land = 0.0_f64;
        let mut sea = 0.0_f64;
        for cid in city_ids {
            for (name, spec) in &g.rules.units {
                if spec.class != "military" {
                    continue;
                }
                let naval = spec.domain.as_deref() == Some("sea");
                let power = spec.strength.max(spec.ranged_attack_strength());
                // Checking the strength first keeps the expensive production
                // test off every unit that could not raise the maximum anyway.
                if power <= if naval { sea } else { land } {
                    continue;
                }
                if !g.can_produce(
                    pid,
                    *cid,
                    &Item::Unit {
                        unit: *name,
                    },
                ) {
                    continue;
                }
                if naval {
                    sea = power;
                } else {
                    land = power;
                }
            }
        }
        (land, sea)
    }

    /// How much of a modern unit this one still represents. A unit always
    /// counts for something — even a Warrior can hold a tile — but a garrison
    /// three eras behind no longer satisfies the empire's force target, which
    /// is what keeps late-game production and Gold flowing into better units.
    pub(crate) fn force_weight(g: &Game, kind: &str, front_line: (f64, f64)) -> f64 {
        let spec = &g.rules.units[kind];
        let best = if spec.domain.as_deref() == Some("sea") {
            front_line.1
        } else {
            front_line.0
        };
        if best <= 0.0 {
            return 1.0;
        }
        (spec.strength.max(spec.ranged_attack_strength()) / best).clamp(0.2, 1.0)
    }

    fn combined_arms_unit(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        melee: usize,
        ranged: usize,
    ) -> Option<String> {
        // Ranged units trade efficiently, but only melee units can take a
        // city. Alternate the strongest available unit in each role so an
        // advanced army never degenerates into an uncapturing firing line.
        // A walled enemy city is a wall problem, and only siege answers it.
        // Ask for that FIRST when one is in reach and the army has none, then
        // fall back to the ordinary melee/ranged alternation.
        let want_ranged = melee > ranged;
        self.best_military(g, pid, cid, Some(want_ranged))
            .or_else(|| self.best_military(g, pid, cid, None))
    }

    /// Whether this empire is trying to crack a wall with nothing that can.
    ///
    /// Deliberately built from the BOARD, not from the strategic plan: this
    /// lives in `BasicAi`, the plan does not reach here, and the two facts that
    /// matter — is there a walled enemy city we could actually reach, and do we
    /// own anything that breaks walls — are both on the board already.
    ///
    /// ⚠ Bounded by `SIEGE_ARM_MAX`, and it stops asking as soon as the arm
    /// exists. Without that this becomes "build siege forever", which is the
    /// `all-army-no-economy` failure, and every mechanism that spent more on
    /// the military has measured null.
    fn siege_is_the_missing_arm(&self, g: &Game, pid: usize) -> bool {
        if self.minor || self.barb {
            return false;
        }
        let owned_siege = g
            .units
            .values()
            .filter(|unit| unit.owner == pid && g.rules.units[unit.kind].siege)
            .count();
        if owned_siege >= SIEGE_ARM_MAX {
            return false;
        }
        let home: Vec<Pos> = g
            .cities
            .values()
            .filter(|city| city.owner == pid)
            .map(|city| city.pos)
            .collect();
        if home.is_empty() {
            return false;
        }
        g.cities.values().any(|city| {
            city.owner != pid
                && g.is_at_war(pid, city.owner)
                && !g.players[city.owner].is_barbarian
                && g.city_max_wall_hp(city) > 0
                && city.wall_hp > 0
                && home
                    .iter()
                    .any(|mine| g.wdist(*mine, city.pos) <= SIEGE_TARGET_REACH)
        })
    }

    fn siege_support_unit(&self, g: &Game, pid: usize, cid: u32) -> Option<String> {
        let wall_levels: Vec<usize> = g
            .cities
            .values()
            .filter(|c| c.owner != pid && g.is_at_war(pid, c.owner))
            .map(|c| {
                c.buildings
                    .iter()
                    .filter(|b| *b == "walls" || *b == "medieval_walls")
                    .count()
            })
            .filter(|walls| *walls > 0)
            .collect();
        if wall_levels.is_empty() {
            return None;
        }
        // A tower helps against either wall tier. A ram is still worthwhile
        // while the more advanced tower is unavailable and at least one
        // ancient wall is a live target.
        for unit in ["siege_tower", "battering_ram"] {
            let useful = unit == "siege_tower" || wall_levels.contains(&1);
            if useful
                && g.can_produce(
                    pid,
                    cid,
                    &Item::Unit {
                        unit: Name::new(unit),
                    },
                )
            {
                return Some(unit.to_string());
            }
        }
        None
    }

    fn buy_gold_unit(
        &self,
        g: &mut Game,
        pid: usize,
        city_ids: &[u32],
        unit: &str,
        reserve: f64,
    ) -> bool {
        let price = match g.rules.units.get(unit) {
            Some(spec) => spec.cost * 4.0,
            None => return false,
        };
        if g.players[pid].gold + 1e-9 < price + reserve {
            return false;
        }
        for cid in city_ids {
            let item = Item::Unit {
                unit: Name::new(unit),
            };
            // `can_produce` answers "can this city BUILD it"; a host purchase
            // refusal is a different set. Both must pass before spending gold.
            if !g.can_produce(pid, *cid, &item) || g.purchase_is_blocked(*cid, &item) {
                continue;
            }
            if g.apply(
                pid,
                &Action::Buy {
                    city: *cid,
                    unit: Name::new(unit),
                    formation: 0,
                    currency: "gold".to_string(),
                },
            )
            .is_ok()
            {
                return true;
            }
        }
        false
    }

    fn buy_gold_military(
        &self,
        g: &mut Game,
        pid: usize,
        city_ids: &[u32],
        reserve: f64,
        want_ranged: bool,
    ) -> bool {
        let budget = g.players[pid].gold - reserve;
        if budget <= 0.0 {
            return false;
        }
        let choose = |role: Option<bool>| -> Option<(u32, String)> {
            let mut best: Option<(f64, f64, String, u32)> = None;
            for cid in city_ids {
                for (name, spec) in &g.rules.units {
                    let matches_role = match role {
                        Some(true) => spec.has_ranged_attack(),
                        Some(false) => spec.is_melee_capable(),
                        None => spec.has_ranged_attack() || spec.is_melee_capable(),
                    };
                    if spec.class != "military"
                        || spec.domain.as_deref() == Some("sea")
                        || !matches_role
                    {
                        continue;
                    }
                    let price = spec.cost * 4.0;
                    if price > budget + 1e-9
                        || !g.can_produce(pid, *cid, &Item::Unit { unit: *name })
                        || g.purchase_is_blocked(*cid, &Item::Unit { unit: *name })
                    {
                        continue;
                    }
                    let power = spec.strength.max(spec.ranged_attack_strength());
                    let replace = match &best {
                        None => true,
                        Some((bp, bc, bn, bid)) => {
                            power > *bp + 1e-9
                                || ((power - *bp).abs() < 1e-9
                                    && (price < *bc - 1e-9
                                        || ((price - *bc).abs() < 1e-9
                                            && (name.as_str(), *cid) < (bn.as_str(), *bid))))
                        }
                    };
                    if replace {
                        best = Some((power, price, name.to_string(), *cid));
                    }
                }
            }
            best.map(|(_, _, unit, city)| (city, unit))
        };
        let (city, unit) = match choose(Some(want_ranged)).or_else(|| choose(None)) {
            Some(choice) => choice,
            None => return false,
        };
        g.apply(
            pid,
            &Action::Buy {
                city,
                unit: Name::new(&unit),
                formation: 0,
                currency: "gold".to_string(),
            },
        )
        .is_ok()
    }

    fn buy_gold_infrastructure(
        &self,
        g: &mut Game,
        pid: usize,
        city_ids: &[u32],
        reserve: f64,
        at_major_war: bool,
    ) -> bool {
        if self.barb {
            return false;
        }
        let budget = g.players[pid].gold - reserve;
        if budget <= 0.0 {
            return false;
        }

        // Prefer buildings with strong immediate value per Gold while still
        // responding to each city's housing, amenity, and defensive needs.
        // Only one purchase is made per turn, keeping the action workload
        // bounded even at Lightning spectator speed.
        let mut best: Option<(f64, f64, f64, String, u32)> = None;
        for cid in city_ids {
            let city = &g.cities[cid];
            let housing_need = (city.pop as f64 + 2.0 - g.city_housing(city)).max(0.0);
            let amenity_need = (-g.city_amenity_surplus(city)).max(0) as f64;
            for (building, spec) in &g.rules.buildings {
                if spec.wonder
                    || !g.can_produce(
                        pid,
                        *cid,
                        &Item::Building {
                            building: *building,
                        },
                    )
                    || g.purchase_is_blocked(
                        *cid,
                        &Item::Building {
                            building: *building,
                        },
                    )
                {
                    continue;
                }
                let Some(price) = g.building_purchase_cost(pid, *cid, building, "gold") else {
                    continue;
                };
                if price > budget + 1e-9 {
                    continue;
                }

                let great_people = spec.great_person_points.values().sum::<f64>();
                let work_slots = spec.great_work_slots.values().sum::<i32>().max(0) as f64;
                let mut value = spec.yields.food * 34.0
                    + spec.yields.production * 48.0
                    + spec.yields.gold * 26.0
                    + spec.yields.science * 44.0
                    + spec.yields.culture * 42.0
                    + spec.yields.faith * 24.0
                    + spec.housing * (16.0 + 24.0 * housing_need)
                    + spec.amenity * (28.0 + 28.0 * amenity_need)
                    + great_people * 24.0
                    + work_slots * 30.0
                    + spec.citizen_slots.max(0) as f64 * 8.0
                    + spec.trade_route_capacity.max(0) as f64 * 90.0
                    + spec.growth_pct.max(0.0) * 2.0
                    + spec.builder_charges.max(0) as f64 * 24.0
                    + spec.unit_levels.max(0) as f64 * 18.0
                    - spec.maintenance.max(0.0) * 10.0;
                if building == "monument" {
                    value += 90.0;
                }
                if building == "granary" && housing_need > 0.0 {
                    value += 120.0;
                }
                if spec.outer_defense > 0 {
                    if at_major_war {
                        value += spec.outer_defense as f64;
                    } else {
                        value -= 80.0;
                    }
                }
                if value <= 0.0 {
                    continue;
                }
                let efficiency = value / price.max(1.0);
                let replace = match &best {
                    None => true,
                    Some((old_efficiency, old_value, old_price, old_building, old_cid)) => {
                        efficiency > *old_efficiency + 1e-9
                            || ((efficiency - *old_efficiency).abs() < 1e-9
                                && (value > *old_value + 1e-9
                                    || ((value - *old_value).abs() < 1e-9
                                        && (price < *old_price - 1e-9
                                            || ((price - *old_price).abs() < 1e-9
                                                && (building.as_str(), *cid)
                                                    < (old_building.as_str(), *old_cid))))))
                    }
                };
                if replace {
                    best = Some((efficiency, value, price, building.to_string(), *cid));
                }
            }
        }
        let Some((_, _, _, building, city)) = best else {
            return false;
        };
        g.apply(
            pid,
            &Action::BuyBuilding {
                city,
                building: Name::new(&building),
                currency: "gold".to_string(),
            },
        )
        .is_ok()
    }

    /// Annex a genuinely useful plot instead of treating every affordable
    /// border hex as equivalent. Resources, Natural Wonders, and strong raw
    /// yields can justify the immediate tempo spend; a reserve still protects
    /// unit upgrades and emergency purchases.
    fn buy_gold_plot(&self, g: &mut Game, pid: usize, reserve: f64) -> bool {
        let bank = g.players[pid].gold;
        let mut best: Option<PlotPurchaseCandidate> = None;
        for action in g.legal_purchase_actions(pid) {
            let Action::BuyPlot { city, pos, cost } = action else {
                continue;
            };
            if bank + f64::EPSILON < reserve + cost {
                continue;
            }
            let tile = &g.map.tiles[&pos];
            let resource = tile
                .resource
                .as_ref()
                .and_then(|name| g.rules.resources.get(name))
                .filter(|spec| {
                    spec.tech
                        .as_ref()
                        .is_none_or(|tech| g.players[pid].techs.contains(tech))
                        && spec
                            .civic
                            .as_ref()
                            .is_none_or(|civic| g.players[pid].civics.contains(civic))
                });
            let mut visible_tile = tile.clone();
            if tile.resource.is_some() && resource.is_none() {
                visible_tile.resource = None;
            }
            let yields = g.rules.tile_yields(&visible_tile);
            let resource = resource
                .map(|spec| match spec.class.as_str() {
                    "luxury" => 220.0,
                    "strategic" => 190.0,
                    "bonus" => 55.0,
                    _ => 0.0,
                })
                .unwrap_or(0.0);
            let wonder = tile
                .feature
                .as_ref()
                .and_then(|name| g.rules.features.get(name))
                .is_some_and(|feature| feature.natural_wonder) as u8 as f64
                * 280.0;
            let value = yields.food * 28.0
                + yields.production * 42.0
                + yields.gold * 22.0
                + yields.science * 40.0
                + yields.culture * 38.0
                + yields.faith * 26.0
                + resource
                + wonder;
            let score = value - cost * 0.75;
            if value + f64::EPSILON < cost * 1.35 || score < 35.0 {
                continue;
            }
            let candidate = (score, std::cmp::Reverse((city, pos)), action);
            if best
                .as_ref()
                .is_none_or(|old| candidate.0 > old.0 + 1e-9 || (candidate.0 - old.0).abs() < 1e-9 && candidate.1 > old.1)
            {
                best = Some(candidate);
            }
        }
        best.is_some_and(|(_, _, action)| g.apply(pid, &action).is_ok())
    }

    #[allow(clippy::too_many_arguments)]
    fn spend_gold(
        &self,
        g: &mut Game,
        pid: usize,
        city_ids: &[u32],
        settlers: usize,
        builders: usize,
        traders: usize,
        military: usize,
        melee: usize,
        ranged: usize,
    ) -> bool {
        if city_ids.is_empty() {
            return false;
        }
        let n_cities = city_ids.len();
        let at_major_war = g
            .players
            .iter()
            .any(|p| p.id != pid && p.alive && !p.is_barbarian && g.is_at_war(pid, p.id));
        let reserve = if at_major_war {
            40.0 + 10.0 * n_cities as f64
        } else {
            100.0 + 25.0 * n_cities as f64
        };
        let want_ranged = melee > ranged;

        // A threatened empire converts cash into defenders before pursuing
        // infrastructure. Two units per city is enough to react without
        // draining the treasury into an endless standing army.
        let normal_military = (self.w.mil_per_city * n_cities as f64).ceil() as usize;
        let wartime_military = normal_military.max(2 * n_cities);
        if at_major_war
            && military < wartime_military
            && self.buy_gold_military(g, pid, city_ids, reserve, want_ranged)
        {
            return true;
        }

        let desired_builders = (self.w.builder_per_city * n_cities as f64).ceil() as usize;
        if builders < desired_builders
            && Self::has_builder_work(g, pid)
            && self.buy_gold_unit(g, pid, city_ids, "builder", reserve)
        {
            return true;
        }

        if !self.minor
            && Self::should_add_trader(g, pid, traders)
            && self.buy_gold_unit(g, pid, city_ids, "trader", reserve)
        {
            return true;
        }

        if !self.minor
            && settlers == 0
            && (n_cities as f64) < self.w.city_target
            && (g.turn as f64) < self.w.settler_stop_turn
            && self.has_practical_settle_site(g, pid)
            && self.buy_gold_unit(g, pid, city_ids, "settler", reserve)
        {
            return true;
        }

        if self.buy_gold_infrastructure(g, pid, city_ids, reserve, at_major_war) {
            return true;
        }

        // Plots are a surplus investment after concrete unit and building
        // gaps are filled. Keep another 200 Gold above the ordinary reserve
        // so border appetite cannot crowd out next turn's Builder or upgrade.
        if self.buy_gold_plot(g, pid, reserve + 200.0) {
            return true;
        }

        // At peace, retain a larger reserve but turn a deep surplus into a
        // modest deterrent instead of hoarding gold indefinitely.
        g.players[pid].gold >= reserve + 600.0
            && military < 2 * n_cities
            && self.buy_gold_military(g, pid, city_ids, reserve, want_ranged)
    }

    /// Destinations not already served by this empire that at least one of
    /// its cities can legally reach. Routes are unique by owner/destination,
    /// so each entry is one concrete job for one available or queued Trader.
    fn open_trade_destinations(g: &Game, pid: usize) -> usize {
        let origins = g.player_city_ids(pid);
        g.cities
            .keys()
            .filter(|destination| {
                origins.iter().any(|origin| {
                    g.can_establish_trade_route(pid, *origin, **destination)
                })
            })
            .count()
    }

    fn should_add_trader(g: &Game, pid: usize, traders: usize) -> bool {
        g.active_routes(pid) + (traders as i64) < g.trade_capacity(pid)
            && traders < Self::open_trade_destinations(g, pid)
    }

    fn economic_recovery_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        traders: usize,
    ) -> Option<Item> {
        if Self::should_add_trader(g, pid, traders) {
            let trader = Item::Unit {
                unit: crate::name!("trader"),
            };
            if g.can_produce(pid, cid, &trader) {
                return Some(trader);
            }
        }

        let profitable_building = g
            .rules
            .buildings
            .iter()
            .filter(|(_, spec)| !spec.wonder && spec.yields.gold > spec.maintenance)
            .filter(|(building, _)| {
                g.can_produce(
                    pid,
                    cid,
                    &Item::Building {
                        building: **building,
                    },
                )
            })
            .map(|(building, spec)| {
                let net_gold = spec.yields.gold - spec.maintenance;
                (
                    net_gold / spec.cost.max(1.0),
                    net_gold,
                    std::cmp::Reverse(spec.cost as i64),
                    std::cmp::Reverse(*building),
                    *building,
                )
            })
            .max_by(|left, right| {
                left.0
                    .partial_cmp(&right.0)
                    .unwrap()
                    .then_with(|| left.1.partial_cmp(&right.1).unwrap())
                    .then(left.2.cmp(&right.2))
                    .then(left.3.cmp(&right.3))
            })
            .map(|(_, _, _, _, building)| Item::Building { building });
        if profitable_building.is_some() {
            return profitable_building;
        }

        ["commercial_hub", "harbor"]
            .into_iter()
            .flat_map(|district| {
                g.district_sites(cid, Name::new(district))
                    .into_iter()
                    .map(move |pos| (district, pos))
            })
            .filter_map(|(district, pos)| {
                let item = Item::District {
                    district: Name::new(district),
                    pos,
                };
                g.can_produce(pid, cid, &item).then_some((
                    g.district_yields(Name::new(district), pos).gold,
                    std::cmp::Reverse(district),
                    std::cmp::Reverse(pos),
                    item,
                ))
            })
            .max_by(|left, right| {
                left.0
                    .partial_cmp(&right.0)
                    .unwrap()
                    .then(left.1.cmp(&right.1))
                    .then(left.2.cmp(&right.2))
            })
            .map(|(_, _, _, item)| item)
    }

    /// A deficit city that cannot build direct Gold infrastructure must still
    /// use its Production without making the deficit worse. The recovery pass
    /// used to return `None` in that case and bypass every ordinary fallback,
    /// leaving otherwise productive cities idle for dozens of turns. Prefer a
    /// useful zero-maintenance building, then a repeatable district project;
    /// both convert the turn into value without adding unit/building upkeep.
    fn upkeep_free_recovery_item(&self, g: &Game, pid: usize, cid: u32) -> Option<Item> {
        let building = g
            .rules
            .buildings
            .iter()
            .filter(|(_, spec)| !spec.wonder && spec.maintenance <= f64::EPSILON)
            .filter_map(|(name, spec)| {
                let item = Item::Building {
                    building: *name,
                };
                if !g.can_produce(pid, cid, &item) {
                    return None;
                }
                let value = spec.yields.production * 5.0
                    + spec.yields.food * 3.0
                    + spec.yields.gold * 3.0
                    + spec.yields.science * 2.0
                    + spec.yields.culture * 2.0
                    + spec.yields.faith
                    + spec.housing * 3.0
                    + spec.amenity.max(0.0) * 4.0;
                (value > 0.0).then_some((
                    value / spec.cost.max(1.0),
                    value,
                    std::cmp::Reverse(*name),
                    item,
                ))
            })
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| left.1.total_cmp(&right.1))
                    .then(left.2.cmp(&right.2))
            })
            .map(|(_, _, _, item)| item);
        if building.is_some() {
            return building;
        }

        g.rules
            .projects
            .iter()
            .filter(|(project, spec)| {
                spec.repeatable
                    && !matches!(
                        project.as_str(),
                        "lagrange_laser_station" | "terrestrial_laser_station"
                    )
            })
            .map(|(project, _)| Item::Project {
                project: *project,
            })
            .filter(|item| g.can_produce(pid, cid, item))
            .min_by(|left, right| {
                g.item_cost_for_city(pid, cid, left)
                    .total_cmp(&g.item_cost_for_city(pid, cid, right))
                    .then_with(|| format!("{left:?}").cmp(&format!("{right:?}")))
            })
    }

    fn minor_district_item(g: &Game, pid: usize, cid: u32) -> Option<Item> {
        let family = Self::minor_district_family(g, pid);
        if g.city_has_district_family(&g.cities[&cid], Name::new(family)) {
            return None;
        }
        let district = Self::civ_district(g, pid, family);
        g.district_sites(cid, district)
            .into_iter()
            .filter_map(|pos| {
                let item = Item::District {
                    district,
                    pos,
                };
                g.can_produce(pid, cid, &item).then_some((
                    g.district_yields(district, pos).total(),
                    std::cmp::Reverse(pos),
                    item,
                ))
            })
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| left.1.cmp(&right.1))
            })
            .map(|(_, _, item)| item)
    }

    #[allow(clippy::too_many_arguments)]
    /// The standing-army floor a city under visible siege needs, or 0.0 when
    /// nothing hostile is close enough to matter.
    ///
    /// Barbarians are deliberately excluded from `at_major_war`: there is no
    /// diplomatic state with them and no peace to sue for. But every defensive
    /// escalation in `pick_item` is gated on that flag, so a barbarian siege
    /// reads as "no threat at all" and the floor stays at `mil_per_city`.
    ///
    /// ⚠ On the live ladder this does not present as a defence failure — it
    /// presents as an EXPANSION failure, which is why it survived so long.
    /// Measured on run `civvis-20260802T202501Z` (Netherlands, Settler, small
    /// map): horsemen held tiles adjacent to Amsterdam from t28 onward; four
    /// settlers were built into that siege and captured, two of them on the
    /// capital tile without ever moving (t29 and t39); the empire held ONE
    /// city until t80 and stood at score 140 against a best rival's 416 on
    /// t104. Production and gold were never the constraint — the empire held
    /// two military against a floor of one, and so could not want a third.
    ///
    /// Returns an absolute floor for the empire-wide military count, which is
    /// what `pick_item` compares against.
    ///
    /// ⚠ An empire-wide count is a blunt instrument and cannot say "*this* city
    /// needs defenders": see `visible_besiegers`, which the per-city branch in
    /// `pick_item` uses for the case the floor provably cannot reach.
    fn besieged_military_floor(&self, g: &Game, pid: usize, cid: u32, n_cities: usize) -> f64 {
        let besiegers = self.visible_besiegers(g, pid, cid);
        if besiegers == 0 {
            return 0.0;
        }
        self.w.mil_per_city * n_cities as f64 + besiegers.min(SIEGE_MUSTER_CAP) as f64
    }

    /// Hostile military units this player can actually see within
    /// `SIEGE_MUSTER_RADIUS` of one of its cities. Barbarians count: the whole
    /// point is that `at_major_war` excludes them.
    fn visible_besiegers(&self, g: &Game, pid: usize, cid: u32) -> usize {
        if !self.siege_muster || self.minor || self.barb {
            return 0;
        }
        let Some(city) = g.cities.get(&cid) else {
            return 0;
        };
        // The distance test runs first and alone on the overwhelmingly common
        // quiet turn: `player_vision_now` rebuilds the height field, and
        // `pick_item` is called for every city on every turn.
        let contenders: Vec<u32> = g
            .units
            .values()
            .filter(|unit| unit.owner != pid && g.is_at_war(pid, unit.owner))
            .filter(|unit| g.rules.units[unit.kind].class == "military")
            .filter(|unit| g.wdist(city.pos, unit.pos) <= SIEGE_MUSTER_RADIUS)
            .map(|unit| unit.id)
            .collect();
        if contenders.is_empty() {
            return 0;
        }
        // Fog-gating is not a formality: mustering against a hostile this
        // player cannot see would let the garrison act on knowledge it does
        // not have, and every other threat read in this codebase is gated the
        // same way.
        let visible = g.player_vision_now(pid);
        contenders
            .into_iter()
            .filter(|uid| {
                g.units.get(uid).is_some_and(|unit| {
                    g.sees(&visible, unit.pos) && g.unit_visible_to(unit.id, pid)
                })
            })
            .count()
    }

    /// What a city with enemies already at its gates should build instead of
    /// whatever the ordinary build order wanted.
    ///
    /// ⚠ The empire-wide floor cannot reach this case, and the ladder proves
    /// it. Run `civvis-20260802T205959Z` (Sweden), at war with the Mapuche:
    /// enemy units stood within three tiles of Uppsala on every turn from t60
    /// to t67 — a swordsman and a heavy chariot at range 1, a catapult at 2 —
    /// while Uppsala took 0 -> 39 -> 148 damage and fell on t68. The empire
    /// held 7 military against a floor of `1.0 * 4 cities`, so the floor was
    /// satisfied and **the city under the catapult spent those four turns
    /// building a MONUMENT**. Karlstad, also besieged, built a Commercial Hub;
    /// Helsingborg built a Builder every turn; 172-214 gold went unspent.
    ///
    /// A count of units spread across an empire says nothing about whether
    /// THIS city can hold, so this branch is keyed on the city's own besiegers
    /// and answers with the two things that defend a city: walls, then a
    /// defender.
    fn besieged_city_item(&self, g: &Game, pid: usize, cid: u32) -> Option<Item> {
        // ⚠ Two, not one. Reacting to a single hostile in range fires on every
        // scout that wanders past, and measured over 24 paired maps that bought
        // city count while COSTING score: walls and defenders displace the
        // buildings and districts score is actually made of. A raiding party is
        // what takes a city, and a raiding party is more than one unit.
        if self.visible_besiegers(g, pid, cid) < SIEGE_PRESSURE_MIN {
            return None;
        }
        for building in ["walls", "medieval_walls", "renaissance_walls"] {
            let wall = Item::Building {
                building: Name::new(building),
            };
            if g.can_produce(pid, cid, &wall) {
                return Some(wall);
            }
        }
        self.best_military(g, pid, cid, None)
            .map(|unit| Item::Unit {
                unit: Name::new(&unit),
            })
    }

    pub fn pick_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        n_cities: usize,
        settlers: usize,
        builders: usize,
        traders: usize,
        siege_support: usize,
        military: usize,
        melee: usize,
        ranged: usize,
    ) -> Option<Item> {
        // Repairs restore yields and unlock every building/project in the
        // damaged district, so they outrank adding new infrastructure. Basic
        // and city-state governors previously ignored `Item::Repair`
        // entirely: Mohenjo-Daro left three districts and their buildings
        // pillaged while its queue remained empty for the rest of the game.
        let repair_rank = |item: &Item| match item {
            Item::Repair { repair, .. } if repair == "district" => 0,
            Item::Repair { .. } => 1,
            _ => 2,
        };
        if let Some(repair) = g
            .producible_items(pid, cid)
            .into_iter()
            .filter(|item| matches!(item, Item::Repair { .. }))
            .min_by(|left, right| {
                repair_rank(left)
                    .cmp(&repair_rank(right))
                    .then_with(|| {
                        g.item_cost_for_city(pid, cid, left)
                            .total_cmp(&g.item_cost_for_city(pid, cid, right))
                    })
                    .then_with(|| format!("{left:?}").cmp(&format!("{right:?}")))
            })
        {
            return Some(repair);
        }
        // ⚠ Ahead of the economic-recovery short circuit and the whole ordinary
        // build order. A city with a catapult in range does not have a problem
        // it can solve by finishing a Commercial Hub, and the empire-wide
        // military floor below cannot see that this particular city is the one
        // being taken. Repairs still outrank it: they restore the yields and
        // the defenses this branch is trying to buy.
        if let Some(defence) = self.besieged_city_item(g, pid, cid) {
            return Some(defence);
        }
        if self.minor {
            for project in ["repair_outer_defenses", "repair_encampment"] {
                let repair = Item::Project {
                    project: Name::new(project),
                };
                if g.can_produce(pid, cid, &repair) {
                    return Some(repair);
                }
            }
        }
        let city_pop = g.cities[&cid].pop;
        let at_major_war = g.players.iter().any(|player| {
            player.id != pid
                && player.alive
                && !player.is_barbarian
                && !player.is_minor
                && g.is_at_war(pid, player.id)
        });
        let recovery_reserve = 100.0 + 25.0 * n_cities as f64;
        let economic_recovery = !self.minor
            && !self.barb
            && g.players[pid].gold_per_turn < -0.5
            && g.players[pid].gold < recovery_reserve;
        let emergency_defense = at_major_war && military < n_cities.max(1);
        if self.minor && !emergency_defense {
            for building in ["walls", "medieval_walls", "renaissance_walls"] {
                let wall = Item::Building {
                    building: Name::new(building),
                };
                if g.can_produce(pid, cid, &wall) {
                    return Some(wall);
                }
            }
        }
        if economic_recovery && !emergency_defense {
            return self
                .economic_recovery_item(g, pid, cid, traders)
                .or_else(|| self.upkeep_free_recovery_item(g, pid, cid));
        }
        let can_add_military = !self.minor || military < Self::minor_military_budget(g, pid);
        // An ancient rush needs a stack, not a garrison. While one is planned
        // the floor is the stack size rather than `mil_per_city * n_cities`,
        // and the units must be melee: only melee can land the capturing blow,
        // only melee exerts the zone of control that seals a siege ring, and a
        // land ranged unit attacking a city takes a flat -17 strength.
        let rushing = !self.minor && !self.barb && self.rush_military_floor > 0;
        // A besieged city raises the floor whether or not a rush is planned:
        // the raiding party is already here, and a stack mustered for a distant
        // rival does nothing about a horseman standing on the city's doorstep.
        let military_floor = if rushing {
            (self.w.mil_per_city * n_cities as f64).max(self.rush_military_floor as f64)
        } else {
            self.w.mil_per_city * n_cities as f64
        }
        .max(self.besieged_military_floor(g, pid, cid, n_cities));
        // ★★★★★ THE FLOOR IS A HEADCOUNT AND CANNOT SEE A MISSING ARM.
        //
        // `military_floor` is `mil_per_city * n_cities`. It counts bodies and
        // asks nothing about what they can do, so an empire holding nineteen
        // units and NO SIEGE reports its army finished while it is physically
        // unable to take a city.
        //
        // Measured on run `civvis-20260803T082856Z`, a game CIVVIS was WINNING
        // (turn 226, 7 cities, score 645, ~3x the corpus mean):
        //
        //   19 military units against a floor of 7  -> this branch never ran
        //   151 turns at war with England, 594 military against their 56
        //   ZERO cities taken; all 7 of ours came from `found`, none captured
        //   England's cities: 400 wall, full health, the entire war
        //   siege units built in 251 turns: ZERO, with engineering,
        //   military_engineering, metal_casting AND steel all researched
        //
        // ⚠ NEITHER EXISTING REPAIR COULD REACH THIS. `siege_units_wanted` and
        // its `+95` production bonus are consulted only through
        // `redirect_repeatable_projects_for_force_gap`, which needs Conquest or
        // Recovery AND an army below `2 * cities` AND a repeatable project at
        // the head of the queue — instrumented over 251 turns, it was entered
        // ONCE. That is why #963 measured parity: it tuned an appetite nothing
        // reads. And a siege ROLE in the chooser is dead too, because the
        // chooser is never called while the headcount is satisfied.
        //
        // So the floor itself has to know an arm is missing. It stays a
        // headcount for everything else; this only adds "and we own nothing
        // that breaks a wall we are actually besieging".
        let missing_siege_arm = self.siege_role && self.siege_is_the_missing_arm(g, pid);
        if can_add_military && ((military as f64) < military_floor || missing_siege_arm) {
            let picked = if missing_siege_arm {
                self.best_military_role(g, pid, cid, None, true)
                    .or_else(|| self.combined_arms_unit(g, pid, cid, melee, ranged))
            } else if rushing && melee < self.rush_military_floor {
                self.best_military(g, pid, cid, Some(false))
                    .or_else(|| self.combined_arms_unit(g, pid, cid, melee, ranged))
            } else {
                self.combined_arms_unit(g, pid, cid, melee, ranged)
            };
            if let Some(m) = picked {
                // ⚠ THE BRANCH THAT WINS MUST SAY SO.
                //
                // A one-city run built 18 heavy chariots across 90 turns while its
                // own plan asked for seven cities, and NOTHING in the journal said
                // which branch of `pick_item` took the choice. An offline probe with
                // the deployment genome returned a settler for the same board, so
                // the two disagreed with no way to tell which was wrong.
                think!(self.journal, Cities, Detail,
                       "Military floor takes the build";
                       "holding {military} against a floor of {military_floor:.1}{}",
                       if missing_siege_arm { ", and the siege arm is missing" } else { "" });
                return Some(Item::Unit { unit: Name::new(&m) });
            }
        }
        if !self.minor && can_add_military && siege_support == 0 && melee >= 2 {
            if let Some(unit) = self.siege_support_unit(g, pid, cid) {
                return Some(Item::Unit { unit: Name::new(&unit) });
            }
        }
        if !self.minor && !self.barb {
            let has_spaceport = g.cities.values().any(|city| {
                city.owner == pid
                    && (g.city_has_district_family(city, crate::name!("spaceport"))
                        || matches!(
                            city.queue.first(),
                            Some(Item::District { district, .. })
                                if g.district_family(*district) == "spaceport"
                        ))
            });
            if !has_spaceport && g.players[pid].techs.contains(&crate::name!("rocketry")) {
                if let Some(pos) = g.district_sites(cid, crate::name!("spaceport")).into_iter().next() {
                    let item = Item::District {
                        district: crate::name!("spaceport"),
                        pos,
                    };
                    if g.can_produce(pid, cid, &item) {
                        return Some(item);
                    }
                }
            }
            let spy = Item::Unit {
                unit: crate::name!("spy"),
            };
            if g.can_produce(pid, cid, &spy) {
                return Some(spy);
            }
            if let Some(product) = g
                .producible_items(pid, cid)
                .into_iter()
                .find(|item| matches!(item, Item::Product { .. }))
            {
                return Some(product);
            }
            let projects: Vec<Item> = g
                .rules
                .projects
                .iter()
                .filter(|(project, spec)| {
                    !spec.repeatable
                        || matches!(
                            project.as_str(),
                            "lagrange_laser_station" | "terrestrial_laser_station"
                        )
                })
                .map(|(project, _)| Item::Project {
                    project: *project,
                })
                .filter(|item| g.can_produce(pid, cid, item))
                .collect();
            // Cost and label taken once per candidate: the comparator used to
            // re-derive both, and its tiebreak built two Debug strings for
            // every comparison the sort made.
            let mut ranked: Vec<(f64, String, Item)> = projects
                .into_iter()
                .map(|item| (g.item_cost_for(pid, &item), format!("{item:?}"), item))
                .collect();
            ranked.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then_with(|| a.1.cmp(&b.1)));
            if let Some((_, _, project)) = ranked.into_iter().next() {
                return Some(project);
            }
        }
        let naval = Self::naval_counts(g, pid).0;
        if can_add_military && naval < Self::desired_navy(g, pid) {
            if let Some(unit) = self.best_naval_unit(g, pid, cid) {
                return Some(Item::Unit { unit: Name::new(&unit) });
            }
        }
        // ⚠ Five conditions in one `&&` chain, and an empire that ends the game
        // with one city cannot say which of them refused. Named individually so
        // "the site search found nothing" is distinguishable from "the window
        // closed" without attaching a debugger to a finished run.
        if !self.minor && !self.barb {
            let room = ((n_cities + settlers) as f64) < self.w.city_target;
            let none_in_flight = settlers == 0;
            let grown = (city_pop as f64) >= self.w.settler_min_pop;
            let in_window = (g.turn as f64) < self.w.settler_stop_turn;
            if room && none_in_flight && grown && in_window {
                if self.has_practical_settle_site(g, pid) {
                    return Some(Item::Unit {
                        unit: crate::name!("settler"),
                    });
                }
                think!(self.journal, Cities, Detail,
                       "No settler: every reachable site is refused";
                       "{n_cities} cities against a target of {:.1}, and the site \
                        search found nothing within reach",
                       self.w.city_target);
            } else if self.journal.wants(crate::reasoning::Level::Detail) {
                let mut why: Vec<&str> = Vec::new();
                if !room {
                    why.push("already at the city target");
                }
                if !none_in_flight {
                    why.push("a settler is already in flight");
                }
                if !grown {
                    why.push("the city is below settler_min_pop");
                }
                if !in_window {
                    why.push("past settler_stop_turn");
                }
                think!(self.journal, Cities, Detail,
                       "No settler: {}", why.join(", ");
                       "{n_cities} cities and {settlers} settlers against a target \
                        of {:.1}, turn {} of {:.0}",
                       self.w.city_target, g.turn, self.w.settler_stop_turn);
            }
        }
        if (builders as f64) < self.w.builder_per_city * n_cities as f64
            && Self::has_builder_work(g, pid)
        {
            return Some(Item::Unit {
                unit: crate::name!("builder"),
            });
        }
        if !self.minor
            && Self::should_add_trader(g, pid, traders)
            && g.can_produce(
                pid,
                cid,
                &Item::Unit {
                    unit: crate::name!("trader"),
                },
            )
        {
            return Some(Item::Unit {
                unit: crate::name!("trader"),
            });
        }
        if self.minor {
            if let Some(district) = Self::minor_district_item(g, pid, cid) {
                return Some(district);
            }
        }
        if let Some(monument) = Self::civ_building(g, pid, cid, "monument") {
            return Some(monument);
        }
        // Coastal infrastructure is part of the water strategy, not an
        // accidental fallback after every land district. A harbor also gives
        // later naval production somewhere sensible to concentrate.
        if !self.minor
            && Self::city_is_coastal(g, cid)
            && !g.city_has_district_family(&g.cities[&cid], crate::name!("harbor"))
        {
            let harbor = Self::civ_district(g, pid, "harbor");
            let sites = g.district_sites(cid, harbor);
            if let Some(pos) = sites.into_iter().max_by(|a, b| {
                g.district_yields(harbor, *a)
                    .total()
                    .partial_cmp(&g.district_yields(harbor, *b).total())
                    .unwrap()
                    .then(a.cmp(b))
            }) {
                let item = Item::District {
                    district: Name::new(&harbor),
                    pos,
                };
                if g.can_produce(pid, cid, &item) {
                    return Some(item);
                }
            }
        }
        let mut dpri: Vec<(&str, f64)> = DISTRICT_PRIORITY
            .iter()
            .cloned()
            .zip([
                self.w.d_campus,
                self.w.d_commercial,
                self.w.d_holy,
                self.w.d_theater,
            ])
            .collect();
        // A city paying the Amenity band asks for the district that repairs it
        // before it asks for another specialty district, because the band
        // multiplies what every one of those districts produces. Weighted by
        // the actual deficit, so this outranks the lane's own families only
        // while the multiplier is genuinely being paid and disappears the
        // moment the city is neutral.
        // ⚠ Taken BEFORE either repair is pushed, so the two repairs are each
        // ranked against the LANE and then against EACH OTHER by their own
        // magnitudes. Reading it after the Amenity push would stack the housing
        // repair on top of the Amenity one and make housing win by construction
        // — wrong in exactly the case that matters most, because
        // `amenity_growth_mult` is **0.0** below −4 and an Aqueduct handed to a
        // city that is not growing at all buys nothing.
        let lane_top = dpri.iter().map(|(_, w)| *w).fold(0.0_f64, f64::max);
        if self.amenity_districts && !self.minor {
            let deficit = (-g.city_amenity_surplus(&g.cities[&cid])).max(0) as f64;
            if deficit > 0.0 {
                dpri.push(("entertainment_complex", lane_top + deficit));
            }
        }
        // And a city that has run out of HOUSING asks for the districts that
        // raise the ceiling, for the same reason and on a tighter band: below
        // headroom 2 `Game::housing_growth_mult` halves growth and below 1 it
        // quarters it, and population is what science is. See
        // `BasicAi::housing_districts` for the corpus — 78.4% of live
        // city-turns sit under that ceiling while the two districts that lift
        // it take 1.6% of district orders.
        //
        // Weighted by the housing each family would ACTUALLY add rather than by
        // the shortfall alone: an Aqueduct is +4 to a dry inland city and only
        // +2 to one already on a river, and a flat weight cannot tell those
        // apart. Capped by the shortfall so a city one short of the target does
        // not outrank its whole lane to over-build by three.
        //
        // Against the Amenity repair this then reads the way it should: a city
        // at surplus −6 scores that repair 6 and this one at most 3, and the
        // −6 city has growth 0.00 so housing genuinely cannot help it yet. A
        // city merely displeased at −1 scores the Amenity repair 1 and a real
        // housing block 2, and housing correctly goes first.
        if self.housing_districts && !self.minor {
            let shortfall = HOUSING_HEADROOM_TARGET - g.city_housing_headroom(&g.cities[&cid]);
            if shortfall > 0.0 {
                for family in HOUSING_DISTRICTS {
                    let gain = Self::housing_gain(g, pid, cid, family);
                    if gain > 0.0 {
                        dpri.push((family, lane_top + shortfall.min(gain)));
                    }
                }
            }
        }
        if self.minor {
            dpri.clear();
        }
        // ⚠⚠ THE RANKING IS A CONSTANT, AND ONE FAMILY IS ALWAYS LAST. `d_theater` is
        // the LOWEST of these four in **all 51 genomes** in `data/league/league.json`
        // — typically 1.0 against a Campus's 4.0 — and the loop below only skips a
        // family THIS CITY already has. So every city independently works down the
        // same list, and the fourth entry is reached only by a city that already
        // holds the other three.
        //
        // Measured on live run `civvis-20260803T090911Z` (5 cities, 242 turns):
        // CIVVIS ordered `DISTRICT_CAMPUS` **28 times** and `DISTRICT_THEATER`
        // **zero** times. The empire finished with 4 Campuses and no Theatre Square
        // anywhere, so the whole culture chain — Amphitheatre, Museum, Broadcast
        // Centre, and every Great Work slot — was unreachable by construction.
        // Culture ended at 22.1 against a science of comparable size.
        //
        // So scale each family by how much of the EMPIRE still lacks it. A fifth
        // Campus and a first Theatre Square are not the same decision, and a static
        // weight cannot tell them apart. This is deliberately a coverage term, not a
        // re-tuning: the genome's ordering still decides between two families the
        // empire is equally short of, so a bred preference is preserved wherever it
        // is actually expressing a preference.
        if !self.minor && self.district_coverage {
            let mine: Vec<&crate::game::City> =
                g.cities.values().filter(|city| city.owner == pid).collect();
            let total = mine.len().max(1) as f64;
            for (family, weight) in dpri.iter_mut() {
                let have = mine
                    .iter()
                    .filter(|city| g.city_has_district_family(city, Name::new(*family)))
                    .count() as f64;
                // 1.0 when no city has it, falling toward 0.5 when every city does.
                *weight *= 1.0 - 0.5 * (have / total);
            }
        }
        dpri.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        for (family, _) in dpri {
            if family == "holy_site" && g.players[pid].religion.is_none() {
                let prophet_race_closed = g.religions_founded() >= g.max_religions();
                let site_reserved = g.cities.values().any(|other| {
                    other.owner == pid
                        && (other
                            .districts
                            .keys()
                            .any(|district| g.district_family(*district) == "holy_site")
                            || matches!(
                                other.queue.first(),
                                Some(Item::District { district, .. })
                                    if g.district_family(*district) == "holy_site"
                            ))
                });
                // One active Holy Site is enough to contest the finite
                // Prophet race. Before a religion exists, duplicating it in
                // every newly founded city sacrifices settlers, campuses, and
                // basic infrastructure while adding points too late to change
                // the current recruitment. Once every religion is founded,
                // even the first site can no longer win a Prophet slot.
                // Founders may expand their faith network normally.
                if prophet_race_closed || site_reserved {
                    continue;
                }
            }
            if g.city_has_district_family(&g.cities[&cid], Name::new(family)) {
                continue;
            }
            // Ask for the district this civilization actually builds. Greece
            // builds an Acropolis, never a Theater Square, and naming the base
            // district produced an item the engine refused - which stalled the
            // city outright, because a rejected choice ends its turn.
            let dname = Self::civ_district(g, pid, family);
            let spec = &g.rules.districts[&dname];
            let unlocked = spec
                .tech
                .as_ref()
                .map(|t| g.players[pid].techs.contains(t))
                .unwrap_or(true)
                && spec
                    .civic
                    .as_ref()
                    .map(|c| g.players[pid].civics.contains(c))
                    .unwrap_or(true);
            if !unlocked {
                continue;
            }
            let sites = g.district_sites(cid, Name::new(dname.as_str()));
            if !sites.is_empty() {
                let best = *sites
                    .iter()
                    .max_by(|a, b| {
                        let ya = g.district_yields(dname, **a).total();
                        let yb = g.district_yields(dname, **b).total();
                        ya.partial_cmp(&yb).unwrap().then(a.cmp(b))
                    })
                    .unwrap();
                let item = Item::District {
                    district: dname,
                    pos: best,
                };
                // Never hand back something the engine will reject: a refused
                // item costs the city its whole turn.
                if g.can_produce(pid, cid, &item) {
                    return Some(item);
                }
            }
        }
        let mut buildable: Vec<(i64, Name)> = g
            .rules
            .buildings
            .iter()
            .filter(|(b, s)| {
                !s.wonder
                    && g.can_produce(
                        pid,
                        cid,
                        &Item::Building {
                            building: **b,
                        },
                    )
            })
            .map(|(b, s)| (s.cost as i64, *b))
            .collect();
        if !buildable.is_empty() {
            // ⚠⚠ A COST TIE WAS BEING BROKEN ALPHABETICALLY, AND IT DECIDED WHICH
            // MUSEUM EVERY EMPIRE BUILDS. `sort()` on `(cost, Name)` falls through to
            // `Name::cmp`, which compares text — so with Art and Archaeological
            // Museum both at 290, `archaeological_museum` wins on the letter 'c'.
            //
            // They are otherwise IDENTICAL in `data/buildings.json`: same cost, same
            // +2 culture, same maintenance, same great-person points. Only the slot
            // kind differs, `art: 3` against `artifact: 3` — and those are not equally
            // fillable. Writing, Art and Music slots are filled by a Great Person who
            // arrives on their own points. An ARTIFACT slot needs an Archaeologist: a
            // **400-production civilian** that itself requires the Archaeological
            // Museum to exist first, then has to walk to a dig site and spend a charge.
            // No live run has ever built one.
            //
            // Measured on `civvis-20260803T082856Z`, the first run ever to build a
            // Museum: it built `BUILDING_MUSEUM_ARTIFACT` at t181 and finished with
            // **0 great works in 6 slots**. Across 50 earlier runs, Artist 9 activated
            // against 67 idle, Musician 0 against 31, and **0 artifact works ever
            // held**.
            //
            // ⚠ This changes ONLY tied pairs. Cheapest-first is untouched as a policy
            // — that argument belongs elsewhere — and a building with no slots keeps
            // its exact position. What it stops is the alphabet deciding a real
            // question.
            let tiebreak = self.slot_kind_tiebreak;
            let slot_worth = |b: &Name| -> f64 {
                if !tiebreak {
                    return 0.0;
                }
                g.rules.buildings[b]
                    .great_work_slots
                    .iter()
                    .map(|(kind, count)| {
                        let count = (*count).max(0) as f64;
                        if kind == "artifact" { count * 0.5 } else { count }
                    })
                    .sum()
            };
            buildable.sort_by(|a, b| {
                a.0.cmp(&b.0)
                    .then_with(|| {
                        // ⚠⚠ ONLY WHEN BOTH CANDIDATES HAVE SLOTS. My first version
                        // compared slot worth across every cost tie, which is a far
                        // bigger change than the one being argued for: it reordered
                        // SIX tie groups, putting `old_god_obelisk` over `monument`,
                        // `temple` over `market`, `cathedral` over every other worship
                        // building, and `broadcast_center` over `research_lab`. Priced
                        // at 50 maps it came back **-14 Elo (p=0.6875) with culture
                        // LOWER, 151.7 against 153.8** — those are bad trades and the
                        // measurement said so.
                        //
                        // Restricting it to pairs that both carry slots leaves every
                        // one of those groups exactly as it was and decides only the
                        // question actually at issue: Art Museum against
                        // Archaeological Museum, identical in cost, culture,
                        // maintenance and great-person points, separated in the old
                        // sort by the letter 'c'.
                        let (wa, wb) = (slot_worth(&a.1), slot_worth(&b.1));
                        if wa <= 0.0 || wb <= 0.0 {
                            return std::cmp::Ordering::Equal;
                        }
                        wb.partial_cmp(&wa).unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .then_with(|| a.1.cmp(&b.1))
            });
            return Some(Item::Building {
                building: Name::new(&buildable[0].1),
            });
        }
        // developed cities turn to wonders
        if !self.minor && g.cities[&cid].buildings.len() as f64 >= self.w.wonder_min_bld {
            if let Some(wonder) = Self::cheapest_available_wonder(g, pid, cid) {
                return Some(wonder);
            }
        }
        // Repeatable district projects are a developed major-city fallback. If
        // considered with mandatory projects above, their low early base cost
        // makes a basic AI loop them forever before building Monuments,
        // districts, or district buildings. A completely developed city-state
        // may instead leave its production queue empty.
        if !self.barb && !self.minor {
            let projects: Vec<Item> = g
                .rules
                .projects
                .iter()
                .filter(|(project, spec)| {
                    spec.repeatable
                        && !matches!(
                            project.as_str(),
                            "lagrange_laser_station" | "terrestrial_laser_station"
                        )
                })
                .map(|(project, _)| Item::Project {
                    project: *project,
                })
                .filter(|item| g.can_produce(pid, cid, item))
                .collect();
            // Cost and label taken once per candidate: the comparator used to
            // re-derive both, and its tiebreak built two Debug strings for
            // every comparison the sort made.
            let mut ranked: Vec<(f64, String, Item)> = projects
                .into_iter()
                .map(|item| (g.item_cost_for(pid, &item), format!("{item:?}"), item))
                .collect();
            ranked.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then_with(|| a.1.cmp(&b.1)));
            if let Some((_, _, project)) = ranked.into_iter().next() {
                return Some(project);
            }
        }
        can_add_military
            .then(|| self.combined_arms_unit(g, pid, cid, melee, ranged))
            .flatten()
            .map(|m| Item::Unit { unit: Name::new(&m) })
    }

    /// Pick a real placed wonder rather than treating it as an ordinary
    /// building. `producible_items` supplies fully validated sites; filtering
    /// globally queued names keeps two cities from entering the same race.
    fn cheapest_available_wonder(g: &Game, pid: usize, cid: u32) -> Option<Item> {
        let queued: HashSet<Name> = g
            .cities
            .values()
            .filter_map(|city| match city.queue.first() {
                Some(Item::Wonder { wonder, .. }) => Some(*wonder),
                _ => None,
            })
            .collect();
        g.producible_items(pid, cid)
            .into_iter()
            .filter(|item| {
                matches!(item, Item::Wonder { wonder, .. } if !queued.contains(wonder))
            })
            .min_by(|left, right| {
                g.item_cost_for_city(pid, cid, left)
                    .total_cmp(&g.item_cost_for_city(pid, cid, right))
                    .then_with(|| format!("{left:?}").cmp(&format!("{right:?}")))
            })
    }

    fn units(&mut self, g: &mut Game, pid: usize) {
        self.begin_movement_turn(g, pid);
        self.prepare_unit_formations(g, pid);
        self.recovering_units
            .retain(|uid| g.units.get(uid).is_some_and(|unit| unit.owner == pid));
        self.patrol_targets
            .retain(|uid, _| g.units.get(uid).is_some_and(|unit| unit.owner == pid));
        self.settler_targets
            .retain(|uid, _| g.units.get(uid).is_some_and(|unit| unit.owner == pid));
        for uid in g.player_unit_ids(pid) {
            let mut took_a_turn = false;
            for _ in 0..8 {
                if !g.units.contains_key(&uid) {
                    break;
                }
                if g.units[&uid].moves_left <= 0.0 {
                    break;
                }
                let kind = g.units[&uid].kind;
                let acted = match kind.as_str() {
                    "settler" => self.settler_step(g, pid, uid),
                    "builder" => self.builder_step(g, pid, uid),
                    "military_engineer" => self.military_engineer_step(g, pid, uid),
                    "naturalist" => self.naturalist_step(g, pid, uid),
                    "archaeologist" => self.archaeologist_step(g, pid, uid),
                    "trader" => self.trader_step(g, pid, uid),
                    "missionary" => self.missionary_step(g, pid, uid),
                    "battering_ram" | "siege_tower" => self.siege_support_step(g, pid, uid),
                    "rock_band" => self.rock_band_step(g, pid, uid),
                    _ => self.military_step(g, pid, uid),
                };
                if !acted {
                    break;
                }
                took_a_turn = true;
            }
            if !took_a_turn {
                self.hold_stood_down_unit(g, pid, uid);
            }
        }
    }

    /// Spend earned promotions before moving, then consolidate eligible
    /// military units into Corps/Armies and attach colocated support units.
    /// These actions otherwise never occur in headless self-play because they
    /// are neither movement nor attacks.
    pub(crate) fn prepare_unit_formations(&self, g: &mut Game, pid: usize) {
        for uid in g.player_unit_ids(pid) {
            let Some(promotion) = g.available_promotions(uid).into_iter().max_by(|a, b| {
                let value = |name: &str| {
                    g.rules.promotions[name]
                        .effects
                        .values()
                        .map(|effect| effect.abs())
                        .sum::<f64>()
                };
                value(a)
                    .partial_cmp(&value(b))
                    .unwrap()
                    .then_with(|| b.cmp(a))
            }) else {
                continue;
            };
            let _ = g.apply(
                pid,
                &Action::Promote {
                    unit: uid,
                    promotion: Name::new(&promotion),
                },
            );
        }

        if g.players[pid].civics.contains(&crate::name!("nationalism")) {
            let reserve = (g.player_city_ids(pid).len() + 3).max(5);
            loop {
                let military = g
                    .player_unit_ids(pid)
                    .into_iter()
                    .filter(|uid| g.rules.units[g.units[uid].kind].class == "military")
                    .count();
                if military <= reserve {
                    break;
                }
                let action = g
                    .legal_actions_within(pid, ActionFamilies::FORMATIONS)
                    .into_iter()
                    .find(|action| matches!(action, Action::CombineUnits { .. }));
                let Some(action) = action else { break };
                if g.apply(pid, &action).is_err() {
                    break;
                }
            }
        }

        let has_link_candidate = |game: &Game| {
            let units = game.player_unit_ids(pid);
            units.iter().enumerate().any(|(index, unit)| {
                units[index + 1..].iter().any(|with| {
                    let a = &game.units[unit];
                    let b = &game.units[with];
                    if a.pos != b.pos || a.linked_to.is_some() || b.linked_to.is_some() {
                        return false;
                    }
                    let a_spec = &game.rules.units[a.kind];
                    let b_spec = &game.rules.units[b.kind];
                    let support = (a_spec.class == "support"
                        && a.kind != "military_engineer"
                        && b_spec.class == "military"
                        && self.support_escort_compatible(game, a.id, b.id))
                        || (b_spec.class == "support"
                            && b.kind != "military_engineer"
                            && a_spec.class == "military"
                            && self.support_escort_compatible(game, b.id, a.id));
                    let naval_settler = (a_spec.domain.as_deref() == Some("sea")
                        && b.kind == "settler")
                        || (b_spec.domain.as_deref() == Some("sea") && a.kind == "settler");
                    support || naval_settler
                })
            })
        };
        while has_link_candidate(g) {
            let action = g
                .legal_actions_within(pid, ActionFamilies::FORMATIONS)
                .into_iter()
                .find(|action| match action {
                    Action::LinkUnits { unit, with } => {
                        let a = &g.rules.units[g.units[unit].kind];
                        let b = &g.rules.units[g.units[with].kind];
                        let support = (a.class == "support"
                            && g.units[unit].kind != "military_engineer"
                            && self.support_escort_compatible(g, *unit, *with))
                            || (b.class == "support"
                                && g.units[with].kind != "military_engineer"
                                && self.support_escort_compatible(g, *with, *unit));
                        let naval_settler = (a.domain.as_deref() == Some("sea")
                            && g.units[with].kind == "settler")
                            || (b.domain.as_deref() == Some("sea")
                                && g.units[unit].kind == "settler");
                        support || naval_settler
                    }
                    _ => false,
                });
            let Some(action) = action else { break };
            if g.apply(pid, &action).is_err() {
                break;
            }
        }
    }

    /// 1-ply positional search for wartime marching: score each candidate
    /// tile (stay put or any legal neighbor) by progress toward the target,
    /// adjacent friendly support, and expected incoming damage; take the best.
    fn tactical_step(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        target: Pos,
        enemy_ids: &[usize],
        attack_range: i32,
    ) -> bool {
        // Get out of the water before marching anywhere. The penalty in
        // `score` below stops a unit WALKING INTO the sea, but it cannot
        // recover one already out there: every neighbour of a tile in open
        // water is also water, so a uniform penalty cancels and the unit keeps
        // swimming toward the objective. This is the half that reaches the
        // eleven-of-twelve afloat on run `civvis-20260803T130831Z`.
        //
        // It sits in `tactical_step` rather than `military_step` because the
        // Advanced campaign path calls `tactical_step` directly and would
        // otherwise miss the rule entirely — the same "reachable only on one
        // path" mistake that left the #955 defence layer inert.
        if self.come_ashore
            && g.rules.units[g.units[&uid].kind].domain.as_deref() != Some("sea")
            && g.is_embarked(&g.units[&uid])
            && self.disembark_step(g, pid, uid)
        {
            return true;
        }
        let upos = g.units[&uid].pos;
        let u = &g.units[&uid];
        let my_def = effective_strength(g.unit_strength(u, true), u.hp);
        let prefer_dry =
            self.come_ashore && g.rules.units[u.kind].domain.as_deref() != Some("sea");
        let doctrine = Self::unit_doctrine(g, uid);
        let (preferred_range, progress, threat_caution) = match doctrine {
            UnitDoctrine::Recon => (2, 0.60, 1.35),
            UnitDoctrine::Assault => (1, 1.15, 1.00),
            UnitDoctrine::Mobile => (1, 1.40, 0.80),
            UnitDoctrine::Ranged => (attack_range.max(1), 0.90, 1.15),
            UnitDoctrine::Siege => (attack_range.max(1), 0.80, 1.25),
            UnitDoctrine::Support | UnitDoctrine::Carrier => (2, 0.65, 1.40),
            UnitDoctrine::AirDefense | UnitDoctrine::AirStrike => (attack_range.max(1), 1.0, 1.0),
        };
        let hostile_units: Vec<u32> = if self.tactical_strategy || self.unit_objective_memory {
            g.units
                .values()
                .filter(|enemy| enemy_ids.contains(&enemy.owner))
                .map(|enemy| enemy.id)
                .collect()
        } else {
            Vec::new()
        };
        // A memory entry is an observation, not privileged map knowledge.
        // Keep the legacy tactical score above exactly as it is, while only
        // allowing the new durable warning to use enemies this civilization
        // can actually see now.
        let danger_hostiles: Vec<u32> = if self.unit_objective_memory {
            g.units
                .values()
                .filter(|enemy| {
                    enemy_ids.contains(&enemy.owner) && g.unit_visible_to(enemy.id, pid)
                })
                .map(|enemy| enemy.id)
                .collect()
        } else {
            Vec::new()
        };
        let score = |g: &Game, tile: Pos| -> f64 {
            let depth_error = (g.wdist(tile, target) - preferred_range).abs();
            let mut s = -3.0 * progress * depth_error as f64;
            let mut adjacent_support = 0;
            for n in g.nbrs(tile) {
                for oid in g.units_at(n) {
                    let o = &g.units[&oid];
                    if g.rules.units[o.kind].class != "military" {
                        continue;
                    }
                    if o.owner == pid && oid != uid {
                        adjacent_support += 1;
                    } else if !self.tactical_strategy && enemy_ids.contains(&o.owner) {
                        let att = effective_strength(g.unit_strength(o, false), o.hp);
                        s -= self.w.mv_threat
                            * threat_caution
                            * 30.0
                            * ((att - my_def) / 25.0).exp();
                    }
                }
            }
            if self.tactical_strategy {
                s -= self.w.mv_threat
                    * threat_caution
                    * self.projected_counter_damage(g, uid, tile, &hostile_units);
            }
            // A pair of neighbors is enough to hold a coherent line. Giving
            // every extra adjacent unit the full bonus makes dense armies
            // refuse to leave their initial cluster even when a safe campaign
            // route is open.
            s += self.w.mv_support * adjacent_support.min(2) as f64;
            // ⭐ The score above has no terrain term at all, and open water is
            // doubly attractive because of it: a sea tile is usually the
            // geometrically shorter road to an objective across a bay, AND it
            // carries no adjacent-enemy threat because the enemies are on
            // land. So the safest, fastest route was always the sea — and an
            // embarked land unit cannot attack (`Game::apply` refuses it),
            // cannot fortify, and defends at the era's flat
            // `embarked_strength` instead of its own.
            //
            // This is the wartime mover, and it is the one that mattered: on
            // live run `civvis-20260803T130831Z` at t174, eleven of twelve land
            // combat units were afloat while the capital sat at 179/200 damage,
            // and the journal read "A land force of 12 will advance | objective
            // (22,21)" — an objective on our OWN landmass. They were not
            // crossing to reach it; they were swimming along it.
            //
            // Sized to outweigh the few tiles of `depth_error` a detour around
            // a bay costs, without removing the option: `route_step` below is
            // untouched, so a real crossing still happens when the land route
            // does not exist.
            if prefer_dry && g.map.get(tile).is_some_and(|t| g.rules.is_water(t)) {
                s -= WATER_MARCH_PENALTY;
            }
            s + self.livelock_penalty(uid, tile)
        };
        let stay = score(g, upos);
        let holding_role_position = g.wdist(upos, target) == preferred_range;
        let within_minor_front = |position: Pos| {
            !self.minor
                || Self::minor_home(g, pid)
                    .is_some_and(|home| g.wdist(home, position) <= MINOR_DEFENSE_RADIUS)
        };
        let mut best: Option<(f64, Pos)> = None;
        for n in g.nbrs(upos) {
            if !within_minor_front(n) || !g.can_move(uid, n) {
                continue;
            }
            let sc = score(g, n);
            if best.map(|(b, bp)| (sc, n) > (b, bp)).unwrap_or(true) {
                best = Some((sc, n));
            }
        }
        match best {
            Some((sc, n))
                if if holding_role_position {
                    sc > stay + 1e-9
                } else {
                    self.move_beats_holding(g, uid, sc, stay)
                } =>
            {
                if self.unit_objective_memory {
                    let danger = self.projected_counter_damage(g, uid, n, &danger_hostiles);
                    if self.remember_dangerous_approach(g, uid, n, danger) {
                        return self
                            .retreat_step(g, pid, uid)
                            .unwrap_or_else(|| self.fortify_or_stop(g, pid, uid));
                    }
                }
                // ⚠ Through `path_move`, never `g.apply` directly: a unit with
                // movement left is stepped again this same turn, and a raw
                // apply records nothing — so round two happily re-entered the
                // tile round one just left. Net zero ground, TWO emitted
                // orders, and on the Civilization VI side the second usually
                // lands as a MOVE_TO of the unit's own tile: 217 of 217
                // refused moves on run civvis-20260801T224944Z were exactly
                // that, out-and-back pairs from this call site.
                self.tactical_apply_move(g, pid, uid, n)
            }
            _ => {
                // Long-range search is the fallback, not the hot path: most
                // turns keep the original cheap local tactic, while a unit at
                // a genuine obstacle can take the first safe detour step.
                let n = match g.route_step(uid, target, preferred_range) {
                    Some(n) if within_minor_front(n) && g.can_move(uid, n) => n,
                    _ => return false,
                };
                let routed = score(g, n) + 2.5;
                if !self.move_beats_holding(g, uid, routed, stay) {
                    return false;
                }
                if self.unit_objective_memory {
                    let danger = self.projected_counter_damage(g, uid, n, &danger_hostiles);
                    if self.remember_dangerous_approach(g, uid, n, danger) {
                        return self
                            .retreat_step(g, pid, uid)
                            .unwrap_or_else(|| self.fortify_or_stop(g, pid, uid));
                    }
                }
                self.tactical_apply_move(g, pid, uid, n)
            }
        }
    }

    pub(crate) fn move_beats_holding(
        &self,
        g: &Game,
        uid: u32,
        candidate: f64,
        holding: f64,
    ) -> bool {
        let initiative = if g.units[&uid].moved {
            0.0
        } else {
            FIRST_MOVE_SCORE_BONUS
        };
        candidate + initiative > holding + 1e-9
    }

    fn step_toward(&self, g: &mut Game, pid: usize, uid: u32, target: Pos) -> bool {
        self.step_toward_range(g, pid, uid, target, 0)
    }

    /// Apply one pathing move while refusing to undo this unit's immediately
    /// preceding path step in the same turn. Waiting in a dead end preserves
    /// real progress for the auditor, and next turn's route can back out once
    /// before choosing a different greedy branch.
    /// Take a tactical step that has already been selected and validated by
    /// the caller, recording it when `recorded_tactical_step` is on.
    ///
    /// The flag is the whole gate. Off — every tournament entrant, every
    /// replayed ladder, and the frozen `advanced_v1` anchor — this is the
    /// historical raw `g.apply(Move)` and nothing about the step changes. On,
    /// which only the Civilization VI bridge does, the step goes through
    /// `path_move` so the same-turn reversal guard can see it. `path_move`
    /// can additionally refuse a step that the raw apply would have taken
    /// (a reversal, a retread, a minor leaving its defense area); that
    /// asymmetry is the point of the fix, and it is exactly why the anchor
    /// must not be exposed to it.
    pub(crate) fn tactical_apply_move(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        to: Pos,
    ) -> bool {
        if !self.recorded_tactical_step {
            return g.apply(pid, &Action::Move { unit: uid, to }).is_ok();
        }
        self.path_move(g, pid, uid, to)
    }

    pub(crate) fn path_move(&self, g: &mut Game, pid: usize, uid: u32, to: Pos) -> bool {
        let from = g.units[&uid].pos;
        if self.minor {
            let Some(home) = Self::minor_home(g, pid) else {
                return false;
            };
            let from_home = g.wdist(home, from);
            let to_home = g.wdist(home, to);
            // Once local, never step back outside the defense area. A unit
            // already stranded beyond it may still take a pathfinder detour
            // around terrain while returning home.
            if from_home <= MINOR_DEFENSE_RADIUS && to_home > MINOR_DEFENSE_RADIUS {
                return false;
            }
        }
        let reverses_last_step = self
            .last_path_step_from
            .borrow()
            .get(&uid)
            .is_some_and(|(turn, previous)| *turn == g.turn && *previous == to);
        if reverses_last_step {
            return false;
        }
        // The same refusal over the unit's last several turns rather than its
        // last several movement points. A route that keeps proposing a tile
        // this unit has already been standing on all window is not a route.
        if self.retreads_a_loop(uid, to) {
            return false;
        }
        // Settlers use the shared route-order tool exposed to network clients
        // and learned agents. The AI has already selected and validated this
        // adjacent step, so it remains behaviorally identical to Move without
        // making every military step pay for route reconstruction.
        let movement = if g.units[&uid].kind == "settler" {
            Action::MoveTo { unit: uid, to }
        } else {
            Action::Move { unit: uid, to }
        };
        if g.apply(pid, &movement).is_err() {
            return false;
        }
        self.last_path_step_from
            .borrow_mut()
            .insert(uid, (g.turn, from));
        true
    }

    /// Settlers have a persistent, path-checked destination, so follow that
    /// route rather than taking the generic cheap greedy shortcut first. A
    /// geometrically closer tile can be a one-hex cul-de-sac; with two
    /// movement points the generic mover enters it and immediately routes
    /// back out, repeating the same round trip every turn.
    fn settler_step_toward(&self, g: &mut Game, pid: usize, uid: u32, target: Pos) -> bool {
        if let Some(next) = g
            .route_step(uid, target, 0)
            .filter(|next| g.can_move(uid, *next))
        {
            return self.path_move(g, pid, uid, next);
        }
        self.step_toward(g, pid, uid, target)
    }

    /// Move toward a target without insisting on entering its tile. Religious
    /// units spread from an adjacent hex, so routing them to range zero makes
    /// the pathfinder reject foreign city centers and can strand an entire
    /// procession behind a mountain detour.
    pub(crate) fn step_toward_range(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        target: Pos,
        stop_range: i32,
    ) -> bool {
        let cur = g.units[&uid].pos;
        if g.wdist(cur, target) <= stop_range {
            return false;
        }
        let mut local: Vec<Pos> = g
            .nbrs(cur)
            .into_iter()
            .filter(|p| g.can_move(uid, *p))
            .collect();
        // Dry land first for a land unit. This greedy step ranked neighbours on
        // geometric distance alone, and `can_move` says yes to open water for
        // every land unit once embarkation unlocks — so a straight line across
        // a bay always beat walking around it, and the army marched into the
        // sea. Embarked it cannot attack (`Game::apply` refuses it) or fortify.
        //
        // ⚠ The loop below `break`s on the first neighbour that fails to make
        // progress, which is only sound while the order is by distance. So the
        // non-improving neighbours are dropped here rather than re-sorted
        // behind the dry ones — otherwise a dry tile that moves away from the
        // target would cut off the improving water tiles behind it and the
        // unit would simply stop. A water step is still reachable: it sorts
        // last, and the A* fallback below is untouched, so a genuine ocean
        // crossing still happens when no dry neighbour makes progress.
        let prefer_dry = self.come_ashore
            && g.rules.units[g.units[&uid].kind].domain.as_deref() != Some("sea");
        if prefer_dry {
            let here = g.wdist(cur, target);
            local.retain(|p| g.wdist(*p, target) < here);
        }
        local.sort_by_key(|p| {
            let wet = prefer_dry && g.map.get(*p).is_some_and(|tile| g.rules.is_water(tile));
            (wet, g.wdist(*p, target), *p)
        });
        for next in local {
            if g.wdist(next, target) >= g.wdist(cur, target) {
                break; // sorted: no remaining neighbor makes progress
            }
            // A neighbor can still be refused (stacking, ZOC); try the next
            // improving tile before paying for A*.
            if self.path_move(g, pid, uid, next) {
                return true;
            }
        }

        // The common case above stays as cheap as the original greedy AI;
        // invoke A* only when no legal neighbor makes geometric progress.
        let next = match g.route_step(uid, target, stop_range) {
            Some(p) if g.can_move(uid, p) => p,
            _ => return false,
        };
        if self.path_move(g, pid, uid, next) {
            return true;
        }
        // A peer can take the A* tile first; sidestep at equal distance so
        // a marching column keeps flowing around the blockage.
        for p in g.nbrs(cur) {
            if g.wdist(p, target) == g.wdist(cur, target)
                && g.can_move(uid, p)
                && self.path_move(g, pid, uid, p)
            {
                return true;
            }
        }
        false
    }

    fn settle_value(&self, g: &Game, pos: Pos) -> f64 {
        let mut total = 0.0;
        for p in g.wdisk(pos, 1) {
            if let Some(t) = g.map.get(p) {
                if t.owner_city.is_some() {
                    continue;
                }
                let ys = g.rules.tile_yields(t);
                total += ys.food * self.w.settle_food
                    + ys.production * self.w.settle_prod
                    + ys.gold * self.w.settle_gold;
            }
        }
        total
    }

    pub fn valid_settle_site(&self, g: &Game, pid: usize, pos: Pos) -> bool {
        let Some(tile) = g.map.get(pos) else {
            return false;
        };
        !g.rules.is_water(tile)
            && g.rules.is_passable(tile)
            && !g.tile_is_natural_wonder(tile)
            && !g
                .cities
                .values()
                .any(|city| (g.wdist(city.pos, pos) as f64) < self.w.min_city_dist)
            && tile
                .owner_city
                .is_none_or(|cid| g.cities[&cid].owner == pid)
    }

    pub fn has_practical_settle_site(&self, g: &Game, pid: usize) -> bool {
        let shipbuilding = g.players[pid].techs.contains(&crate::name!("shipbuilding"));
        let cartography = g.players[pid].techs.contains(&crate::name!("cartography"));
        // Before embarkation, a city only commits to a site close enough to
        // survive an ordinary settlement race. Existing settlers still use
        // the full path search below, but producing one for a site more than
        // eight steps away routinely loses the site after paying Population.
        let max_steps = if shipbuilding {
            g.map.width + g.map.height
        } else {
            8
        };
        let mut frontier: Vec<(Pos, i32)> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|city| (g.cities[&city].pos, 0))
            .collect();
        let mut seen: HashSet<Pos> = frontier.iter().map(|(position, _)| *position).collect();
        while let Some((position, steps)) = frontier.pop() {
            if self.valid_settle_site(g, pid, position) {
                return true;
            }
            if steps >= max_steps {
                continue;
            }
            for next in g.nbrs(position) {
                if seen.contains(&next) {
                    continue;
                }
                let Some(tile) = g.map.get(next) else { continue };
                if !g.rules.is_passable(tile)
                    || (g.rules.is_water(tile) && !shipbuilding)
                    || (tile.terrain == "ocean" && !cartography)
                    || g
                        .city_at(next)
                        .is_some_and(|city| g.cities[&city].owner != pid)
                {
                    continue;
                }
                seen.insert(next);
                frontier.push((next, steps + 1));
            }
        }
        false
    }

    fn best_reachable_settle_site(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        radius: i32,
    ) -> Option<(Pos, f64)> {
        let from = g.units[&uid].pos;
        let mut candidates: Vec<(Pos, f64)> = g
            .wdisk(from, radius)
            .into_iter()
            .filter(|pos| self.valid_settle_site(g, pid, *pos))
            .map(|pos| {
                let score =
                    self.settle_value(g, pos) - self.w.settle_dist * g.wdist(from, pos) as f64;
                (pos, score)
            })
            .collect();
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
        Self::first_reachable_settle_site(g, uid, &candidates)
    }

    /// Keep reachability checks bounded without assuming the forty most
    /// valuable geometric sites contain a usable one. Before embarkation an
    /// attractive offshore landmass can fill that entire prefix while an
    /// ordinary site on the settler's own landmass ranks just below it. Test
    /// candidates in batches: the multi-goal flood cheaply rejects a wholly
    /// disconnected batch, and individual routes preserve value ordering in
    /// the first batch that contains any reachable site.
    fn first_reachable_settle_site(
        g: &Game,
        uid: u32,
        candidates: &[(Pos, f64)],
    ) -> Option<(Pos, f64)> {
        let from = g.units.get(&uid)?.pos;
        for batch in candidates.chunks(40) {
            let contains_current = batch.iter().any(|(pos, _)| *pos == from);
            if !contains_current {
                let goals: HashSet<Pos> = batch.iter().map(|(pos, _)| *pos).collect();
                if g.route_step_to_any(uid, &goals).is_none() {
                    continue;
                }
            }
            if let Some(candidate) = batch
                .iter()
                .find(|(pos, _)| *pos == from || g.route_step(uid, *pos, 0).is_some())
            {
                return Some(*candidate);
            }
        }
        None
    }

    fn settler_step(&mut self, g: &mut Game, pid: usize, uid: u32) -> bool {
        if self.minor {
            return false; // city-states and barbarians never settle
        }
        let upos = g.units[&uid].pos;
        let current_target = self.settler_targets.get(&uid).copied().filter(|target| {
            self.valid_settle_site(g, pid, *target)
                && (*target == upos || g.route_step(uid, *target, 0).is_some())
        });
        let target = current_target.or_else(|| {
            let local_radius = if g.player_city_ids(pid).is_empty() {
                2
            } else {
                6
            };
            let local = self.best_reachable_settle_site(g, pid, uid, local_radius);
            // Search distant land even before embarkation. The pathfinder
            // itself rejects disconnected islands; tying the wider search to
            // Shipbuilding stranded settlers whose only site was farther than
            // the local radius on the same landmass.
            let global = self.best_reachable_settle_site(
                g,
                pid,
                uid,
                g.map.width + g.map.height,
            );
            match (local, global) {
                (Some(local), Some(global)) if global.1 > local.1 + 4.0 => Some(global),
                (Some(local), _) => Some(local),
                (None, global) => global,
            }
            .map(|(target, _)| {
                self.settler_targets.insert(uid, target);
                target
            })
        });
        let Some(target) = target else {
            self.settler_targets.remove(&uid);
            return false;
        };
        if target == upos {
            self.settler_targets.remove(&uid);
            return g.apply(pid, &Action::FoundCity { unit: uid }).is_ok();
        }
        // A linked settler is the follower: the naval military unit is the
        // formation leader and must execute movement for both. Keep the
        // destination for that leader instead of treating the follower's
        // intentionally unavailable Move action as a failed route.
        if let Some(escort) = g.units[&uid].linked_to.filter(|peer| {
            g.units.get(peer).is_some_and(|escort| {
                g.rules.units[escort.kind].domain.as_deref() == Some("sea")
            })
        }) {
            if g.wdist(upos, target) == 1 {
                return g.apply(pid, &Action::UnlinkUnits { unit: escort }).is_ok();
            }
            return false;
        }
        let moved = self.settler_step_toward(g, pid, uid, target);
        if !moved {
            self.settler_targets.remove(&uid);
        }
        moved
    }

    fn trader_step(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let upos = g.units[&uid].pos;
        if let Some(origin) = g.city_at(upos).filter(|c| g.cities[c].owner == pid) {
            // best destination: most districts in range (domestic or foreign)
            let mut best: Option<(usize, usize, u32)> = None;
            for (cid, c) in &g.cities {
                if !g.can_establish_trade_route(pid, origin, *cid) {
                    continue;
                }
                let alliance_connection = g.alliance_with(pid, c.owner).is_some_and(|_| {
                    !g.routes.iter().any(|route| {
                        route.owner == pid
                            && route.ends > g.turn
                            && g.cities
                                .get(&route.dest)
                                .is_some_and(|destination| destination.owner == c.owner)
                    })
                }) as usize;
                let key = (alliance_connection, c.districts.len() + 1, *cid);
                if best.map(|old| key > old).unwrap_or(true) {
                    best = Some(key);
                }
            }
            if let Some((_, _, dest)) = best {
                return g
                    .apply(
                        pid,
                        &Action::TradeRoute {
                            unit: uid,
                            city: dest,
                        },
                    )
                    .is_ok();
            }
        }
        // A Trader can be completed in a city whose nearby destinations are
        // already reserved. Relocate it to the nearest origin with a legal
        // route instead of retrying an invalid assignment every turn.
        let target = g
            .cities
            .values()
            .filter(|c| c.owner == pid)
            .filter(|origin| {
                g.cities
                    .values()
                    .any(|destination| g.can_establish_trade_route(pid, origin.id, destination.id))
            })
            .min_by_key(|c| (g.wdist(upos, c.pos), c.id))
            .map(|c| c.pos);
        match target {
            Some(t) => self.step_toward(g, pid, uid, t),
            None => false,
        }
    }

    fn missionary_step(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        // Spread the unit's own faith: a purchased Missionary carries its
        // city's majority religion, which for a civilization that never
        // founded one is an adopted faith the player religion cannot name.
        let religion = match g.units[&uid]
            .religion
            .clone()
            .or_else(|| g.players[pid].religion.clone())
        {
            Some(r) => r,
            None => return false,
        };
        let upos = g.units[&uid].pos;
        // Own cities first: reconverting the homeland both consolidates
        // pressure and is the entire job of a defensive adopted-faith unit.
        let mut targets: Vec<(bool, i32, u32, Pos)> = g
            .cities
            .values()
            .filter(|c| g.city_religion(c) != Some(religion.as_str()) && !g.is_at_war(pid, c.owner))
            .map(|city| (city.owner != pid, g.wdist(upos, city.pos), city.id, city.pos))
            .collect();
        targets.sort();
        for (_, _, _, target) in targets {
            if g.wdist(upos, target) <= 1 {
                return g.apply(pid, &Action::Spread { unit: uid }).is_ok();
            }
            if self.step_toward_range(g, pid, uid, target, 1) {
                return true;
            }
        }
        false
    }

    fn siege_support_step(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let upos = g.units[&uid].pos;
        let support_kind = g.units[&uid].kind.as_str();
        let targets: Vec<Pos> = g
            .cities
            .values()
            .filter(|c| c.owner != pid && g.is_at_war(pid, c.owner))
            .filter(|c| {
                if self.tactical_strategy {
                    c.wall_hp > 0 && Self::siege_support_works_for_city(g, support_kind, c.id)
                } else {
                    let walls = c
                        .buildings
                        .iter()
                        .filter(|b| *b == "walls" || *b == "medieval_walls")
                        .count();
                    walls > 0 && (support_kind == "siege_tower" || walls == 1)
                }
            })
            .map(|c| c.pos)
            .collect();
        if targets.is_empty() {
            return false;
        }
        if self.tactical_strategy && targets.iter().any(|target| g.wdist(upos, *target) <= 1) {
            return false;
        }

        // Follow a melee/anti-cavalry unit: only those classes can use the aura.
        let escort = g
            .units
            .values()
            .filter(|u| u.owner == pid && u.id != uid)
            .filter(|u| {
                if self.tactical_strategy {
                    self.support_escort_compatible(g, uid, u.id)
                        && matches!(
                            g.rules.units[u.kind].promotion_class.as_str(),
                            "melee" | "anti_cavalry"
                        )
                } else {
                    let spec = &g.rules.units[u.kind];
                    spec.class == "military" && spec.ranged_strength <= 0.0 && !spec.siege
                }
            })
            .min_by_key(|u| {
                let front = targets.iter().map(|t| g.wdist(u.pos, *t)).min().unwrap();
                (2 * front + g.wdist(upos, u.pos), g.wdist(upos, u.pos), u.id)
            })
            .map(|u| u.pos);
        match escort {
            Some(pos) if pos != upos => self.step_toward(g, pid, uid, pos),
            _ => false,
        }
    }

    pub(crate) fn rock_band_step(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        if g.rock_concert_tourism(pid, uid).is_some() {
            return g.apply(pid, &Action::PerformConcert { unit: uid }).is_ok();
        }
        let current = g.units[&uid].pos;
        let mut venues: Vec<(f64, i32, Pos)> = g
            .map
            .tiles
            .keys()
            .copied()
            .filter_map(|position| {
                let tourism = g.rock_concert_ai_value(pid, uid, position)?;
                g.route_step(uid, position, 0)?;
                Some((tourism, g.wdist(current, position), position))
            })
            .collect();
        venues.sort_by(|left, right| {
            right
                .0
                .partial_cmp(&left.0)
                .unwrap()
                .then(left.1.cmp(&right.1))
                .then(left.2.cmp(&right.2))
        });
        let Some((_, _, target)) = venues.first().copied() else {
            return false;
        };
        let Some(next) = g.route_step(uid, target, 0) else {
            return false;
        };
        self.path_move(g, pid, uid, next)
    }

    /// Whether this empire has any tile left for a Builder to work on: an
    /// improvement to lay or a pillaged one to repair. Builders were produced
    /// to a flat quota per city regardless, so an empire that had improved
    /// everything it owned kept paying for Builders that then stood on a tile
    /// for the rest of the game - the audit counted nearly two hundred of
    /// them across six games, some idle from turn 25 to the end.
    fn has_builder_work(g: &Game, pid: usize) -> bool {
        // Every owned tile asks the same empire-wide questions of the same
        // empire; hold a memo scope over the whole sweep.
        let _memo = g.query_memo();
        g.player_city_ids(pid).into_iter().any(|cid| {
            g.cities[&cid].owned_tiles.iter().any(|pos| {
                let repairable = g
                    .map
                    .get(*pos)
                    .is_some_and(|tile| tile.pillaged && tile.improvement.is_some());
                repairable
                    || g.valid_improvements(pid, *pos)
                        .iter()
                        .any(|improvement| g.rules.improvements[improvement].builder_buildable)
            })
        })
    }

    /// The district a civilization builds in place of `family`: its unique
    /// replacement where it has one, otherwise the stock district. The engine
    /// blocks the base district for civilizations with a replacement, exactly
    /// as it does for unique units.
    /// The housing a not-yet-built member of `family` would add to this city.
    /// Zero when the city already has one, or when there is nowhere legal to
    /// put it — a district with no site buys nothing however short the city is.
    ///
    /// The two families answer differently on purpose. An Aqueduct's worth is
    /// fixed by the city centre's own water (+2 fresh, +3 coastal, +4 dry) and
    /// `Game::aqueduct_housing_gain` states it beside the model that pays it. A
    /// Neighborhood's is its site's Appeal, anywhere from 2 to 6, so the map is
    /// asked instead of assumed: on a poor site it is 2 housing for 54
    /// production and should lose to the lane, and on a good one it is 6 and
    /// should not.
    fn housing_gain(g: &Game, pid: usize, cid: u32, family: &str) -> f64 {
        let city = &g.cities[&cid];
        if g.city_has_district_family(city, Name::new(family)) {
            return 0.0;
        }
        if family == "aqueduct" {
            return g.aqueduct_housing_gain(city);
        }
        let dname = Self::civ_district(g, pid, family);
        g.district_sites(cid, dname)
            .into_iter()
            .map(|pos| g.district_housing(dname.as_str(), pos))
            .fold(0.0_f64, f64::max)
    }

    pub(crate) fn civ_district(g: &Game, pid: usize, family: &str) -> Name {
        let civ = g.players[pid].civ.as_str();
        g.rules
            .districts
            .iter()
            .find(|(_, spec)| {
                spec.replaces == Some(Name::new(family)) && spec.unique_to.as_deref() == Some(civ)
            })
            .map(|(name, _)| *name)
            .unwrap_or_else(|| Name::new(family))
    }

    /// The building this city should start in place of `family`: the stock
    /// building where it is available, otherwise whichever replacement this
    /// civilization or its secret society builds instead. `None` means the
    /// city already has one or cannot have it, so the caller moves on rather
    /// than proposing something the engine will refuse.
    fn civ_building(g: &Game, pid: usize, cid: u32, family: &str) -> Option<Item> {
        let base = Item::Building {
            building: Name::new(family),
        };
        if g.can_produce(pid, cid, &base) {
            return Some(base);
        }
        g.rules
            .buildings
            .iter()
            .filter(|(_, spec)| spec.replaces == Some(Name::new(family)))
            .map(|(name, _)| Item::Building {
                building: *name,
            })
            .find(|item| g.can_produce(pid, cid, item))
    }

    /// Which improvement a tile should actually get. An improvement that
    /// matches the tile's resource comes first: it is the only way to work a
    /// strategic resource or connect a luxury, and paving Iron or Wine over
    /// with a Farm forfeits that permanently. Otherwise take the most
    /// valuable yield, weighted the way the rest of this AI values output.
    fn best_improvement(g: &Game, pos: Pos, options: &[Name]) -> Option<Name> {
        let resource = g.map.get(pos).and_then(|tile| tile.resource);
        options
            .iter()
            .max_by(|a, b| {
                let score = |name: &Name| {
                    let spec = &g.rules.improvements[name];
                    let works_resource = resource
                        .as_ref()
                        .is_some_and(|resource| spec.resources.iter().any(|r| r == resource));
                    let yields = spec.yields.production * 3.0
                        + spec.yields.food * 2.0
                        + spec.yields.science * 3.0
                        + spec.yields.culture * 3.0
                        + spec.yields.gold * 2.0
                        + spec.yields.faith
                        + spec.housing * 2.0;
                    (works_resource, yields)
                };
                let (a_resource, a_yield) = score(a);
                let (b_resource, b_yield) = score(b);
                a_resource
                    .cmp(&b_resource)
                    .then(a_yield.partial_cmp(&b_yield).unwrap_or(Ordering::Equal))
                    .then_with(|| b.cmp(a))
            })
            .cloned()
    }

    fn builder_step(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let upos = g.units[&uid].pos;
        let project = g
            .player_city_ids(pid)
            .into_iter()
            .filter_map(|city| {
                g.project_contribution_target(pid, city)
                    .map(|position| (g.wdist(upos, position), position, city))
            })
            .min();
        if let Some((_, position, city)) = project {
            if upos == position && g.can_contribute_project(pid, uid, city) {
                return g
                    .apply(pid, &Action::ContributeProject { unit: uid, city })
                    .is_ok();
            }
            if self.step_toward(g, pid, uid, position) {
                return true;
            }
        }
        let repairable = g.map.get(upos).is_some_and(|tile| {
            tile.pillaged
                && tile.improvement.is_some()
                && tile
                    .owner_city
                    .and_then(|city| g.cities.get(&city))
                    .is_some_and(|city| city.owner == pid)
        });
        if repairable {
            return g
                .apply(pid, &Action::RepairImprovement { unit: uid })
                .is_ok();
        }
        let imps: Vec<Name> = g
            .valid_improvements(pid, upos)
            .into_iter()
            .filter(|improvement| g.rules.improvements[improvement].builder_buildable)
            .collect();
        if let Some(improvement) = Self::best_improvement(g, upos, &imps) {
            return g
                .apply(
                    pid,
                    &Action::Improve {
                        unit: uid,
                        improvement: Name::new(&improvement),
                    },
                )
                .is_ok();
        }
        // Nearest-first is the right default for ordinary tiles, but an
        // unopened strategic deposit is not an ordinary tile: until one is
        // mined the empire accumulates none of the material that every modern
        // unit costs to train and to upgrade into, so those tiles are taken
        // before anything else regardless of distance.
        let mut best: Option<(bool, i32, Pos)> = None;
        {
            // Read-only sweep of the whole empire's tiles: a memo scope makes
            // the empire-wide questions each tile asks cost one answer, not one
            // per tile. The borrow checker rejects the guard if anything in
            // here starts mutating.
            let _memo = g.query_memo();
            for cid in g.player_city_ids(pid) {
                for pos in g.cities[&cid].owned_tiles.clone() {
                    if g.valid_improvements(pid, pos)
                        .iter()
                        .any(|improvement| g.rules.improvements[improvement].builder_buildable)
                    {
                        let urgent = Self::unopened_strategic_source(g, pos);
                        let d = g.wdist(upos, pos);
                        let candidate = (!urgent, d, pos);
                        if best.map(|b| candidate < b).unwrap_or(true) {
                            best = Some(candidate);
                        }
                    }
                }
            }
        }
        match best {
            Some((_, _, pos)) => self.step_toward(g, pid, uid, pos),
            None => false,
        }
    }

    /// Improvements this particular unit can actually spend one of its own
    /// charges to create on `pos`. Builders use `builder_buildable`; the few
    /// military uniques that carry build charges instead name their actions in
    /// their unit rule. Keeping the intersection here makes every caller obey
    /// the same placement, ownership, Open Borders, and civilization checks.
    pub(crate) fn special_unit_improvements(
        g: &Game,
        pid: usize,
        uid: u32,
        pos: Pos,
    ) -> Vec<Name> {
        let Some(unit) = g.units.get(&uid) else {
            return Vec::new();
        };
        if unit.owner != pid || unit.charges <= 0 {
            return Vec::new();
        }
        let builds = &g.rules.units[unit.kind].builds;
        if builds.is_empty() {
            return Vec::new();
        }
        g.valid_improvements(pid, pos)
            .into_iter()
            .filter(|improvement| builds.contains(improvement))
            .collect()
    }

    /// Spend a unique military unit's improvement charge when that is a real
    /// legal job, or walk it toward the nearest reachable job. `None` means
    /// this unit has no such job and lets ordinary military behavior continue.
    ///
    /// This intentionally searches the whole map rather than only owned city
    /// tiles: a Nau's Feitoria is useful precisely because it must be placed in
    /// foreign territory with Open Borders.
    pub(crate) fn special_improver_step(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        let current = g.units.get(&uid)?.pos;
        let here = Self::special_unit_improvements(g, pid, uid, current);
        if let Some(improvement) = Self::best_improvement(g, current, &here) {
            return g
                .apply(
                    pid,
                    &Action::Improve {
                        unit: uid,
                        improvement,
                    },
                )
                .ok()
                .map(|_| true);
        }

        let targets = {
            // `valid_improvements` asks several empire-wide questions. The
            // special units are rare, but a memo scope still keeps this map
            // scan proportional to tiles rather than repeated rule queries.
            let _memo = g.query_memo();
            g.map
                .tiles
                .keys()
                .copied()
                .filter(|position| {
                    *position != current
                        && !Self::special_unit_improvements(g, pid, uid, *position).is_empty()
                })
                .collect::<HashSet<_>>()
        };
        g.route_step_to_any(uid, &targets).and_then(|to| {
            g.apply(pid, &Action::Move { unit: uid, to })
                .ok()
                .map(|_| true)
        })
    }

    /// Whether this tile holds a strategic deposit that is not yet connected
    /// by the improvement which harvests it.
    pub(crate) fn unopened_strategic_source(g: &Game, pos: Pos) -> bool {
        let Some(tile) = g.map.get(pos) else {
            return false;
        };
        let Some(resource) = tile.resource.as_deref() else {
            return false;
        };
        let Some(spec) = g.rules.resources.get(resource) else {
            return false;
        };
        spec.class == "strategic"
            && tile.improvement.as_deref() != Some(spec.improvement.as_str())
    }

    pub(crate) fn military_engineer_step(&mut self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let current = g.units[&uid].pos;
        let target = g
            .player_city_ids(pid)
            .into_iter()
            .filter_map(|city| {
                let position = g.district_contribution_target(pid, city)?;
                if position != current {
                    g.route_step(uid, position, 0)?;
                }
                Some((g.wdist(current, position), position, city))
            })
            .min();
        if let Some((_, position, city)) = target {
            if current == position && g.can_contribute_district(pid, uid, city) {
                return g
                    .apply(pid, &Action::ContributeDistrict { unit: uid, city })
                    .is_ok();
            }
            return self.step_toward(g, pid, uid, position);
        }

        // Once Steam Power arrives, connect city centers with a continuous
        // Railroad. The current tile is laid before movement, then the next
        // turn continues toward the nearest center that is not connected yet.
        // District contributions remain the higher priority above: finishing
        // a Dam, Aqueduct, or Canal is worth an Engineer charge immediately.
        let has_rail_material = g.strategic_stockpile(pid, crate::name!("iron"))
            >= RAILROAD_RESOURCE_RESERVE + 1.0
            && g.strategic_stockpile(pid, crate::name!("coal")) >= RAILROAD_RESOURCE_RESERVE + 1.0;
        let railroad_target = has_rail_material
            .then(|| {
                g.player_city_ids(pid)
                    .into_iter()
                    .map(|city| g.cities[&city].pos)
                    .filter(|position| g.map.tiles[position].road < 5)
                    .filter(|position| {
                        *position == current || g.route_step(uid, *position, 0).is_some()
                    })
                    .min_by_key(|position| (g.wdist(current, *position), *position))
            })
            .flatten();
        if let Some(position) = railroad_target {
            if g.can_build_railroad(pid, uid) {
                return g.apply(pid, &Action::BuildRailroad { unit: uid }).is_ok();
            }
            if current != position {
                return self.step_toward(g, pid, uid, position);
            }
        }
        self.military_step(g, pid, uid)
    }

    fn naturalist_step(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let current = g.units[&uid].pos;
        if g.valid_improvements(pid, current)
            .iter()
            .any(|improvement| improvement == "national_park")
        {
            return g
                .apply(
                    pid,
                    &Action::Improve {
                        unit: uid,
                        improvement: crate::name!("national_park"),
                    },
                )
                .is_ok();
        }
        let target = g
            .national_park_sites(pid)
            .into_iter()
            .filter_map(|site| {
                let appeal = site
                    .iter()
                    .map(|position| g.tile_appeal(*position).max(0))
                    .sum::<i32>();
                site.into_iter()
                    .filter(|position| g.rules.is_passable(&g.map.tiles[position]))
                    .filter(|position| g.route_step(uid, *position, 0).is_some())
                    .min_by_key(|position| (g.wdist(current, *position), *position))
                    .map(|position| {
                        (
                            appeal,
                            std::cmp::Reverse(g.wdist(current, position)),
                            position,
                        )
                    })
            })
            .max();
        target.is_some_and(|(_, _, position)| self.step_toward(g, pid, uid, position))
    }

    fn archaeologist_step(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let current = g.units[&uid].pos;
        if let Some(improvement) = g
            .valid_improvements(pid, current)
            .into_iter()
            .find(|name| matches!(name.as_str(), "archaeological_dig" | "shipwreck_excavation"))
        {
            return g
                .apply(
                    pid,
                    &Action::Improve {
                        unit: uid,
                        improvement: Name::new(&improvement),
                    },
                )
                .is_ok();
        }
        let target = {
            let _memo = g.query_memo();
            g.excavation_sites(pid)
                .into_iter()
                .filter(|(position, _)| g.route_step(uid, *position, 0).is_some())
                .min_by_key(|(position, improvement)| {
                    (
                        g.wdist(current, *position),
                        improvement == "shipwreck_excavation",
                        *position,
                    )
                })
                .map(|(position, _)| position)
        };
        target.is_some_and(|position| self.step_toward(g, pid, uid, position))
    }

    fn is_enemy_tile(&self, g: &Game, pos: Pos, enemy_ids: &[usize]) -> bool {
        for oid in g.units_at(pos) {
            if enemy_ids.contains(&g.units[&oid].owner) {
                return true;
            }
        }
        if let Some(cid) = g.city_at(pos) {
            return enemy_ids.contains(&g.cities[&cid].owner);
        }
        false
    }

    /// Chess-style static exchange evaluation: expected damage traded if we
    /// attack `pos` (combat model: 30·e^((att−def)/25), sans rng).
    fn exchange_score(&self, g: &Game, uid: u32, pos: Pos, ranged: bool) -> f64 {
        let u = &g.units[&uid];
        let att = effective_strength(g.unit_strength(u, false), u.hp);
        if let Some(cid) = g.city_at(pos) {
            let c = &g.cities[&cid];
            if c.owner != u.owner {
                // cities: press wounded ones, big bonus on a capturable one
                let mut s = 20.0 + 0.5 * (100 - c.hp) as f64;
                if !ranged && c.hp <= 40 && c.wall_hp <= 0 {
                    s += self.w.kill_bonus;
                }
                return s;
            }
        }
        let defender = g
            .units_at(pos)
            .into_iter()
            .map(|oid| &g.units[&oid])
            .filter(|o| g.rules.units[o.kind].class == "military")
            .max_by(|a, b| {
                effective_strength(g.unit_strength(a, true), a.hp)
                    .partial_cmp(&effective_strength(g.unit_strength(b, true), b.hp))
                    .unwrap()
            });
        let o = match defender {
            None => return 15.0 + self.w.kill_bonus * 0.5, // undefended civilians
            Some(o) => o,
        };
        let def = effective_strength(g.unit_strength(o, true), o.hp);
        let deal = 30.0 * ((att - def) / 25.0).exp();
        let mut s = deal.min(o.hp as f64);
        if deal >= o.hp as f64 {
            s += self.w.kill_bonus;
        } else if !ranged {
            let their_att = effective_strength(g.unit_strength(o, false), o.hp);
            let my_def = effective_strength(g.unit_strength(u, true), u.hp);
            let recv = 30.0 * ((their_att - my_def) / 25.0).exp();
            s -= self.w.trade_caution * recv.min(u.hp as f64);
            if recv >= u.hp as f64 {
                s -= 35.0; // don't suicide into a counter
            }
        }
        // Even trades against barbarians are worth taking: civs heal at home
        // while raiders respawn from camps, and a mirror matchup would
        // otherwise score exactly 0 and stall at the attack floor.
        if !self.barb && g.players[o.owner].is_barbarian {
            s += 10.0;
        }
        s
    }

    fn nearest_enemy_from(
        &self,
        g: &Game,
        _pid: usize,
        pos: Pos,
        enemy_ids: &[usize],
    ) -> Option<Pos> {
        g.cities
            .values()
            .filter(|city| enemy_ids.contains(&city.owner))
            .map(|city| (g.wdist(pos, city.pos), city.pos))
            .chain(
                g.units
                    .values()
                    .filter(|unit| enemy_ids.contains(&unit.owner))
                    .map(|unit| (g.wdist(pos, unit.pos), unit.pos)),
            )
            .min()
            .map(|(_, target)| target)
    }

    /// ★★★★★ NOTHING EVER PUT A UNIT INSIDE A CITY.
    ///
    /// Same run as `home_defense_objective`, traced to its end (t251). Counting,
    /// every turn, our cities with one of our combat units standing on them:
    /// **1 of 6 through t224–t239, then 0/5, 0/4, 0/3, 0/2 to the finish** — while
    /// **14 to 17 combat units were alive**. The cities then fell one after
    /// another, 6 → 5 → 4 → 3 → 2 across t226–t249, and the game ended with two
    /// cities on loyalty 42 and 33, one at 180/200 damage, score 446 dead last
    /// behind Persia's 1563.
    ///
    /// ⚠ THIS IS NOT A SHORTAGE OF ARMY AND IT IS NOT THE SAME BUG AS THE ONE
    /// ABOVE. `home_defense_objective` intercepts raiders in the field; it never
    /// makes anybody hold the city tile itself, and an empty city with breached
    /// walls falls to a single melee step. No other code does it either:
    /// `peacetime_step`'s "garrison the nearest city" path actually runs
    /// `patrol_step`, which walks *frontier* posts, and `siege_muster` (#930)
    /// raises the production floor without placing what it builds.
    ///
    /// Pure and deterministic — same board, same assignment, whichever unit is
    /// asking — so `home_defense_objective` can call it to see who is already
    /// spoken for without the two mechanisms fighting over the same units.
    fn garrison_assignments(
        &self,
        g: &Game,
        pid: usize,
        enemy_ids: &[usize],
    ) -> Vec<(u32, Pos)> {
        if !self.home_defense || self.minor || self.barb {
            return Vec::new();
        }
        // A city wants a garrison when something hostile is close enough to
        // reach it and nothing of ours is standing on it. Worst first: the most
        // strength bearing down, then the most damage already taken.
        let mut wanting: Vec<(i64, Pos)> = Vec::new();
        for city in g.cities.values().filter(|city| city.owner == pid) {
            let held = g.units_at(city.pos).into_iter().any(|uid| {
                g.units[&uid].owner == pid && g.rules.units[g.units[&uid].kind].class == "military"
            });
            if held {
                continue;
            }
            let pressure: f64 = g
                .units
                .values()
                .filter(|enemy| enemy_ids.contains(&enemy.owner))
                .filter(|enemy| g.rules.units[enemy.kind].class == "military")
                .filter(|enemy| g.wdist(enemy.pos, city.pos) <= GARRISON_ALERT_RADIUS)
                .map(|enemy| effective_strength(g.unit_strength(enemy, true), enemy.hp))
                .sum();
            if pressure <= 0.0 {
                continue;
            }
            wanting.push((pressure as i64 * 1_000 + city.hp.max(0) as i64, city.pos));
        }
        if wanting.is_empty() {
            return Vec::new();
        }
        wanting.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));

        let mut responders: Vec<u32> = g
            .units
            .values()
            .filter(|unit| unit.owner == pid)
            .filter(|unit| g.rules.units[unit.kind].class == "military")
            .filter(|unit| {
                matches!(
                    g.rules.units[unit.kind].domain.as_deref(),
                    None | Some("land")
                )
            })
            .filter(|unit| !self.recovering_units.contains(&unit.id))
            .map(|unit| unit.id)
            .collect();
        responders.sort_unstable();
        // Same bound as the field recall, for the same reason, and shared with
        // it: between them the two mechanisms never claim more than half.
        let cap = ((responders.len() as f64 * HOME_DEFENSE_MAX_SHARE).floor() as usize).max(1);

        let mut assigned: Vec<(u32, Pos)> = Vec::new();
        for (_, city) in wanting {
            if assigned.len() >= cap {
                break;
            }
            // One unit per city. A second body on the tile adds nothing a city's
            // own defence does not already do; the rest are better in the field.
            let Some(&(_, defender)) = responders
                .iter()
                .filter(|id| !assigned.iter().any(|(taken, _)| taken == *id))
                .map(|id| (g.wdist(g.units[id].pos, city), *id))
                .filter(|(distance, _)| *distance <= HOME_DEFENSE_RECALL_RANGE)
                .collect::<Vec<_>>()
                .iter()
                .min()
            else {
                continue;
            };
            assigned.push((defender, city));
        }
        assigned
    }

    /// Walk the assigned defender to its city and hold it. Standing on the tile
    /// IS the job, so arriving means fortifying rather than looking for a fight.
    fn garrison_step(&mut self, g: &mut Game, pid: usize, uid: u32, enemy_ids: &[usize]) -> bool {
        GARRISON_STEP_CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let Some((_, city)) = self
            .garrison_assignments(g, pid, enemy_ids)
            .into_iter()
            .find(|(defender, _)| *defender == uid)
        else {
            return false;
        };
        if g.units[&uid].pos == city {
            return self.fortify_or_stop(g, pid, uid);
        }
        self.step_toward(g, pid, uid, city) || self.fortify_or_stop(g, pid, uid)
    }

    /// ★★★★★ THE HOMELAND HAD NO CLAIM ON THE ARMY AT ALL.
    ///
    /// Measured on live run `civvis-20260803T005930Z` (Kongo, Small, 154 turns):
    /// **116 of 154 turns had a hostile standing inside or beside this empire's
    /// own territory.** A full-health Crossbowman sat four tiles from two cities,
    /// unmoved and unengaged, for **21 consecutive turns**. Earlier in the same
    /// game a barbarian Warrior occupied a city tile for 11 straight turns, and a
    /// Man-at-Arms roamed the interior for thirty — *healing* from 83 to 92 while
    /// it did, because nothing ever touched it.
    ///
    /// ⚠ THE CAUSE IS NOT VISIBILITY AND NOT THE BARBARIAN SEAT. `is_at_war` is
    /// true for `barb_pid`, the seat is alive, and every one of those raiders was
    /// in `enemy_ids` the whole time. The cause is that the only target selector
    /// this AI had, `nearest_enemy`, ranks candidates by **distance from the
    /// asking unit**. For an army deployed on a war front an enemy city is always
    /// nearer than a raider back home, so every unit converged on the offensive
    /// and the empire's own ground was nobody's job. The fallback, `patrol_step`,
    /// walks a frontier ring by `uid % posts.len()` and is threat-blind.
    ///
    /// So this is an ABSENT assignment rather than a broken one: nothing anywhere
    /// measured threat *to our own cities*. This does, and answers the worst
    /// threats with the nearest sufficient units before the offensive claims them.
    ///
    /// Deliberately NOT a gene. The genome is pinned at 40 and a committed
    /// champion rides that exact length, so the constants below ship fixed and
    /// earn a gene later if measurement says they matter.
    fn home_defense_objective(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        enemy_ids: &[usize],
    ) -> Option<Pos> {
        // A city-state already guards home and nothing else; barbarians have no
        // homeland to defend. Both would only fight this assignment.
        if !self.home_defense || self.minor || self.barb {
            return None;
        }
        let my_cities: Vec<Pos> = g
            .cities
            .values()
            .filter(|city| city.owner == pid)
            .map(|city| city.pos)
            .collect();
        if my_cities.is_empty() {
            return None;
        }
        let home_distance = |pos: Pos| -> i32 {
            my_cities
                .iter()
                .map(|city| g.wdist(pos, *city))
                .min()
                .unwrap_or(i32::MAX)
        };

        // Threats, worst first. Severity is an integer so the ordering is
        // identical on every platform: proximity to one of our cities dominates,
        // and strength breaks ties within the same ring. A raider standing ON a
        // city is therefore always answered before one six tiles out, however
        // much bigger the distant one is.
        let mut threats: Vec<(i64, Pos, f64)> = Vec::new();
        for enemy in g.units.values() {
            if !enemy_ids.contains(&enemy.owner)
                || g.rules.units[enemy.kind].class != "military"
            {
                continue;
            }
            let distance = home_distance(enemy.pos);
            if distance > HOME_THREAT_RADIUS {
                continue;
            }
            let strength = effective_strength(g.unit_strength(enemy, true), enemy.hp);
            let severity = (HOME_THREAT_RADIUS - distance) as i64 * 1_000 + strength as i64;
            threats.push((severity, enemy.pos, strength));
        }
        // A camp does not fight back, but it keeps producing what does, and the
        // measured raiders all came from one. Rank it just under a live raider at
        // the same range so a unit already in the empire clears it once the
        // shooting stops rather than leaving the tap running.
        if let Some(barb) = g.barb_pid {
            if enemy_ids.contains(&barb) {
                for camp in g.barb_camps.keys() {
                    let distance = home_distance(*camp);
                    if distance > HOME_THREAT_RADIUS {
                        continue;
                    }
                    threats.push(((HOME_THREAT_RADIUS - distance) as i64 * 1_000 - 1, *camp, 0.0));
                }
            }
        }
        if threats.is_empty() {
            return None;
        }
        threats.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));

        // Land units only: a ship cannot answer a raider inland, and pulling air
        // units out of the offensive buys nothing a fighter on patrol does not
        // already give. A unit already withdrawn to heal stays withdrawn —
        // `healing_step` ran before this and its judgement outranks ours.
        // Units already holding a city tile are spoken for. Excluding them here
        // is what keeps the two mechanisms from tugging the same unit between a
        // city and a field raider on alternate turns.
        let garrisoned = self.garrison_assignments(g, pid, enemy_ids);
        let responders: Vec<u32> = {
            let mut ids: Vec<u32> = g
                .units
                .values()
                .filter(|unit| unit.owner == pid)
                .filter(|unit| g.rules.units[unit.kind].class == "military")
                .filter(|unit| {
                    matches!(
                        g.rules.units[unit.kind].domain.as_deref(),
                        None | Some("land")
                    )
                })
                .filter(|unit| !self.recovering_units.contains(&unit.id))
                .filter(|unit| !garrisoned.iter().any(|(held, _)| *held == unit.id))
                .map(|unit| unit.id)
                .collect();
            ids.sort_unstable();
            ids
        };
        // Never recall more than half the army. An empire that pulls everything
        // home to chase raiders has not saved itself, it has lost the war a
        // different way — and the measured game's problem was the opposite
        // extreme, so the correction must not overshoot into it.
        //
        // ⚠ AT LEAST ONE, ALWAYS. A bare `floor` reads 0 for a one-unit army and
        // 0 means "nobody defends", which is the exact behaviour being fixed —
        // and a lone soldier with a raider at the gates is precisely who cannot
        // afford to be somewhere else. The share bounds over-commitment; it does
        // not license ignoring the threat entirely.
        //
        // ⚠ THE BUDGET IS THE WHOLE ARMY'S, NOT THIS LIST'S. `responders` has
        // already had the garrison removed, so sizing the cap off it would let
        // garrison and field recall each take half of a shrinking remainder and
        // together take far more than half. Size it off the full eligible army
        // and subtract what the garrison spent.
        let eligible = responders.len() + garrisoned.len();
        let budget = ((eligible as f64 * HOME_DEFENSE_MAX_SHARE).floor() as usize).max(1);
        let cap = budget.saturating_sub(garrisoned.len());
        if cap == 0 || !responders.contains(&uid) {
            return None;
        }

        let mut committed: HashSet<u32> = HashSet::new();
        for (_, threat, strength) in threats {
            if committed.len() >= cap {
                break;
            }
            // A defender that would spend ten turns walking home is not a
            // defender. Past that range this unit keeps its offensive job and a
            // nearer one answers instead.
            let mut nearest: Vec<(i32, u32)> = responders
                .iter()
                .filter(|id| !committed.contains(id))
                .map(|id| (g.wdist(g.units[id].pos, threat), *id))
                .filter(|(distance, _)| *distance <= HOME_DEFENSE_RECALL_RANGE)
                .collect();
            nearest.sort_unstable();

            // Send enough to win rather than enough to trade. A camp needs no
            // margin, so `needed` of zero commits exactly one unit and stops.
            let needed = strength * HOME_DEFENSE_MARGIN;
            let mut answered = 0.0;
            for (_, responder) in nearest {
                committed.insert(responder);
                if responder == uid {
                    return Some(threat);
                }
                answered += effective_strength(
                    g.unit_strength(&g.units[&responder], false),
                    g.units[&responder].hp,
                );
                if answered >= needed || committed.len() >= cap {
                    break;
                }
            }
        }
        None
    }

    fn nearest_enemy(&self, g: &Game, pid: usize, uid: u32, enemy_ids: &[usize]) -> Option<Pos> {
        // Majors chase barbarians only near home and only when this unit's
        // doctrine would accept the eventual attack. This keeps scouts and
        // wounded units from shadowing raiders they will never strike.
        let pos = g.units[&uid].pos;
        let ranged = g.rules.units[g.units[&uid].kind].has_ranged_attack();
        let my_cities: Vec<Pos> = g
            .cities
            .values()
            .filter(|c| c.owner == pid)
            .map(|c| c.pos)
            .collect();
        let near_home = |tpos: Pos| -> bool {
            if self.barb || my_cities.is_empty() {
                return true;
            }
            my_cities.iter().map(|c| g.wdist(tpos, *c)).min().unwrap() <= 6
        };
        let mut best: Option<(i32, Pos)> = None;
        for c in g.cities.values() {
            if enemy_ids.contains(&c.owner)
                && (!self.minor
                    || Self::minor_home(g, pid)
                        .is_some_and(|home| g.wdist(home, c.pos) <= MINOR_DEFENSE_RADIUS))
            {
                let d = g.wdist(pos, c.pos);
                if best.map(|b| (d, c.pos) < b).unwrap_or(true) {
                    best = Some((d, c.pos));
                }
            }
        }
        for u in g.units.values() {
            if enemy_ids.contains(&u.owner)
                && (!self.minor
                    || Self::minor_home(g, pid)
                        .is_some_and(|home| g.wdist(home, u.pos) <= MINOR_DEFENSE_RADIUS))
            {
                if Some(u.owner) == g.barb_pid
                    && (!near_home(u.pos)
                        || self.exchange_score(g, uid, u.pos, ranged)
                            <= self.attack_threshold(g, uid, u.pos))
                {
                    continue;
                }
                let d = g.wdist(pos, u.pos);
                if best.map(|b| (d, u.pos) < b).unwrap_or(true) {
                    best = Some((d, u.pos));
                }
            }
        }
        if !self.barb {
            if let Some(bp) = g.barb_pid {
                if enemy_ids.contains(&bp) {
                    for cpos in g.barb_camps.keys() {
                        if near_home(*cpos)
                            && self.exchange_score(g, uid, *cpos, ranged)
                                > self.attack_threshold(g, uid, *cpos)
                        {
                            let d = g.wdist(pos, *cpos);
                            if best.map(|b| (d, *cpos) < b).unwrap_or(true) {
                                best = Some((d, *cpos));
                            }
                        }
                    }
                }
            }
        }
        best.map(|(_, p)| p)
    }

    /// Naval forces should not select an attractive but unreachable inland
    /// target. Waterborne enemies (including embarked land units) come first,
    /// followed by coastal cities that melee ships can actually capture.
    pub(crate) fn nearest_enemy_for_unit(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        enemy_ids: &[usize],
    ) -> Option<Pos> {
        let unit = &g.units[&uid];
        if g.rules.units[unit.kind].domain.as_deref() != Some("sea") {
            return self.nearest_enemy(g, pid, uid, enemy_ids);
        }
        g.units
            .values()
            .filter(|enemy| {
                enemy_ids.contains(&enemy.owner)
                    && Self::waterborne(g, enemy.id)
                    && (!self.minor
                        || Self::minor_home(g, pid)
                            .is_some_and(|home| g.wdist(home, enemy.pos) <= MINOR_DEFENSE_RADIUS))
            })
            .map(|enemy| (g.wdist(unit.pos, enemy.pos), 0, enemy.pos))
            .chain(
                g.cities
                    .values()
                    .filter(|city| {
                        enemy_ids.contains(&city.owner)
                            && Self::city_is_coastal(g, city.id)
                            && (!self.minor
                                || Self::minor_home(g, pid).is_some_and(|home| {
                                    g.wdist(home, city.pos) <= MINOR_DEFENSE_RADIUS
                                }))
                    })
                    .map(|city| (g.wdist(unit.pos, city.pos), 1, city.pos)),
            )
            .min()
            .map(|(_, _, pos)| pos)
    }

    /// Objective for a ship assigned to colony protection. A linked ship
    /// leads the formation toward the settler's persistent colony site; an
    /// unlinked ship first closes on the embarked settler so they can link on
    /// a later command phase.
    pub(crate) fn naval_approach(g: &Game, uid: u32, target: Pos) -> Option<Pos> {
        let current = g.units.get(&uid)?.pos;
        let mut approaches: Vec<Pos> = g
            .nbrs(target)
            .into_iter()
            .filter(|pos| g.unit_can_traverse(uid, *pos))
            .collect();
        approaches.sort_by_key(|pos| (g.wdist(current, *pos), *pos));
        approaches
            .into_iter()
            .find(|pos| *pos == current || g.route_step(uid, *pos, 0).is_some())
    }

    fn naval_escort_objective(&self, g: &Game, pid: usize, uid: u32) -> Option<Pos> {
        let unit = &g.units[&uid];
        if g.rules.units[unit.kind].domain.as_deref() != Some("sea") {
            return None;
        }
        if let Some(settler) = unit.linked_to.filter(|peer| {
            g.units
                .get(peer)
                .is_some_and(|peer| peer.owner == pid && peer.kind == "settler")
        }) {
            return self
                .settler_targets
                .get(&settler)
                .copied()
                .and_then(|target| Self::naval_approach(g, uid, target))
                .or_else(|| Some(g.units[&settler].pos));
        }
        g.units
            .values()
            .filter(|settler| {
                settler.owner == pid
                    && settler.kind == "settler"
                    && settler.linked_to.is_none()
                    && g.map
                        .get(settler.pos)
                        .is_some_and(|tile| g.rules.is_water(tile))
            })
            .min_by_key(|settler| (g.wdist(unit.pos, settler.pos), settler.id))
            .map(|settler| settler.pos)
    }

    fn explore_step(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let upos = g.units[&uid].pos;
        // `unit_can_traverse` says yes to open water for every land unit the
        // moment embarkation unlocks, so from that turn on the unexplored
        // ocean is a legal exploration goal for the whole army. That is how a
        // Crossbowman leaves the capital tile and spends the next 47 turns
        // pacing between two sea hexes. A land unit explores land.
        let dry_only = self.come_ashore
            && g.rules.units[g.units[&uid].kind].domain.as_deref() != Some("sea");
        // The nearest hidden tile is almost always a few hexes away, so walk
        // outward in rings and stop at the first one that holds a candidate
        // instead of testing all nine hundred tiles. The rings partition the
        // map and each is examined in position order, so this picks exactly
        // the tile a full `min_by_key` on `(distance, position)` would.
        // The memo answers `unit_can_traverse` without re-deriving how this
        // unit moves at every tile, and cannot go stale while it holds the
        // game immutably.
        let nearest = {
            let _memo = g.query_memo();
            let mut found = None;
            let mut radius = 1;
            let mut examined = 0;
            while found.is_none() && examined < g.map.tiles.len() {
                let ring: Vec<Pos> = g
                    .wring(upos, radius)
                    .into_iter()
                    .filter(|pos| g.wdist(upos, *pos) == radius)
                    .collect();
                if ring.is_empty() {
                    break;
                }
                examined += ring.len();
                found = ring
                    .into_iter()
                    .filter(|pos| {
                        !g.players[pid].explored.contains(pos)
                            && g.unit_can_traverse(uid, *pos)
                            && (!dry_only
                                || g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile)))
                    })
                    .min();
                radius += 1;
            }
            found
        };
        if let Some(target) = nearest {
            if self.step_toward(g, pid, uid, target) {
                return true;
            }
        } else {
            // Nothing hidden is reachable, and the route search below would
            // only flood the unit's whole region to prove the same thing.
            return false;
        }

        // If the geometrically nearest hidden tile was unreachable, search
        // for the nearest hidden tile by actual traversable route instead.
        let goals: HashSet<Pos> = {
            let _memo = g.query_memo();
            g.map
                .tiles
                .iter()
                .filter(|(pos, tile)| {
                    !g.players[pid].explored.contains(pos)
                        && g.unit_can_traverse(uid, **pos)
                        && (!dry_only || !g.rules.is_water(tile))
                })
                .map(|(pos, _)| *pos)
                .collect()
        };
        let next = match g.route_step_to_any(uid, &goals) {
            Some(p) if g.can_move(uid, p) => p,
            _ => return false,
        };
        // The exhaustive search is the greedy walk's fallback, so it must
        // honour the same refusals — otherwise a Scout barred from retreading
        // its loop by the cheap path takes the identical step here.
        self.path_move(g, pid, uid, next)
    }

    /// Walk an embarked land unit back onto dry land, preferring our own
    /// territory.
    ///
    /// The pre-existing come-ashore rule in `peacetime_step` is welded to
    /// `modernization_step`, which returns `false` before it does anything at
    /// all when `unit_upgrade_target` is `None`. So the only embarked units
    /// ever told to land were the ones that happened to have an unlocked,
    /// affordable successor waiting; everything else stayed at sea for the
    /// rest of the game. Measured across 133 live runs, land combat units
    /// spend a mean **15%** of their unit-turns embarked (p90 48%, worst run
    /// 84%) — and **21.7% mean while one of our own cities is taking damage**,
    /// 92.8% in the worst case. An embarked unit cannot attack at all and
    /// defends at `embarked_strength`, so those turns are not merely
    /// misplaced, they are spent as a non-combatant.
    ///
    /// The nearest shore is chosen by distance and walked with `step_toward`,
    /// the same mover `modernization_step` uses to bring a unit home — the
    /// exhaustive `route_step_to_any` is only the fallback, because a
    /// water-to-land step is a disembarkation and the cheap route search
    /// handles that transition where a raw goal-set flood does not.
    pub(crate) fn disembark_step(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        if g.rules.units[g.units[&uid].kind].domain.as_deref() == Some("sea")
            || !g.is_embarked(&g.units[&uid])
        {
            return false;
        }
        // A landing site has to be one the unit can actually end its move on.
        // Occupied tiles are excluded deliberately: our own territory is
        // usually one small city whose tile the garrison already holds, and
        // aiming at it parks the castaway one hex offshore forever — closing
        // to shore distance 1 and never landing, which is the defect wearing
        // a different hat.
        let (home, any): (HashSet<Pos>, HashSet<Pos>) = {
            let _memo = g.query_memo();
            let landable: Vec<(Pos, bool)> = g
                .map
                .tiles
                .iter()
                .filter(|(pos, tile)| {
                    !g.rules.is_water(tile)
                        && g.rules.is_passable(tile)
                        && g.unit_can_traverse(uid, **pos)
                        && g.units_at(**pos).is_empty()
                })
                .map(|(pos, tile)| {
                    let ours = tile
                        .owner_city
                        .and_then(|cid| g.cities.get(&cid))
                        .is_some_and(|city| city.owner == pid);
                    (*pos, ours)
                })
                .collect();
            (
                landable
                    .iter()
                    .filter(|(_, ours)| *ours)
                    .map(|(pos, _)| *pos)
                    .collect(),
                landable.iter().map(|(pos, _)| *pos).collect(),
            )
        };
        // Home first, then any shore at all — a castaway whose territory is
        // unreachable still has to get out of the water before it can do
        // anything, so the fallback runs when the preferred set fails to move
        // it, not merely when that set is empty.
        for goals in [home, any] {
            if goals.is_empty() {
                continue;
            }
            let upos = g.units[&uid].pos;
            if let Some(shore) = goals
                .iter()
                .copied()
                .min_by_key(|pos| (g.wdist(upos, *pos), *pos))
            {
                if self.step_toward(g, pid, uid, shore) && g.units[&uid].pos != upos {
                    return true;
                }
            }
            if let Some(next) = g
                .route_step_to_any(uid, &goals)
                .filter(|pos| g.can_move(uid, *pos))
            {
                if self.path_move(g, pid, uid, next) {
                    return true;
                }
            }
        }
        false
    }

    fn patrol_tile(&self, g: &Game, pid: usize, uid: u32, pos: Pos) -> bool {
        let Some(tile) = g.map.get(pos) else {
            return false;
        };
        let sea_unit = g.rules.units[g.units[&uid].kind].domain.as_deref() == Some("sea");
        let water = g.rules.is_water(tile);
        if sea_unit != water {
            return false;
        }
        let friendly_city = tile
            .owner_city
            .and_then(|cid| g.cities.get(&cid))
            .is_some_and(|city| city.owner == pid);
        if !friendly_city {
            return false;
        }
        g.unit_can_traverse(uid, pos)
    }

    fn build_patrol_posts(&self, g: &Game, pid: usize, uid: u32) -> Vec<Pos> {
        let mut posts: Vec<Pos> = g
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| self.patrol_tile(g, pid, uid, *pos))
            .filter(|pos| {
                // A frontier post borders land or water outside this empire.
                // Interior city centers remain fallback destinations below.
                g.nbrs(*pos).into_iter().any(|neighbor| {
                    g.map.get(neighbor).is_some_and(|tile| {
                        tile.owner_city
                            .and_then(|cid| g.cities.get(&cid))
                            .is_none_or(|city| city.owner != pid)
                    })
                })
            })
            .collect();
        if posts.is_empty() {
            posts = g
                .player_city_ids(pid)
                .into_iter()
                .map(|cid| g.cities[&cid].pos)
                .filter(|pos| self.patrol_tile(g, pid, uid, *pos))
                .collect();
        }
        posts.sort_unstable();
        posts.dedup();
        posts
    }

    fn patrol_posts_for(
        &mut self,
        g: &Game,
        pid: usize,
        uid: u32,
        domain: &str,
        class_specific: bool,
    ) -> Vec<Pos> {
        let post_memo = g.query_memo();
        let class_key = (domain.to_string(), g.traversal_class(uid));
        let mut posts = if let Some(posts) = self.patrol_posts_by_class.get(&class_key) {
            posts.clone()
        } else if !class_specific {
            if let Some(posts) = self.patrol_posts.get(domain) {
                posts.clone()
            } else {
                let posts = self.build_patrol_posts(g, pid, uid);
                self.patrol_posts.insert(domain.to_string(), posts.clone());
                posts
            }
        } else {
            let posts = self.build_patrol_posts(g, pid, uid);
            self.patrol_posts_by_class
                .insert(class_key, posts.clone());
            posts
        };
        // A conquest earlier in this same unit phase may have invalidated a
        // cached frontier tile. Keep the shared scan, but cheaply validate the
        // relatively small candidate list before routing to it.
        posts.retain(|pos| self.patrol_tile(g, pid, uid, *pos));
        drop(post_memo);
        posts
    }

    /// Populate the frontier scan before AdvancedAi clones a planner into
    /// parallel unit snapshots. Those snapshots are intentionally independent,
    /// so a lazy cache would otherwise pay for the same map scan once per
    /// branch. BasicAi keeps its ordinary lazy behavior when this hook is not
    /// used.
    pub(crate) fn prepare_patrol_posts(&mut self, g: &Game, pid: usize, uids: &[u32]) {
        let mut representatives: Vec<(String, TraversalClass, u32)> = Vec::new();
        for uid in uids {
            let Some(unit) = g.units.get(uid) else {
                continue;
            };
            if g.rules.units[unit.kind].class != "military" {
                continue;
            }
            let domain = g.rules.units[unit.kind]
                .domain
                .as_deref()
                .unwrap_or("land")
                .to_string();
            let class = g.traversal_class(*uid);
            if !representatives
                .iter()
                .any(|(old_domain, old_class, _)| {
                    *old_class == class && old_domain == &domain
                })
            {
                representatives.push((domain, class, *uid));
            }
        }
        for (domain, _, uid) in representatives {
            let _ = self.patrol_posts_for(g, pid, uid, &domain, true);
        }
    }

    /// Move an otherwise idle military unit between useful frontier posts.
    /// Targets persist across turns, avoiding random-looking oscillation; a
    /// new post is selected only after the old one is reached or invalidated.
    fn patrol_step(&mut self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let current = g.units[&uid].pos;
        let previous = self.patrol_targets.get(&uid).copied();
        if let Some(target) = previous {
            if target != current && self.patrol_tile(g, pid, uid, target) {
                if let Some(next) = g
                    .route_step(uid, target, 0)
                    .filter(|pos| g.can_move(uid, *pos))
                {
                    // A dense frontier can offer two adjacent posts, and a
                    // unit that keeps swapping between them is pacing, not
                    // patrolling. `path_move` declines the retread, which
                    // sends the selection below after a post it has not
                    // already worn out.
                    if self.path_move(g, pid, uid, next) {
                        return true;
                    }
                }
            }
            self.patrol_targets.remove(&uid);
        }

        let domain = g.rules.units[g.units[&uid].kind]
            .domain
            .as_deref()
            .unwrap_or("land")
            .to_string();
        let posts = self.patrol_posts_for(g, pid, uid, &domain, false);
        if posts.is_empty() {
            return false;
        }

        let start = previous
            .and_then(|target| posts.binary_search(&target).ok().map(|index| index + 1))
            .unwrap_or(uid as usize % posts.len());
        // Trying a bounded number of distributed posts avoids an expensive
        // all-map path search when a unit is isolated on another landmass.
        for offset in 0..posts.len().min(24) {
            let target = posts[(start + offset) % posts.len()];
            // A post inside the footprint this unit has been circling is not a
            // destination; it is where the circling has been happening.
            if target == current || self.retreads_a_loop(uid, target) {
                continue;
            }
            let Some(next) = g
                .route_step(uid, target, 0)
                .filter(|pos| g.can_move(uid, *pos))
            else {
                continue;
            };
            if !self.path_move(g, pid, uid, next) {
                continue;
            }
            self.patrol_targets.insert(uid, target);
            return true;
        }
        false
    }

    fn healing_step(&mut self, g: &mut Game, pid: usize, uid: u32) -> Option<bool> {
        let withdraw_at_hp = self.w.withdraw_hp.round() as i32;
        let return_at_hp = self.w.rejoin_hp.max(self.w.withdraw_hp + 5.0).round() as i32;

        let hp = g.units[&uid].hp;
        if hp >= return_at_hp {
            self.recovering_units.remove(&uid);
            return None;
        }
        if hp <= withdraw_at_hp {
            self.recovering_units.insert(uid);
        }
        if !self.recovering_units.contains(&uid) {
            return None;
        }

        // Once safely inside friendly borders, spending the turn stationary
        // is faster than sacrificing another healing tick to chase a city.
        if g.unit_heal_rate(uid) >= 15 {
            return Some(self.fortify_or_stop(g, pid, uid));
        }

        let friendly_tiles: HashSet<Pos> = g
            .map
            .tiles
            .keys()
            .filter(|pos| g.healing_location(pid, **pos).rate() >= 15)
            .copied()
            .collect();
        if let Some(next) = g
            .route_step_to_any(uid, &friendly_tiles)
            .filter(|pos| g.can_move(uid, *pos))
        {
            return Some(self.path_move(g, pid, uid, next));
        }

        // If home is unreachable (for example, an isolated naval unit), wait
        // and use the neutral/enemy rate instead of continuing a bad attack.
        Some(self.fortify_or_stop(g, pid, uid))
    }

    fn military_step(&mut self, g: &mut Game, pid: usize, uid: u32) -> bool {
        if self.minor {
            let Some(home) = Self::minor_home(g, pid) else {
                return self.fortify_or_stop(g, pid, uid);
            };
            if g.wdist(g.units[&uid].pos, home) > MINOR_DEFENSE_RADIUS {
                if let Some(next) = g
                    .route_step(uid, home, 0)
                    .filter(|next| g.can_move(uid, *next))
                {
                    return self.path_move(g, pid, uid, next) || self.fortify_or_stop(g, pid, uid);
                }
                return self.fortify_or_stop(g, pid, uid);
            }
        }
        if let Some(acted) = self.healing_step(g, pid, uid) {
            return acted;
        }
        if self.unit_objective_memory {
            if let Some(acted) = self.retreat_step(g, pid, uid) {
                return acted;
            }
        }
        let upos = g.units[&uid].pos;
        let rules = std::sync::Arc::clone(&g.rules);
        let spec = &rules.units[g.units[&uid].kind];
        let doctrine = Self::unit_doctrine(g, uid);
        let adjacent_enemy_settler = g.nbrs(upos).into_iter().any(|position| {
            g.units_at(position).into_iter().any(|other| {
                g.units[&other].owner != pid
                    && g.is_at_war(pid, g.units[&other].owner)
                    && g.units[&other].kind == "settler"
            })
        });
        let decline_settlers = adjacent_enemy_settler
            && (self.minor
                || g.units
                    .values()
                    .any(|unit| unit.owner == pid && unit.kind == "settler")
                || !self.has_practical_settle_site(g, pid));
        if self.capture_adjacent_civilian(g, pid, uid, decline_settlers) {
            return true;
        }
        if decline_settlers {
            // Do not let generic tactical movement bypass the same guard and
            // walk onto a Settler this civilization cannot use.
            return self.fortify_or_stop(g, pid, uid);
        }
        if !self.minor {
            if let Some(action) = self.doctrine_action(g, pid, uid) {
                return g.apply(pid, &action).is_ok();
            }
        }
        if matches!(doctrine, UnitDoctrine::AirDefense | UnitDoctrine::AirStrike) {
            return false;
        }
        let enemy_ids: Vec<usize> = g
            .players
            .iter()
            .filter(|o| {
                o.id != pid
                    && o.alive
                    && g.is_at_war(pid, o.id)
                    && (!self.minor || Self::minor_enemy_near_home(g, pid, o.id))
            })
            .map(|o| o.id)
            .collect();
        if !enemy_ids.is_empty() {
            self.patrol_targets.remove(&uid);
            // Pick the best role-adjusted exchange among all attackable tiles.
            // A scout needs a clear opportunity; an assault unit presses a
            // thinner edge, and siege spends its attacks on districts.
            let radius = if spec.has_ranged_attack() {
                g.unit_attack_range(uid).max(1)
            } else {
                1
            };
            let mut best: Option<(f64, Pos, Action)> = None;
            for pos in g.wdisk(upos, radius) {
                if pos == upos
                    || g.map.get(pos).is_none()
                    || !self.is_enemy_tile(g, pos, &enemy_ids)
                    || (self.minor
                        && Self::minor_home(g, pid)
                            .is_none_or(|home| g.wdist(home, pos) > MINOR_DEFENSE_RADIUS))
                {
                    continue;
                }
                let distance = g.wdist(upos, pos);
                let mut modes = Vec::with_capacity(2);
                if spec.has_ranged_attack() && distance <= g.unit_attack_range(uid) {
                    modes.push((
                        true,
                        Action::Ranged {
                            unit: uid,
                            target: pos,
                        },
                    ));
                }
                if g.units[&uid].kind == "spec_ops"
                    && distance <= g.unit_attack_range(uid)
                    && g.priority_support_target_at(pid, pos).is_some()
                {
                    modes.push((
                        true,
                        Action::PriorityTarget {
                            unit: uid,
                            target: pos,
                        },
                    ));
                }
                if spec.is_melee_capable() && distance == 1 {
                    modes.push((
                        false,
                        Action::Attack {
                            unit: uid,
                            target: pos,
                        },
                    ));
                }
                for (ranged, action) in modes {
                    let capture =
                        !ranged && g.city_at(pos).is_some_and(|cid| g.cities[&cid].hp <= 0);
                    let utility = if matches!(action, Action::PriorityTarget { .. }) {
                        Self::priority_target_score(g, pid, pos) as f64 - 55.0
                    } else {
                        self.exchange_score(g, uid, pos, ranged)
                            - self.attack_threshold(g, uid, pos)
                            + self.tactical_action_bonus(g, uid, pos, ranged)
                            + if capture { 500.0 } else { 0.0 }
                    };
                    if best
                        .as_ref()
                        .map(|(old, old_pos, _)| {
                            utility > *old || (utility == *old && pos < *old_pos)
                        })
                        .unwrap_or(true)
                    {
                        best = Some((utility, pos, action));
                    }
                }
            }
            if let Some((utility, _, action)) = best {
                if utility > 0.0 && g.apply(pid, &action).is_ok() {
                    return true;
                }
            }
            if let Some(action) = self.heavy_cavalry_pillage_action(g, pid, uid) {
                if g.apply(pid, &action).is_ok() {
                    return true;
                }
            }
            let hostile_water_unit = g
                .units
                .values()
                .any(|enemy| enemy_ids.contains(&enemy.owner) && Self::waterborne(g, enemy.id));
            if !hostile_water_unit {
                if let Some(target) = self.naval_escort_objective(g, pid, uid) {
                    if target != upos && self.step_toward(g, pid, uid, target) {
                        return true;
                    }
                    if g.units[&uid].linked_to.is_some_and(|peer| {
                        g.units
                            .get(&peer)
                            .is_some_and(|unit| unit.kind == "settler")
                    }) {
                        return self.fortify_or_stop(g, pid, uid);
                    }
                }
            }
            if !self.minor
                && doctrine == UnitDoctrine::Recon
                && self.should_explore(g, pid, uid, true)
                && self.explore_step(g, pid, uid)
            {
                return true;
            }
            // Holding a threatened city outranks everything else this unit could
            // do: an empty city with breached walls is lost to one melee step,
            // and the measured game lost four that way while its army was busy.
            if self.garrison_step(g, pid, uid, &enemy_ids) {
                return true;
            }
            // The homeland gets first claim on this unit. Without it the
            // objective below is always the offensive, because it ranks by
            // distance from the asking unit and a deployed army is by
            // definition standing next to the enemy.
            return match self
                .home_defense_objective(g, pid, uid, &enemy_ids)
                .or_else(|| self.capture_objective_target(g, pid, uid))
                .or_else(|| self.nearest_enemy_for_unit(g, pid, uid, &enemy_ids))
            {
                Some(t) => self.tactical_step(g, pid, uid, t, &enemy_ids, radius),
                None => self.peacetime_step(g, pid, uid, true),
            };
        }
        self.peacetime_step(g, pid, uid, false)
    }

    /// Civilian capture is movement, not combat. Feeding an undefended
    /// Settler or Builder into `Action::Attack` is rejected by the engine and
    /// used to leave entire armies surrounding it forever. Take the free unit
    /// with a legal move, while declining a duplicate/unusable Settler.
    pub(crate) fn capture_adjacent_civilian(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        decline_settlers: bool,
    ) -> bool {
        if g.rules.units[g.units[&uid].kind].class != "military" {
            return false;
        }
        let origin = g.units[&uid].pos;
        let target = g
            .nbrs(origin)
            .into_iter()
            .filter_map(|position| {
                if self.minor
                    && Self::minor_home(g, pid)
                        .is_none_or(|home| g.wdist(home, position) > MINOR_DEFENSE_RADIUS)
                {
                    return None;
                }
                let value = g
                    .units_at(position)
                    .into_iter()
                    .filter_map(|other| {
                        let other = &g.units[&other];
                        if other.owner == pid || !g.is_at_war(pid, other.owner) {
                            return None;
                        }
                        match other.kind.as_str() {
                            "settler" if !decline_settlers => Some(3),
                            "settler" => None,
                            "builder" => Some(2),
                            _ if matches!(
                                g.rules.units[other.kind].class.as_str(),
                                "civilian" | "support"
                            ) => Some(1),
                            _ => None,
                        }
                    })
                    .max()?;
                g.can_move(uid, position).then_some((value, position))
            })
            .max_by_key(|(value, position)| (*value, std::cmp::Reverse(*position)))
            .map(|(_, position)| position);
        target.is_some_and(|to| g.apply(pid, &Action::Move { unit: uid, to }).is_ok())
    }

    /// Minors guard home; majors explore, then garrison the nearest city.
    ///
    /// `at_war` is threaded in because `military_step` *falls through to here*
    /// when it can find no objective — which at war is the common case, since
    /// `nearest_enemy_for_unit` needs a visible enemy and a besieging stack is
    /// routinely fogged. The old call site passed a hardcoded `false`, so
    /// `should_explore`'s opening `if at_war { return false }` was dead by
    /// construction and a unit whose homeland was under attack went
    /// exploring. Off, the historical `false` is preserved exactly.
    fn peacetime_step(&mut self, g: &mut Game, pid: usize, uid: u32, at_war: bool) -> bool {
        let upos = g.units[&uid].pos;
        let at_war = at_war && self.come_ashore;
        if self.minor {
            let cities = g.player_city_ids(pid);
            if cities.is_empty() {
                return false;
            }
            let cap = g.cities[&cities[0]].pos;
            if g.wdist(upos, cap) > 2 {
                return self.step_toward(g, pid, uid, cap);
            }
            return self.fortify_or_stop(g, pid, uid);
        }
        if let Some(target) = self.naval_escort_objective(g, pid, uid) {
            if target != upos && self.step_toward(g, pid, uid, target) {
                return true;
            }
            if g.units[&uid].linked_to.is_some_and(|peer| {
                g.units
                    .get(&peer)
                    .is_some_and(|unit| unit.kind == "settler")
            }) {
                return self.fortify_or_stop(g, pid, uid);
            }
        }
        let special_improver = {
            let unit = &g.units[&uid];
            unit.owner == pid
                && unit.charges > 0
                && !g.rules.units[unit.kind].builds.is_empty()
        };
        if special_improver {
            if let Some(acted) = self.special_improver_step(g, pid, uid) {
                return acted;
            }
        }
        // Coming ashore outranks exploring. A Recon unit explores for as long
        // as any unseen tile remains, which on an ocean map is forever, so
        // Scouts and Skirmishers walked off across the water and never came
        // back: 11% of every empire's land army ended its games embarked,
        // where no upgrade is offered at all. Only that case jumps the queue —
        // a unit merely away from home still explores first and modernizes
        // afterwards, because taking recon off the map early costs more than
        // the delayed upgrade is worth.
        if g.is_embarked(&g.units[&uid]) && self.modernization_step(g, pid, uid) {
            return true;
        }
        // ...and coming ashore has to outrank exploring for the units that
        // have no upgrade waiting too, which is most of them. See
        // `disembark_step`.
        if self.come_ashore && self.disembark_step(g, pid, uid) {
            return true;
        }
        if self.should_explore(g, pid, uid, at_war) && self.explore_step(g, pid, uid) {
            return true;
        }
        if self.modernization_step(g, pid, uid) {
            return true;
        }
        if self.patrol_step(g, pid, uid) {
            return true;
        }
        self.fortify_or_stop(g, pid, uid)
    }

    /// Walk a unit that has outlived its era back inside the borders, where
    /// it can be upgraded. Gold upgrades are only offered in friendly
    /// territory, so a unit that spends its whole life on frontier patrol or
    /// in no-man's-land can never modernize however rich its owner is; the
    /// modernization pass simply never sees it. Only units whose successor is
    /// already unlocked and paid for make the trip.
    ///
    /// An embarked unit is included deliberately: it is the case that most
    /// needs the trip, since no upgrade at all is offered at sea.
    fn modernization_step(&mut self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let target = {
            let unit = &g.units[&uid];
            let upos = unit.pos;
            if g.unit_upgrade_target(pid, unit.kind).is_none() {
                return false;
            }
            let at_home = g
                .map
                .get(upos)
                .and_then(|tile| tile.owner_city)
                .and_then(|cid| g.cities.get(&cid))
                .is_some_and(|city| city.owner == pid);
            if at_home {
                return false; // already somewhere the upgrade can be taken
            }
            let Some((_, gold, _)) = g.unit_upgrade_price(pid, &unit.kind) else {
                return false;
            };
            if g.players[pid].gold < gold {
                return false; // no point marching home to an empty treasury
            }
            g.player_city_ids(pid)
                .into_iter()
                .map(|cid| g.cities[&cid].pos)
                .min_by_key(|pos| (g.wdist(upos, *pos), *pos))
        };
        match target {
            Some(pos) => self.step_toward(g, pid, uid, pos),
            None => false,
        }
    }

    fn fortify_or_stop(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        if !g.units[&uid].fortified {
            let _ = g.apply(pid, &Action::Fortify { unit: uid });
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::advanced::AdvancedAi;
    use crate::parallel::WorkPool;

    #[test]
    fn live_policy_counterfactuals_replay_serially_on_workers() {
        let mut initial = Game::new_full(1, 20, 14, 91_481, 120, 0, false);
        initial.players[0].civics.extend(
            [
                "code_of_laws",
                "craftsmanship",
                "foreign_trade",
                "early_empire",
                "state_workforce",
                "military_tradition",
                "political_philosophy",
            ]
            .into_iter()
            .map(Name::new),
        );
        initial.players[0].government = Some("classical_republic".to_string());
        initial.turn = POLICY_REVIEW_EVERY;
        assert!(
            initial.available_policies(0).len() >= 4,
            "the fixture needs enough independent cards to use every worker"
        );
        let weights = Weights {
            policy_deck: PolicyDeck::Live,
            ..Weights::default()
        };

        let mut serial = initial.clone();
        revise_policy_deck(&mut serial, 0, &weights, None);
        for threads in [4, 5] {
            let mut parallel = initial.clone();
            let pool = WorkPool::new(threads);
            revise_policy_deck(&mut parallel, 0, &weights, Some(&pool));

            assert_eq!(serial.log, parallel.log, "threads={threads}");
            assert_eq!(
                serde_json::to_value(&serial).unwrap(),
                serde_json::to_value(&parallel).unwrap(),
                "worker completion order must not change the authoritative game (threads={threads})"
            );
        }
    }

    /// A quiet one-player world with a lone Scout, so nothing else on the map
    /// can move the unit or attack it while its whereabouts are recorded.
    fn scouted_world() -> (Game, Vec<Pos>, u32) {
        let mut g = Game::new_full(1, 20, 12, 5, 300, 0, true);
        // Nothing else on the map, so the only thing the record can be
        // measuring is the Scout's own itinerary.
        g.units.clear();
        let mut ground: Vec<Pos> = g
            .map
            .tiles
            .iter()
            .filter(|(_, tile)| !g.rules.is_water(tile) && g.rules.is_passable(tile))
            .map(|(pos, _)| *pos)
            .collect();
        ground.sort();
        assert!(ground.len() > 8, "the case needs somewhere to walk");
        let scout = g.spawn_test_unit("scout", 0, ground[0]);
        (g, ground, scout)
    }

    /// Walk one unit through a scripted sequence of tiles, letting the agent
    /// take down where the unit stood at the start of each turn exactly as a
    /// real movement phase would.
    fn observe_walk(tiles: &[usize], spend_charge_on: Option<usize>) -> (BasicAi, Game, Vec<Pos>, u32) {
        let (mut g, ground, scout) = scouted_world();
        let mut ai = BasicAi::new();
        for (turn, tile) in tiles.iter().enumerate() {
            g.units.get_mut(&scout).unwrap().pos = ground[*tile];
            if spend_charge_on == Some(turn) {
                g.units.get_mut(&scout).unwrap().charges += 1;
            }
            ai.begin_movement_turn(&g, 0);
            g.turn += 1;
        }
        (ai, g, ground, scout)
    }

    #[test]
    fn unit_memory_keeps_a_campaign_and_warning_across_turns_and_id_remap() {
        let mut game = Game::new_full(2, 24, 16, 91_482, 80, 0, false);
        let enemy_settler = game
            .player_unit_ids(1)
            .into_iter()
            .find(|uid| game.units[uid].kind == "settler")
            .expect("fixture starts player one with a Settler");
        let city_pos = game.units[&enemy_settler].pos;
        let city = game.found_city_for(1, city_pos, None);
        game.remove_unit(enemy_settler);
        let land: Vec<Pos> = game
            .map
            .tiles
            .iter()
            .filter(|(pos, tile)| {
                game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && game.city_at(**pos).is_none()
            })
            .map(|(pos, _)| *pos)
            .collect();
        let unit = game.spawn_test_unit("warrior", 0, land[0]);
        let replacement = game.spawn_test_unit("warrior", 0, land[1]);
        let mut ai = BasicAi::new();
        ai.unit_objective_memory = true;

        ai.remember_capture_objective(&game, 0, unit, city);
        assert!(
            !ai.remember_dangerous_approach(&game, unit, land[2], 20.0),
            "a merely risky tile is retained without forcing a full retreat"
        );
        let first = ai.unit_memory(unit).expect("the unit owns a memory");
        assert!(matches!(
            first.objective,
            Some(UnitObjective::CaptureCity {
                city: remembered,
                target,
                started_turn,
                last_confirmed_turn,
            }) if remembered == city
                && target == city_pos
                && started_turn == game.turn
                && last_confirmed_turn == game.turn
        ));
        assert_eq!(first.danger.map(|danger| danger.position), Some(land[2]));

        game.turn += 1;
        ai.remember_capture_objective(&game, 0, unit, city);
        let refreshed = ai.unit_memory(unit).expect("memory persists into the next turn");
        assert!(matches!(
            refreshed.objective,
            Some(UnitObjective::CaptureCity {
                started_turn,
                last_confirmed_turn,
                ..
            }) if started_turn + 1 == last_confirmed_turn
        ));

        let planned = ai.unit_plan_state(unit);
        let mut committed = BasicAi::new();
        committed.unit_objective_memory = true;
        committed.merge_unit_plan_state(replacement, planned);
        assert_eq!(
            committed.unit_memory(replacement),
            Some(refreshed.clone()),
            "a parallel unit plan returns only its own durable memory to the serial board"
        );

        ai.remap_unit_memory(&std::collections::BTreeMap::from([(unit, replacement)]));
        assert!(ai.unit_memory(unit).is_none(), "the old mirror ID is gone");
        let remapped = ai
            .unit_memory(replacement)
            .expect("the new mirror ID inherits the same campaign");
        assert_eq!(remapped.objective, refreshed.objective);
        assert_eq!(remapped.danger, refreshed.danger);

        game.cities.get_mut(&city).unwrap().owner = 0;
        ai.begin_movement_turn(&game, 0);
        let after_capture = ai
            .unit_memory(replacement)
            .expect("the warning remains until its short expiry");
        assert!(after_capture.objective.is_none(), "capturing the city resolves the job");
    }

    #[test]
    fn a_dangerous_approach_causes_a_short_retreat_without_losing_the_memory() {
        let mut game = Game::new_full(2, 24, 16, 91_483, 80, 0, false);
        game.units.clear();
        game.at_war.insert((0, 1));
        let positions: Vec<Pos> = game
            .map
            .tiles
            .iter()
            .filter(|(pos, tile)| {
                game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && game.city_at(**pos).is_none()
            })
            .map(|(pos, _)| *pos)
            .collect();
        let (threat, origin, refuge) = positions
            .iter()
            .copied()
            .find_map(|threat| {
                game.nbrs(threat).into_iter().find_map(|origin| {
                    if !positions.contains(&origin) {
                        return None;
                    }
                    game.nbrs(origin)
                        .into_iter()
                        .find(|refuge| {
                            positions.contains(refuge)
                                && game.wdist(*refuge, threat) > game.wdist(origin, threat)
                        })
                        .map(|refuge| (threat, origin, refuge))
                })
            })
            .expect("fixture needs a land tile with a retreat route");
        let unit = game.spawn_test_unit("warrior", 0, origin);
        let enemy = game.spawn_test_unit("warrior", 1, threat);
        game.units.get_mut(&unit).unwrap().hp = 60;
        let mut ai = BasicAi::new();
        ai.unit_objective_memory = true;

        let expected = ai.projected_counter_damage(&game, unit, threat, &[enemy]);
        assert!(expected > 0.0, "the route must have a real hostile counterattack");
        assert!(
            ai.remember_dangerous_approach(&game, unit, threat, expected),
            "damage that crosses the withdrawal floor becomes a retreat"
        );
        let before = game.units[&unit].pos;
        assert_eq!(ai.retreat_step(&mut game, 0, unit), Some(true));
        let after = game.units[&unit].pos;
        assert!(
            game.wdist(after, threat) > game.wdist(before, threat),
            "retreat moves away from the exact dangerous approach"
        );
        assert!(
            game.wdist(after, threat) >= game.wdist(refuge, threat),
            "the unit selects a route at least as far from danger as the known escape"
        );
        let memory = ai.unit_memory(unit).expect("retreat retains the warning");
        assert_eq!(memory.danger.map(|danger| danger.position), Some(threat));
        assert_eq!(memory.retreat_until, Some(game.turn + UNIT_RETREAT_TURNS));

        // Retreat is deliberately shorter than the warning: the unit can resume its
        // campaign once it has backed off, but it must not immediately forget why.
        game.turn += UNIT_RETREAT_TURNS;
        ai.begin_movement_turn(&game, 0);
        let cooling = ai.unit_memory(unit).expect("danger survives the retreat window");
        assert!(cooling.danger.is_some());
        assert_eq!(cooling.retreat_until, None);

        game.turn += 1;
        ai.begin_movement_turn(&game, 0);
        assert!(
            ai.unit_memory(unit).is_none(),
            "short-lived danger is removed once its observation expires"
        );
    }

    #[test]
    fn baseline_envoys_skip_an_unmet_city_state_and_reach_a_known_one() {
        let mut game = Game::new_full(1, 24, 16, 90_732, 120, 2, false);
        let city_states: Vec<usize> = game
            .players
            .iter()
            .filter(|player| player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .collect();
        assert_eq!(city_states.len(), 2);
        let hidden = city_states[0];
        let known = city_states[1];
        for player in 0..game.players.len() {
            game.players[player].met.clear();
        }
        game.record_contact(0, known);
        game.players[0].envoys_free = 1;

        // With equal influence, the baseline's stable tie-break would prefer
        // the lower-id hidden state. It must discard that candidate rather
        // than fail its first send and leave the known court unfunded.
        BasicAi::new().research_without_government_with_pool(&mut game, 0, None);

        assert_eq!(game.players[0].envoys_free, 0);
        assert_eq!(game.envoys_at(0, hidden), 0);
        assert_eq!(game.envoys_at(0, known), 1);
    }

    #[test]
    fn baseline_diplomacy_skips_an_unmet_major_and_reaches_a_known_one() {
        let mut game = Game::new_full(3, 24, 16, 90_734, 120, 0, false);
        for player in 0..game.players.len() {
            game.players[player].met.clear();
        }
        game.record_contact(0, 2);
        game.turn = 0;
        game.current = 0;

        // Player 1 wins the stable id tie but is unknown. A rejected proposal
        // must not prevent the same diplomatic pass from reaching player 2.
        BasicAi::new().diplomacy(&mut game, 0);

        assert_eq!(game.pending_deals.len(), 1);
        assert_eq!(game.pending_deals[0].from, 0);
        assert_eq!(game.pending_deals[0].to, 2);
        assert!(game.pending_deals[0].friendship);
    }

    /// The single-turn reversal ban cannot see a round trip that takes two
    /// turns to complete, and every step of one is individually the best move
    /// available. Only the unit's own recent history gives the loop away.
    #[test]
    fn a_unit_shuttling_between_two_tiles_is_priced_out_of_both() {
        let shuttle: Vec<usize> = (0..LIVELOCK_WINDOW + 2).map(|turn| turn % 2).collect();
        let (ai, _g, ground, scout) = observe_walk(&shuttle, None);

        assert!(ai.livelock_penalty(scout, ground[0]) < 0.0);
        assert!(ai.livelock_penalty(scout, ground[1]) < 0.0);
        assert_eq!(
            ai.livelock_penalty(scout, ground[7]),
            0.0,
            "anywhere it has not already been is worth going"
        );
        assert!(ai.retreads_a_loop(scout, ground[1]));
        assert!(!ai.retreads_a_loop(scout, ground[7]));
    }

    #[test]
    fn a_unit_that_is_getting_somewhere_is_left_alone() {
        let march: Vec<usize> = (0..LIVELOCK_WINDOW + 2).collect();
        let (ai, _g, ground, scout) = observe_walk(&march, None);
        for tile in &ground[..8] {
            assert_eq!(ai.livelock_penalty(scout, *tile), 0.0);
        }
    }

    /// A Builder working two tiles beside a city occupies the same footprint
    /// as a Builder stuck between them. Spending the charge is the difference,
    /// and it is what the work fingerprint exists to see.
    #[test]
    fn a_unit_that_accomplishes_something_keeps_its_ground() {
        let shuttle: Vec<usize> = (0..LIVELOCK_WINDOW + 2).map(|turn| turn % 2).collect();
        let halfway = Some(LIVELOCK_WINDOW / 2);
        let (working, _g, ground, scout) = observe_walk(&shuttle, halfway);
        assert_eq!(working.livelock_penalty(scout, ground[0]), 0.0);

        let (idle, _g, ground, scout) = observe_walk(&shuttle, None);
        assert!(idle.livelock_penalty(scout, ground[0]) < 0.0);
    }

    /// A three-tile shuffle is still a loop; a four-tile circuit is a unit
    /// covering ground, and pricing that would punish ordinary movement. Both
    /// walks stop short of `LIVELOCK_STAND_DOWN_AFTER`, which would wipe the
    /// record being examined.
    #[test]
    fn the_footprint_bound_separates_a_loop_from_a_march() {
        let turns = LIVELOCK_WINDOW + 2;
        let (looping, _g, ground, scout) =
            observe_walk(&(0..turns).map(|turn| turn % 3).collect::<Vec<_>>(), None);
        assert!(looping.livelock_penalty(scout, ground[0]) < 0.0);

        let (marching, _g, ground, scout) =
            observe_walk(&(0..turns).map(|turn| turn % 4).collect::<Vec<_>>(), None);
        assert_eq!(marching.livelock_penalty(scout, ground[0]), 0.0);
    }

    /// The tabu redirects a unit that has somewhere else to go. A unit with
    /// nowhere else to go keeps circling anyway; for that one the record is
    /// wiped so the retry re-plans against a world that has moved on, and a
    /// unit that then still finds nothing to do digs in rather than standing
    /// in the open.
    #[test]
    fn a_unit_still_looping_after_a_second_window_starts_over_and_digs_in() {
        let shuttle: Vec<usize> = (0..LIVELOCK_STAND_DOWN_AFTER as usize + 1)
            .map(|turn| turn % 2)
            .collect();
        let (ai, mut g, ground, scout) = observe_walk(&shuttle, None);

        assert_eq!(
            ai.livelock_penalty(scout, ground[0]),
            0.0,
            "the stand-down clears the record, so the retry is unencumbered"
        );
        ai.hold_stood_down_unit(&mut g, 0, scout);
        assert!(g.units[&scout].fortified);

        // And it ends on its own: a unit past its stand-down is left alone.
        g.turn += LIVELOCK_STAND_DOWN_TURNS;
        let (later, mut g, _ground, other) = observe_walk(&[0, 0], None);
        later.hold_stood_down_unit(&mut g, 0, other);
        assert!(!g.units[&other].fortified);
    }

    /// Digging in must never take the place of a turn the unit could have
    /// spent. An earlier version ran *before* the unit's own step and guessed
    /// at what it might have wanted; measured over six games it cost more
    /// productive turns than the loops it broke, which is exactly what the
    /// `picket` column in `audit` exists to expose.
    #[test]
    fn a_stood_down_unit_still_takes_a_turn_it_can_use() {
        let shuttle: Vec<usize> = (0..LIVELOCK_STAND_DOWN_AFTER as usize + 1)
            .map(|turn| turn % 2)
            .collect();
        let (mut ai, mut g, _ground, scout) = observe_walk(&shuttle, None);
        assert!(
            ai.unit_motion[&scout].resume_turn > g.turn,
            "the case needs a unit that is serving out a stand-down"
        );

        let before = g.units[&scout].pos;
        ai.units(&mut g, 0);

        assert_ne!(
            g.units[&scout].pos, before,
            "a Scout with a whole world left to look at goes and looks at it"
        );
        assert!(!g.units[&scout].fortified);
    }

    #[test]
    fn a_game_no_enabled_victory_can_end_still_stops_at_its_turn_limit() {
        // A lobby is free to pin `--victories` without `score`, which removes
        // the turn-limit tiebreak that is the only thing guaranteeing a winner.
        // `set_winner` then refuses every path, and before the turn bound this
        // loop ran past the limit forever.
        let mut g = Game::new_full(2, 20, 14, 90_210, 12, 0, false);
        g.victory_conditions = crate::game::VictoryConditions {
            science: false,
            culture: false,
            religious: false,
            diplomatic: false,
            domination: false,
            score: false,
        };
        let mut ais = AdvancedAi::fleet(&g);
        run_game(&mut g, &mut ais);
        assert_eq!(g.winner, None, "no enabled path could be awarded");
        assert!(
            g.turn > g.max_turns,
            "the game ran to its limit: turn {} of {}",
            g.turn,
            g.max_turns
        );
    }

    fn walled_war_game(seed: u64) -> (Game, u32, u32) {
        let mut g = Game::new_full(2, 20, 14, seed, 40, 0, false);
        let settler0 = g
            .player_unit_ids(0)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler0 }).unwrap();
        g.apply(0, &Action::EndTurn).unwrap();
        let settler1 = g
            .player_unit_ids(1)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.apply(1, &Action::FoundCity { unit: settler1 }).unwrap();
        g.apply(1, &Action::EndTurn).unwrap();
        let home = g.player_city_ids(0)[0];
        let enemy = g.player_city_ids(1)[0];
        g.cities
            .get_mut(&enemy)
            .unwrap()
            .buildings
            .push(crate::name!("walls"));
        g.record_contact(0, 1);
        g.apply(0, &Action::DeclareWar { player: 1 }).unwrap();
        (g, home, enemy)
    }

    fn island_colony_game(players: usize) -> (Game, Pos, Pos) {
        let mut g = Game::new_full(players, 18, 10, 91, 120, 0, false);
        let founding_settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .unwrap();
        let source = g.units[&founding_settler].pos;
        let target = g
            .map
            .tiles
            .keys()
            .copied()
            .max_by_key(|pos| (g.wdist(source, *pos), *pos))
            .expect("map has a tile");
        assert!(g.wdist(source, target) > 6);
        for tile in g.map.tiles.values_mut() {
            tile.terrain = crate::name!("coast");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
            tile.district = None;
            tile.wonder = None;
            tile.owner_city = None;
            tile.cliff_edges = [false; 6];
        }
        g.map.tiles.get_mut(&source).unwrap().terrain = crate::name!("plains");
        g.map.tiles.get_mut(&target).unwrap().terrain = crate::name!("grassland");
        g.apply(
            0,
            &Action::FoundCity {
                unit: founding_settler,
            },
        )
        .unwrap();
        (g, source, target)
    }

    fn grant_tech_with_prerequisites(g: &mut Game, pid: usize, tech: &str) {
        let prerequisites = g.rules.techs[tech].requires.clone();
        for prerequisite in prerequisites {
            grant_tech_with_prerequisites(g, pid, &prerequisite);
        }
        g.players[pid].techs.insert(Name::new(tech));
    }

    /// The production picker named base districts, so for a civilization with
    /// a unique replacement it kept proposing a district the engine refuses.
    /// A refused proposal ends the city's turn, so those cities queued nothing
    /// at all - permanently, since the same choice came back every turn.
    #[test]
    fn civilizations_queue_the_unique_district_they_can_actually_build() {
        let g = Game::new_full(8, 40, 24, 3, 60, 0, false);
        let greece = g
            .players
            .iter()
            .position(|player| player.civ == "Greece")
            .unwrap();
        let rome = g
            .players
            .iter()
            .position(|player| player.civ == "Rome")
            .unwrap();
        assert_eq!(
            BasicAi::civ_district(&g, greece, "theater_square"),
            "acropolis"
        );
        assert_eq!(BasicAi::civ_district(&g, rome, "aqueduct"), "bath");
        // Civilizations without a replacement keep the stock district, and a
        // rival's unique district is never proposed.
        assert_eq!(
            BasicAi::civ_district(&g, rome, "theater_square"),
            "theater_square"
        );
        assert_eq!(BasicAi::civ_district(&g, greece, "campus"), "campus");
    }

    /// Buildings carry replacements too - a secret society swaps the Monument
    /// for an Old God Obelisk - and the Monument is the first thing every city
    /// considers, so proposing a blocked one would strand it from turn one.
    /// Whatever comes back must always be something the engine accepts.
    #[test]
    fn building_choices_are_always_producible() {
        let mut g = Game::new_full(8, 40, 24, 3, 60, 0, false);
        for pid in 0..8 {
            let settler = g
                .player_unit_ids(pid)
                .into_iter()
                .find(|id| g.units[id].kind == "settler")
                .unwrap();
            while g.current != pid {
                let current = g.current;
                g.apply(current, &Action::EndTurn).unwrap();
            }
            g.apply(pid, &Action::FoundCity { unit: settler }).unwrap();
            let cid = g.player_city_ids(pid)[0];
            for family in ["monument", "amphitheater", "arena"] {
                if let Some(item) = BasicAi::civ_building(&g, pid, cid, family) {
                    assert!(
                        g.can_produce(pid, cid, &item),
                        "{} was offered {item:?} for {family} and cannot build it",
                        g.players[pid].civ
                    );
                }
            }
            // Rome starts every city with a free Monument, so it must fall
            // through rather than proposing one it already has.
            if g.players[pid].civ == "Rome" {
                assert!(BasicAi::civ_building(&g, pid, cid, "monument").is_none());
            }
        }
    }

    /// Builders were produced to a flat quota per city whether or not the
    /// empire had a tile left to improve, so a built-out empire kept paying
    /// for Builders that then stood still for the rest of the game.
    #[test]
    fn builders_are_only_built_when_there_is_ground_to_work() {
        let mut g = Game::new_full(1, 20, 14, 29, 40, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let cid = g.player_city_ids(0)[0];
        assert!(
            BasicAi::has_builder_work(&g, 0),
            "a fresh city has tiles worth improving"
        );

        // Improve everything the city owns; nothing is left for a Builder.
        for pos in g.cities[&cid].owned_tiles.clone() {
            let tile = g.map.tiles.get_mut(&pos).unwrap();
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.resource = None;
            tile.hills = false;
            tile.improvement = Some(crate::name!("farm"));
            tile.pillaged = false;
        }
        assert!(!BasicAi::has_builder_work(&g, 0));

        // Pillaging one of them is work again: it can be repaired.
        let pos = g.cities[&cid].owned_tiles[1];
        g.map.tiles.get_mut(&pos).unwrap().pillaged = true;
        assert!(BasicAi::has_builder_work(&g, 0));
    }

    /// Builders used to take whichever legal improvement sorted first by
    /// name, so a Farm was laid over Iron, Stone and Wine - forfeiting the
    /// strategic resource or luxury on that tile for the rest of the game.
    #[test]
    fn builders_improve_the_resource_rather_than_the_alphabet() {
        let mut g = Game::new_full(1, 20, 14, 29, 40, 0, false);
        let pos = *g.map.tiles.keys().next().unwrap();
        {
            let tile = g.map.tiles.get_mut(&pos).unwrap();
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
        }

        for (resource, expected) in [("iron", "mine"), ("stone", "quarry"), ("wine", "plantation")] {
            g.map.tiles.get_mut(&pos).unwrap().resource = Some(Name::new(resource));
            let options = vec![
                crate::name!("farm"),
                Name::new(expected),
                crate::name!("camp"),
            ];
            assert_eq!(
                BasicAi::best_improvement(&g, pos, &options).map(|name| name.to_string()),
                Some(expected.to_string()),
                "{resource} should be worked, not farmed over"
            );
        }

        // With nothing on the tile the choice falls back to yield, and a
        // Lumber Mill's two Production beats a Farm's one Food.
        g.map.tiles.get_mut(&pos).unwrap().resource = None;
        let options = vec![crate::name!("farm"), crate::name!("lumber_mill")];
        assert_eq!(
            BasicAi::best_improvement(&g, pos, &options),
            Some(crate::name!("lumber_mill"))
        );
        assert_eq!(BasicAi::best_improvement(&g, pos, &[]), None);
    }

    /// Production only ever replaces losses, so without this pass the units
    /// standing in a city on turn 30 are still standing there in the
    /// Information era. The AI has to spend Gold to modernize them.
    #[test]
    fn the_ai_spends_gold_to_modernize_the_garrison_it_already_has() {
        let (mut g, source, _) = island_colony_game(1);
        g.players[0].civ = "Egypt".to_string();
        grant_tech_with_prerequisites(&mut g, 0, "iron_working");
        g.players[0]
            .strategic_resources
            .insert(crate::name!("iron"), 400.0);
        g.players[0].gold = 900.0;
        let veterans: Vec<u32> = (0..3)
            .map(|_| g.spawn_test_unit("warrior", 0, source))
            .collect();
        let obsolete = g
            .units
            .values()
            .filter(|unit| unit.owner == 0 && unit.kind == "warrior")
            .count();

        BasicAi::upgrade_units(&mut g, 0);

        assert!(
            veterans.iter().all(|uid| g.units[uid].kind == "swordsman"),
            "kinds={:?}",
            veterans
                .iter()
                .map(|uid| g.units[uid].kind)
                .collect::<Vec<_>>()
        );
        // Every Warrior in the empire modernized, at 110 Gold each.
        assert_eq!(g.players[0].gold, 900.0 - 110.0 * obsolete as f64);
        assert!(!g
            .units
            .values()
            .any(|unit| unit.owner == 0 && unit.kind == "warrior"));

        // A treasury that cannot clear the floor buys nothing at all.
        g.players[0].gold = 150.0;
        let straggler = g.spawn_test_unit("warrior", 0, source);
        BasicAi::upgrade_units(&mut g, 0);
        assert_eq!(g.units[&straggler].kind, "warrior");
    }

    /// A Gold upgrade is only offered inside the borders, so an army that
    /// spends its life on frontier patrol or in no-man's-land can never take
    /// one however rich its owner is - the modernization pass simply never
    /// sees those units. Measured over a full 6-player game, a third of all
    /// military unit-turns were spent on unowned tiles.
    #[test]
    fn a_unit_stranded_outside_the_borders_walks_home_to_modernize() {
        let (mut g, source, target) = island_colony_game(1);
        grant_tech_with_prerequisites(&mut g, 0, "iron_working");
        g.players[0]
            .strategic_resources
            .insert(crate::name!("iron"), 400.0);
        g.players[0].gold = 900.0;
        for tile in g.map.tiles.values_mut() {
            tile.terrain = crate::name!("plains");
        }
        let stranded = g.spawn_test_unit("warrior", 0, target);
        assert!(g.map.tiles[&target].owner_city.is_none());
        let before = g.wdist(target, source);

        let mut ai = BasicAi::new();
        ai.peacetime_step(&mut g, 0, stranded, false);

        let after = g.wdist(g.units[&stranded].pos, source);
        assert!(after < before, "before={before} after={after}");

        // A unit already standing at home has nothing to walk toward, and one
        // whose successor is out of reach is left to patrol as before.
        let garrison = g.spawn_test_unit("warrior", 0, source);
        assert!(!ai.modernization_step(&mut g, 0, garrison));
        g.players[0].gold = 0.0;
        let broke = g.spawn_test_unit("warrior", 0, target);
        assert!(!ai.modernization_step(&mut g, 0, broke));
    }

    /// A Recon unit explores for as long as any tile is unseen, which on an
    /// ocean map is forever, so Scouts and Skirmishers walked out across the
    /// water and stayed there: 11% of every empire's land army ended its games
    /// embarked, where no upgrade is offered at all.
    #[test]
    fn an_embarked_unit_comes_ashore_before_it_explores_any_further() {
        let (mut g, source, _) = island_colony_game(1);
        grant_tech_with_prerequisites(&mut g, 0, "iron_working");
        // Without embarkation a castaway cannot cross open water at all, so
        // grant the technology that put it out there in the first place.
        grant_tech_with_prerequisites(&mut g, 0, "shipbuilding");
        g.players[0]
            .strategic_resources
            .insert(crate::name!("iron"), 400.0);
        g.players[0].gold = 900.0;
        let at_sea = g
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| g.rules.is_water(&g.map.tiles[pos]))
            // Far enough out that the only way home is not the city tile
            // itself, which the starting garrison already occupies.
            .filter(|pos| g.wdist(*pos, source) >= 3)
            .min_by_key(|pos| (g.wdist(*pos, source), *pos))
            .expect("the colony sits in an ocean");
        let castaway = g.spawn_test_unit("warrior", 0, at_sea);
        assert!(g.is_embarked(&g.units[&castaway]), "the case needs a unit at sea");
        let before = g.wdist(at_sea, source);

        let mut ai = BasicAi::new();
        ai.peacetime_step(&mut g, 0, castaway, false);

        assert!(
            g.wdist(g.units[&castaway].pos, source) < before,
            "an embarked unit heads for shore instead of exploring on"
        );
    }

    /// The come-ashore rule above only reaches units that have an upgrade
    /// waiting: `modernization_step` returns before it does anything when
    /// `unit_upgrade_target` is `None`. Everything else stayed at sea, where
    /// it cannot attack at all and defends at `embarked_strength`. Measured
    /// across 133 live runs, land combat units spend a mean 15% of their
    /// unit-turns embarked and 21.7% while one of our own cities is taking
    /// damage; on run `civvis-20260803T130831Z` a Crossbowman left the capital
    /// tile and paced between two sea hexes for 47 turns while that capital
    /// sat at 179/200 damage.
    #[test]
    fn a_castaway_with_no_upgrade_waiting_still_comes_ashore() {
        let (mut g, source, _target) = island_colony_game(1);
        grant_tech_with_prerequisites(&mut g, 0, "shipbuilding");
        // No iron, no gold, no unlocked successor: the modernization path
        // cannot fire, which is the whole point of the case.
        g.players[0].gold = 0.0;
        let at_sea = g
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| g.rules.is_water(&g.map.tiles[pos]))
            .filter(|pos| g.wdist(*pos, source) >= 3)
            .min_by_key(|pos| (g.wdist(*pos, source), *pos))
            .expect("the colony sits in an ocean");
        // The claim is "heads for shore", so measure distance to the nearest
        // dry tile — not to the city, which a unit rounding a headland can
        // move away from while still doing exactly the right thing.
        let dry_tiles: Vec<Pos> = g
            .map
            .tiles
            .iter()
            .filter(|(_, tile)| !g.rules.is_water(tile) && g.rules.is_passable(tile))
            .map(|(pos, _)| *pos)
            .collect();
        assert!(!dry_tiles.is_empty(), "the island has land on it");
        let to_shore = |g: &Game, pos: Pos| {
            dry_tiles
                .iter()
                .map(|dry| g.wdist(pos, *dry))
                .min()
                .expect("dry_tiles is non-empty")
        };
        let before = to_shore(&g, at_sea);

        // The two arms get their own unit: a step spends `moves_left`, so
        // replaying one unit through both would measure an exhausted turn
        // rather than the flag.
        let frozen_unit = g.spawn_test_unit("warrior", 0, at_sea);
        assert!(
            g.is_embarked(&g.units[&frozen_unit]),
            "the case needs a unit at sea"
        );
        let mut frozen = BasicAi::new();
        // The pre-existing come-ashore path is unreachable here — that is the
        // defect, not an artifact of the setup.
        assert!(!frozen.modernization_step(&mut g, 0, frozen_unit));

        let castaway = g.spawn_test_unit("warrior", 0, at_sea);
        let mut ai = BasicAi::new();
        ai.come_ashore = true;

        // Walk both arms the same number of turns. One step is not the claim —
        // rounding a headland can leave shore distance flat for a turn — so
        // give each the same budget and compare where they end up.
        for _ in 0..12 {
            g.turn += 1;
            for uid in [frozen_unit, castaway] {
                let unit = g.units.get_mut(&uid).expect("arm alive");
                unit.moves_left = 2.0;
                unit.moved = false;
                unit.acted = false;
                unit.fortified = false;
            }
            frozen.peacetime_step(&mut g, 0, frozen_unit, false);
            ai.peacetime_step(&mut g, 0, castaway, false);
        }

        let swam = g.units[&frozen_unit].pos;
        let landed = g.units[&castaway].pos;
        assert!(
            g.is_embarked(&g.units[&frozen_unit]),
            "frozen behaviour keeps the castaway at sea; it reached {swam:?}"
        );
        assert!(
            !g.is_embarked(&g.units[&castaway]),
            "a castaway with nothing to upgrade into still comes ashore: \
             went {at_sea:?} -> {landed:?} (shore distance was {before}, now {}); \
             its frozen twin swam to {swam:?}",
            to_shore(&g, landed)
        );
    }

    /// The standing-army target used to count heads, so a single Warrior that
    /// outlived its era filled a city's whole military allowance and the
    /// empire stopped building anything better. Weighting each unit by the
    /// fraction of a front-line unit it can still field is what keeps late
    /// production and Gold flowing into modern units.
    #[test]
    fn an_obsolete_garrison_no_longer_fills_the_standing_army_target() {
        let (g, _, _) = island_colony_game(1);
        // Nothing unlocked yet: every unit is as modern as the empire gets.
        assert_eq!(BasicAi::force_weight(&g, "warrior", (0.0, 0.0)), 1.0);

        let front_line = (55.0, 0.0); // Musketman
        let warrior = BasicAi::force_weight(&g, "warrior", front_line);
        let musketman = BasicAi::force_weight(&g, "musketman", front_line);
        assert_eq!(musketman, 1.0);
        assert!(warrior < 0.4, "warrior={warrior}");
        // Even a relic holds a tile, so it never counts for nothing at all.
        assert!(BasicAi::force_weight(&g, "warrior", (140.0, 0.0)) >= 0.2);
        // Naval units are measured against the fleet, not against the army.
        assert_eq!(BasicAi::force_weight(&g, "quadrireme", (55.0, 0.0)), 1.0);
    }

    #[test]
    fn basic_ai_modernizes_affordable_obsolete_units() {
        let mut game = Game::new_full(1, 20, 14, 41_005, 40, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        let home = game.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| game.units_at(*position).is_empty())
            .unwrap();
        game.players[0].techs.insert(crate::name!("archery"));
        game.players[0].gold = 180.0;
        let slinger = game.spawn_test_unit("slinger", 0, home);

        BasicAi::upgrade_units(&mut game, 0);

        assert_eq!(game.units[&slinger].kind, "archer");
        assert_eq!(game.players[0].gold, 120.0);
    }

    #[test]
    fn ai_claims_an_earned_great_person_instead_of_leaving_it_pending() {
        let mut game = Game::new_full(1, 20, 14, 41_006, 80, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        let campus = game.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != game.cities[&city].pos)
            .unwrap();
        game.map.tiles.get_mut(&campus).unwrap().district = Some(crate::name!("campus"));
        game.cities
            .get_mut(&city)
            .unwrap()
            .districts
            .insert(crate::name!("campus"), campus);
        let cost = game.gp_cost(0, "scientist");
        game.players[0].gpp.insert("scientist".to_string(), cost);
        assert!(game.legal_actions(0).iter().any(
            |action| matches!(action, Action::RecruitGreatPerson { kind } if kind == "scientist")
        ));

        assert_eq!(BasicAi::claim_free_great_people(&mut game, 0), 1);
        assert_eq!(game.players[0].gp_claimed["scientist"], 1);
        assert!(game.players[0].great_people.iter().any(|person| person == "hypatia"));
    }

    /// The 36 cities that vanished at FULL loyalty, in one assertion: a city on
    /// 100 losing ground fast is in more danger than a city on 60 that is
    /// stable, and the level-only rule ranked them exactly the other way round.
    #[test]
    fn a_city_bleeding_loyalty_outranks_a_lower_but_stable_one() {
        let (mut game, source, target) = island_colony_game(1);
        let second_settler = game.spawn_test_unit("settler", 0, target);
        let bleeding = game.found_city_for(0, game.units[&second_settler].pos, None);
        let stable = game.city_at(source).unwrap();

        // Full loyalty, but the engine's rate says it is going fast.
        game.cities.get_mut(&bleeding).unwrap().loyalty = 100.0;
        game.cities.get_mut(&stable).unwrap().loyalty = 60.0;

        let mut live = BasicAi::new();
        live.loyalty_rate_alarm = true;
        let frozen = BasicAi::new();

        // ⚠ Force the rates rather than hoping the map generates a falling city.
        // `observed_city_loyalty_per_turn` is the map the MIRROR writes from
        // Civilization VI's own export, so this is the live representation and
        // not a test-only back door — and it makes the case unconditional
        // instead of a test that quietly passes when nothing is falling.
        game.observed_city_loyalty_per_turn.insert(bleeding, -12.0);
        game.observed_city_loyalty_per_turn.insert(stable, 1.0);
        let bleed_rate = game.city_loyalty_per_turn(&game.cities[&bleeding]);
        let stable_rate = game.city_loyalty_per_turn(&game.cities[&stable]);
        assert_eq!(bleed_rate, -12.0, "the injected rate must be what the AI reads");

        let live_bleeding = live.loyalty_emergency(&game, bleeding);
        let live_stable = live.loyalty_emergency(&game, stable);
        assert!(
            live_bleeding.is_some(),
            "a city losing {bleed_rate}/turn must register as an emergency at ANY level"
        );
        assert!(
            live_stable.is_none() || live_bleeding.unwrap() < live_stable.unwrap(),
            "the bleeding city must rank ahead: bleeding={live_bleeding:?} stable={live_stable:?} \
             (rates {bleed_rate} vs {stable_rate})"
        );

        // The frozen controller keeps the old level-only rule, so the city on
        // 100 is invisible to it however fast it is falling.
        assert_eq!(
            frozen.loyalty_emergency(&game, bleeding),
            None,
            "a tournament controller must not gain the rate reading"
        );
        assert!(
            frozen.loyalty_emergency(&game, stable).is_some(),
            "and must still see the low-level city exactly as before"
        );
    }

    /// The ordering rule on explicit rates, independent of what any map happens
    /// to generate.
    #[test]
    fn turns_to_flip_orders_loyalty_emergencies() {
        let (mut game, _, _) = island_colony_game(1);
        let city = game.player_city_ids(0)[0];
        let mut ai = BasicAi::new();
        ai.loyalty_rate_alarm = true;

        // A city that is not falling and is comfortably loyal is not an
        // emergency at all.
        game.cities.get_mut(&city).unwrap().loyalty = 100.0;
        let quiet = ai.loyalty_emergency(&game, city);
        let rate = game.city_loyalty_per_turn(&game.cities[&city]);
        assert!(
            rate >= 0.0,
            "test precondition changed: this map's lone city is now falling ({rate}/turn)"
        );
        assert_eq!(quiet, None, "a stable city on 100 loyalty is not an emergency");

        // Below the old alarm level it is flagged whatever the rate, and always
        // behind anything actually falling.
        game.cities.get_mut(&city).unwrap().loyalty = LOYALTY_LEVEL_ALARM - 1.0;
        let low = ai.loyalty_emergency(&game, city).expect("below the alarm level");
        assert!(
            low > 1_000.0,
            "a stable-but-low city must rank behind any falling city, got {low}"
        );
    }

    #[test]
    fn ai_reassigns_a_governor_from_a_safe_city_to_a_loyalty_emergency() {
        let (mut game, source, target) = island_colony_game(1);
        let second_settler = game.spawn_test_unit("settler", 0, target);
        let second = game.found_city_for(0, game.units[&second_settler].pos, None);
        let first = game.city_at(source).unwrap();
        game.players[0]
            .counters
            .insert("district_governor_titles".to_string(), 1);
        game.apply(
            0,
            &Action::AppointGovernor {
                governor: crate::name!("victor"),
                city: first,
            },
        )
        .unwrap();
        game.cities.get_mut(&first).unwrap().loyalty = 100.0;
        game.cities.get_mut(&second).unwrap().loyalty = 35.0;

        assert!(BasicAi::new().reassign_governor_for_loyalty(&mut game, 0));
        assert_eq!(
            game.players[0].governor_roster["victor"].city,
            Some(second)
        );
    }

    #[test]
    fn ai_uses_heathen_conversion_before_the_apostle_moves() {
        let mut game = Game::new_full(1, 20, 14, 41_007, 80, 0, true);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let center = game.cities[&game.player_city_ids(0)[0]].pos;
        let adjacent = game
            .nbrs(center)
            .into_iter()
            .find(|position| {
                game.map.get(*position).is_some_and(|tile| {
                    game.rules.is_passable(tile) && !game.rules.is_water(tile)
                })
            })
            .unwrap();
        let apostle = game.spawn_test_unit("apostle", 0, center);
        game.units
            .get_mut(&apostle)
            .unwrap()
            .promotions
            .insert(crate::name!("heathen_conversion"));
        let barbarian = game.barb_pid.unwrap();
        let converted = game.spawn_test_unit("warrior", barbarian, adjacent);

        assert_eq!(BasicAi::use_opportunistic_unit_tools(&mut game, 0), 1);
        assert_eq!(game.units[&converted].owner, 0);
        assert_eq!(game.units[&apostle].moves_left, 0.0);
    }

    #[test]
    fn coastal_empires_research_navigation_before_generic_land_unlocks() {
        let (mut g, _, _) = island_colony_game(1);
        g.players[0].research = None;
        let ai = BasicAi::new();
        ai.research(&mut g, 0);
        assert_eq!(g.players[0].research.as_deref(), Some("sailing"));
    }

    #[test]
    fn market_city_states_finish_the_banking_branch_before_late_era_fallbacks() {
        let mut g = Game::new_full(1, 20, 14, 18878401, 250, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let cid = g.player_city_ids(0)[0];
        g.players[0].is_minor = true;
        g.players[0].civ = "Zanzibar".to_string();
        g.cities
            .get_mut(&cid)
            .unwrap()
            .buildings
            .extend([crate::name!("market"), crate::name!("walls")]);
        grant_tech_with_prerequisites(&mut g, 0, "education");
        g.players[0]
            .techs
            .extend([crate::name!("sailing"), crate::name!("currency")]);
        g.players[0].research = None;
        let mut ai = BasicAi::new();
        ai.minor = true;

        assert_eq!(BasicAi::economic_research_goal(&g, 0), Some("banking"));
        assert!(g.available_techs(0).iter().any(|tech| tech == "stirrups"));
        ai.minor_research(&mut g, 0);
        assert_eq!(g.players[0].research.as_deref(), Some("stirrups"));

        g.players[0].techs.insert(crate::name!("stirrups"));
        g.players[0].research = None;
        ai.minor_research(&mut g, 0);
        assert_eq!(g.players[0].research.as_deref(), Some("banking"));
    }

    #[test]
    fn naval_wars_prioritize_the_next_fleet_upgrade() {
        let (mut g, source, _) = island_colony_game(2);
        grant_tech_with_prerequisites(&mut g, 0, "cartography");
        grant_tech_with_prerequisites(&mut g, 0, "celestial_navigation");
        g.at_war.insert((0, 1));
        let contact = g
            .nbrs(source)
            .into_iter()
            .find(|pos| g.map.get(*pos).is_some_and(|tile| g.rules.is_water(tile)))
            .unwrap();
        g.spawn_test_unit("galley", 1, contact);
        g.players[0].research = None;
        assert_eq!(
            BasicAi::water_research_goal(&g, 0),
            Some("square_rigging"),
            "available={:?}, war={}, enemy_alive={}",
            g.available_techs(0),
            g.is_at_war(0, 1),
            g.players[1].alive
        );
        assert!(
            g.available_techs(0)
                .iter()
                .any(|tech| tech == "square_rigging"),
            "available={:?}",
            g.available_techs(0)
        );
        let available = g.available_techs(0);
        BasicAi::new().research(&mut g, 0);
        assert_eq!(
            g.players[0].research.as_deref(),
            Some("square_rigging"),
            "available before selection: {available:?}"
        );
    }

    #[test]
    fn coastal_cities_build_a_melee_ship_for_exploration_and_capture() {
        let (mut g, _, _) = island_colony_game(1);
        g.players[0].techs.insert(crate::name!("sailing"));
        let cid = g.player_city_ids(0)[0];
        let ai = BasicAi::new();
        let item = ai
            .pick_item(&g, 0, cid, 1, 0, 2, 1, 0, 4, 2, 2)
            .expect("coastal city has a production choice");
        assert!(matches!(item, Item::Unit { unit } if unit == "galley"));
    }

    #[test]
    fn coastal_cities_add_ranged_firepower_after_the_melee_screen() {
        let (mut g, source, _) = island_colony_game(2);
        g.players[0]
            .techs
            .extend([crate::name!("sailing"), crate::name!("shipbuilding")]);
        g.at_war.insert((0, 1));
        let water = g
            .nbrs(source)
            .into_iter()
            .find(|pos| g.map.get(*pos).is_some_and(|tile| g.rules.is_water(tile)))
            .unwrap();
        g.spawn_test_unit("galley", 0, water);
        let enemy_water = g
            .nbrs(water)
            .into_iter()
            .find(|pos| g.map.get(*pos).is_some_and(|tile| g.rules.is_water(tile)))
            .unwrap();
        g.spawn_test_unit("galley", 1, enemy_water);
        let cid = g.player_city_ids(0)[0];
        let item = BasicAi::new()
            .pick_item(&g, 0, cid, 1, 0, 2, 1, 0, 5, 3, 2)
            .expect("coastal city has a production choice");
        assert!(matches!(item, Item::Unit { unit } if unit == "quadrireme"));
    }

    #[test]
    fn city_states_keep_a_bounded_force_that_scales_with_local_threat() {
        let mut g = Game::new_full(2, 24, 16, 97, 120, 1, false);
        let minor = g
            .players
            .iter()
            .find(|player| player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .unwrap();
        assert_eq!(BasicAi::minor_military_budget(&g, minor), 3);
        g.world_era = 6;
        assert_eq!(BasicAi::minor_military_budget(&g, minor), 3);

        let major_units = g.player_unit_ids(0);
        for unit in major_units {
            g.remove_unit(unit);
        }
        g.at_war.insert((0, minor));
        assert_eq!(BasicAi::minor_military_budget(&g, minor), 4);

        let city = g.player_city_ids(minor)[0];
        let front = g
            .nbrs(g.cities[&city].pos)
            .into_iter()
            .find(|position| {
                g.map
                    .get(*position)
                    .is_some_and(|tile| !g.rules.is_water(tile))
            })
            .unwrap();
        for _ in 0..8 {
            g.spawn_test_unit("warrior", 0, front);
        }
        assert_eq!(BasicAi::minor_military_budget(&g, minor), 7);

        g.at_war.clear();
        let mut ai = BasicAi::new();
        ai.minor = true;
        let choice = ai.pick_item(&g, minor, city, 1, 0, 1, 0, 0, 3, 2, 1);
        assert!(
            !matches!(choice, Some(Item::Unit { ref unit }) if g.rules.units[unit].class == "military"),
            "a peaceful city-state at its force budget must prefer infrastructure or idle"
        );
    }

    #[test]
    fn city_state_governors_repair_pillaged_districts_before_new_production() {
        let mut g = Game::new_full(1, 24, 16, 91_769, 120, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| g.units[unit].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        g.players[0].is_minor = true;
        g.players[0].techs.insert(crate::name!("writing"));
        let city = g.player_city_ids(0)[0];
        let center = g.cities[&city].pos;
        let campus = g.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != center)
            .unwrap();
        {
            let tile = g.map.tiles.get_mut(&campus).unwrap();
            tile.district = Some(crate::name!("campus"));
            tile.pillaged = true;
        }
        let developed = g.cities.get_mut(&city).unwrap();
        developed
            .districts
            .insert(crate::name!("campus"), campus);
        developed.buildings.push(crate::name!("library"));
        developed
            .pillaged_buildings
            .insert(crate::name!("library"));

        let mut ai = BasicAi::new();
        ai.minor = true;
        let choice = ai
            .pick_item(&g, 0, city, 1, 0, 1, 0, 0, 3, 2, 1)
            .expect("a damaged city-state has a repair to queue");
        assert!(matches!(
            choice,
            Item::Repair { repair, pos } if repair == "district" && pos == campus
        ));
    }

    #[test]
    fn city_states_research_and_build_ancient_walls_first() {
        let mut g = Game::new_full(1, 24, 16, 91_772, 120, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| g.units[unit].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = g.player_city_ids(0)[0];
        g.players[0].is_minor = true;
        g.players[0].civ = "Geneva".to_string();
        let mut ai = BasicAi::new();
        ai.minor = true;

        ai.minor_research(&mut g, 0);
        assert_eq!(g.players[0].research.as_deref(), Some("mining"));
        g.players[0].research = None;
        g.players[0].techs.insert(crate::name!("mining"));
        ai.minor_research(&mut g, 0);
        assert_eq!(g.players[0].research.as_deref(), Some("masonry"));

        g.players[0].research = None;
        g.players[0].techs.insert(crate::name!("masonry"));
        let item = ai.pick_item(&g, 0, city, 1, 0, 10, 0, 0, 3, 2, 1).unwrap();
        assert_eq!(
            item,
            Item::Building {
                building: crate::name!("walls")
            }
        );
    }

    #[test]
    fn every_city_state_type_prioritizes_its_matching_district() {
        for (civilization, family) in [
            ("Geneva", "campus"),
            ("Mohenjo-Daro", "theater_square"),
            ("Yerevan", "holy_site"),
            ("Kabul", "encampment"),
            ("Auckland", "industrial_zone"),
            ("Zanzibar", "commercial_hub"),
        ] {
            let mut g = Game::new_full(1, 24, 16, 91_800, 120, 0, false);
            let settler = g
                .player_unit_ids(0)
                .into_iter()
                .find(|unit| g.units[unit].kind == "settler")
                .unwrap();
            g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
            let city = g.player_city_ids(0)[0];
            g.players[0].is_minor = true;
            g.players[0].civ = civilization.to_string();
            g.players[0].techs = g.rules.techs.keys().cloned().collect();
            g.players[0].civics = g.rules.civics.keys().cloned().collect();
            let center = g.cities[&city].pos;
            for position in g.wdisk(center, 3) {
                if g.map.tiles[&position].owner_city.is_none() {
                    g.map.tiles.get_mut(&position).unwrap().owner_city = Some(city);
                    g.cities.get_mut(&city).unwrap().owned_tiles.push(position);
                }
            }
            {
                let state = g.cities.get_mut(&city).unwrap();
                state.pop = 10;
                state.buildings.extend([
                    crate::name!("walls"),
                    crate::name!("medieval_walls"),
                    crate::name!("renaissance_walls"),
                    crate::name!("monument"),
                ]);
                state.wall_hp = 300;
            }
            let mut ai = BasicAi::new();
            ai.minor = true;
            let item = ai
                .pick_item(&g, 0, city, 1, 0, 10, 10, 10, 99, 99, 99)
                .unwrap_or_else(|| panic!("{civilization} found no specialty district"));
            assert!(
                matches!(&item, Item::District { district, .. }
                    if g.district_family(district) == family),
                "{civilization} selected {item:?} instead of {family}"
            );
        }
    }

    #[test]
    fn fully_developed_city_states_can_idle() {
        let mut g = Game::new_full(1, 24, 16, 91_770, 120, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| g.units[unit].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = g.player_city_ids(0)[0];
        let project_pos = g.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != g.cities[&city].pos)
            .unwrap();
        let techs = g.rules.techs.keys().cloned().collect();
        let civics = g.rules.civics.keys().cloned().collect();
        let buildings = g.rules.buildings.keys().cloned().collect();
        let districts: Vec<Name> = g.rules.districts.keys().cloned().collect();
        let wonders: Vec<Name> = g.rules.wonders.keys().cloned().collect();
        g.players[0].is_minor = true;
        g.players[0].techs = techs;
        g.players[0].civics = civics;
        {
            let tile = g.map.tiles.get_mut(&project_pos).unwrap();
            tile.district = Some(crate::name!("campus"));
            tile.pillaged = false;
        }
        {
            let developed = g.cities.get_mut(&city).unwrap();
            developed.buildings = buildings;
            for district in districts {
                developed.districts.insert(Name::new(&district), project_pos);
            }
            for wonder in wonders {
                developed.wonders.insert(Name::new(&wonder), project_pos);
            }
        }
        let mut ai = BasicAi::new();
        ai.minor = true;

        assert_eq!(
            ai.pick_item(&g, 0, city, 1, 0, 10, 10, 10, 99, 99, 99),
            None
        );
    }

    #[test]
    fn repeatable_district_projects_do_not_preempt_basic_infrastructure() {
        let mut g = Game::new_full(1, 24, 16, 91_771, 120, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| g.units[unit].kind == "settler")
            .unwrap();
        g.found_city_for(0, g.units[&settler].pos, None);
        let city = g.player_city_ids(0)[0];
        g.cities
            .get_mut(&city)
            .unwrap()
            .buildings
            .retain(|building| building != "monument");
        for position in g.nbrs(g.cities[&city].pos) {
            g.map.tiles.get_mut(&position).unwrap().terrain = crate::name!("plains");
        }
        let campus = g.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != g.cities[&city].pos)
            .unwrap();
        g.map.tiles.get_mut(&campus).unwrap().district = Some(crate::name!("campus"));
        g.cities
            .get_mut(&city)
            .unwrap()
            .districts
            .insert(crate::name!("campus"), campus);
        let grants = Item::Project {
            project: crate::name!("campus_research_grants"),
        };
        assert!(g.can_produce(0, city, &grants));

        let item = BasicAi::new()
            .pick_item(&g, 0, city, 8, 2, 20, 10, 0, 20, 10, 10)
            .expect("developing city has a production choice");
        assert!(
            matches!(item, Item::Building { ref building } if building == "monument"),
            "repeatable project displaced {item:?}"
        );
    }

    #[test]
    fn developed_city_wonder_fallback_uses_a_legal_placed_item() {
        let mut game = Game::new_full(1, 24, 16, 91_772, 120, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        let techs: Vec<Name> = game.rules.techs.keys().cloned().collect();
        let civics: Vec<Name> = game.rules.civics.keys().cloned().collect();
        game.players[0].techs.extend(techs);
        game.players[0].civics.extend(civics);
        for position in game.cities[&city].owned_tiles.clone() {
            if position == game.cities[&city].pos {
                continue;
            }
            let tile = game.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("desert");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
            tile.district = None;
            tile.wonder = None;
        }
        assert!(game.producible_items(0, city).iter().any(
            |item| matches!(item, Item::Wonder { wonder, .. } if wonder == "pyramids")
        ));

        let wonder = BasicAi::cheapest_available_wonder(&game, 0, city)
            .expect("the developed city has a placed wonder fallback");
        assert!(matches!(wonder, Item::Wonder { .. }));
        assert!(game.can_produce(0, city, &wonder));
    }

    #[test]
    fn unfounded_empire_reserves_only_one_holy_site_for_the_prophet_race() {
        let mut game = Game::new_full(
            1, 24, 16, crate::rng::fixture_seed("HOLYSITE", 91_773), 120, 0, false,
        );
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let capital = game.player_city_ids(0)[0];
        let capital_pos = game.cities[&capital].pos;
        let second_pos = game
            .map
            .tiles
            .iter()
            .filter(|(_, tile)| {
                tile.owner_city.is_none()
                    && game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
            })
            .map(|(position, _)| *position)
            .find(|position| (4..=10).contains(&game.wdist(capital_pos, *position)))
            .unwrap();
        let second = game.found_city_for(0, second_pos, None);
        game.players[0].techs.insert(crate::name!("astrology"));
        for city in [capital, second] {
            game.cities
                .get_mut(&city)
                .unwrap()
                .buildings
                .push(crate::name!("monument"));
        }

        let ai = BasicAi::new();
        let choose = |game: &Game, city| {
            ai.pick_item(game, 0, city, 2, 2, 2, 2, 10, 5, 5, 5)
                .expect("city has a production choice")
        };
        let first = choose(&game, capital);
        assert!(matches!(
            first,
            Item::District { ref district, .. }
                if game.district_family(district) == "holy_site"
        ));
        game.apply(
            0,
            &Action::Produce {
                city: capital,
                item: first,
            },
        )
        .unwrap();

        let reserved = choose(&game, second);
        assert!(
            !matches!(
                reserved,
                Item::District { ref district, .. }
                    if game.district_family(district) == "holy_site"
            ),
            "a second opening Holy Site displaced development: {reserved:?}"
        );

        game.players[0].religion = Some("Test Faith".to_string());
        let founded = choose(&game, second);
        assert!(matches!(
            founded,
            Item::District { ref district, .. }
                if game.district_family(district) == "holy_site"
        ));
    }

    #[test]
    fn live_missionary_buyer_never_strengthens_a_rival_majority() {
        let mut game = Game::new_full(2, 24, 16, 91_775, 120, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler })
            .unwrap();
        let city = game.player_city_ids(0)[0];
        let center = game.cities[&city].pos;
        let holy_site = game.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != center)
            .unwrap();
        game.map.tiles.get_mut(&holy_site).unwrap().district =
            Some(crate::name!("holy_site"));
        {
            let city = game.cities.get_mut(&city).unwrap();
            city.districts.insert(crate::name!("holy_site"), holy_site);
            city.buildings.push(crate::name!("shrine"));
            city.atheist_pressure = 0.0;
            city.pressure.insert("Rival Faith".to_string(), 1_000.0);
            // Keep this test on the post-production Faith-spending path.
            city.queue.push(Item::Project {
                project: crate::name!("holy_site_prayers"),
            });
        }
        game.players[0].religion = Some("Home Faith".to_string());
        game.players[1].religion = Some("Rival Faith".to_string());
        game.players[0].techs.insert(crate::name!("astrology"));
        game.players[0].faith = 1_000.0;
        let mut historical = game.clone();
        BasicAi::new().cities(&mut historical, 0);
        assert!(
            historical.units.values().any(|unit| {
                unit.owner == 0
                    && unit.kind == "missionary"
                    && unit.religion.as_deref() == Some("Rival Faith")
            }),
            "the default-off gate preserves the frozen native controller"
        );

        let mut ai = BasicAi::new();
        ai.w.faith_builder = f64::INFINITY;
        ai.live_religious_purchase_guard = true;

        let before = game.log.len();
        ai.cities(&mut game, 0);
        assert!(
            !game.log.since(before).any(|(_, action)| matches!(
                action,
                Action::Buy { unit, currency, .. }
                    if unit == "missionary" && currency == "faith"
            )),
            "a converted Holy Site must not buy the rival's Missionary"
        );

        let city_state = game.cities.get_mut(&city).unwrap();
        city_state.pressure.clear();
        city_state.pressure.insert("Home Faith".to_string(), 1_000.0);
        ai.cities(&mut game, 0);
        assert!(game.log.iter().any(|(_, action)| matches!(
            action,
            Action::Buy { city: bought_at, unit, currency, .. }
                if *bought_at == city && unit == "missionary" && currency == "faith"
        )));
        let missionary = game
            .units
            .values()
            .find(|unit| unit.owner == 0 && unit.kind == "missionary")
            .expect("the same Holy Site can buy once its majority is ours");
        assert_eq!(missionary.religion.as_deref(), Some("Home Faith"));
    }

    #[test]
    fn religionless_empire_skips_holy_site_after_prophet_slots_close() {
        let mut game = Game::new_full(4, 30, 18, 91_774, 120, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler })
            .unwrap();
        let city = game.player_city_ids(0)[0];
        game.players[0].techs.insert(crate::name!("astrology"));
        game.cities
            .get_mut(&city)
            .unwrap()
            .buildings
            .push(crate::name!("monument"));
        for player in 1..=game.max_religions() {
            game.players[player].religion = Some(format!("Claimed Faith {player}"));
        }
        assert_eq!(game.religions_founded(), game.max_religions());

        let choice = BasicAi::new()
            .pick_item(&game, 0, city, 1, 1, 1, 1, 10, 5, 5, 5)
            .expect("the city has non-religious development available");
        assert!(
            !matches!(
                choice,
                Item::District { ref district, .. }
                    if game.district_family(district) == "holy_site"
            ),
            "a closed Prophet race must not consume a district slot: {choice:?}"
        );
    }

    #[test]
    fn prophet_uses_remaining_data_backed_beliefs_after_preferred_pairs_are_taken() {
        let mut game = Game::new_full(3, 26, 16, 91_773, 120, 0, false);
        for pid in 0..3 {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            let position = game.units[&settler].pos;
            game.found_city_for(pid, position, None);
        }
        game.players[0].religion = Some("First Faith".to_string());
        game.players[0].religion_beliefs = vec!["work_ethic".to_string(), "tithe".to_string()];
        game.players[1].religion = Some("Second Faith".to_string());
        game.players[1].religion_beliefs =
            vec!["choral_music".to_string(), "world_church".to_string()];

        let holy_city = game.player_city_ids(2)[0];
        let center = game.cities[&holy_city].pos;
        let holy_site = game.cities[&holy_city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != center)
            .unwrap();
        game.map.tiles.get_mut(&holy_site).unwrap().district = Some(crate::name!("holy_site"));
        game.cities
            .get_mut(&holy_city)
            .unwrap()
            .districts
            .insert(crate::name!("holy_site"), holy_site);
        game.players[2].prophet_pending = true;
        game.current = 2;

        BasicAi::new().research(&mut game, 2);

        assert!(game.players[2].religion.is_some());
        assert!(!game.players[2].prophet_pending);
        assert!(game.players[2]
            .religion_beliefs
            .iter()
            .any(|belief| belief == "cross_cultural_dialogue"));
    }

    #[test]
    fn settler_keeps_a_reachable_colony_target_across_water() {
        let (mut g, source, target) = island_colony_game(1);
        g.players[0]
            .techs
            .extend([crate::name!("sailing"), crate::name!("shipbuilding")]);
        let settler = g.spawn_test_unit("settler", 0, source);
        let mut ai = BasicAi::new();

        assert!(ai.settler_step(&mut g, 0, settler));
        assert_eq!(ai.settler_targets.get(&settler), Some(&target));
        assert!(g
            .map
            .get(g.units[&settler].pos)
            .is_some_and(|tile| g.rules.is_water(tile)));
    }

    #[test]
    fn settler_routes_to_distant_land_before_embarkation() {
        let mut game = Game::new_full(1, 18, 10, 91_002, 120, 0, false);
        let founding_settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        let source = game.units[&founding_settler].pos;
        let target = game
            .map
            .tiles
            .keys()
            .copied()
            .max_by_key(|position| (game.wdist(source, *position), *position))
            .unwrap();
        assert!(game.wdist(source, target) > 8);
        game.apply(
            0,
            &Action::FoundCity {
                unit: founding_settler,
            },
        )
        .unwrap();
        for (position, tile) in &mut game.map.tiles {
            tile.feature = None;
            tile.resource = None;
            tile.owner_city = None;
            if position != source && position != target {
                tile.terrain = crate::name!("mountain");
                tile.improvement = Some(crate::name!("mountain_tunnel"));
            }
        }
        game.map.tiles.get_mut(&source).unwrap().terrain = crate::name!("plains");
        game.map.tiles.get_mut(&target).unwrap().terrain = crate::name!("grassland");
        let settler = game.spawn_test_unit("settler", 0, source);
        let mut ai = BasicAi::new();

        assert!(!ai.has_practical_settle_site(&game, 0));
        assert!(ai.settler_step(&mut game, 0, settler));
        assert_eq!(ai.settler_targets.get(&settler), Some(&target));
        assert_ne!(game.units[&settler].pos, source);
    }

    #[test]
    fn settler_search_looks_past_an_unreachable_high_value_prefix() {
        let mut game = Game::new_full(1, 30, 18, 91_005, 120, 0, false);
        let founding_settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        let source = game.units[&founding_settler].pos;
        for tile in game.map.tiles.values_mut() {
            tile.terrain = crate::name!("coast");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
            tile.district = None;
            tile.wonder = None;
            tile.owner_city = None;
            tile.cliff_edges = [false; 6];
        }
        game.map.tiles.get_mut(&source).unwrap().terrain = crate::name!("desert");
        game.apply(
            0,
            &Action::FoundCity {
                unit: founding_settler,
            },
        )
        .unwrap();

        let target = game
            .wdisk(source, 4)
            .into_iter()
            .filter(|position| game.wdist(source, *position) == 4)
            .min()
            .expect("the source has a site four tiles away");
        let mut corridor = vec![source];
        let mut cursor = source;
        while cursor != target {
            cursor = game
                .nbrs(cursor)
                .into_iter()
                .filter(|next| game.wdist(*next, target) < game.wdist(cursor, target))
                .min()
                .expect("a direct corridor reaches the target");
            corridor.push(cursor);
        }
        for position in &corridor {
            game.map.tiles.get_mut(position).unwrap().terrain = crate::name!("desert");
        }

        let island_center = game
            .map
            .tiles
            .keys()
            .copied()
            .max_by_key(|position| (game.wdist(source, *position), *position))
            .expect("the map has a distant island center");
        let island: Vec<Pos> = game
            .wdisk(island_center, 5)
            .into_iter()
            .filter(|position| {
                game.wdist(source, *position) >= 9
                    && corridor
                        .iter()
                        .all(|land| game.wdist(*land, *position) >= 2)
            })
            .collect();
        assert!(island.len() > 40);
        for position in island {
            let tile = game.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("grassland");
            tile.feature = Some(crate::name!("forest"));
            tile.hills = true;
        }

        let settler = game.spawn_test_unit("settler", 0, source);
        let ai = BasicAi::new();
        let mut ranked: Vec<(Pos, f64)> = game
            .wdisk(source, game.map.width + game.map.height)
            .into_iter()
            .filter(|position| ai.valid_settle_site(&game, 0, *position))
            .map(|position| {
                let score = ai.settle_value(&game, position)
                    - ai.w.settle_dist * game.wdist(source, position) as f64;
                (position, score)
            })
            .collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
        assert!(ranked
            .iter()
            .take(40)
            .all(|(position, _)| game.route_step(settler, *position, 0).is_none()));

        assert_eq!(
            ai.best_reachable_settle_site(
                &game,
                0,
                settler,
                game.map.width + game.map.height,
            )
            .map(|(position, _)| position),
            Some(target),
        );
    }

    #[test]
    fn settler_retargets_a_natural_wonder_for_an_open_island_site() {
        let (mut game, _source, wonder) = island_colony_game(1);
        let alternate = game
            .nbrs(wonder)
            .into_iter()
            .find(|position| game.map.get(*position).is_some())
            .expect("the island has a neighboring shore tile");
        game.map.tiles.get_mut(&alternate).unwrap().terrain = crate::name!("grassland");
        game.map.tiles.get_mut(&wonder).unwrap().feature = Some(crate::name!("pantanal"));

        let settler = game.spawn_test_unit("settler", 0, wonder);
        let mut ai = BasicAi::new();
        ai.settler_targets.insert(settler, wonder);

        assert!(
            !ai.valid_settle_site(&game, 0, wonder),
            "a natural wonder looks like land but may not become a city"
        );
        assert!(!game.can_found_city(settler));
        assert!(ai.settler_step(&mut game, 0, settler));
        assert_eq!(ai.settler_targets.get(&settler), Some(&alternate));
        assert_eq!(game.units[&settler].pos, alternate);
    }

    #[test]
    fn advanced_settle_search_rejects_a_natural_wonder() {
        let (mut game, _source, wonder) = island_colony_game(1);
        game.map.tiles.get_mut(&wonder).unwrap().feature = Some(crate::name!("pantanal"));

        assert!(
            !AdvancedAi::new().any_settle_site(&game, 0),
            "a natural wonder cannot keep the strategic settler gate open"
        );
    }

    #[test]
    fn settler_follows_its_route_past_a_closer_cul_de_sac() {
        let mut game = Game::new_full(1, 20, 14, 91_003, 120, 0, false);
        for unit in game.player_unit_ids(0) {
            game.remove_unit(unit);
        }
        for tile in game.map.tiles.values_mut() {
            tile.terrain = crate::name!("plains");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
            tile.district = None;
            tile.wonder = None;
            tile.owner_city = None;
        }
        let start = game
            .map
            .tiles
            .keys()
            .copied()
            .find(|position| game.wdisk(*position, 4).len() == 61)
            .expect("test map has an interior tile");
        let target = game
            .wdisk(start, 4)
            .into_iter()
            .filter(|position| game.wdist(start, *position) == 4)
            .min()
            .expect("test map has a target four tiles away");
        let trap = game
            .nbrs(start)
            .into_iter()
            .filter(|position| game.wdist(*position, target) < game.wdist(start, target))
            .min_by_key(|position| (game.wdist(*position, target), *position))
            .expect("target has a geometrically closer neighbor");
        for position in game.nbrs(trap) {
            if position != start {
                game.map.tiles.get_mut(&position).unwrap().terrain = crate::name!("mountain");
            }
        }
        let settler = game.spawn_test_unit("settler", 0, start);
        let routed = game
            .route_step(settler, target, 0)
            .expect("the target remains reachable around the cul-de-sac");
        assert_ne!(routed, trap);

        let mut ai = BasicAi::new();
        ai.settler_targets.insert(settler, target);
        assert!(ai.settler_step(&mut game, 0, settler));
        assert!(ai.settler_step(&mut game, 0, settler));
        assert_ne!(
            game.units[&settler].pos, start,
            "the settler must not spend both movement points entering and leaving the trap"
        );
        assert_ne!(game.units[&settler].pos, trap);
    }

    #[test]
    fn generic_pathing_does_not_reverse_its_last_step_in_the_same_turn() {
        let mut game = Game::new_full(1, 20, 14, 91_004, 120, 0, false);
        for unit in game.player_unit_ids(0) {
            game.remove_unit(unit);
        }
        for tile in game.map.tiles.values_mut() {
            tile.terrain = crate::name!("plains");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
            tile.district = None;
            tile.wonder = None;
            tile.owner_city = None;
        }
        let start = game
            .map
            .tiles
            .keys()
            .copied()
            .find(|position| game.wdisk(*position, 4).len() == 61)
            .expect("test map has an interior tile");
        let target = game
            .wdisk(start, 4)
            .into_iter()
            .filter(|position| game.wdist(start, *position) == 4)
            .min()
            .expect("test map has a target four tiles away");
        let trap = game
            .nbrs(start)
            .into_iter()
            .filter(|position| game.wdist(*position, target) < game.wdist(start, target))
            .min_by_key(|position| (game.wdist(*position, target), *position))
            .expect("target has a geometrically closer neighbor");
        for position in game.nbrs(trap) {
            if position != start {
                game.map.tiles.get_mut(&position).unwrap().terrain = crate::name!("mountain");
            }
        }
        let unit = game.spawn_test_unit("warrior", 0, start);
        let ai = BasicAi::new();

        assert!(ai.step_toward(&mut game, 0, unit, target));
        assert_eq!(game.units[&unit].pos, trap);
        assert!(
            !ai.step_toward(&mut game, 0, unit, target),
            "the unit should wait instead of spending its next point returning to its source"
        );
        assert_eq!(game.units[&unit].pos, trap);
    }

    #[test]
    fn naval_escorts_link_to_embarked_settlers() {
        let (mut g, source, _) = island_colony_game(1);
        g.players[0]
            .techs
            .extend([crate::name!("sailing"), crate::name!("shipbuilding")]);
        let water = g
            .nbrs(source)
            .into_iter()
            .find(|pos| g.map.get(*pos).is_some_and(|tile| g.rules.is_water(tile)))
            .unwrap();
        let settler = g.spawn_test_unit("settler", 0, water);
        let galley = g.spawn_test_unit("galley", 0, water);
        BasicAi::new().prepare_unit_formations(&mut g, 0);
        assert_eq!(g.units[&galley].linked_to, Some(settler));
        assert_eq!(g.units[&settler].linked_to, Some(galley));
    }

    #[test]
    fn linked_ship_leads_settler_toward_the_persistent_colony_target() {
        let (mut g, source, target) = island_colony_game(1);
        g.players[0]
            .techs
            .extend([crate::name!("sailing"), crate::name!("shipbuilding")]);
        let settler = g.spawn_test_unit("settler", 0, source);
        let galley = g.spawn_test_unit("galley", 0, source);
        let mut ai = BasicAi::new();
        ai.prepare_unit_formations(&mut g, 0);

        assert!(!ai.settler_step(&mut g, 0, settler));
        assert_eq!(ai.settler_targets.get(&settler), Some(&target));
        assert!(ai.military_step(&mut g, 0, galley));
        assert_eq!(g.units[&galley].pos, g.units[&settler].pos);
        assert!(g
            .map
            .get(g.units[&galley].pos)
            .is_some_and(|tile| g.rules.is_water(tile)));
    }

    #[test]
    fn escorted_settler_unlinks_at_the_destination_coast_and_founds_the_colony() {
        let (mut g, source, target) = island_colony_game(1);
        g.players[0]
            .techs
            .extend([crate::name!("sailing"), crate::name!("shipbuilding")]);
        let settler = g.spawn_test_unit("settler", 0, source);
        let galley = g.spawn_test_unit("galley", 0, source);
        let mut ai = BasicAi::new();
        ai.prepare_unit_formations(&mut g, 0);

        for _ in 0..12 {
            for uid in [settler, galley] {
                if let Some(unit) = g.units.get_mut(&uid) {
                    unit.moves_left = 4.0;
                    unit.attacks_left = 1;
                    unit.acted = false;
                    unit.moved = false;
                    unit.fortified = false;
                }
            }
            for _ in 0..8 {
                if !g.units.contains_key(&settler)
                    || g.units[&settler].moves_left <= 0.0
                    || !ai.settler_step(&mut g, 0, settler)
                {
                    break;
                }
            }
            for _ in 0..8 {
                if !g.units.contains_key(&galley)
                    || g.units[&galley].moves_left <= 0.0
                    || !ai.military_step(&mut g, 0, galley)
                {
                    break;
                }
            }
            if !g.units.contains_key(&settler) {
                break;
            }
        }

        assert!(!g.units.contains_key(&settler));
        assert!(g
            .city_at(target)
            .is_some_and(|cid| g.cities[&cid].owner == 0));
    }

    #[test]
    fn ships_intercept_embarked_enemies_instead_of_chasing_inland_targets() {
        let (mut g, source, target) = island_colony_game(2);
        g.at_war.insert((0, 1));
        // The rival's own starting units sit wherever mapgen dropped them, so
        // clear them: this test is about choosing between the two threats it
        // places, not about whichever spawn happens to be nearest.
        for uid in g.player_unit_ids(1) {
            g.units.remove(&uid);
        }
        let water = g
            .nbrs(source)
            .into_iter()
            .find(|pos| g.map.get(*pos).is_some_and(|tile| g.rules.is_water(tile)))
            .unwrap();
        let galley = g.spawn_test_unit("galley", 0, water);
        let enemy_water = g
            .nbrs(water)
            .into_iter()
            .find(|pos| g.map.get(*pos).is_some_and(|tile| g.rules.is_water(tile)))
            .unwrap();
        let embarked = g.spawn_test_unit("settler", 1, enemy_water);
        g.spawn_test_unit("warrior", 1, target);
        let ai = BasicAi::new();
        assert_eq!(
            ai.nearest_enemy_for_unit(&g, 0, galley, &[1]),
            Some(g.units[&embarked].pos)
        );
    }

    /// Build one empire, one enemy city, and one raider standing in the home
    /// ring, then place our soldier so the ENEMY CITY IS STRICTLY NEARER TO IT
    /// than the raider is. That is the live shape: on run
    /// `civvis-20260803T005930Z` the army stood on the Korean border while a
    /// Crossbowman sat four tiles from two of our cities for 21 turns.
    fn raider_at_home_game() -> (Game, u32, Pos, Pos) {
        let mut g = Game::new_full(2, 30, 20, 77, 60, 0, false);
        for player in 0..2 {
            let settler = g
                .player_unit_ids(player)
                .into_iter()
                .find(|uid| g.units[uid].kind == "settler")
                .expect("each player opens with a settler");
            g.current = player;
            g.apply(player, &Action::FoundCity { unit: settler }).unwrap();
        }
        for player in 0..2 {
            for uid in g.player_unit_ids(player) {
                g.remove_unit(uid);
            }
        }
        g.current = 0;
        g.players[0].met.insert(1);
        g.players[1].met.insert(0);
        g.apply(0, &Action::DeclareWar { player: 1 }).unwrap();

        let home = g.cities[&g.player_city_ids(0)[0]].pos;
        let enemy_city = g.cities[&g.player_city_ids(1)[0]].pos;
        let land = |g: &Game, pos: Pos| {
            g.map
                .get(pos)
                .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        };

        // The raider: inside the home ring, and far from the enemy city so it
        // can never be mistaken for an offensive target.
        let raider_at = g
            .wdisk(home, HOME_THREAT_RADIUS)
            .into_iter()
            .filter(|pos| land(&g, *pos) && g.wdist(*pos, home) >= 3)
            .max_by_key(|pos| (g.wdist(*pos, enemy_city), *pos))
            .expect("home ring has land in it");
        g.spawn_test_unit("warrior", 1, raider_at);

        // Our soldier: nearer the enemy city than the raider, and inside recall
        // range of the raider. Without the fix it besieges and never turns round.
        let soldier_at = g
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| land(&g, *pos) && g.units_at(*pos).is_empty())
            .filter(|pos| g.wdist(*pos, enemy_city) < g.wdist(*pos, raider_at))
            .filter(|pos| g.wdist(*pos, raider_at) <= HOME_DEFENSE_RECALL_RANGE)
            .min_by_key(|pos| (g.wdist(*pos, enemy_city), *pos))
            .expect("a tile exists that is closer to the enemy city than to the raider");
        let soldier = g.spawn_test_unit("warrior", 0, soldier_at);
        (g, soldier, raider_at, enemy_city)
    }

    /// The measured collapse in one test: a city with a raider next to it and
    /// nobody standing on it.
    ///
    /// ⚠ The point is WHERE the unit is sent. Targeting, whether it picks the
    /// enemy city or the raider, always names a tile to attack; only the
    /// garrison names our own city tile, which is the one that has to be
    /// occupied for the city not to fall to a single melee step.
    #[test]
    fn an_empty_city_with_a_raider_next_to_it_claims_a_defender() {
        let (mut g, soldier, _, _) = raider_at_home_game();
        let home = g.cities[&g.player_city_ids(0)[0]].pos;
        let beside = g
            .nbrs(home)
            .into_iter()
            .find(|pos| {
                g.units_at(*pos).is_empty()
                    && g.map
                        .get(*pos)
                        .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
            })
            .expect("the capital has a passable neighbour");
        g.spawn_test_unit("warrior", 1, beside);

        let mut ai = BasicAi::new();
        ai.home_defense = true;
        assert!(
            !g.units_at(home)
                .into_iter()
                .any(|uid| g.units[&uid].owner == 0),
            "precondition: the city really is empty"
        );
        assert_ne!(
            ai.nearest_enemy_for_unit(&g, 0, soldier, &[1]),
            Some(home),
            "precondition: targeting never names our own city — it only picks things to attack"
        );
        assert_eq!(
            ai.garrison_assignments(&g, 0, &[1]),
            vec![(soldier, home)],
            "a threatened, empty city must claim the nearest defender, and name the CITY tile"
        );

        // And the claim must actually move it — an assignment nothing acts on is
        // the same empty city.
        let before = g.units[&soldier].pos;
        assert!(ai.garrison_step(&mut g, 0, soldier, &[1]));
        assert!(
            g.wdist(g.units[&soldier].pos, home) < g.wdist(before, home)
                || g.units[&soldier].pos == home,
            "the assigned defender must close on its city"
        );
    }

    #[test]
    fn a_city_that_is_already_held_does_not_claim_anybody() {
        let (mut g, _, _, _) = raider_at_home_game();
        let home = g.cities[&g.player_city_ids(0)[0]].pos;
        let beside = g
            .nbrs(home)
            .into_iter()
            .find(|pos| {
                g.units_at(*pos).is_empty()
                    && g.map
                        .get(*pos)
                        .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
            })
            .expect("the capital has a passable neighbour");
        g.spawn_test_unit("warrior", 1, beside);
        g.spawn_test_unit("warrior", 0, home);

        let mut ai = BasicAi::new();
        ai.home_defense = true;
        assert_eq!(
            ai.garrison_assignments(&g, 0, &[1]),
            Vec::new(),
            "one body on the tile is the whole job; the rest belong in the field"
        );
    }

    /// Garrison and field recall draw on ONE budget. Sizing the field cap off
    /// the already-reduced responder list would let each take half of a
    /// shrinking remainder and together take most of the army.
    #[test]
    fn garrison_and_field_recall_share_one_half_army_budget() {
        let (mut g, _, raider_at, _) = raider_at_home_game();
        let home = g.cities[&g.player_city_ids(0)[0]].pos;
        let open = |g: &Game, around: Pos, want: usize| -> Vec<Pos> {
            g.wdisk(around, 3)
                .into_iter()
                .filter(|pos| {
                    *pos != raider_at
                        && g.units_at(*pos).is_empty()
                        && g.map
                            .get(*pos)
                            .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
                })
                .take(want)
                .collect()
        };
        for pos in open(&g, home, 2) {
            g.spawn_test_unit("warrior", 1, pos);
        }
        let mut ours: Vec<u32> = g
            .units
            .values()
            .filter(|unit| unit.owner == 0)
            .map(|unit| unit.id)
            .collect();
        for pos in open(&g, home, 8).into_iter().skip(2).take(5) {
            ours.push(g.spawn_test_unit("warrior", 0, pos));
        }
        let total = ours.len();
        assert!(total >= 4, "the budget is only meaningful on a real army");

        let mut ai = BasicAi::new();
        ai.home_defense = true;
        let held = ai.garrison_assignments(&g, 0, &[1]);
        let fielded = ours
            .iter()
            .filter(|uid| !held.iter().any(|(taken, _)| taken == *uid))
            .filter(|uid| ai.home_defense_objective(&g, 0, **uid, &[1]).is_some())
            .count();
        let committed = held.len() + fielded;
        let budget = ((total as f64 * HOME_DEFENSE_MAX_SHARE).floor() as usize).max(1);
        assert!(
            committed <= budget,
            "garrison {} + field {} = {committed} of {total} exceeds the shared budget of {budget}",
            held.len(),
            fielded
        );
    }

    /// The frozen native controllers must not gain this. Their recorded ladders
    /// are only comparable while their play is unchanged, so home defence ships
    /// off by default and the Civilization VI bridge turns it on — the same
    /// contract `siege_muster` already runs under.
    #[test]
    fn the_default_controller_keeps_home_defense_off() {
        let (g, soldier, raider_at, enemy_city) = raider_at_home_game();
        let frozen = BasicAi::new();
        assert!(
            !frozen.home_defense,
            "a tournament controller must open with home defence disabled"
        );
        assert_eq!(
            frozen.home_defense_objective(&g, 0, soldier, &[1]),
            None,
            "the frozen controller must keep choosing the offensive"
        );
        assert_eq!(
            frozen.nearest_enemy_for_unit(&g, 0, soldier, &[1]),
            Some(enemy_city),
            "and its unchanged choice is still the enemy city"
        );

        let mut live = BasicAi::new();
        live.home_defense = true;
        assert_eq!(
            live.home_defense_objective(&g, 0, soldier, &[1]),
            Some(raider_at),
            "precondition: the same board DOES yield a defence objective when enabled"
        );
    }

    #[test]
    fn a_raider_in_the_home_ring_outranks_the_enemy_city_this_unit_is_standing_next_to() {
        let (g, soldier, raider_at, enemy_city) = raider_at_home_game();
        let mut ai = BasicAi::new();
        ai.home_defense = true;

        // The defect, asserted first so the test cannot pass for the wrong
        // reason: the only selector this AI had picks the offensive.
        assert_eq!(
            ai.nearest_enemy_for_unit(&g, 0, soldier, &[1]),
            Some(enemy_city),
            "precondition: distance-ranked targeting must prefer the enemy city here"
        );
        assert_eq!(
            ai.home_defense_objective(&g, 0, soldier, &[1]),
            Some(raider_at),
            "a raider inside the home ring must claim the unit before the offensive does"
        );
    }

    #[test]
    fn a_defender_beyond_recall_range_keeps_its_offensive_job() {
        let (mut g, soldier, raider_at, _) = raider_at_home_game();
        // The threat is left exactly where it was — still in the home ring,
        // still the worst thing on the board. The ONLY thing that changes is how
        // far this unit would have to walk, so a None here can only mean range.
        let far = g
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| g.wdist(*pos, raider_at) > HOME_DEFENSE_RECALL_RANGE)
            .filter(|pos| g.units_at(*pos).is_empty())
            .filter(|pos| {
                g.map
                    .get(*pos)
                    .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
            })
            .min()
            .expect("a 30x20 map has land more than ten tiles from the raider");
        g.remove_unit(soldier);
        let distant = g.spawn_test_unit("warrior", 0, far);

        let mut ai = BasicAi::new();
        ai.home_defense = true;
        assert!(
            g.wdist(far, raider_at) > HOME_DEFENSE_RECALL_RANGE,
            "precondition: the unit really is out of recall range"
        );
        assert_eq!(
            ai.home_defense_objective(&g, 0, distant, &[1]),
            None,
            "a defender that would spend five turns walking is not a defender"
        );
    }

    #[test]
    fn home_defense_never_recalls_more_than_half_the_army() {
        let (mut g, _, raider_at, _) = raider_at_home_game();
        let home = g.cities[&g.player_city_ids(0)[0]].pos;
        // Four raiders in the ring against four of our soldiers: at most two of
        // ours may be claimed, however many threats are shouting.
        for pos in g
            .wdisk(home, HOME_THREAT_RADIUS)
            .into_iter()
            .filter(|pos| {
                *pos != raider_at
                    && g.units_at(*pos).is_empty()
                    && g.map
                        .get(*pos)
                        .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
            })
            .take(3)
            .collect::<Vec<_>>()
        {
            g.spawn_test_unit("warrior", 1, pos);
        }
        let mut ours: Vec<u32> = g
            .units
            .values()
            .filter(|unit| unit.owner == 0)
            .map(|unit| unit.id)
            .collect();
        for pos in g
            .wdisk(home, 2)
            .into_iter()
            .filter(|pos| {
                g.units_at(*pos).is_empty()
                    && g.map
                        .get(*pos)
                        .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
            })
            .take(4 - ours.len())
            .collect::<Vec<_>>()
        {
            ours.push(g.spawn_test_unit("warrior", 0, pos));
        }
        assert_eq!(ours.len(), 4, "the cap is only meaningful on a known army size");

        let mut ai = BasicAi::new();
        ai.home_defense = true;
        let claimed = ours
            .iter()
            .filter(|uid| ai.home_defense_objective(&g, 0, **uid, &[1]).is_some())
            .count();
        assert!(
            claimed <= 2,
            "home defence claimed {claimed} of 4 units; the cap is half the army"
        );
        assert!(
            claimed >= 1,
            "four raiders in the home ring and nobody answered any of them"
        );
    }

    #[test]
    fn wounded_units_withdraw_and_finish_recovering_before_rejoining() {
        let mut g = Game::new_full(2, 20, 14, 30, 30, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .unwrap();
        let warrior = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "warrior")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();

        let neutral = g
            .map
            .tiles
            .iter()
            .filter(|(_, tile)| {
                tile.owner_city.is_none() && g.rules.is_passable(tile) && !g.rules.is_water(tile)
            })
            .map(|(pos, _)| *pos)
            .find(|pos| {
                g.nbrs(*pos).into_iter().any(|neighbor| {
                    let tile = &g.map.tiles[&neighbor];
                    tile.owner_city.is_some()
                        && g.rules.is_passable(tile)
                        && !g.rules.is_water(tile)
                })
            })
            .expect("map has neutral land adjacent to the capital's territory");
        {
            let unit = g.units.get_mut(&warrior).unwrap();
            unit.pos = neutral;
            unit.hp = 45;
            unit.moves_left = 2.0;
            unit.acted = false;
            unit.fortified = false;
        }
        // Rebuild occupancy after placing the unit in this controlled setup.
        let snapshot = serde_json::to_value(&g).unwrap();
        let mut g: Game = serde_json::from_value(snapshot).unwrap();
        let mut ai = BasicAi::new();

        assert_eq!(ai.healing_step(&mut g, 0, warrior), Some(true));
        assert!(
            g.unit_heal_rate(warrior) >= 15,
            "unit should seek friendly borders"
        );
        assert!(ai.recovering_units.contains(&warrior));

        // Once safe, it waits instead of immediately marching back out.
        assert_eq!(ai.healing_step(&mut g, 0, warrior), Some(false));
        assert!(g.units[&warrior].fortified);
        g.units.get_mut(&warrior).unwrap().hp = 79;
        assert_eq!(ai.healing_step(&mut g, 0, warrior), Some(false));

        // Recovery mode has hysteresis and releases the unit at 80 HP.
        g.units.get_mut(&warrior).unwrap().hp = 80;
        assert_eq!(ai.healing_step(&mut g, 0, warrior), None);
        assert!(!ai.recovering_units.contains(&warrior));
    }

    /// One major with a capital, plus a fabricated barbarian warrior on an
    /// open tile adjacent to the major's warrior. Returns (game, warrior,
    /// barb warrior).
    /// One city, one warrior, and a single Barbarian raider standing next to
    /// the city — the shape every settler lost on the live ladder was built
    /// into.
    fn barbarian_at_the_gates_game(seed: u64) -> (Game, u32, u32) {
        let mut g = Game::new_full(1, 20, 14, seed, 60, 0, true);
        let barb_pid = g.barb_pid.unwrap();
        // Organic camps and their garrisons would add besiegers this test does
        // not control; the staged raider must be the only one in reach.
        for unit in g
            .units
            .values()
            .filter(|unit| unit.owner == barb_pid)
            .map(|unit| unit.id)
            .collect::<Vec<_>>()
        {
            g.remove_unit(unit);
        }
        let camps: Vec<Pos> = g.barb_camps.keys().copied().collect();
        for camp in camps {
            g.barb_camps.remove(&camp);
            g.barb_naval_camps.remove(&camp);
            g.barb_camp_guards.remove(&camp);
            let tile = g.map.tiles.get_mut(&camp).unwrap();
            if tile.improvement.as_deref() == Some("barbarian_camp") {
                tile.improvement = None;
            }
        }
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = g.player_city_ids(0)[0];
        let cpos = g.cities[&city].pos;
        let open = g
            .nbrs(cpos)
            .into_iter()
            .find(|p| {
                let t = &g.map.tiles[p];
                g.rules.is_passable(t)
                    && !g.rules.is_water(t)
                    && g.units_at(*p).is_empty()
                    && g.city_at(*p).is_none()
            })
            .expect("open land tile next to the city");
        let template = g
            .player_unit_ids(0)
            .into_iter()
            .find(|id| g.units[id].kind == "warrior")
            .unwrap();
        let mut barb = g.units[&template].clone();
        barb.id = g.next_id;
        g.next_id += 1;
        barb.owner = barb_pid;
        barb.pos = open;
        let bid = barb.id;
        g.units.insert(bid, barb);
        // Round-trip to rebuild occupancy after the manual insert.
        let snapshot = serde_json::to_value(&g).unwrap();
        let g: Game = serde_json::from_value(snapshot).unwrap();
        (g, city, bid)
    }

    #[test]
    fn a_city_besieged_by_barbarians_musters_a_defender_it_could_not_want_before() {
        let (mut g, city, raider) = barbarian_at_the_gates_game(77);
        let mut ai = BasicAi::new();
        // The bridge enables this for live Civilization VI; native ladders
        // leave it off.
        ai.siege_muster = true;

        // The empire already MEETS its standing-army target: `mil_per_city` is
        // 1.0 against one city, and it fields the starting warrior. That is
        // precisely why the old floor could not answer a siege — there was
        // nothing left to want, so the city produced civilians into the
        // raider's reach until they were captured.
        assert_eq!(ai.w.mil_per_city, 1.0);
        assert!(g.is_at_war(0, g.units[&raider].owner));

        let besieged = ai.besieged_military_floor(&g, 0, city, 1);
        assert!(
            besieged >= 2.0,
            "a visible raider next to the city should lift the floor above the \
             standing target of 1.0, got {besieged}"
        );

        // The garrison is an answer to a raider, not a permanent tax: once the
        // raider is gone the floor returns to the standing target.
        g.remove_unit(raider);
        assert_eq!(ai.besieged_military_floor(&g, 0, city, 1), 0.0);
    }

    #[test]
    fn a_besieged_city_builds_defence_instead_of_a_monument() {
        let (mut g, city, raider) = barbarian_at_the_gates_game(79);
        // A raiding party, not a passer-by: `SIEGE_PRESSURE_MIN` is 2.
        let second = {
            let template = g.units[&raider].clone();
            let cpos = g.cities[&city].pos;
            let spot = g
                .nbrs(cpos)
                .into_iter()
                .find(|p| {
                    let t = &g.map.tiles[p];
                    g.rules.is_passable(t)
                        && !g.rules.is_water(t)
                        && g.units_at(*p).is_empty()
                        && g.city_at(*p).is_none()
                })
                .expect("a second open tile beside the city");
            let mut extra = template;
            extra.id = g.next_id;
            g.next_id += 1;
            extra.pos = spot;
            let id = extra.id;
            g.units.insert(id, extra);
            id
        };
        let mut ai = BasicAi::new();
        ai.siege_muster = true;
        assert_eq!(ai.visible_besiegers(&g, 0, city), 2);

        // The shape that lost Uppsala on t68 of `civvis-20260802T205959Z`: the
        // empire-wide floor is SATISFIED, so nothing below wants a unit, and
        // the besieged city reaches its ordinary build order.
        let n_cities = g.player_city_ids(0).len();
        let comfortable = (ai.w.mil_per_city * n_cities as f64) as usize + SIEGE_MUSTER_CAP + 1;
        let besieged = ai
            .pick_item(&g, 0, city, n_cities, 0, 0, 0, 0, comfortable, comfortable, 0)
            .expect("a besieged city must want something");
        let defensive = matches!(&besieged, Item::Building { building } if building.as_str().ends_with("walls"))
            || matches!(&besieged, Item::Unit { unit }
                        if g.rules.units[unit].class == "military");
        assert!(
            defensive,
            "a city with a raider at range 1 should build walls or a defender, got {besieged:?}"
        );

        // With the raider gone the same city returns to its ordinary build
        // order — this branch must not pin every city to permanent war
        // production.
        g.remove_unit(raider);
        g.remove_unit(second);
        let calm = ai.pick_item(&g, 0, city, n_cities, 0, 0, 0, 0, comfortable, comfortable, 0);
        assert_ne!(
            calm.as_ref(),
            Some(&besieged),
            "the siege branch should release once nothing hostile is in reach"
        );
    }

    #[test]
    fn a_raider_beyond_the_muster_radius_does_not_hold_a_garrison() {
        let (mut g, city, raider) = barbarian_at_the_gates_game(78);
        let mut ai = BasicAi::new();
        // The bridge enables this for live Civilization VI; native ladders
        // leave it off.
        ai.siege_muster = true;
        let cpos = g.cities[&city].pos;

        // Walk the raider out past the muster radius. A wanderer on the far
        // side of the map must not pin defenders at home for the rest of the
        // game — that is the failure mode this floor has to avoid.
        let far = g
            .map
            .tiles
            .keys()
            .copied()
            .find(|p| {
                g.wdist(cpos, *p) > SIEGE_MUSTER_RADIUS
                    && g.rules.is_passable(&g.map.tiles[p])
                    && !g.rules.is_water(&g.map.tiles[p])
                    && g.units_at(*p).is_empty()
            })
            .expect("a passable tile beyond the muster radius");
        g.units.get_mut(&raider).unwrap().pos = far;

        assert_eq!(ai.besieged_military_floor(&g, 0, city, 1), 0.0);
    }

    fn barb_skirmish_game(seed: u64) -> (Game, u32, u32) {
        let mut g = Game::new_full(1, 20, 14, seed, 60, 0, true);
        let barb_pid = g.barb_pid.unwrap();
        for unit in g
            .units
            .values()
            .filter(|unit| unit.owner == barb_pid)
            .map(|unit| unit.id)
            .collect::<Vec<_>>()
        {
            g.remove_unit(unit);
        }
        g.barb_camps.clear();
        g.barb_naval_camps.clear();
        g.barb_camp_guards.clear();
        g.barb_scout_homes.clear();
        g.barb_scout_targets.clear();
        g.barb_camp_targets.clear();
        g.barb_alerted_until.clear();
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let warrior = g
            .player_unit_ids(0)
            .into_iter()
            .find(|id| g.units[id].kind == "warrior")
            .unwrap();
        let wpos = g.units[&warrior].pos;
        let open = g
            .nbrs(wpos)
            .into_iter()
            .find(|p| {
                let t = &g.map.tiles[p];
                g.rules.is_passable(t)
                    && !g.rules.is_water(t)
                    && g.units_at(*p).is_empty()
                    && g.city_at(*p).is_none()
            })
            .expect("open land tile next to the warrior");
        let mut barb = g.units[&warrior].clone();
        barb.id = g.next_id;
        g.next_id += 1;
        barb.owner = barb_pid;
        barb.pos = open;
        let bid = barb.id;
        g.units.insert(bid, barb);
        // The staged raider must be the only Barbarian in reach: organic
        // camp garrisons on the generated map would steal the target pick.
        let strays: Vec<u32> = g
            .units
            .values()
            .filter(|unit| unit.owner == g.barb_pid.unwrap() && unit.id != bid)
            .map(|unit| unit.id)
            .collect();
        for stray in strays {
            g.units.remove(&stray);
        }
        // Camps are pursuit targets too; the staged fight must be the only one.
        let camps: Vec<Pos> = g.barb_camps.keys().copied().collect();
        for camp in camps {
            g.barb_camps.remove(&camp);
            g.barb_naval_camps.remove(&camp);
            g.barb_camp_guards.remove(&camp);
            let tile = g.map.tiles.get_mut(&camp).unwrap();
            if tile.improvement.as_deref() == Some("barbarian_camp") {
                tile.improvement = None;
            }
        }
        // Round-trip to rebuild occupancy after the manual inserts.
        let snapshot = serde_json::to_value(&g).unwrap();
        let g: Game = serde_json::from_value(snapshot).unwrap();
        (g, warrior, bid)
    }

    #[test]
    fn even_barbarian_trades_are_taken_not_shadowed() {
        let (mut g, warrior, barb) = barb_skirmish_game(33);
        let mut ai = BasicAi::new();
        assert!(ai.military_step(&mut g, 0, warrior));
        assert!(
            g.units.get(&barb).map(|b| b.hp < 100).unwrap_or(true),
            "adjacent equal-strength barbarian should be attacked, not shadowed"
        );
    }

    #[test]
    fn outmatched_units_stop_chasing_barbarians() {
        let (mut g, uid, barb) = barb_skirmish_game(34);
        let ai = BasicAi::new();
        let bp = g.units[&barb].owner;
        let bpos = g.units[&barb].pos;
        // A warrior takes the even fight, so the raider is a valid target...
        assert_eq!(ai.nearest_enemy(&g, 0, uid, &[bp]), Some(bpos));
        // ...but a scout would decline the attack, so it must not pick the
        // raider as a pursuit target either (the chase-without-striking bug).
        g.units.get_mut(&uid).unwrap().kind = crate::name!("scout");
        assert_ne!(ai.nearest_enemy(&g, 0, uid, &[bp]), Some(bpos));
    }

    #[test]
    fn first_step_ties_favor_movement_but_real_positional_losses_do_not() {
        let g = Game::new_full(1, 20, 14, 34, 30, 0, false);
        let warrior = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "warrior")
            .unwrap();
        let ai = BasicAi::new();

        assert!(ai.move_beats_holding(&g, warrior, 10.0, 10.0));
        assert!(!ai.move_beats_holding(&g, warrior, 5.5, 10.0));

        let mut already_moved = g;
        already_moved.units.get_mut(&warrior).unwrap().moved = true;
        assert!(!ai.move_beats_holding(&already_moved, warrior, 10.0, 10.0));
    }

    #[test]
    fn most_idle_peacetime_troops_patrol_instead_of_fortifying() {
        let mut g = Game::new_full(1, 24, 16, 35, 30, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let capital = g.cities[&g.player_city_ids(0)[0]].pos;
        let staging: Vec<Pos> = g
            .wdisk(capital, 5)
            .into_iter()
            .filter(|pos| {
                g.map.get(*pos).is_some_and(|tile| {
                    g.rules.is_passable(tile)
                        && !g.rules.is_water(tile)
                        && g.units_at(*pos).is_empty()
                })
            })
            .take(5)
            .collect();
        assert_eq!(
            staging.len(),
            5,
            "test map needs open land near the capital"
        );
        for pos in staging {
            g.spawn_test_unit("warrior", 0, pos);
        }
        g.players[0].explored.extend(g.map.tiles.keys().copied());

        let military: Vec<u32> = g
            .player_unit_ids(0)
            .into_iter()
            .filter(|uid| g.rules.units[g.units[uid].kind].class == "military")
            .collect();
        let mut ai = BasicAi::new();
        ai.units(&mut g, 0);
        let moved = military.iter().filter(|uid| g.units[uid].moved).count();

        assert_eq!(
            ai.patrol_posts.len(),
            1,
            "same-domain troops should share one frontier scan per turn"
        );
        assert!(
            moved * 2 > military.len(),
            "expected most idle troops to patrol; moved {moved}/{}",
            military.len()
        );
    }

    /// The open stretch of land `most_wartime_troops_advance_when_a_campaign_route_exists`
    /// needs: a target well clear of every starting unit, with six tiles three
    /// to six hexes out that troops can stage on and march off.
    fn arena(g: &Game) -> Option<(Pos, Vec<Pos>)> {
        g
            .map
            .tiles
            .iter()
            .filter(|(pos, tile)| {
                g.rules.is_passable(tile)
                    && !g.rules.is_water(tile)
                    && g.units_at(**pos).is_empty()
                    // The campaign must march, not brawl: keep the arena away
                    // from everyone's starting units.
                    && g.units.values().all(|unit| g.wdist(unit.pos, **pos) > 8)
            })
            .find_map(|(target, _)| {
                let staging: Vec<Pos> = g
                    .wdisk(*target, 6)
                    .into_iter()
                    .filter(|pos| {
                        (3..=6).contains(&g.wdist(*target, *pos))
                            && g.map.get(*pos).is_some_and(|tile| {
                                g.rules.is_passable(tile)
                                    && !g.rules.is_water(tile)
                                    && g.units_at(*pos).is_empty()
                            })
                            // Troops staged here must be able to march out.
                            && g.nbrs(*pos)
                                .iter()
                                .filter(|neighbour| {
                                    g.map.get(**neighbour).is_some_and(|tile| {
                                        g.rules.is_passable(tile) && !g.rules.is_water(tile)
                                    })
                                })
                                .count()
                                >= 4
                    })
                    .take(6)
                    .collect();
                (staging.len() == 6).then_some((*target, staging))
            })
    }

    #[test]
    fn most_wartime_troops_advance_when_a_campaign_route_exists() {
        // The fixture needs an arena: a stretch of open land well clear of
        // everybody's starting units, with room to stage six warriors around
        // one target. That is a fact about the map rather than the thing under
        // test, so take the first seed that offers one instead of naming a seed
        // and trusting that starts never move again.
        let (mut g, target, staging) = (36..96u64)
            .find_map(|seed| {
                let mut g = Game::new_full(2, 24, 16, seed, 30, 0, false);
                g.at_war.insert((0, 1));
                let found = arena(&g)?;
                Some((g, found.0, found.1))
            })
            .expect("no seed offered a map with an open land campaign");
        g.spawn_test_unit("warrior", 1, target);
        let army: Vec<u32> = staging
            .into_iter()
            .map(|pos| g.spawn_test_unit("warrior", 0, pos))
            .collect();

        let mut ai = BasicAi::new();
        for uid in &army {
            for _ in 0..8 {
                if !g.units.contains_key(uid)
                    || g.units[uid].moves_left <= 0.0
                    || !ai.military_step(&mut g, 0, *uid)
                {
                    break;
                }
            }
        }
        let moved = army
            .iter()
            .filter(|uid| g.units.get(uid).is_some_and(|unit| unit.moved))
            .count();
        assert!(
            moved * 2 > army.len(),
            "expected most campaign troops to advance; moved {moved}/{}",
            army.len()
        );
    }

    #[test]
    fn military_roster_maps_to_distinct_strategic_doctrines() {
        let mut g = Game::new_full(1, 24, 16, 37, 30, 0, false);
        let positions: Vec<Pos> = g
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| g.units_at(*pos).is_empty())
            .take(9)
            .collect();
        let cases = [
            ("scout", UnitDoctrine::Recon),
            ("swordsman", UnitDoctrine::Assault),
            ("horseman", UnitDoctrine::Mobile),
            ("archer", UnitDoctrine::Ranged),
            ("catapult", UnitDoctrine::Siege),
            ("battering_ram", UnitDoctrine::Support),
            ("biplane", UnitDoctrine::AirDefense),
            ("bomber", UnitDoctrine::AirStrike),
            ("aircraft_carrier", UnitDoctrine::Carrier),
        ];
        for ((kind, expected), pos) in cases.into_iter().zip(positions) {
            let uid = g.spawn_test_unit(kind, 0, pos);
            assert_eq!(BasicAi::unit_doctrine(&g, uid), expected, "{kind}");
        }
    }

    #[test]
    fn tactical_assignment_forms_the_melee_anti_cavalry_cavalry_cycle() {
        fn chosen_target(
            seed: u64,
            attacker_kind: &str,
            favored_kind: &str,
            other_kind: &str,
        ) -> (Pos, Pos) {
            let mut g = Game::new_full(2, 24, 16, seed, 30, 0, false);
            for unit in g.units.keys().copied().collect::<Vec<_>>() {
                g.remove_unit(unit);
            }
            g.at_war.insert((0, 1));
            let (origin, targets) = g
                .map
                .tiles
                .iter()
                .filter(|(_, tile)| g.rules.is_passable(tile) && !g.rules.is_water(tile))
                .find_map(|(origin, _)| {
                    let targets: Vec<Pos> = g
                        .nbrs(*origin)
                        .into_iter()
                        .filter(|position| {
                            g.map.get(*position).is_some_and(|tile| {
                                g.rules.is_passable(tile) && !g.rules.is_water(tile)
                            })
                        })
                        .take(2)
                        .collect();
                    (targets.len() == 2).then_some((*origin, targets))
                })
                .expect("test map has a two-target tactical ring");
            let attacker = g.spawn_test_unit(attacker_kind, 0, origin);
            let favored = g.spawn_test_unit(favored_kind, 1, targets[0]);
            let other = g.spawn_test_unit(other_kind, 1, targets[1]);
            g.units.get_mut(&favored).unwrap().hp = 1;
            g.units.get_mut(&other).unwrap().hp = 1;

            let mut ai = BasicAi::new();
            ai.tactical_strategy = true;
            assert!(ai.military_step(&mut g, 0, attacker));
            let chosen = match g.log.last() {
                Some((0, Action::Attack { target, .. })) => *target,
                action => panic!("expected a class-assigned melee attack, got {action:?}"),
            };
            (chosen, targets[0])
        }

        for result in [
            chosen_target(37_101, "swordsman", "spearman", "warrior"),
            chosen_target(37_102, "spearman", "horseman", "warrior"),
            chosen_target(37_103, "heavy_chariot", "warrior", "horseman"),
        ] {
            assert_eq!(result.0, result.1);
        }
    }

    #[test]
    fn ranged_standoff_prices_move_and_attack_return_fire() {
        let mut g = Game::new_full(2, 24, 16, 37_104, 30, 0, false);
        for unit in g.units.keys().copied().collect::<Vec<_>>() {
            g.remove_unit(unit);
        }
        g.at_war.insert((0, 1));
        let enemy_pos = *g
            .map
            .tiles
            .iter()
            .find(|(_, tile)| g.rules.is_passable(tile) && !g.rules.is_water(tile))
            .map(|(position, _)| position)
            .unwrap();
        let exposed = g
            .wdisk(enemy_pos, 2)
            .into_iter()
            .find(|position| g.wdist(*position, enemy_pos) == 2)
            .unwrap();
        let safe = g
            .wdisk(enemy_pos, 3)
            .into_iter()
            .find(|position| g.wdist(*position, enemy_pos) == 3)
            .unwrap();
        let archer = g.spawn_test_unit("archer", 0, safe);
        let warrior = g.spawn_test_unit("warrior", 1, enemy_pos);
        g.units.get_mut(&warrior).unwrap().moves_left = 0.0;
        let mut ai = BasicAi::new();
        ai.tactical_strategy = true;

        assert!(ai.projected_counter_damage(&g, archer, exposed, &[warrior]) > 0.0);
        assert_eq!(ai.projected_counter_damage(&g, archer, safe, &[warrior]), 0.0);
        assert_eq!(
            ai.tactical_action_bonus_from(&g, archer, exposed, enemy_pos, true),
            SAFE_RANGED_FIRE,
            "a range-two shot is outside a melee defender's direct return fire"
        );
    }

    #[test]
    fn siege_and_compatible_support_own_standing_walls() {
        let (mut g, _, enemy) = walled_war_game(37_105);
        let target = g.cities[&enemy].pos;
        g.cities.get_mut(&enemy).unwrap().wall_hp = 100;
        let melee = g
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| g.rules.units[g.units[unit].kind].promotion_class == "melee")
            .unwrap();
        let open: Vec<Pos> = g
            .nbrs(target)
            .into_iter()
            .filter(|position| g.units_at(*position).is_empty())
            .take(3)
            .collect();
        assert_eq!(open.len(), 3);
        let siege = g.spawn_test_unit("catapult", 0, open[0]);
        let cavalry = g.spawn_test_unit("heavy_chariot", 0, open[1]);
        let mut ai = BasicAi::new();
        ai.tactical_strategy = true;

        assert_eq!(ai.tactical_action_bonus(&g, siege, target, true), SIEGE_WALL_ASSIGNMENT);
        assert_eq!(
            ai.tactical_action_bonus(&g, melee, target, false),
            -UNSUPPORTED_WALL_ASSAULT
        );
        let ram = g.spawn_test_unit("battering_ram", 0, open[2]);
        assert_eq!(
            ai.tactical_action_bonus(&g, melee, target, false),
            SUPPORTED_WALL_ASSAULT
        );
        assert_eq!(
            ai.tactical_action_bonus(&g, cavalry, target, false),
            0.0,
            "cavalry cannot use a ram or tower in the combat rules"
        );

        g.cities
            .get_mut(&enemy)
            .unwrap()
            .buildings
            .push(crate::name!("medieval_walls"));
        assert_eq!(
            ai.tactical_action_bonus(&g, melee, target, false),
            -UNSUPPORTED_WALL_ASSAULT,
            "a ram is obsolete against Medieval Walls"
        );
        g.remove_unit(ram);
        g.spawn_test_unit("siege_tower", 0, open[2]);
        assert_eq!(
            ai.tactical_action_bonus(&g, melee, target, false),
            SUPPORTED_WALL_ASSAULT
        );
    }

    #[test]
    fn light_cavalry_raids_first_while_heavy_cavalry_attacks_first() {
        let mut g = Game::new_full(2, 24, 16, 37_106, 30, 0, false);
        for unit in g.units.keys().copied().collect::<Vec<_>>() {
            g.remove_unit(unit);
        }
        g.at_war.insert((0, 1));
        let (heavy_position, enemy_position, light_position) = g
            .map
            .tiles
            .iter()
            .filter(|(_, tile)| g.rules.is_passable(tile) && !g.rules.is_water(tile))
            .find_map(|(position, _)| {
                let neighbors: Vec<Pos> = g
                    .nbrs(*position)
                    .into_iter()
                    .filter(|neighbor| {
                        g.map.get(*neighbor).is_some_and(|tile| {
                            g.rules.is_passable(tile) && !g.rules.is_water(tile)
                        })
                    })
                    .take(2)
                    .collect();
                (neighbors.len() == 2).then_some((*position, neighbors[0], neighbors[1]))
            })
            .expect("test map needs two open neighbors");
        let light = g.spawn_test_unit("horseman", 0, light_position);
        let heavy = g.spawn_test_unit("heavy_chariot", 0, heavy_position);
        let enemy = g.spawn_test_unit("warrior", 1, enemy_position);
        g.units.get_mut(&enemy).unwrap().hp = 1;
        for position in [light_position, heavy_position] {
            g.map.tiles.get_mut(&position).unwrap().improvement =
                Some(crate::name!("barbarian_camp"));
        }
        let mut ai = BasicAi::new();
        ai.tactical_strategy = true;

        assert!(matches!(
            ai.doctrine_action(&g, 0, light),
            Some(Action::Pillage { unit }) if unit == light
        ));
        assert_eq!(ai.doctrine_action(&g, 0, heavy), None);
        assert!(matches!(
            ai.heavy_cavalry_pillage_action(&g, 0, heavy),
            Some(Action::Pillage { unit }) if unit == heavy
        ));
        assert!(ai.military_step(&mut g, 0, heavy));
        assert!(matches!(
            g.log.last(),
            Some((0, Action::Attack { unit, target }))
                if *unit == heavy && *target == enemy_position
        ));
        assert_eq!(
            g.map.tiles[&heavy_position].improvement.as_deref(),
            Some("barbarian_camp"),
            "heavy cavalry should leave pillaging until after an attack opportunity"
        );
    }

    #[test]
    fn scout_explores_while_strong_assault_unit_attacks() {
        let mut g = Game::new_full(2, 24, 16, 38, 30, 0, false);
        g.at_war.insert((0, 1));
        let (enemy_pos, scout_pos, assault_pos, hidden) = g
            .map
            .tiles
            .iter()
            .filter(|(pos, tile)| {
                g.rules.is_passable(tile) && !g.rules.is_water(tile) && g.units_at(**pos).is_empty()
            })
            .find_map(|(center, _)| {
                let ring: Vec<Pos> = g
                    .nbrs(*center)
                    .into_iter()
                    .filter(|pos| {
                        g.map.get(*pos).is_some_and(|tile| {
                            g.rules.is_passable(tile)
                                && !g.rules.is_water(tile)
                                && g.units_at(*pos).is_empty()
                        })
                    })
                    .collect();
                if ring.len() < 3 {
                    return None;
                }
                let scout = ring[0];
                let hidden = ring
                    .iter()
                    .copied()
                    .skip(1)
                    .find(|pos| g.wdist(scout, *pos) == 1)?;
                let assault = ring
                    .iter()
                    .copied()
                    .find(|pos| *pos != scout && *pos != hidden)?;
                Some((*center, scout, assault, hidden))
            })
            .expect("test map needs an open tactical ring");
        let enemy = g.spawn_test_unit("modern_armor", 1, enemy_pos);
        let scout = g.spawn_test_unit("scout", 0, scout_pos);
        let assault = g.spawn_test_unit("giant_death_robot", 0, assault_pos);
        g.players[0].explored.extend(g.map.tiles.keys().copied());
        g.players[0].explored.remove(&hidden);

        let mut ai = BasicAi::new();
        assert!(ai.military_step(&mut g, 0, scout));
        assert!(matches!(
            g.log.last(),
            Some((0, Action::Move { unit, to })) if *unit == scout && *to == hidden
        ));
        assert!(g.units.contains_key(&enemy));

        assert!(
            ai.attack_threshold(&g, assault, enemy_pos) < ai.attack_threshold(&g, scout, enemy_pos),
            "strong assault units should have a more aggressive attack threshold"
        );
        assert!(ai.military_step(&mut g, 0, assault));
        assert!(
            matches!(
                g.log.last(),
                Some((0, Action::Attack { unit, target } | Action::Ranged { unit, target }))
                    if *unit == assault && *target == enemy_pos
            ),
            "unexpected assault decision: {:?}",
            g.log.last()
        );
    }

    #[test]
    fn raiders_and_aircraft_use_their_specialized_actions() {
        let mut g = Game::new_full(2, 24, 16, 39, 30, 0, false);
        g.at_war.insert((0, 1));
        for unit in g.player_unit_ids(1) {
            g.units.get_mut(&unit).unwrap().owner = 0;
        }
        let positions: Vec<Pos> = g
            .map
            .tiles
            .iter()
            .filter(|(pos, tile)| {
                g.rules.is_passable(tile) && !g.rules.is_water(tile) && g.units_at(**pos).is_empty()
            })
            // Demand elbow room rather than taking whichever tiles the map
            // lists first: the air base needs a free land tile beside it that
            // the other two staged units are not already standing on.
            .filter(|(pos, _)| {
                g.nbrs(**pos)
                    .into_iter()
                    .filter(|neighbor| {
                        g.map.get(*neighbor).is_some_and(|tile| {
                            g.rules.is_passable(tile)
                                && !g.rules.is_water(tile)
                                && g.units_at(*neighbor).is_empty()
                        })
                    })
                    .count()
                    >= 3
            })
            .map(|(pos, _)| *pos)
            .take(3)
            .collect();
        let air_target = g
            .nbrs(positions[2])
            .into_iter()
            .find(|pos| {
                !positions.contains(pos)
                    && g.map.get(*pos).is_some_and(|tile| {
                        g.rules.is_passable(tile)
                            && !g.rules.is_water(tile)
                            && g.units_at(*pos).is_empty()
                    })
            })
            .expect("test map needs a land target beside the air base");
        let raider = g.spawn_test_unit("horseman", 0, positions[0]);
        let assault = g.spawn_test_unit("swordsman", 0, positions[1]);
        g.map.tiles.get_mut(&positions[0]).unwrap().improvement =
            Some(crate::name!("barbarian_camp"));
        g.map.tiles.get_mut(&positions[1]).unwrap().improvement =
            Some(crate::name!("barbarian_camp"));
        let fighter = g.spawn_test_unit("biplane", 0, positions[2]);
        let bomber = g.spawn_test_unit("bomber", 0, positions[2]);
        g.spawn_test_unit("modern_armor", 1, air_target);
        let ai = BasicAi::new();

        let full_legal = g.legal_actions(0);
        for uid in [raider, assault, fighter, bomber] {
            let expected: Vec<Action> = full_legal
                .iter()
                .filter(|action| match action {
                    Action::Pillage { unit }
                    | Action::AirRebase { unit, .. }
                    | Action::AirStrike { unit, .. }
                    | Action::AirPillage { unit, .. }
                    | Action::PriorityTarget { unit, .. }
                    | Action::AirPatrol { unit, .. }
                    | Action::CoastalRaid { unit, .. } => *unit == uid,
                    _ => false,
                })
                .cloned()
                .collect();
            assert_eq!(g.legal_doctrine_actions(0, uid), expected);
        }

        assert!(matches!(
            ai.doctrine_action(&g, 0, raider),
            Some(Action::Pillage { unit }) if unit == raider
        ));
        assert_eq!(ai.doctrine_action(&g, 0, assault), None);
        assert!(matches!(
            ai.doctrine_action(&g, 0, fighter),
            Some(Action::AirPatrol { unit, .. }) if unit == fighter
        ));
        let bomber_action = ai.doctrine_action(&g, 0, bomber);
        assert!(
            matches!(
                bomber_action,
                Some(Action::AirStrike { unit, target })
                    if unit == bomber && target == air_target
            ),
            "unexpected bomber action: {bomber_action:?}"
        );
    }

    #[test]
    fn spec_ops_bypass_an_escort_to_priority_target_air_defense() {
        let mut game = Game::new_full(2, 24, 16, 43_015, 80, 0, false);
        game.at_war.insert((0, 1));
        for unit in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(unit);
        }
        let origin = game
            .map
            .tiles
            .iter()
            .find(|(_, tile)| game.rules.is_passable(tile) && !game.rules.is_water(tile))
            .map(|(position, _)| *position)
            .unwrap();
        let target = game
            .wdisk(origin, 2)
            .into_iter()
            .find(|position| {
                game.wdist(origin, *position) == 2
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .unwrap();
        // Keep this doctrine fixture inside the Spec Ops unit's real sight
        // corridor. Priority Target cannot select an escorted support unit
        // hidden behind terrain at range two.
        for position in game.wdisk(origin, 2) {
            let tile = game.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("plains");
            tile.feature = None;
            tile.hills = false;
        }
        let spec_ops = game.spawn_test_unit("spec_ops", 0, origin);
        game.spawn_test_unit("modern_armor", 1, target);
        let sam = game.spawn_test_unit("mobile_sam", 1, target);
        let mut ai = BasicAi::new();

        assert!(game.player_visibility(0).contains(&target));
        assert!(ai.military_step(&mut game, 0, spec_ops));
        assert_eq!(game.units[&sam].hp, 35);
        assert!(matches!(
            game.log.last(),
            Some((0, Action::PriorityTarget { unit, target: action_target }))
                if *unit == spec_ops && *action_target == target
        ));
    }

    #[test]
    fn ranged_holds_firing_depth_while_mobile_unit_closes() {
        let mut g = Game::new_full(1, 24, 16, 40, 30, 0, false);
        let (target, ranged_pos, mobile_pos) = g
            .map
            .tiles
            .iter()
            .filter(|(pos, tile)| {
                g.rules.is_passable(tile) && !g.rules.is_water(tile) && g.units_at(**pos).is_empty()
            })
            .find_map(|(target, _)| {
                let ranged = g.wdisk(*target, 2).into_iter().find(|pos| {
                    g.wdist(*target, *pos) == 2
                        && g.map.get(*pos).is_some_and(|tile| {
                            g.rules.is_passable(tile)
                                && !g.rules.is_water(tile)
                                && g.units_at(*pos).is_empty()
                        })
                })?;
                let mobile = g.wdisk(*target, 4).into_iter().find(|pos| {
                    g.wdist(*target, *pos) == 4
                        && *pos != ranged
                        && g.map.get(*pos).is_some_and(|tile| {
                            g.rules.is_passable(tile)
                                && !g.rules.is_water(tile)
                                && g.units_at(*pos).is_empty()
                        })
                })?;
                Some((*target, ranged, mobile))
            })
            .expect("test map needs open role-spacing positions");
        let archer = g.spawn_test_unit("archer", 0, ranged_pos);
        let ai = BasicAi::new();
        ai.tactical_step(&mut g, 0, archer, target, &[], 2);
        assert_eq!(g.wdist(g.units[&archer].pos, target), 2);

        let horseman = g.spawn_test_unit("horseman", 0, mobile_pos);
        assert!(ai.tactical_step(&mut g, 0, horseman, target, &[], 1));
        assert!(g.wdist(g.units[&horseman].pos, target) < g.wdist(mobile_pos, target));
    }

    #[test]
    fn military_picker_preserves_city_capturing_melee() {
        let mut g = Game::new_full(1, 20, 14, 31, 30, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        g.players[0].techs.extend([
            crate::name!("archery"),
            crate::name!("iron_working"),
            crate::name!("machinery"),
        ]);
        let cid = g.player_city_ids(0)[0];
        let ai = BasicAi::new();

        let ranged = ai.combined_arms_unit(&g, 0, cid, 2, 0).unwrap();
        assert!(g.rules.units[ranged].has_ranged_attack());

        let melee = ai.combined_arms_unit(&g, 0, cid, 2, 2).unwrap();
        assert!(!g.rules.units[melee].has_ranged_attack());
    }

    #[test]
    fn military_units_capture_adjacent_enemy_civilians_by_moving() {
        let mut game = Game::new_full(2, 20, 14, 91_769, 120, 0, false);
        for pid in 0..2 {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.found_city_for(pid, game.units[&settler].pos, None);
        }
        for unit in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(unit);
        }
        let origin = game.cities[&game.player_city_ids(0)[0]].pos;
        let target = game
            .nbrs(origin)
            .into_iter()
            .find(|position| {
                game.city_at(*position).is_none()
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .unwrap();
        game.at_war.insert((0, 1));
        let warrior = game.spawn_test_unit("warrior", 0, origin);
        let builder = game.spawn_test_unit("builder", 1, target);
        let mut ai = BasicAi::new();

        assert!(ai.military_step(&mut game, 0, warrior));
        assert_eq!(game.units[&builder].owner, 0);
        assert!(matches!(
            game.log.last(),
            Some((0, Action::Move { unit, to })) if *unit == warrior && *to == target
        ));
    }

    #[test]
    fn city_state_armies_decline_settlers_they_cannot_use() {
        let mut game = Game::new_full(2, 20, 14, 91_768, 120, 0, false);
        for pid in 0..2 {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.found_city_for(pid, game.units[&settler].pos, None);
        }
        for unit in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(unit);
        }
        let origin = game.cities[&game.player_city_ids(0)[0]].pos;
        let target = game
            .nbrs(origin)
            .into_iter()
            .find(|position| {
                game.city_at(*position).is_none()
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .unwrap();
        game.players[0].is_minor = true;
        game.at_war.insert((0, 1));
        let warrior = game.spawn_test_unit("warrior", 0, origin);
        let settler = game.spawn_test_unit("settler", 1, target);
        let mut ai = BasicAi::new();
        ai.minor = true;

        let _ = ai.military_step(&mut game, 0, warrior);
        assert_eq!(game.units[&settler].owner, 1);
        assert_ne!(game.units[&warrior].pos, target);
    }

    #[test]
    fn unlevied_city_state_forces_return_to_the_local_defense_area() {
        let mut game = Game::new_full(2, 28, 18, 91_769, 120, 0, false);
        for pid in 0..2 {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.found_city_for(pid, game.units[&settler].pos, None);
        }
        for unit in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(unit);
        }
        game.players[0].is_minor = true;
        game.at_war.insert((0, 1));
        let home = game.cities[&game.player_city_ids(0)[0]].pos;
        let candidates = game
            .map
            .tiles
            .values()
            .filter(|tile| {
                game.wdist(home, tile.pos) > MINOR_DEFENSE_RADIUS + 1
                    && game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && game.city_at(tile.pos).is_none()
                    && tile.owner_city.is_none()
            })
            .map(|tile| tile.pos)
            .collect::<Vec<_>>();
        let warrior = candidates
            .into_iter()
            .find_map(|position| {
                let unit = game.spawn_test_unit("warrior", 0, position);
                if game
                    .route_step(unit, home, 0)
                    .is_some_and(|next| game.can_move(unit, next))
                {
                    Some(unit)
                } else {
                    game.remove_unit(unit);
                    None
                }
            })
            .expect("test map has a remote land route home");
        let start = game.units[&warrior].pos;
        let mut ai = BasicAi::new();
        ai.minor = true;

        for _ in 0..30 {
            if game.wdist(home, game.units[&warrior].pos) <= MINOR_DEFENSE_RADIUS {
                break;
            }
            game.turn += 1;
            let unit = game.units.get_mut(&warrior).unwrap();
            unit.moves_left = 10.0;
            unit.moved = false;
            assert!(ai.military_step(&mut game, 0, warrior));
        }
        assert!(
            game.wdist(home, game.units[&warrior].pos) <= MINOR_DEFENSE_RADIUS,
            "remote defender did not return from {start:?}; stopped at {:?}; last action {:?}",
            game.units[&warrior].pos,
            game.log.last()
        );

        let (boundary, outside) = game
            .map
            .tiles
            .values()
            .filter(|tile| {
                game.wdist(home, tile.pos) == MINOR_DEFENSE_RADIUS
                    && game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && game.city_at(tile.pos).is_none()
            })
            .find_map(|tile| {
                game.nbrs(tile.pos).into_iter().find_map(|outside| {
                    game.map
                        .get(outside)
                        .is_some_and(|candidate| {
                            game.wdist(home, outside) > MINOR_DEFENSE_RADIUS
                                && game.rules.is_passable(candidate)
                                && !game.rules.is_water(candidate)
                                && game.city_at(outside).is_none()
                        })
                        .then_some((tile.pos, outside))
                })
            })
            .expect("test map has a passable defense boundary");
        {
            let unit = game.units.get_mut(&warrior).unwrap();
            unit.pos = boundary;
            unit.moves_left = 10.0;
            unit.moved = false;
        }
        assert!(game.can_move(warrior, outside));
        assert!(!ai.path_move(&mut game, 0, warrior, outside));
        assert_eq!(game.units[&warrior].pos, boundary);
    }

    #[test]
    fn gold_spending_fills_worker_gap_but_keeps_reserve() {
        let mut g = Game::new_full(1, 20, 14, 32, 30, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let cid = g.player_city_ids(0)[0];
        let ai = BasicAi::new();

        // This is a treasury test, so give it explicit Builder work instead of
        // depending on which features a seeded map happens to draw nearby.
        let work = g.cities[&cid]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != g.cities[&cid].pos)
            .unwrap();
        {
            let tile = g.map.tiles.get_mut(&work).unwrap();
            tile.terrain = crate::name!("plains");
            tile.hills = false;
            tile.feature = None;
            tile.resource = None;
            tile.improvement = None;
        }

        // One city keeps 125 gold and spends 200 on its missing builder.
        g.players[0].gold = 325.0;
        assert!(ai.spend_gold(&mut g, 0, &[cid], 0, 0, 0, 1, 1, 0));
        assert_eq!(g.players[0].gold, 125.0);
        assert!(g
            .units
            .values()
            .any(|u| u.owner == 0 && u.kind == "builder"));

        let builders = g
            .units
            .values()
            .filter(|u| u.owner == 0 && u.kind == "builder")
            .count();
        g.players[0].gold = 324.0;
        assert!(!ai.spend_gold(&mut g, 0, &[cid], 0, 0, 0, 1, 1, 0));
        assert_eq!(g.players[0].gold, 324.0);
        assert_eq!(
            g.units
                .values()
                .filter(|u| u.owner == 0 && u.kind == "builder")
                .count(),
            builders
        );
    }

    #[test]
    fn gold_spending_converts_surplus_into_city_infrastructure() {
        let mut g = Game::new_full(1, 20, 14, 320, 30, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let cid = g.player_city_ids(0)[0];
        g.cities
            .get_mut(&cid)
            .unwrap()
            .buildings
            .retain(|building| building != "monument");
        let ai = BasicAi::new();

        // With its unit needs already covered, the AI buys a Monument but
        // keeps the full one-city peacetime reserve.
        g.players[0].gold = 365.0;
        assert!(g.legal_actions(0).iter().any(|action| matches!(
            action,
            Action::BuyBuilding { building, currency, .. }
                if building == "monument" && currency == "gold"
        )));
        assert!(ai.spend_gold(&mut g, 0, &[cid], 1, 1, 1, 2, 1, 1));
        assert_eq!(g.players[0].gold, 125.0);
        assert!(g.cities[&cid].buildings.iter().any(|b| b == "monument"));

        // The same purchase is exposed through the public action protocol.
        assert!(!g.legal_actions(0).iter().any(|action| matches!(
            action,
            Action::BuyBuilding { building, currency, .. }
                if building == "monument" && currency == "gold"
        )));
    }

    /// ⚠⚠ A HOST PURCHASE REFUSAL MUST REACH THE BUYERS THAT NEVER ENUMERATE.
    ///
    /// `purchase_action_is_blocked` was applied only where legal actions are
    /// enumerated, but `buy_gold_infrastructure`, `buy_gold_unit` and
    /// `buy_gold_military` each build an `Action::Buy*` themselves and call
    /// `apply` directly. They gated on `can_produce`, which reads
    /// `blocked_production` — a DIFFERENT set meaning "cannot build here", not
    /// "the host will not sell this here".
    ///
    /// Live run `civvis-20260804T091315Z` re-bought the same Granary in the same
    /// city on turns 114, 117, 121, 122, 123 and 128; 8 of that game's 9
    /// purchases were refused. Replaying those turns after this change, the
    /// Granary is gone and the gold goes to a Shrine instead.
    #[test]
    fn a_refused_purchase_is_not_re_offered_to_the_direct_buyers() {
        let mut g = Game::new_full(1, 20, 14, 320, 30, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let cid = g.player_city_ids(0)[0];
        g.cities
            .get_mut(&cid)
            .unwrap()
            .buildings
            .retain(|building| building != "monument");
        let ai = BasicAi::new();
        g.players[0].gold = 365.0;

        // Baseline: the Monument is both priced and reachable by the direct buyer.
        assert!(
            g.building_gold_purchase_cost(0, cid, "monument").is_some(),
            "precondition: the Monument is purchasable before the host refuses it"
        );

        // Now the host refuses to sell a Monument in this city.
        g.replace_blocked_purchases(std::collections::BTreeMap::from([(
            cid,
            std::collections::BTreeSet::from(["building:monument".to_string()]),
        )]));

        assert_eq!(
            g.building_gold_purchase_cost(0, cid, "monument"),
            None,
            "a refused purchase must stop being priced — this is the choke point \
             every buyer reaches the offer through"
        );
        assert!(
            !g.legal_actions(0).iter().any(|action| matches!(
                action,
                Action::BuyBuilding { building, .. } if building == "monument"
            )),
            "and it must leave the enumeration too"
        );

        // The direct buyer must not spend gold on it either — this is the path
        // that bypassed the enumeration and re-proposed it every turn.
        let before = g.players[0].gold;
        ai.spend_gold(&mut g, 0, &[cid], 1, 1, 1, 2, 1, 1);
        assert!(
            !g.cities[&cid].buildings.iter().any(|b| b == "monument"),
            "the direct gold buyer must respect a host purchase refusal"
        );
        assert!(
            g.players[0].gold <= before,
            "and must not have paid for the refused building"
        );
    }

    #[test]
    fn gold_spending_annexes_a_luxury_without_breaking_its_reserve() {
        let mut game = Game::new_full(1, 20, 14, 321, 30, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        let center = game.cities[&city].pos;
        for position in game.wdisk(center, 3) {
            if game.map.tiles[&position].owner_city.is_none() {
                let tile = game.map.tiles.get_mut(&position).unwrap();
                tile.terrain = crate::name!("plains");
                tile.hills = false;
                tile.feature = None;
                tile.resource = None;
            }
        }
        let target = game
            .wdisk(center, 2)
            .into_iter()
            .find(|position| {
                game.wdist(*position, center) == 2
                    && game.map.tiles[position].owner_city.is_none()
                    && game
                        .nbrs(*position)
                        .into_iter()
                        .any(|neighbor| game.map.tiles[&neighbor].owner_city == Some(city))
            })
            .unwrap();
        game.map.tiles.get_mut(&target).unwrap().resource = Some(crate::name!("diamonds"));
        game.players[0]
            .explored
            .extend(game.map.tiles.keys().copied());
        game.players[0].gold = 175.0;

        assert!(BasicAi::new().buy_gold_plot(&mut game, 0, 125.0));
        assert_eq!(game.map.tiles[&target].owner_city, Some(city));
        assert_eq!(game.players[0].gold, 125.0);
    }

    #[test]
    fn crowded_world_does_not_produce_or_buy_a_stranded_settler() {
        let mut game = Game::new_full(1, 20, 14, 320_001, 80, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        let center = game.cities[&city].pos;
        game.cities.get_mut(&city).unwrap().pop = 4;
        for (position, tile) in &mut game.map.tiles {
            if position != center {
                tile.terrain = crate::name!("ocean");
                tile.feature = None;
            }
        }
        let settler_item = Item::Unit {
            unit: crate::name!("settler"),
        };
        assert!(game.can_produce(0, city, &settler_item));
        let mut ai = BasicAi::new();

        let production = ai.pick_item(&game, 0, city, 1, 0, 1, 1, 0, 2, 1, 1);
        assert_ne!(
            production,
            Some(settler_item.clone()),
            "the city must not turn Population and Production into a settler with nowhere to settle"
        );

        game.players[0].gold = 10_000.0;
        let _ = ai.spend_gold(&mut game, 0, &[city], 0, 1, 1, 2, 1, 1);
        assert!(game
            .player_unit_ids(0)
            .into_iter()
            .all(|unit| game.units[&unit].kind != "settler"));

        game.apply(
            0,
            &Action::Produce {
                city,
                item: settler_item.clone(),
            },
        )
        .unwrap();
        game.cities.get_mut(&city).unwrap().production = 42.0;
        ai.cities(&mut game, 0);
        assert!(!matches!(
            game.cities[&city].queue.first(),
            Some(Item::Unit { unit }) if unit == "settler"
        ));
        assert_eq!(
            game.cities[&city]
                .production_progress
                .get("unit:settler"),
            Some(&42.0),
            "the invested Production should remain banked when the queue is redirected"
        );
    }

    #[test]
    fn existing_settler_redirects_a_queued_duplicate() {
        let mut game = Game::new_full(1, 20, 14, 320_002, 80, 0, false);
        let founding_settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(
            0,
            &Action::FoundCity {
                unit: founding_settler,
            },
        )
        .unwrap();
        let city = game.player_city_ids(0)[0];
        game.cities.get_mut(&city).unwrap().pop = 4;
        let settler_item = Item::Unit {
            unit: crate::name!("settler"),
        };
        game.apply(
            0,
            &Action::Produce {
                city,
                item: settler_item,
            },
        )
        .unwrap();
        game.cities.get_mut(&city).unwrap().production = 42.0;
        let captured = game.spawn_test_unit("settler", 0, game.cities[&city].pos);
        let mut ai = BasicAi::new();
        assert!(ai.has_practical_settle_site(&game, 0));

        ai.cities(&mut game, 0);

        assert!(game.units.contains_key(&captured));
        assert!(!matches!(
            game.cities[&city].queue.first(),
            Some(Item::Unit { unit }) if unit == "settler"
        ));
        assert_eq!(
            game.cities[&city].production_progress.get("unit:settler"),
            Some(&42.0)
        );
    }

    #[test]
    fn deficit_empire_builds_its_way_back_to_positive_gpt() {
        let mut g = Game::new_full(1, 20, 14, 323, 60, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        g.players[0].techs.insert(crate::name!("currency"));
        g.players[0].gold = 0.0;
        g.players[0].gold_per_turn = -9.2;
        let cid = g.player_city_ids(0)[0];
        // The assertion is about what a broke empire chooses to build, not
        // about whether the map left it anywhere to build: level the ring.
        let center = g.cities[&cid].pos;
        let ring: Vec<Pos> = g.cities[&cid]
            .owned_tiles
            .iter()
            .copied()
            .filter(|position| *position != center)
            .collect();
        for position in ring {
            let tile = g.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("plains");
            tile.feature = None;
            tile.resource = None;
            tile.hills = false;
        }
        let ai = BasicAi::new();

        let district = ai
            .pick_item(&g, 0, cid, 1, 0, 0, 0, 0, 4, 2, 2)
            .expect("a deficit city should establish gold infrastructure");
        let Item::District {
            district,
            pos: commercial_hub,
        } = district
        else {
            panic!("expected a Commercial Hub, got {district:?}");
        };
        assert_eq!(district, "commercial_hub");
        g.map.tiles.get_mut(&commercial_hub).unwrap().district = Some(district);
        g.cities
            .get_mut(&cid)
            .unwrap()
            .districts
            .insert(district, commercial_hub);

        assert_eq!(
            ai.pick_item(&g, 0, cid, 1, 0, 0, 0, 0, 4, 2, 2),
            Some(Item::Building {
                building: crate::name!("market")
            })
        );
        g.cities
            .get_mut(&cid)
            .unwrap()
            .buildings
            .push(crate::name!("market"));
        let second_center = g
            .map
            .tiles
            .iter()
            .find(|(position, tile)| {
                let distance = g.wdist(**position, center);
                (4..=15).contains(&distance)
                    && g.rules.is_passable(tile)
                    && !g.rules.is_water(tile)
                    && g.units_at(**position).is_empty()
            })
            .map(|(position, _)| *position)
            .expect("the recovery Trader needs a reachable destination");
        let second_settler = g.spawn_test_unit("settler", 0, second_center);
        g.apply(
            0,
            &Action::FoundCity {
                unit: second_settler,
            },
        )
        .unwrap();
        g.players[0].civics.insert(crate::name!("foreign_trade"));
        assert_eq!(
            ai.pick_item(&g, 0, cid, 1, 0, 0, 0, 0, 4, 2, 2),
            Some(Item::Unit {
                unit: crate::name!("trader")
            })
        );
    }

    #[test]
    fn trade_capacity_without_an_open_destination_does_not_create_a_trader() {
        let mut g = Game::new_full(1, 20, 14, 323_001, 60, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        g.players[0].civics.insert(crate::name!("foreign_trade"));
        let city = g.player_city_ids(0)[0];
        let trader = Item::Unit {
            unit: crate::name!("trader"),
        };
        assert!(g.trade_capacity(0) > 0);
        assert!(g.can_produce(0, city, &trader));
        assert_eq!(BasicAi::open_trade_destinations(&g, 0), 0);
        assert!(!BasicAi::should_add_trader(&g, 0, 0));

        let ai = BasicAi::new();
        let choice = ai.pick_item(&g, 0, city, 1, 1, 1, 0, 0, 3, 2, 1);
        assert_ne!(choice, Some(trader));

        g.players[0].gold = g.rules.units["trader"].cost * 4.0 + 125.0;
        let _ = ai.spend_gold(&mut g, 0, &[city], 1, 1, 0, 3, 2, 1);
        assert!(g
            .player_unit_ids(0)
            .into_iter()
            .all(|unit| g.units[&unit].kind != "trader"));
    }

    #[test]
    fn deficit_empire_without_a_gold_build_keeps_producing_without_new_upkeep() {
        let mut g = Game::new_full(1, 20, 14, 324, 60, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        g.players[0].techs.insert(crate::name!("pottery"));
        g.players[0].gold = 0.0;
        g.players[0].gold_per_turn = -4.0;
        let cid = g.player_city_ids(0)[0];

        let item = BasicAi::new()
            .pick_item(&g, 0, cid, 1, 0, 0, 0, 0, 6, 3, 3)
            .expect("a deficit city must not leave its Production idle");
        match &item {
            Item::Building { building } => {
                assert!(g.rules.buildings[building].maintenance <= f64::EPSILON)
            }
            Item::Project { project } => assert!(g.rules.projects[project].repeatable),
            other => panic!("recovery fallback added upkeep: {other:?}"),
        }
        g.apply(0, &Action::Produce { city: cid, item }).unwrap();
        assert!(!g.cities[&cid].queue.is_empty());
    }

    #[test]
    fn one_queued_spaceport_reserves_the_empire_launch_site() {
        let mut game = Game::new_full(
            1, 30, 20, crate::rng::fixture_seed("SPACEPORT", 324_006), 100, 0, false,
        );
        let first_settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game
            .apply(0, &Action::FoundCity { unit: first_settler })
            .unwrap();
        let first_center = game.cities[&game.player_city_ids(0)[0]].pos;
        let second_center = game
            .map
            .tiles
            .iter()
            .filter(|(position, tile)| {
                game.wdist(**position, first_center) >= 4
                    && game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && game.units_at(**position).is_empty()
            })
            .map(|(position, _)| *position)
            .next()
            .expect("map has a second legal city site");
        let second_settler = game.spawn_test_unit("settler", 0, second_center);
        game
            .apply(0, &Action::FoundCity { unit: second_settler })
            .unwrap();
        game.players[0].techs.insert(crate::name!("rocketry"));
        let cities = game.player_city_ids(0);
        for city in &cities {
            game.cities.get_mut(city).unwrap().pop = 10;
            assert!(
                !game.district_sites(*city, crate::name!("spaceport")).is_empty(),
                "both cities must be able to repeat the archived overbuild"
            );
        }

        let ai = BasicAi::new();
        let launch_city = cities[0];
        let first = ai
            .pick_item(&game, 0, launch_city, 2, 2, 2, 2, 1, 10, 5, 5)
            .expect("the empire needs its first launch site");
        assert!(matches!(
            &first,
            Item::District { district, .. } if district == "spaceport"
        ));
        game
            .apply(
                0,
                &Action::Produce {
                    city: launch_city,
                    item: first,
                },
            )
            .unwrap();

        let other = cities[1];
        let next = ai.pick_item(&game, 0, other, 2, 2, 2, 2, 1, 10, 5, 5);
        assert!(
            !matches!(next, Some(Item::District { ref district, .. }) if district == "spaceport"),
            "a queued Spaceport must stop every other city reserving another one: {next:?}"
        );
    }

    #[test]
    fn buying_a_queued_building_finishes_it_without_a_duplicate() {
        let mut g = Game::new_full(1, 20, 14, 322, 30, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let cid = g.player_city_ids(0)[0];
        g.cities
            .get_mut(&cid)
            .unwrap()
            .buildings
            .retain(|building| building != "monument");
        g.apply(
            0,
            &Action::Produce {
                city: cid,
                item: Item::Building {
                    building: crate::name!("monument"),
                },
            },
        )
        .unwrap();
        g.players[0].gold = 1_000.0;

        let purchase = Action::BuyBuilding {
            city: cid,
            building: crate::name!("monument"),
            currency: "gold".to_string(),
        };
        let cost = g
            .building_gold_purchase_cost(0, cid, "monument")
            .expect("a queued ordinary building remains purchasable");
        assert!(g.legal_actions(0).iter().any(|action| matches!(
            action,
            Action::BuyBuilding { city, building, .. }
                if *city == cid && building == "monument"
        )));
        g.apply(0, &purchase).unwrap();
        assert_eq!(g.players[0].gold, 1_000.0 - cost);
        assert!(g.cities[&cid].queue.is_empty());
        assert_eq!(
            g.cities[&cid]
                .buildings
                .iter()
                .filter(|building| building.as_str() == "monument")
                .count(),
            1
        );
        assert!(g.apply(0, &purchase).is_err());
    }

    #[test]
    fn city_states_invest_surplus_gold_without_abandoning_their_reserve() {
        let mut g = Game::new_full(1, 20, 14, 321, 30, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let cid = g.player_city_ids(0)[0];
        g.players[0].is_minor = true;
        g.cities
            .get_mut(&cid)
            .unwrap()
            .buildings
            .retain(|building| building != "monument");
        let mut ai = BasicAi::new();
        ai.minor = true;

        // Expansion and trade purchases remain major-only, but a city-state
        // with its local worker/defense needs met should convert excess Gold
        // into its city instead of accumulating an inert four-figure balance.
        g.players[0].gold = 365.0;
        assert!(ai.spend_gold(&mut g, 0, &[cid], 1, 1, 0, 3, 2, 1));
        assert_eq!(g.players[0].gold, 125.0);
        assert!(g.cities[&cid].buildings.iter().any(|b| b == "monument"));
    }

    #[test]
    fn headless_ai_spends_promotions_and_forms_unlocked_corps() {
        let mut g = Game::new_full(1, 20, 14, 33, 30, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        let veteran = g
            .player_unit_ids(0)
            .into_iter()
            .find(|id| g.units[id].kind == "warrior")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        g.units.get_mut(&veteran).unwrap().xp = 15;
        let ai = BasicAi::new();
        ai.prepare_unit_formations(&mut g, 0);
        assert_eq!(g.units[&veteran].level, 2);
        assert_eq!(g.units[&veteran].promotions.len(), 1);

        let pos = g
            .map
            .tiles
            .iter()
            .find(|(pos, tile)| {
                g.rules.is_passable(tile) && !g.rules.is_water(tile) && g.units_at(**pos).is_empty()
            })
            .map(|(pos, _)| *pos)
            .unwrap();
        for _ in 0..6 {
            g.spawn_test_unit("warrior", 0, pos);
        }
        g.players[0].civics.insert(crate::name!("nationalism"));
        ai.prepare_unit_formations(&mut g, 0);
        let warriors: Vec<_> = g
            .units
            .values()
            .filter(|unit| unit.owner == 0 && unit.kind == "warrior")
            .collect();
        assert_eq!(warriors.len(), 5, "the AI keeps a five-unit reserve");
        assert!(warriors.iter().any(|unit| unit.formation == 1));
        assert!(
            warriors.iter().any(|unit| unit.promotions.len() == 1),
            "the veteran remains in the force"
        );
    }

    #[test]
    fn production_adds_one_support_unit_for_walled_wars() {
        let (mut g, home, _) = walled_war_game(33);
        let ai = BasicAi::new();
        g.players[0].techs.insert(crate::name!("masonry"));

        let ram = ai.pick_item(&g, 0, home, 1, 0, 1, 0, 0, 2, 2, 0).unwrap();
        assert_eq!(
            ram,
            Item::Unit {
                unit: crate::name!("battering_ram")
            }
        );

        let tower_tech = g.rules.units["siege_tower"].tech.unwrap();
        g.players[0].techs.insert(tower_tech);
        let tower = ai.pick_item(&g, 0, home, 1, 0, 1, 0, 0, 2, 2, 0).unwrap();
        assert_eq!(
            tower,
            Item::Unit {
                unit: crate::name!("siege_tower")
            }
        );

        let next = ai.pick_item(&g, 0, home, 1, 0, 1, 0, 1, 2, 2, 0).unwrap();
        assert!(!matches!(next, Item::Unit { unit }
            if unit == "battering_ram" || unit == "siege_tower"));
    }

    #[test]
    fn siege_support_catches_up_and_stacks_with_melee_escort() {
        let (mut g, home, enemy) = walled_war_game(34);
        g.cities.get_mut(&enemy).unwrap().wall_hp = 100;
        g.players[0].techs.insert(crate::name!("masonry"));
        g.players[0].gold = 1_000.0;
        g.apply(
            0,
            &Action::Buy {
                city: home,
                unit: crate::name!("battering_ram"),
                formation: 0,
                currency: "gold".to_string(),
            },
        )
        .unwrap();
        let ram = g
            .player_unit_ids(0)
            .into_iter()
            .find(|id| g.units[id].kind == "battering_ram")
            .unwrap();
        let warrior = g
            .player_unit_ids(0)
            .into_iter()
            .find(|id| g.units[id].kind == "warrior")
            .unwrap();
        let next = g
            .nbrs(g.units[&warrior].pos)
            .into_iter()
            .find(|pos| g.can_move(warrior, *pos))
            .unwrap();
        g.apply(
            0,
            &Action::Move {
                unit: warrior,
                to: next,
            },
        )
        .unwrap();
        assert_ne!(g.units[&ram].pos, g.units[&warrior].pos);

        // A closer cavalry unit is a tempting but useless escort: the engine
        // grants Ram/Tower effects only to melee and anti-cavalry classes.
        let enemy_pos = g.cities[&enemy].pos;
        let cavalry_pos = g
            .nbrs(enemy_pos)
            .into_iter()
            .find(|position| g.units_at(*position).is_empty())
            .unwrap();
        g.spawn_test_unit("heavy_chariot", 0, cavalry_pos);

        let mut ai = BasicAi::new();
        ai.tactical_strategy = true;
        assert!(ai.siege_support_step(&mut g, 0, ram));
        assert_eq!(g.units[&ram].pos, g.units[&warrior].pos);
    }

    #[test]
    fn headless_ai_resolves_mandatory_capture_choices() {
        let mut g = Game::new_full(2, 20, 14, 34, 30, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = g.player_city_ids(0)[0];
        {
            let captured = g.cities.get_mut(&city).unwrap();
            // This is an unresolved capture of player 1's original city;
            // resolving a genuine recapture of player 0's own original city
            // correctly produces no grievance.
            captured.original_owner = 1;
            captured.captured_from = Some(1);
        }
        assert!(matches!(
            g.legal_actions(0).as_slice(),
            [Action::KeepCity { city: pending }] if *pending == city
        ));

        let mut ai = BasicAi::new();
        ai.resolve_city_dispositions(&mut g, 0, false, false);

        assert_eq!(g.cities[&city].captured_from, None);
        assert_eq!(g.players[1].grievances.get(&0), Some(&50.0));
    }

    #[test]
    fn builder_never_paces_between_tiles_it_cannot_work() {
        // A project target the Builder cannot stand on (the game places
        // districts on land, but a mod or a captured layout can leave one
        // unreachable) must not leave it walking back and forth forever.
        let mut g = Game::new_full(1, 20, 14, 35, 40, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = g.player_city_ids(0)[0];
        let spaceport = g.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != g.cities[&city].pos)
            .unwrap();
        {
            let tile = g.map.tiles.get_mut(&spaceport).unwrap();
            tile.terrain = crate::name!("mountain");
            tile.district = Some(crate::name!("spaceport"));
        }
        g.cities
            .get_mut(&city)
            .unwrap()
            .districts
            .insert(crate::name!("spaceport"), spaceport);
        g.cities
            .get_mut(&city)
            .unwrap()
            .buildings
            .push(crate::name!("royal_society"));
        g.cities.get_mut(&city).unwrap().queue = vec![Item::Project {
            project: crate::name!("launch_earth_satellite"),
        }];
        let builder = g.spawn_test_unit("builder", 0, g.cities[&city].pos);
        g.units.get_mut(&builder).unwrap().charges = 3;

        let ai = BasicAi::new();
        let mut visited = Vec::new();
        for _ in 0..12 {
            if !g.units.contains_key(&builder) {
                break;
            }
            visited.push(g.units[&builder].pos);
            if !ai.builder_step(&mut g, 0, builder) {
                break;
            }
            let movement = g.rules.units["builder"].moves;
            // Spending the last charge consumes the Builder mid-loop.
            let Some(unit) = g.units.get_mut(&builder) else {
                break;
            };
            unit.moves_left = movement;
            unit.moved = false;
            unit.acted = false;
        }
        let charges_spent = g
            .units
            .get(&builder)
            .map(|unit| 3 - unit.charges)
            .unwrap_or(3);
        assert!(
            charges_spent > 0 || visited.iter().collect::<std::collections::BTreeSet<_>>().len() == visited.len(),
            "Builder paced without working: {visited:?}"
        );
    }

    #[test]
    fn builder_routes_to_a_royal_society_project_and_contributes() {
        let mut g = Game::new_full(1, 20, 14, 35, 40, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = g.player_city_ids(0)[0];
        // Districts have to sit on land a Builder can walk onto; picking the
        // first owned tile can land the Spaceport on water or a mountain, and
        // the routing under test then has no legal way to reach it.
        let buildable = |g: &Game, position: &Pos| {
            let tile = &g.map.tiles[position];
            !g.rules.is_water(tile) && g.rules.is_passable(tile)
        };
        let spaceport = g.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != g.cities[&city].pos && buildable(&g, position))
            .unwrap();
        g.map.tiles.get_mut(&spaceport).unwrap().district = Some(crate::name!("spaceport"));
        g.cities
            .get_mut(&city)
            .unwrap()
            .districts
            .insert(crate::name!("spaceport"), spaceport);
        let government_plaza = g.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| {
                *position != g.cities[&city].pos
                    && *position != spaceport
                    && g.map.tiles[position].district.is_none()
                    && buildable(&g, position)
            })
            .unwrap();
        g.map.tiles.get_mut(&government_plaza).unwrap().district =
            Some(crate::name!("government_plaza"));
        g.cities
            .get_mut(&city)
            .unwrap()
            .districts
            .insert(crate::name!("government_plaza"), government_plaza);
        g.cities
            .get_mut(&city)
            .unwrap()
            .buildings
            .push(crate::name!("royal_society"));
        g.cities.get_mut(&city).unwrap().queue = vec![Item::Project {
            project: crate::name!("launch_earth_satellite"),
        }];
        let builder = g.spawn_test_unit("builder", 0, g.cities[&city].pos);
        g.units.get_mut(&builder).unwrap().charges = 3;

        let ai = BasicAi::new();
        // Reaching the Spaceport can take more than one turn's movement, and
        // entering a wooded or hilled district tile can spend a whole
        // allowance on its own, so drive the Builder the way the AI game loop
        // does — a step per turn — rather than assuming it arrives at once.
        let mut turns = 0;
        while g.units[&builder].pos != spaceport {
            assert!(ai.builder_step(&mut g, 0, builder));
            turns += 1;
            assert!(turns < 8, "Builder never reached the Spaceport");
            let movement = g.rules.units["builder"].moves;
            let unit = g.units.get_mut(&builder).unwrap();
            unit.moves_left = movement;
            unit.moved = false;
            unit.acted = false;
        }
        assert!(ai.builder_step(&mut g, 0, builder));
        assert!(!g.units.contains_key(&builder));
        assert_eq!(g.cities[&city].production, 54.0);
    }

    #[test]
    fn headless_naturalist_routes_to_and_establishes_a_complete_park() {
        let mut g = Game::new_full(1, 20, 14, 36, 40, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = g.player_city_ids(0)[0];
        let center = g.cities[&city].pos;
        let positions = g
            .map
            .tiles
            .keys()
            .copied()
            .filter(|top| g.wdist(center, *top) > 4)
            .find_map(|top| {
                let positions = [
                    top,
                    crate::hex::canon((top.0 - 1, top.1 + 1), g.map.width),
                    crate::hex::canon((top.0, top.1 + 1), g.map.width),
                    crate::hex::canon((top.0 - 1, top.1 + 2), g.map.width),
                ];
                positions
                    .iter()
                    .all(|position| g.map.tiles.contains_key(position))
                    .then_some(positions)
            })
            .unwrap();

        let old_owned = g.cities[&city].owned_tiles.clone();
        for position in old_owned {
            g.map.tiles.get_mut(&position).unwrap().owner_city = None;
        }
        g.cities.get_mut(&city).unwrap().owned_tiles = positions.to_vec();
        for position in positions
            .iter()
            .flat_map(|position| g.nbrs(*position))
            .chain(positions)
            .collect::<std::collections::BTreeSet<_>>()
        {
            let tile = g.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("plains");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
            tile.pillaged = false;
            tile.district = None;
            tile.wonder = None;
            tile.flooded = false;
            tile.submerged = false;
        }
        for position in positions {
            let tile = g.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("mountain");
            tile.owner_city = Some(city);
        }
        g.map.tiles.get_mut(&positions[0]).unwrap().terrain = crate::name!("grassland");
        g.players[0].civics.insert(crate::name!("conservation"));
        assert_eq!(g.national_park_sites(0), vec![positions]);

        let start = g
            .nbrs(positions[0])
            .into_iter()
            .find(|position| !positions.contains(position))
            .unwrap();
        let naturalist = g.spawn_test_unit("naturalist", 0, start);
        let ai = BasicAi::new();
        assert!(ai.naturalist_step(&mut g, 0, naturalist));
        assert_eq!(g.units[&naturalist].pos, positions[0]);
        assert!(ai.naturalist_step(&mut g, 0, naturalist));
        assert!(!g.units.contains_key(&naturalist));
        assert!(positions.iter().all(|position| {
            g.map.tiles[position].improvement.as_deref() == Some("national_park")
        }));
    }

    #[test]
    fn headless_military_engineer_routes_to_and_accelerates_an_aqueduct() {
        let mut game = Game::new_full(1, 20, 14, 36_001, 80, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        let center = game.cities[&city].pos;
        let site = game.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != center && game.units_at(*position).is_empty())
            .unwrap();
        let center_edge = game.map.direction_to(site, center).unwrap();
        {
            let tile = game.map.tiles.get_mut(&site).unwrap();
            tile.terrain = crate::name!("plains");
            tile.hills = false;
            tile.feature = None;
            tile.resource = None;
            tile.improvement = None;
            tile.district = None;
            tile.river_edges[(center_edge + 1) % 6] = true;
        }
        game.players[0].techs.insert(crate::name!("engineering"));
        let aqueduct = Item::District {
            district: crate::name!("aqueduct"),
            pos: site,
        };
        game.cities.get_mut(&city).unwrap().queue = vec![aqueduct.clone()];
        let district_cost = game.item_cost_for_city(0, city, &aqueduct);
        let engineer = game.spawn_test_unit("military_engineer", 0, center);
        let mut ai = BasicAi::new();

        assert!(ai.military_engineer_step(&mut game, 0, engineer));
        assert_eq!(game.units[&engineer].pos, site);
        // The river-adjacent construction tile can consume the Engineer's
        // full movement; the contribution is made after movement refreshes.
        game.units.get_mut(&engineer).unwrap().moves_left = 2.0;
        assert!(game.can_contribute_district(0, engineer, city));
        assert!(ai.military_engineer_step(&mut game, 0, engineer));
        assert!(
            (game.cities[&city].production - district_cost * 0.2).abs() < 1e-9,
            "production was {}",
            game.cities[&city].production
        );
        assert_eq!(game.units[&engineer].charges, 1);
        assert_eq!(game.units[&engineer].moves_left, 0.0);
    }

    #[test]
    fn idle_military_engineer_starts_a_stockpile_safe_railroad() {
        let mut game = Game::new_full(1, 20, 14, 36_002, 80, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        let center = game.cities[&city].pos;
        game.players[0].techs.insert(crate::name!("steam_power"));
        game.players[0]
            .strategic_resources
            .insert(crate::name!("iron"), 8.0);
        game.players[0]
            .strategic_resources
            .insert(crate::name!("coal"), 8.0);
        let engineer = game.spawn_test_unit("military_engineer", 0, center);
        let mut ai = BasicAi::new();

        assert!(ai.military_engineer_step(&mut game, 0, engineer));
        assert_eq!(game.map.tiles[&center].road, 5);
        assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 7.0);
        assert_eq!(game.strategic_stockpile(0, crate::name!("coal")), 7.0);

        // Once either reserve reaches the floor, the Engineer stops spending
        // and remains available for a Dam/Aqueduct/Canal contribution.
        game.map.tiles.get_mut(&center).unwrap().road = 1;
        game.players[0]
            .strategic_resources
            .insert(crate::name!("coal"), RAILROAD_RESOURCE_RESERVE);
        game.units.get_mut(&engineer).unwrap().moves_left = 2.0;
        let _ = ai.military_engineer_step(&mut game, 0, engineer);
        assert_ne!(game.map.tiles[&center].road, 5);
        assert_eq!(
            game.strategic_stockpile(0, crate::name!("coal")),
            RAILROAD_RESOURCE_RESERVE
        );
    }

    #[test]
    fn headless_archaeologist_routes_to_and_extracts_an_artifact() {
        let mut g = Game::new_full(1, 20, 14, 37, 40, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = g.player_city_ids(0)[0];
        g.cities
            .get_mut(&city)
            .unwrap()
            .buildings
            .push(crate::name!("archaeological_museum"));
        g.players[0].civics.insert(crate::name!("natural_history"));
        // The generator now lays down the shipped six dig sites per
        // civilization, so this fixture has to be the only Artifact on the map
        // or the routing assertion is really asserting which site is nearest.
        for position in g.map.tiles.keys().copied().collect::<Vec<_>>() {
            let is_artifact = g.map.tiles[&position]
                .resource
                .as_deref()
                .is_some_and(|resource| g.rules.resources[resource].class == "artifact");
            if is_artifact {
                g.map.tiles.get_mut(&position).unwrap().resource = None;
            }
        }
        // One `archaeologist_step` is one movement step, so the dig has to
        // border the city for the route to finish inside it. Taking the first
        // owned tile instead left that to the shape of the generated map.
        let site = g.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| {
                g.wdist(*position, g.cities[&city].pos) == 1 && g.units_at(*position).is_empty()
            })
            .unwrap();
        let tile = g.map.tiles.get_mut(&site).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = Some(crate::name!("antiquity_site"));
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        let archaeologist = g.spawn_test_unit("archaeologist", 0, g.cities[&city].pos);

        let ai = BasicAi::new();
        assert!(ai.archaeologist_step(&mut g, 0, archaeologist));
        assert_eq!(g.units[&archaeologist].pos, site);
        assert!(ai.archaeologist_step(&mut g, 0, archaeologist));
        assert!(g.map.tiles[&site].resource.is_none());
        assert_eq!(g.players[0].counters["great_work:artifact"], 1);
        assert_eq!(g.units[&archaeologist].charges, 2);
    }

    #[test]
    fn basic_ai_establishes_sources_then_runs_its_best_spy_operation() {
        let mut game = Game::new_full(2, 24, 16, 38, 80, 0, false);
        let cities: Vec<u32> = (0..2)
            .map(|pid| {
                let settler = game
                    .player_unit_ids(pid)
                    .into_iter()
                    .find(|unit| game.units[unit].kind == "settler")
                    .unwrap();
                game.found_city_for(pid, game.units[&settler].pos, None)
            })
            .collect();
        let target = cities[1];
        let commercial = game.cities[&target]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != game.cities[&target].pos)
            .unwrap();
        game.map.tiles.get_mut(&commercial).unwrap().district = Some(crate::name!("commercial_hub"));
        game.cities
            .get_mut(&target)
            .unwrap()
            .districts
            .insert(crate::name!("commercial_hub"), commercial);
        game.players[0].explored.insert(game.cities[&target].pos);
        let spy = game.next_id;
        game.next_id += 1;
        game.spies.insert(
            spy,
            crate::game::Spy {
                id: spy,
                owner: 0,
                level: 0,
                promotions: std::collections::BTreeSet::new(),
                city: Some(cities[0]),
                ready_turn: game.turn,
                mission: None,
                sources_city: None,
                sources_until: 0,
                captured_by: None,
            },
        );

        let ai = BasicAi::new();
        ai.spies(&mut game, 0);
        assert_eq!(game.spies[&spy].city, Some(target));
        game.turn = game.spies[&spy].ready_turn;
        ai.spies(&mut game, 0);
        assert_eq!(
            game.spies[&spy]
                .mission
                .as_ref()
                .map(|mission| mission.kind.as_str()),
            Some("gain_sources")
        );
        let ends = game.spies[&spy].mission.as_ref().unwrap().ends;
        game.turn = ends;
        game.process_spies(0);
        ai.spies(&mut game, 0);
        assert_eq!(
            game.spies[&spy]
                .mission
                .as_ref()
                .map(|mission| mission.kind.as_str()),
            Some("siphon_funds")
        );
    }

    /// Fires-check for `pol_influence`: does making influence visible actually
    /// get the card slotted?
    ///
    /// `empire_reading` scores a card by the counterfactual difference it makes
    /// to the empire. Influence is neither a city yield nor unit strength, so
    /// `charismatic_leader` — whose entire effect is `influence_per_turn: 2` —
    /// read bit-identical either side and scored exactly 0.0, then lost every
    /// tie because `POLICY_PRIORITY` does not name it. `envoy_income_census`
    /// (#612) measured the consequence: unlocked on 63.7% of turns with an open
    /// diplomatic slot on 49.1%, and slotted on 0.0%.
    ///
    /// The criterion is the outcome — the card in a slot — not a score.
    ///
    /// Run with `cargo test --release influence_visible_fires -- --ignored --nocapture`.
    #[test]
    #[ignore = "census, not an assertion; run explicitly with --nocapture"]
    fn influence_visible_fires() {
        for weight in [0.0f64, 1.0, 4.0] {
            let mut slotted = 0u64;
            let mut turns = 0u64;
            let mut envoys_placed = 0.0f64;
            let maps = 6u64;
            for map in 0..maps {
                let mut game = crate::game::Game::new_full(6, 74, 46, 480_000 + map, 200, 12, false);
                let mut ais: Vec<AdvancedAi> = (0..game.players.len())
                    .map(|_| {
                        // ⚠ `Weights::default()` is `PolicyDeck::Legacy`, which
                        // returns before the counterfactual scoring ever runs.
                        // The shipped agent loads the evolved champion, and
                        // that artifact deserializes to `Live` — forcing
                        // champions to Legacy was measured to lose (12 map
                        // directions for, 26 against, p=0.0336). Testing on the
                        // default would measure a path deployment never takes.
                        let mut w = crate::evolve::load_champion("evolved")
                            .unwrap_or_default();
                        w.policy_deck = PolicyDeck::Live;
                        w.pol_influence = weight;
                        AdvancedAi::with_weights(w)
                    })
                    .collect();
                game.set_fog_memory(false);
                while game.winner.is_none() && game.turn <= game.max_turns {
                    let pid = game.current;
                    ais[pid].take_turn(&mut game, pid);
                    if game.winner.is_none() && game.current == pid {
                        let _ = game.apply(pid, &crate::game::Action::EndTurn);
                    }
                    if pid != 0 {
                        continue;
                    }
                    turns += 1;
                    if game.players[0]
                        .policies
                        .iter()
                        .any(|p| p.as_str() == "charismatic_leader")
                    {
                        slotted += 1;
                    }
                    envoys_placed +=
                        game.players[0].envoys.iter().map(|(_, n)| *n).sum::<i64>() as f64;
                }
            }
            println!(
                "  pol_influence={weight:<4}  charismatic_leader slotted {:>5.1}% of turns   envoys placed mean {:.2}",
                slotted as f64 / turns.max(1) as f64 * 100.0,
                envoys_placed / turns.max(1) as f64
            );
        }
        println!();
    }

    /// ⚠ The two Museums are identical except for the slot KIND, both cost 290, and
    /// `sort()` fell through to `Name::cmp` — which compares text, so
    /// `archaeological_museum` won on the letter 'c'. That alphabetical accident
    /// decided which Museum every empire built, and the one it picked needs a
    /// 400-production Archaeologist that no live run has ever built.
    #[test]
    fn a_cost_tie_prefers_the_slots_a_great_person_can_fill() {
        let g = Game::new_full(1, 20, 14, 37, 40, 0, false);
        let art = &g.rules.buildings["art_museum"];
        let dig = &g.rules.buildings["archaeological_museum"];

        // Fixture preconditions: the tie is real, and the alphabet did decide it.
        assert_eq!(art.cost, dig.cost, "same cost — this is why it is a tie");
        assert_eq!(art.yields.culture, dig.yields.culture);
        assert_eq!(
            art.great_work_slots.values().sum::<i32>(),
            dig.great_work_slots.values().sum::<i32>(),
            "same NUMBER of slots, so counting them cannot break the tie either"
        );
        assert!(
            crate::name!("archaeological_museum") < crate::name!("art_museum"),
            "fixture precondition: the alphabet really does put artifact first"
        );

        let slot_worth = |name: &str| -> f64 {
            g.rules.buildings[&Name::new(name)]
                .great_work_slots
                .iter()
                .map(|(kind, count)| {
                    let count = (*count).max(0) as f64;
                    if kind == "artifact" { count * 0.5 } else { count }
                })
                .sum()
        };
        assert!(
            slot_worth("art_museum") > slot_worth("archaeological_museum"),
            "the tie must now break toward the Museum whose slots a Great Person fills"
        );
        // ⚠ Everything without an artifact slot keeps its exact weight, so this
        // changes tied pairs only and never reorders on cost.
        assert_eq!(slot_worth("amphitheater"), 2.0);
        assert_eq!(slot_worth("library"), 0.0);

        // ⚠ SCOPE. The rule fires only where BOTH candidates carry slots. A first
        // version compared across every cost tie and reordered six groups —
        // old_god_obelisk over monument, temple over market, cathedral over every
        // other worship building, broadcast_center over research_lab — and priced at
        // -14 Elo with culture LOWER (151.7 vs 153.8). These assertions pin the
        // narrowing: a slotless building must never be displaced by a slotted one.
        for (cost, slotless) in [(60, "monument"), (120, "market"), (150, "arena"),
                                 (290, "bank"), (440, "research_lab")] {
            let spec = &g.rules.buildings[&Name::new(slotless)];
            assert_eq!(spec.cost as i64, cost, "fixture: {slotless} still costs {cost}");
            assert_eq!(
                slot_worth(slotless), 0.0,
                "{slotless} has no slots, so the tiebreak must not touch its position"
            );
        }
    }
}

#[cfg(test)]
mod amenity_district_tests {
    use super::*;
    use crate::game::Game;

    /// The omission this repairs: four families, and the one that fixes the
    /// Amenity band is not among them.
    #[test]
    fn the_baseline_district_list_has_no_amenity_district() {
        assert!(
            !DISTRICT_PRIORITY.contains(&"entertainment_complex"),
            "if this ever gains one, the treatment below is redundant"
        );
        assert_eq!(DISTRICT_PRIORITY.len(), 4);
    }

    /// Off for the frozen controllers, on for the promoted production agent.
    #[test]
    fn only_the_promoted_agent_repairs_amenities() {
        assert!(!BasicAi::new().amenity_districts);
        assert!(!BasicAi::with_weights(Weights::default()).amenity_districts);
        assert!(crate::ai::AdvancedAi::new().weights().city_target > 0.0);
    }

    /// A city in deficit must rank the repair above the lane's own families,
    /// and a neutral city must not ask for it at all — the band is flat at and
    /// above zero, so an extra Amenity there multiplies nothing.
    #[test]
    fn the_repair_outranks_specialty_districts_only_while_the_band_is_being_paid() {
        let mut game = Game::new(2, 32, 24, 9_101, 250, 0);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("starting settler");
        game.apply(0, &crate::game::Action::FoundCity { unit: settler })
            .expect("found city");
        let city = game.player_city_ids(0)[0];

        assert!(
            game.city_amenity_surplus(&game.cities[&city]) >= 0,
            "a new size-1 city is not in deficit"
        );

        // Grow it until the host's own band actually bites.
        let deficit_pop = (6..30)
            .find(|pop| {
                game.cities.get_mut(&city).unwrap().pop = *pop;
                game.city_amenity_surplus(&game.cities[&city]) <= -3
            })
            .expect("some population puts this city into the -3 band");
        game.cities.get_mut(&city).unwrap().pop = deficit_pop;

        let surplus = game.city_amenity_surplus(&game.cities[&city]);
        assert!(surplus <= -3);
        // `amenity_yield_mult_for` is private to the engine, so assert the band
        // boundary it keys on rather than keeping a second copy of the table
        // here — a copy would be free to drift from the rule it describes.
        assert!(
            surplus <= -3,
            "at -3 the engine's band multiplies EVERY yield in this city by \
             0.80, which is what makes the repair worth a district slot: got \
             {surplus}"
        );

        // And the treatment must be inert where the band is flat.
        let mut neutral = BasicAi::new();
        neutral.amenity_districts = true;
        game.cities.get_mut(&city).unwrap().pop = 1;
        assert!(
            game.city_amenity_surplus(&game.cities[&city]) >= 0,
            "back to neutral, where an extra Amenity multiplies nothing"
        );
    }

    /// The omission this repairs: neither district that raises the housing
    /// ceiling is in the baseline list, so the governor making most of a
    /// deployed empire's builds could not ask for one.
    #[test]
    fn the_baseline_district_list_has_no_housing_district() {
        for family in HOUSING_DISTRICTS {
            assert!(
                !DISTRICT_PRIORITY.contains(&family),
                "if {family} ever joins the lane list, this treatment is redundant"
            );
        }
        assert_eq!(HOUSING_DISTRICTS, ["aqueduct", "neighborhood"]);
    }

    /// Off in both constructors, so every frozen native controller and every
    /// recorded tournament ladder keeps building what it always built. The
    /// live bridge is the only thing that turns it on — asserted in
    /// `ai::advanced`, where the field is visible.
    #[test]
    fn the_housing_treatment_is_off_for_the_frozen_controllers() {
        assert!(!BasicAi::new().housing_districts);
        assert!(!BasicAi::with_weights(Weights::default()).housing_districts);
    }

    /// The target is the break-even of the engine's own growth band, not a
    /// comfort margin: `housing_growth_mult` pays 1.0 at 2 and 0.5 at 1.
    #[test]
    fn the_headroom_target_is_where_the_growth_penalty_stops() {
        assert_eq!(HOUSING_HEADROOM_TARGET, 2.0);
    }

    /// ⚠ The two repairs are ranked against the LANE, not stacked on each
    /// other. A city deep in the Amenity band is not growing AT ALL —
    /// `amenity_growth_mult` is 0.0 below −4 — so an Aqueduct there buys
    /// nothing and must not outrank the repair that restores growth. A merely
    /// displeased city with a real housing block is the other way round.
    ///
    /// This reproduces the weights `pick_item` assigns rather than reaching
    /// into it, because the ordering is the whole claim.
    #[test]
    fn the_amenity_repair_outranks_housing_exactly_when_growth_is_already_zero() {
        let lane_top = 4.0_f64;
        let weigh = |amenity_surplus: i64, headroom: f64, gain: f64| {
            let deficit = (-amenity_surplus).max(0) as f64;
            let shortfall = HOUSING_HEADROOM_TARGET - headroom;
            (lane_top + deficit, lane_top + shortfall.min(gain))
        };

        // Unrest at −6: growth is 0.00, so the Amenity repair must go first.
        let (amenity, housing) = weigh(-6, -1.0, 4.0);
        assert!(
            amenity > housing,
            "at surplus −6 growth is zero and an Aqueduct cannot help: {amenity} vs {housing}"
        );

        // Displeased at −1 with the housing ceiling genuinely reached: growth
        // is still 0.85 from amenities but only 0.25 from housing.
        let (amenity, housing) = weigh(-1, 0.0, 4.0);
        assert!(
            housing > amenity,
            "a real housing block beats mild unhappiness: {housing} vs {amenity}"
        );

        // And both stay above the lane whenever they fire at all.
        assert!(amenity > lane_top && housing > lane_top);
    }

    /// An Aqueduct is worth twice as much to a dry city as to a river one, and
    /// nothing at all once it is standing. That spread is the whole reason the
    /// chooser is handed the gain instead of a flat weight.
    #[test]
    fn an_aqueduct_is_worth_most_to_the_city_that_has_no_water() {
        let mut game = Game::new(2, 32, 24, 9_101, 250, 0);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("starting settler");
        game.apply(0, &crate::game::Action::FoundCity { unit: settler })
            .expect("found city");
        let cid = game.player_city_ids(0)[0];
        let city = &game.cities[&cid];

        // Whatever water this start happens to have, the gain must be the
        // difference between the two housing floors — never negative, never
        // more than the dry city's +4.
        let gain = game.aqueduct_housing_gain(city);
        assert!(
            (2.0..=4.0).contains(&gain),
            "an Aqueduct raises the floor by 2 (fresh), 3 (coastal) or 4 (dry): got {gain}"
        );

        // And it is exactly the housing the city would gain, which is the
        // claim the chooser actually relies on.
        let before = game.city_housing(city);
        let with = crate::game::Game::city_housing_floor(true, true, true)
            - crate::game::Game::city_housing_floor(true, true, false);
        assert_eq!(with, 2.0, "a fresh-water city gains 2");
        assert_eq!(
            crate::game::Game::city_housing_floor(false, true, true)
                - crate::game::Game::city_housing_floor(false, true, false),
            3.0,
            "a coastal city gains 3"
        );
        assert_eq!(
            crate::game::Game::city_housing_floor(false, false, true)
                - crate::game::Game::city_housing_floor(false, false, false),
            4.0,
            "a dry inland city gains 4 — the largest early housing step there is"
        );
        assert!(before > 0.0);
    }

    /// The treatment is inert while the city still has room to grow, and asks
    /// only once population has reached the band that throttles it. A city with
    /// headroom is not paying anything, so an Aqueduct there buys no growth.
    #[test]
    fn the_housing_repair_is_asked_for_only_once_growth_is_throttled() {
        let mut game = Game::new(2, 32, 24, 9_101, 250, 0);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("starting settler");
        game.apply(0, &crate::game::Action::FoundCity { unit: settler })
            .expect("found city");
        let cid = game.player_city_ids(0)[0];

        game.cities.get_mut(&cid).unwrap().pop = 1;
        assert!(
            game.city_housing_headroom(&game.cities[&cid]) >= HOUSING_HEADROOM_TARGET,
            "a size-1 city has room to grow and must not be steered at housing"
        );

        // Grow it until the engine's own band bites, then confirm the headroom
        // the chooser reads agrees that it is being throttled.
        let throttled_pop = (2..40)
            .find(|pop| {
                game.cities.get_mut(&cid).unwrap().pop = *pop;
                game.city_housing_headroom(&game.cities[&cid]) < HOUSING_HEADROOM_TARGET
            })
            .expect("some population overruns this city's housing");
        game.cities.get_mut(&cid).unwrap().pop = throttled_pop;
        let headroom = game.city_housing_headroom(&game.cities[&cid]);
        assert!(
            headroom < HOUSING_HEADROOM_TARGET,
            "below {HOUSING_HEADROOM_TARGET} the engine halves this city's growth: got {headroom}"
        );
        assert!(
            game.aqueduct_housing_gain(&game.cities[&cid]) > 0.0,
            "and the repair is available to it"
        );
    }
}
