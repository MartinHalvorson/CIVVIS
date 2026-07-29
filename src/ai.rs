//! Scripted AIs (mirrors civvis/ai/). BasicAi reads full state (no fog) —
//! sparring partner, not a fair-play agent.
use crate::name::{AsName, Name};
use crate::game::{effective_strength, Action, ActionFamilies, Game, Item};
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

/// Unlevied city-state forces defend the state and its immediate approaches;
/// ownership transfers to the Suzerain while levied, so those units naturally
/// use the major civilization's unrestricted tactical doctrine instead.
const MINOR_DEFENSE_RADIUS: i32 = 6;

/// Railroads are valuable infrastructure, but every tile consumes one Iron
/// and one Coal. Keep enough of each material for an emergency unit upgrade
/// instead of letting an idle Engineer pave the stockpile down to zero.
const RAILROAD_RESOURCE_RESERVE: f64 = 4.0;

mod advanced;
pub use advanced::{
    AdvancedAi, ForceDomain, ForceGroup, ForcePosture, GrandStrategy, StrategicPlan,
    StrategyCensus, VictoryTarget,
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
    pub forces: Vec<ForceReport>,
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
    // Combat cards move no yield. Ask the units themselves instead: a card
    // worth +5 strength to a standing army is visible here and nowhere else.
    let mut strength = 0.0;
    for uid in g.player_unit_ids(pid) {
        if let Some(unit) = g.units.get(&uid) {
            strength += g.unit_strength(unit, false) + g.unit_strength(unit, true);
        }
    }
    value + w.pol_military * strength
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
/// settles that: a Golden or Heroic Age banks no Era Score at all, so there the
/// tally is read purely as "which lane am I in". In a Normal or Dark Age it is
/// read literally, as the score that buys the next age.
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
        // A Golden or Heroic Age banks no Era Score, so there the projection is
        // only a correlate of what the Golden half is worth — and ranking on it
        // is what lost the first gate. `Banking` keeps the measured number
        // where it is the literal objective and leaves the rest alone.
        let banking = !matches!(g.players[pid].age.as_str(), "golden" | "heroic");
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
fn revise_policy_deck(g: &mut Game, pid: usize, w: &Weights) {
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
    if held.len() as i64 >= total && g.turn % POLICY_REVIEW_EVERY != 0 {
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

    let mut scored: Vec<(f64, usize, String, Name)> = Vec::new();
    for card in &candidates {
        let slot = match g.rules.policies.get(card) {
            Some(spec) => spec.slot.clone(),
            None => continue, // a priority-list entry the ruleset never had
        };
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
        scored.push((gain + hysteresis, rank(card), slot, *card));
    }

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
    /// Fraction by which a challenger must beat the incumbent to take its
    /// slot. Zero re-shuffles the deck on noise; one never swaps at all.
    pub pol_swap_margin: f64,
    /// Which deck this strategy holds.
    ///
    /// **Not a gene.** Deliberately absent from `to_vec`/`from_vec`/`bounds`,
    /// so the GA can neither read nor breed it and the genome stays 48 wide.
    /// It rides on `Weights` for one reason: `AdvancedAi::with_weights` already
    /// carries a genome into the inner `BasicAi`, so an eval arm costs no
    /// change to `src/ai/advanced.rs` or `src/elo.rs`. Set it in a harness;
    /// leave it alone in play.
    #[serde(default)]
    pub policy_deck: PolicyDeck,
    /// How this strategy picks its Dedication at an age transition.
    ///
    /// **Not a gene**, for the same reasons as `policy_deck`: absent from
    /// `to_vec`/`from_vec`/`bounds`, so the genome stays 48 wide.
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
    /// to Live for compatibility, and a deployment-profile A/B showed that
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

#[derive(Clone)]
pub struct BasicAi {
    minor: bool,
    barb: bool,
    culture_focus: bool,
    pursue_religion: bool,
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
    /// Colonies, especially overseas ones, need a fixed destination. Re-scoring
    /// only a short local radius each step strands settlers on shorelines and
    /// can make them reverse course after embarking.
    settler_targets: HashMap<u32, Pos>,
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

    pub fn new() -> BasicAi {
        BasicAi {
            minor: false,
            barb: false,
            culture_focus: false,
            pursue_religion: true,
            w: Weights::default(),
            book_pos: 0,
            recovering_units: HashSet::new(),
            patrol_targets: HashMap::new(),
            patrol_posts: HashMap::new(),
            settler_targets: HashMap::new(),
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
            culture_focus: false,
            pursue_religion: true,
            w,
            book_pos: 0,
            recovering_units: HashSet::new(),
            patrol_targets: HashMap::new(),
            patrol_posts: HashMap::new(),
            settler_targets: HashMap::new(),
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
    /// Reset caches whose contents depend on the current player's borders and
    /// movement capabilities, and take down where every unit is standing
    /// before any of them moves. Persistent destinations live across turns;
    /// the expensive all-map candidate scan does not need to.
    pub(crate) fn begin_movement_turn(&mut self, g: &Game, pid: usize) {
        self.patrol_posts.clear();
        self.observe_unit_motion(g, pid);
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
        self.research_with_government(g, pid, true);
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

    /// Choose research and run the baseline ancillary pass without allowing
    /// the baseline government priority to compete with a strategic caller.
    /// `AdvancedAi` has its own plan-aware government selector later in the
    /// same turn; running both selectors made it adopt two Tier-1 governments
    /// back-to-back and then pay Anarchy when the baseline tried to undo the
    /// strategic choice on the next turn.
    fn research_without_government(&self, g: &mut Game, pid: usize) {
        self.research_with_government(g, pid, false);
    }

    fn research_with_government(&self, g: &mut Game, pid: usize, choose_government: bool) {
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
        revise_policy_deck(g, pid, &self.w);
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
        if !self.minor && g.players[pid].pantheon.is_none() && g.players[pid].faith >= 25.0 {
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
                            founder: Name::new(&founder),
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
                .find(|governor| !g.players[pid].governor_roster.contains_key(*governor));
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
        Self::reassign_governor_for_loyalty(g, pid);
        while g.players[pid].envoys_free > 0 {
            // consolidate on the city-state we already lead in (suzerain push)
            let target = g
                .players
                .iter()
                .filter(|m| m.is_minor && !m.is_barbarian && m.alive && !g.is_at_war(pid, m.id))
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
    fn reassign_governor_for_loyalty(g: &mut Game, pid: usize) -> bool {
        let target = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|city| !g.players[pid].governors.contains(city))
            .filter(|city| g.cities[city].loyalty < 70.0)
            .min_by(|left, right| {
                g.cities[left]
                    .loyalty
                    .total_cmp(&g.cities[right].loyalty)
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
                        std::cmp::Reverse(governor.clone()),
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
                    deal.peace
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
            .filter(|o| o.id != pid && o.alive && !o.is_barbarian)
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
            let kind = g.units[&uid].kind.clone();
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
            let missionaries = g
                .units
                .values()
                .filter(|u| u.owner == pid && u.kind == "missionary")
                .count();
            if missionaries < 2 {
                for cid in &city_ids {
                    if g.cities[cid].districts.contains_key(crate::name!("holy_site")) {
                        let _ = g.apply(
                            pid,
                            &Action::Buy {
                                city: *cid,
                                unit: crate::name!("missionary"),
                                formation: 0,
                                currency: "faith".to_string(),
                            },
                        );
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
            if !matches_role {
                continue;
            }
            if !g.can_produce(pid, cid, &Item::Unit { unit: name.clone() }) {
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
                            unit: (*name).clone(),
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
                (power * 3.0 + role - spec.cost * 0.04, name.clone())
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
                        unit: name.clone(),
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
        let want_ranged = melee > ranged;
        self.best_military(g, pid, cid, Some(want_ranged))
            .or_else(|| self.best_military(g, pid, cid, None))
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
            if !g.can_produce(
                pid,
                *cid,
                &Item::Unit {
                    unit: Name::new(unit),
                },
            ) {
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
                        || !g.can_produce(pid, *cid, &Item::Unit { unit: name.clone() })
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
                            building: building.clone(),
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
        let mut best: Option<(f64, std::cmp::Reverse<(u32, Pos)>, Action)> = None;
        for action in g.legal_actions_within(pid, ActionFamilies::PURCHASES) {
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
                        building: (*building).clone(),
                    },
                )
            })
            .map(|(building, spec)| {
                let net_gold = spec.yields.gold - spec.maintenance;
                (
                    net_gold / spec.cost.max(1.0),
                    net_gold,
                    std::cmp::Reverse(spec.cost as i64),
                    std::cmp::Reverse(building.clone()),
                    building.clone(),
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
                    building: name.clone(),
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
                    std::cmp::Reverse(name.clone()),
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
                project: project.clone(),
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
        if g.city_has_district_family(&g.cities[&cid], Name::new(&family)) {
            return None;
        }
        let district = Self::civ_district(g, pid, family);
        g.district_sites(cid, &district)
            .into_iter()
            .filter_map(|pos| {
                let item = Item::District {
                    district: district,
                    pos,
                };
                g.can_produce(pid, cid, &item).then_some((
                    g.district_yields(&district, pos).total(),
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
    fn pick_item(
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
        let military_floor = if rushing {
            (self.w.mil_per_city * n_cities as f64).max(self.rush_military_floor as f64)
        } else {
            self.w.mil_per_city * n_cities as f64
        };
        if can_add_military && (military as f64) < military_floor {
            let picked = if rushing && melee < self.rush_military_floor {
                self.best_military(g, pid, cid, Some(false))
                    .or_else(|| self.combined_arms_unit(g, pid, cid, melee, ranged))
            } else {
                self.combined_arms_unit(g, pid, cid, melee, ranged)
            };
            if let Some(m) = picked {
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
            let mut projects: Vec<Item> = g
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
                    project: project.clone(),
                })
                .filter(|item| {
                    let Item::Project { project } = item else {
                        return false;
                    };
                    self.project_matches_focus(g, project) && g.can_produce(pid, cid, item)
                })
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
        if !self.minor
            && !self.barb
            && ((n_cities + settlers) as f64) < self.w.city_target
            && settlers == 0
            && (city_pop as f64) >= self.w.settler_min_pop
            && (g.turn as f64) < self.w.settler_stop_turn
            && self.has_practical_settle_site(g, pid)
        {
            return Some(Item::Unit {
                unit: crate::name!("settler"),
            });
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
            let sites = g.district_sites(cid, &harbor);
            if let Some(pos) = sites.into_iter().max_by(|a, b| {
                g.district_yields(&harbor, *a)
                    .total()
                    .partial_cmp(&g.district_yields(&harbor, *b).total())
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
        if self.minor {
            dpri.clear();
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
            if g.city_has_district_family(&g.cities[&cid], Name::new(&family)) {
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
                        let ya = g.district_yields(&dname, **a).total();
                        let yb = g.district_yields(&dname, **b).total();
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
        if !self.minor && self.culture_focus {
            if let Some(amphitheater) = Self::civ_building(g, pid, cid, "amphitheater") {
                return Some(amphitheater);
            }
        }
        if !self.minor && self.culture_focus {
            let empire_wonders = g
                .cities
                .values()
                .filter(|city| city.owner == pid)
                .map(|city| city.wonders.len())
                .sum::<usize>();
            if empire_wonders < 3 {
                let wonder = Self::cheapest_available_wonder(g, pid, cid);
                if wonder.is_some() {
                    return wonder;
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
            buildable.sort();
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
            let mut projects: Vec<Item> = g
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
                    project: project.clone(),
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
                Some(Item::Wonder { wonder, .. }) => Some(wonder.clone()),
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

    fn project_matches_focus(&self, g: &Game, project: &str) -> bool {
        !self.culture_focus || g.rules.projects[project].district != Some(crate::name!("spaceport"))
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
                let kind = g.units[&uid].kind.clone();
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
                        && b_spec.class == "military")
                        || (b_spec.class == "support"
                            && b.kind != "military_engineer"
                            && a_spec.class == "military");
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
                            && g.units[unit].kind != "military_engineer")
                            || (b.class == "support" && g.units[with].kind != "military_engineer");
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
        let upos = g.units[&uid].pos;
        let u = &g.units[&uid];
        let my_def = effective_strength(g.unit_strength(u, true), u.hp);
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
                    } else if enemy_ids.contains(&o.owner) {
                        let att = effective_strength(g.unit_strength(o, false), o.hp);
                        s -= self.w.mv_threat
                            * threat_caution
                            * 30.0
                            * ((att - my_def) / 25.0).exp();
                    }
                }
            }
            // A pair of neighbors is enough to hold a coherent line. Giving
            // every extra adjacent unit the full bonus makes dense armies
            // refuse to leave their initial cluster even when a safe campaign
            // route is open.
            s += self.w.mv_support * adjacent_support.min(2) as f64;
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
                g.apply(pid, &Action::Move { unit: uid, to: n }).is_ok()
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
                self.move_beats_holding(g, uid, routed, stay)
                    && g.apply(pid, &Action::Move { unit: uid, to: n }).is_ok()
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
    fn path_move(&self, g: &mut Game, pid: usize, uid: u32, to: Pos) -> bool {
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
        local.sort_by_key(|p| (g.wdist(*p, target), *p));
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

    fn valid_settle_site(&self, g: &Game, pid: usize, pos: Pos) -> bool {
        let Some(tile) = g.map.get(pos) else {
            return false;
        };
        !g.rules.is_water(tile)
            && g.rules.is_passable(tile)
            && !g
                .cities
                .values()
                .any(|city| (g.wdist(city.pos, pos) as f64) < self.w.min_city_dist)
            && tile
                .owner_city
                .is_none_or(|cid| g.cities[&cid].owner == pid)
    }

    fn has_practical_settle_site(&self, g: &Game, pid: usize) -> bool {
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
                let walls = c
                    .buildings
                    .iter()
                    .filter(|b| *b == "walls" || *b == "medieval_walls")
                    .count();
                walls > 0 && (support_kind == "siege_tower" || walls == 1)
            })
            .map(|c| c.pos)
            .collect();
        if targets.is_empty() {
            return false;
        }

        // Follow the melee unit closest to a compatible walled target. Newer
        // support units normally act after the army, so they naturally step
        // onto the tile their escort just vacated or currently occupies.
        let escort = g
            .units
            .values()
            .filter(|u| u.owner == pid && u.id != uid)
            .filter(|u| {
                let spec = &g.rules.units[u.kind];
                spec.class == "military" && spec.ranged_strength <= 0.0 && !spec.siege
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
    pub(crate) fn civ_district(g: &Game, pid: usize, family: &str) -> Name {
        let civ = g.players[pid].civ.as_str();
        g.rules
            .districts
            .iter()
            .find(|(_, spec)| {
                spec.replaces == Some(Name::new(&family)) && spec.unique_to.as_deref() == Some(civ)
            })
            .map(|(name, _)| name.clone())
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
            .filter(|(_, spec)| spec.replaces == Some(Name::new(&family)))
            .map(|(name, _)| Item::Building {
                building: name.clone(),
            })
            .find(|item| g.can_produce(pid, cid, item))
    }

    /// Which improvement a tile should actually get. An improvement that
    /// matches the tile's resource comes first: it is the only way to work a
    /// strategic resource or connect a luxury, and paving Iron or Wine over
    /// with a Farm forfeits that permanently. Otherwise take the most
    /// valuable yield, weighted the way the rest of this AI values output.
    fn best_improvement(g: &Game, pos: Pos, options: &[Name]) -> Option<Name> {
        let resource = g.map.get(pos).and_then(|tile| tile.resource.clone());
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
                        !g.players[pid].explored.contains(pos) && g.unit_can_traverse(uid, *pos)
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
                .filter(|(pos, _)| {
                    !g.players[pid].explored.contains(pos) && g.unit_can_traverse(uid, **pos)
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

    fn patrol_tile(&self, g: &Game, pid: usize, uid: u32, pos: Pos) -> bool {
        let Some(tile) = g.map.get(pos) else {
            return false;
        };
        let sea_unit = g.rules.units[g.units[&uid].kind].domain.as_deref() == Some("sea");
        let water = g.rules.is_water(tile);
        if sea_unit != water {
            return false;
        }
        if !g.unit_can_traverse(uid, pos) {
            return false;
        }
        tile.owner_city
            .and_then(|cid| g.cities.get(&cid))
            .is_some_and(|city| city.owner == pid)
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
        let post_memo = g.query_memo();
        let mut posts = if let Some(posts) = self.patrol_posts.get(&domain) {
            posts.clone()
        } else {
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
            self.patrol_posts.insert(domain.clone(), posts.clone());
            posts
        };
        // A conquest earlier in this same unit phase may have invalidated a
        // cached frontier tile. Keep the shared scan, but cheaply validate the
        // relatively small candidate list before routing to it.
        posts.retain(|pos| self.patrol_tile(g, pid, uid, *pos));
        drop(post_memo);
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
            return match self.nearest_enemy_for_unit(g, pid, uid, &enemy_ids) {
                Some(t) => self.tactical_step(g, pid, uid, t, &enemy_ids, radius),
                None => self.peacetime_step(g, pid, uid),
            };
        }
        self.peacetime_step(g, pid, uid)
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
    fn peacetime_step(&mut self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let upos = g.units[&uid].pos;
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
        if self.should_explore(g, pid, uid, false) && self.explore_step(g, pid, uid) {
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
            if g.unit_upgrade_target(pid, &unit.kind).is_none() {
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
                .map(|uid| g.units[uid].kind.clone())
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
        ai.peacetime_step(&mut g, 0, stranded);

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
        ai.peacetime_step(&mut g, 0, castaway);

        assert!(
            g.wdist(g.units[&castaway].pos, source) < before,
            "an embarked unit heads for shore instead of exploring on"
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

        assert!(BasicAi::reassign_governor_for_loyalty(&mut game, 0));
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
        g.map.tiles.get_mut(&commercial_hub).unwrap().district = Some(district.clone());
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

        let tower_tech = g.rules.units["siege_tower"].tech.clone().unwrap();
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
    fn culture_focus_skips_space_projects_and_finishes_amphitheaters_first() {
        let mut g = Game::new_full(1, 20, 14, 35, 300, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let cid = g.player_city_ids(0)[0];
        let theater = g.cities[&cid].owned_tiles[1];
        g.players[0].civics.insert(crate::name!("drama_poetry"));
        g.cities
            .get_mut(&cid)
            .unwrap()
            .districts
            .insert(crate::name!("theater_square"), theater);
        g.cities
            .get_mut(&cid)
            .unwrap()
            .buildings
            .push(crate::name!("monument"));

        let mut ai = BasicAi::new();
        ai.culture_focus = true;
        assert!(!ai.project_matches_focus(&g, "launch_earth_satellite"));
        assert!(ai.project_matches_focus(&g, "repair_outer_defenses"));

        let item = ai.pick_item(&g, 0, cid, 1, 1, 1, 0, 0, 1, 1, 0).unwrap();
        assert_eq!(
            item,
            Item::Building {
                building: crate::name!("amphitheater")
            }
        );
    }

    #[test]
    fn siege_support_catches_up_and_stacks_with_melee_escort() {
        let (mut g, home, _) = walled_war_game(34);
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

        assert!(BasicAi::new().siege_support_step(&mut g, 0, ram));
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
        g.cities.get_mut(&city).unwrap().captured_from = Some(1);
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
}
