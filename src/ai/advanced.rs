//! Stateful, hierarchical AI for major civilizations.
//!
//! `BasicAi` deliberately remains the small deterministic baseline.  This
//! agent adds a shared strategic model so research, production, diplomacy,
//! civilian work, and military movement pursue the same medium-term goal.
use super::{Ai, BasicAi, ForceReport, PlanReport, UnitDoctrine, Weights};
use crate::name::Name;
use crate::game::{
    Action, ActionFamilies, CityDirective, CityRole, CongressResolution, DiplomaticDeal, Game, Item,
};
use crate::reasoning::{plain, Journal};
use crate::rules::Yields;
use crate::think;
use crate::Pos;
use std::collections::{BTreeMap, BTreeSet, HashSet};

/// Local strength ratio a force group needs before it will advance or press an
/// attack unsupported. Below it the group holds on its own account, whatever
/// else is happening in the empire.
const LOCAL_SUPERIORITY_FLOOR: f64 = 0.72;
/// Radius `threatened_city` scores hostiles in. A group already inside it is
/// part of the defence rather than a column marching to it.
const THREAT_RELIEF_RADIUS: i32 = 6;
/// Turns of march a group is allowed in order to count as a relief force. Long
/// enough to cover a neighbouring front, short enough that an army on the far
/// side of the map keeps prosecuting its own campaign.
const RELIEF_MARCH_TURNS: f64 = 3.0;

/// Turn the ancient-rush window shuts, after which ordinary campaign rules
/// resume. `rush_census` finds the first walled capital at turn 80 and 43% of
/// empires holding `masonry` by then; 60 leaves the lane a margin on the wrong
/// side of that and keeps it honestly *ancient*.
const RUSH_WINDOW_CLOSES: u32 = 60;
/// Tiles a rush will march. Measured capital separations on 6p 74x46 run a
/// median 13 and a p90 17, so 16 covers roughly nine seats in ten while
/// refusing the marches that cannot arrive before the window shuts.
const RUSH_REACH: i32 = 16;
/// Melee units the stack needs before it opens, and the floor the standing
/// army is raised to while a rush is planned.
///
/// Two is what the siege actually needs: two melee units placed three apart
/// seal a six-neighbour ring, and the readiness gate
/// (`early_rush_stack_ready`) separately asks the engine's own damage curve
/// whether the staged force can finish the city before it dies — so the count
/// does not have to carry that job. Measured against 3 and 4 on 12 maps, 2
/// took the most cities (12/12) and killed the most empires.
///
/// ⚠ `RUSH_REACH` was swept to 11 and measured clearly worse — first war
/// slipped turn 34 to 51 and blows by turn 60 fell 17.9 to 4.9 — because the
/// median capital separation is 13, so a shorter reach leaves most seats with
/// no legal victim at all. Do not tighten it.
const RUSH_STACK: usize = 2;
/// Melee the empire keeps *building* while a rush is on, as distinct from the
/// stack that opens it.
///
/// These two numbers want opposite things and were one constant for too long.
/// Raising the opening stack to 3 converted better (14 of 24 games saw an
/// empire killed, against 10) but declared twelve turns later, which is the
/// wrong trade inside a window: the median kill slipped turn 47 to 56. Opening
/// at 2 and continuing to build to 4 gets both — the war starts the turn it
/// can, and the reinforcements walk into a siege already in progress.
const RUSH_ARMY: usize = 4;
/// Inner edge of the existing 3..=5 peacetime staging ring. The connected
/// treatment asks whether a current land melee unit can route to this edge;
/// it is not a fitted reach threshold.
const RUSH_STAGING_RANGE: i32 = 3;

/// Local hostile-over-friendly strength at which a city becomes a Bastion and
/// stops growing. Deliberately the same 0.45 `threatened_city` treats as a
/// locally competitive force rather than a passing scout, so the city's own
/// governor and the empire's recovery alarm cannot disagree about what counts
/// as a threat.
const BASTION_PRESSURE: f64 = 0.45;

/// Turns between a settler finishing and its city standing. `settle_siting_census`
/// measures the chosen site a median three tiles from where the alternatives
/// were judged, and a settler moves two.
const SETTLE_LAG: u32 = 3;

/// Turns a founded city must then stand to be worth its settler. A new city
/// starts at one population working its centre tile, so this is deliberately
/// not ambitious: it is the point where the city has returned the production
/// and the population the settler cost, not where it has become good.
const SETTLE_PAYBACK: u32 = 15;



/// The per-tile discount `settle_sites` already applies inside the search
/// radius. Named here because a census that scores siting against raw
/// `settle_value` is scoring it against an objective the agent never held.
const SETTLE_DISTANCE_PENALTY: f64 = 0.9;

/// How far above its empire's per-city mean a yield must stand before it names
/// the city's role. At 1.0 every city would be typed by a coin flip around the
/// average; 1.15 asks for a real lead, so an empire of interchangeable cities
/// correctly gets no roles at all rather than arbitrary ones.
const ROLE_MARGIN: f64 = 1.15;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GrandStrategy {
    Expansion,
    Science,
    Culture,
    Religion,
    Diplomacy,
    Conquest,
    Recovery,
}

impl GrandStrategy {
    pub fn as_str(self) -> &'static str {
        match self {
            GrandStrategy::Expansion => "expansion",
            GrandStrategy::Science => "science",
            GrandStrategy::Culture => "culture",
            GrandStrategy::Religion => "religion",
            GrandStrategy::Diplomacy => "diplomacy",
            GrandStrategy::Conquest => "conquest",
            GrandStrategy::Recovery => "recovery",
        }
    }
}

/// How many turns an agent actually spent pursuing each grand strategy.
///
/// A plan is chosen every few turns and only the latest one is visible in
/// `plan_report`, so an end-of-game snapshot cannot say whether a war was ever
/// prosecuted or merely survived. Wars in this engine last 50-150 turns and
/// take almost nothing (measured: 17 declarations, 4 cities, 0 capitals over 12
/// full-length six-player games), and the difference between "the AI chose
/// Conquest and failed to execute it" and "the AI was in Recovery the whole
/// time" is not otherwise observable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StrategyCensus {
    pub expansion: u32,
    pub science: u32,
    pub culture: u32,
    pub religion: u32,
    pub diplomacy: u32,
    pub conquest: u32,
    pub recovery: u32,
    /// Force-group turns by posture. A different denominator from the strategy
    /// counts above — one turn contributes one strategy and as many postures as
    /// the empire had force groups — because deciding to conquer and actually
    /// advancing on a city are separate failures and only the second one shows
    /// up as a captured city.
    pub muster: u32,
    pub advance: u32,
    pub engage: u32,
    pub hold: u32,
    pub recover: u32,
    /// Which disjunct sent a group to `Hold`, attributed causally: a group
    /// below [`LOCAL_SUPERIORITY_FLOOR`] holds on its own weakness whether or
    /// not a city is threatened, and is counted as `hold_weak` even when one
    /// is. Only a group strong enough to advance, halted to relieve a city it
    /// can actually reach, counts as `hold_threatened`.
    pub hold_threatened: u32,
    pub hold_weak: u32,
}

impl StrategyCensus {
    pub fn total(&self) -> u32 {
        self.expansion
            + self.science
            + self.culture
            + self.religion
            + self.diplomacy
            + self.conquest
            + self.recovery
    }

    fn count_posture(&mut self, posture: ForcePosture) {
        let slot = match posture {
            ForcePosture::Muster => &mut self.muster,
            ForcePosture::Advance => &mut self.advance,
            ForcePosture::Engage => &mut self.engage,
            ForcePosture::Hold => &mut self.hold,
            ForcePosture::Recover => &mut self.recover,
        };
        *slot += 1;
    }

    /// Force-group turns counted, which is not [`StrategyCensus::total`].
    pub fn posture_total(&self) -> u32 {
        self.muster + self.advance + self.engage + self.hold + self.recover
    }

    fn count(&mut self, strategy: GrandStrategy) {
        let slot = match strategy {
            GrandStrategy::Expansion => &mut self.expansion,
            GrandStrategy::Science => &mut self.science,
            GrandStrategy::Culture => &mut self.culture,
            GrandStrategy::Religion => &mut self.religion,
            GrandStrategy::Diplomacy => &mut self.diplomacy,
            GrandStrategy::Conquest => &mut self.conquest,
            GrandStrategy::Recovery => &mut self.recovery,
        };
        *slot += 1;
    }

    /// Accumulate another agent's turns into this total.
    pub fn absorb(&mut self, other: &StrategyCensus) {
        self.expansion += other.expansion;
        self.science += other.science;
        self.culture += other.culture;
        self.religion += other.religion;
        self.diplomacy += other.diplomacy;
        self.conquest += other.conquest;
        self.recovery += other.recovery;
        self.muster += other.muster;
        self.advance += other.advance;
        self.engage += other.engage;
        self.hold += other.hold;
        self.recover += other.recover;
        self.hold_threatened += other.hold_threatened;
        self.hold_weak += other.hold_weak;
    }
}

/// A concrete game-ending objective. Unlike `GrandStrategy`, which may
/// temporarily become Expansion or Recovery, this remains fixed for the
/// lifetime of a deliberately targeted AI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VictoryTarget {
    Science,
    Culture,
    Religion,
    Diplomacy,
    Domination,
    Score,
}

impl VictoryTarget {
    pub const ALL: [VictoryTarget; 6] = [
        VictoryTarget::Science,
        VictoryTarget::Culture,
        VictoryTarget::Religion,
        VictoryTarget::Diplomacy,
        VictoryTarget::Domination,
        VictoryTarget::Score,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            VictoryTarget::Science => "science",
            VictoryTarget::Culture => "culture",
            VictoryTarget::Religion => "religious",
            VictoryTarget::Diplomacy => "diplomatic",
            VictoryTarget::Domination => "domination",
            VictoryTarget::Score => "score",
        }
    }

    fn strategy(self) -> GrandStrategy {
        match self {
            VictoryTarget::Science => GrandStrategy::Science,
            VictoryTarget::Culture => GrandStrategy::Culture,
            VictoryTarget::Religion => GrandStrategy::Religion,
            VictoryTarget::Diplomacy => GrandStrategy::Diplomacy,
            VictoryTarget::Domination => GrandStrategy::Conquest,
            VictoryTarget::Score => GrandStrategy::Expansion,
        }
    }
}

impl std::str::FromStr for VictoryTarget {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "science" => Ok(VictoryTarget::Science),
            "culture" => Ok(VictoryTarget::Culture),
            "religion" | "religious" => Ok(VictoryTarget::Religion),
            "diplomacy" | "diplomatic" => Ok(VictoryTarget::Diplomacy),
            "domination" | "conquest" => Ok(VictoryTarget::Domination),
            "score" => Ok(VictoryTarget::Score),
            _ => Err(format!("unknown victory target {value:?}")),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrategicPlan {
    pub strategy: GrandStrategy,
    pub target_player: Option<usize>,
    pub target_city: Option<u32>,
    pub threatened_city: Option<u32>,
    pub desired_cities: usize,
    pub assessed_turn: u32,
    /// Whether this plan is an ancient rush. Carried on the plan rather than
    /// re-derived, because the production valuation runs it for every
    /// candidate item in every city and `early_rush_victim` walks the world.
    pub rush: bool,
}

/// Movement domain for a coordinated force. The same planner operates on
/// armies, fleets, and future domains without baking land-unit assumptions
/// into the campaign layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ForceDomain {
    Land,
    Sea,
}

impl ForceDomain {
    pub fn as_str(self) -> &'static str {
        match self {
            ForceDomain::Land => "land",
            ForceDomain::Sea => "sea",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ForcePosture {
    Muster,
    Advance,
    Engage,
    Hold,
    Recover,
}

impl ForcePosture {
    pub fn as_str(self) -> &'static str {
        match self {
            ForcePosture::Muster => "muster",
            ForcePosture::Advance => "advance",
            ForcePosture::Engage => "engage",
            ForcePosture::Hold => "hold",
            ForcePosture::Recover => "recover",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ForceRole {
    Recon,
    Vanguard,
    Mobile,
    Ranged,
    Siege,
    Support,
    AirStrike,
}

/// A deterministic, inspectable order shared by a group of nearby units.
/// `focus_target` is recomputed every turn from attacks available to the
/// entire force, preventing units from selecting unrelated victims.
#[derive(Clone, Debug, PartialEq)]
pub struct ForceGroup {
    pub id: u32,
    pub domain: ForceDomain,
    pub units: Vec<u32>,
    pub anchor: Pos,
    pub objective: Pos,
    pub focus_target: Option<Pos>,
    pub posture: ForcePosture,
    pub readiness: f64,
    pub local_strength_ratio: f64,
}

#[derive(Default)]
struct EmpireCounts {
    settlers: usize,
    builders: usize,
    traders: usize,
    scouts: usize,
    military: usize,
    melee: usize,
    ranged: usize,
    naval: usize,
    naval_melee: usize,
    naval_ranged: usize,
    naval_raider: usize,
    carriers: usize,
    aircraft: usize,
    siege: usize,
    support: usize,
    air_defense: usize,
    military_engineers: usize,
    missionaries: usize,
}

#[derive(Clone, Copy)]
struct VictoryFocus {
    strategy: GrandStrategy,
    progress: i32,
}

impl EmpireCounts {
    fn add_unit(&mut self, g: &Game, name: &str) {
        match name {
            "settler" => self.settlers += 1,
            "builder" => self.builders += 1,
            "trader" => self.traders += 1,
            "missionary" => self.missionaries += 1,
            "military_engineer" => {
                self.support += 1;
                self.military_engineers += 1;
            }
            "scout" => {
                self.scouts += 1;
                self.military += 1;
                self.melee += 1;
            }
            _ => {
                let spec = &g.rules.units[name];
                if spec.class == "military" {
                    self.military += 1;
                    if spec.domain.as_deref() == Some("sea") {
                        self.naval += 1;
                        match spec.promotion_class.as_str() {
                            "naval_melee" => self.naval_melee += 1,
                            "naval_ranged" => self.naval_ranged += 1,
                            "naval_raider" => self.naval_raider += 1,
                            "naval_carrier" => self.carriers += 1,
                            _ => {}
                        }
                    } else if spec.domain.as_deref() == Some("air") {
                        self.aircraft += 1;
                    } else {
                        if spec.is_melee_capable() {
                            self.melee += 1;
                        }
                        if spec.has_ranged_attack() {
                            self.ranged += 1;
                        }
                    }
                    if spec.siege && spec.domain.as_deref() != Some("air") {
                        self.siege += 1;
                    }
                } else if spec.class == "support" {
                    self.support += 1;
                    if spec.anti_air_strength > 0.0 {
                        self.air_defense += 1;
                    }
                }
            }
        }
    }

    fn add_item(&mut self, g: &Game, item: &Item) {
        match item {
            Item::Unit { unit } | Item::Formation { unit, .. } => self.add_unit(g, unit),
            _ => {}
        }
    }
}

/// Consecutive turns a committed settler may fail to move before its site is
/// released. Three is long enough to walk around a unit standing in the way
/// and short enough that a genuinely unreachable site cannot hold a settler
/// hostage — the failure mode #492 was merged to remove.
const SETTLER_STALL_LIMIT: u32 = 3;

#[derive(Clone)]
pub struct AdvancedAi {
    base: BasicAi,
    plan: Option<StrategicPlan>,
    census: StrategyCensus,
    settler_targets: BTreeMap<u32, Pos>,
    builder_targets: BTreeMap<u32, Pos>,
    major_war_since: Option<u32>,
    last_campaign_progress: u32,
    last_city_count: usize,
    peace_until: u32,
    victory_planning: bool,
    victory_target: Option<VictoryTarget>,
    forced_target_player: Option<usize>,
    force_groups: Vec<ForceGroup>,
    force_groups_dirty: bool,
    /// Hold only the force groups that could actually reach the threatened
    /// city, instead of every group in the empire.
    ///
    /// **Off by default, on measurement.** It does what it says — over eight
    /// six-player games it cut holds by a group strong enough to advance from
    /// 19.0% of force-group turns to 10.4%, and the ones left standing were
    /// 8.8 hexes from the emergency rather than 13.2 — and that bought
    /// nothing. Pre-registered at 120 mirrored maps against the shipped
    /// behaviour it scored 49.2% (Wilson 40.4%..58.0%, Elo-equivalent -6,
    /// sign p=0.8555), `promotion gate: INCONCLUSIVE`.
    ///
    /// The reading that survives is that mobility is not the binding
    /// constraint: an army freed to march still arrives with 81% of its
    /// units three eras stale and converts a spent garrison into a capture
    /// 22% of the time. Kept behind this flag, and reachable as the
    /// `advanced_relief_scoped` entrant, so it can be re-measured once the
    /// conversion bottleneck moves rather than re-derived from scratch.
    pub scoped_relief_hold: bool,
    /// Exclude victory lanes the empire cannot finish before the game ends.
    ///
    /// **Off by default, on the pre-registered rule.** Routing toward a lane
    /// that is arithmetically out of reach looked like a defect rather than a
    /// preference, and the filter demonstrably fires — but at 120 mirrored
    /// maps it measured no stronger than the permissive control: 49.6% paired
    /// score (95% Wilson CI 40.8%..58.4%), Elo-equivalent -3, sign p=1.0000,
    /// promotion gate INCONCLUSIVE. The pre-registration said a failure ships
    /// the flag off with the null recorded, so it does.
    ///
    /// Reachable as the `advanced_lane_reachable` entrant, so it can be
    /// re-measured once victory routing actually binds rather than re-derived
    /// from scratch. Worth knowing before re-running it: in that eval 103 of
    /// 120 `advanced` wins were religious, so the science lane the filter
    /// exists to refuse was rarely the one being contested.
    pub refuse_unreachable_lanes: bool,
    /// Test the finite Prophet race before the opportunistic war, not after.
    ///
    /// **Off by default until measured.** `assess()` reaches
    /// `religious_opening_viable` only after an arm that fires on a bare power
    /// ratio — `turn >= 55 && cities >= 2 && my_power > weakest_rival * 1.80 +
    /// 20.0`. `war_census` records this agent opening its wars at a mean
    /// **11.5×** advantage, so that test is satisfied many times over whenever
    /// it is asked, and `religious_opening_viable` hard-stops at turn 120 (180
    /// once a religion exists). Those two windows overlap on turns 55..120,
    /// which is the whole of the prophet race.
    ///
    /// The arm below already argues its own case: a Prophet is a *finite
    /// global* slot, and pursuing it occupies one city's production while the
    /// rest of the empire carries on. An opportunity to attack a weak
    /// neighbour is not finite in the same way — it is still there ten turns
    /// later, and this engine converts it into a domination victory in 0 of
    /// every 48 games measured.
    ///
    /// `at_war` keeps its priority either way: a war already running is not an
    /// opportunity, it is a fact. With the flag off the cascade is
    /// arithmetically identical to `at_war || A || B` reaching Conquest first,
    /// so this ships zero behaviour change until the entrant is selected.
    pub prophet_before_opportunism: bool,
    /// What a settler is worth against everything else the city could build.
    ///
    /// **1.0 by default — the shipped behaviour exactly.** The settler arm of
    /// `production_value` scores `920.0 + site_value * 4.0`, and **920 is a
    /// hardcoded literal**: not a gene, so no run of `evolve` has tuned it, and
    /// not a doctrine lever, so the macro search has never perturbed it.
    ///
    /// `expansion_funnel` (`docs/EVAL.md`, 2026-07-28) measured what that
    /// literal decides. Over 48 seats and 12 full games, the empire was short
    /// of its own planned city target, permitted a settler, and had a reachable
    /// site on **3391 seat-turns — 25.8% of all of them — and built something
    /// else every single time.** The count of turns blocked by having nowhere
    /// to go was **zero**. Cities end at 4.02 against a planned 5.25, so this
    /// is not a disagreement about how many cities to want; the agent simply
    /// never pays for them.
    ///
    /// ⚠ The funnel says the settler loses. It does **not** say by how much, so
    /// any value here is unconstrained by the measurement. Pick one, register
    /// it, run it once — do not sweep several against the same maps, which is
    /// the selection bias that dissolved this repository's coordinate-descent
    /// result on resampling.
    pub settler_price: f64,
    /// How much better a candidate must be before a city abandons what it is
    /// already building. **1.0 by default, which disables preemption entirely
    /// and reproduces the shipped behaviour exactly.**
    ///
    /// `advanced_production` skips any city whose queue is non-empty, so
    /// `production_value` is consulted only on an idle city — this agent never
    /// reconsiders a build once started. `expansion_funnel` measured what that
    /// costs: over 48 seats, on **25.8% of all seat-turns** the empire was
    /// short of its own planned city target, permitted a settler, had a
    /// reachable site, and every city was mid-build. The genuine valuation
    /// loss — a free city choosing something else — is only **2.6%**.
    ///
    /// The plan above this re-assesses every 5 turns (`plan_stale`); the queue
    /// underneath re-assesses never. Switching is close to free here because
    /// `City::production_progress` banks a paused build by item key, which is
    /// the Civ 6 rule and the reason a strong human switches to a settler
    /// routinely.
    ///
    /// ⚠ A margin at or below 1.0 means "switch on any improvement", which
    /// invites oscillation between two nearly equal candidates re-scored every
    /// turn. That is why the disabled value is 1.0 rather than 0.0: the flag is
    /// a *ratio*, and the off state is the identity.
    pub preempt_margin: f64,
    /// Let an assigned Religion lane expand first, like every other lane.
    ///
    /// ⚠⚠ **MEASURED AND REJECTED. Leave it off.** Applied consistently to the
    /// acting agent and the macro search's branches together
    /// (`StrategicAi::set_religion_may_expand`, entrant
    /// `strategic_religion_expand`) it measured **−53 Elo** over 120 mirrored
    /// maps, sign p=0.0014 against it. Applied to the actor alone it was a null
    /// end-to-end (4 helped / 7 hurt, p=0.5488). The skipped test turns out to
    /// be load-bearing: expansion costs value inside the rollout horizon, so
    /// permitting it makes the religion lane project *worse* and the search
    /// routes away from the lane it actually converts. See `docs/EVAL.md`,
    /// 2026-07-28.
    ///
    /// Kept reachable, on the `advanced_lane_reachable` precedent, so the axis
    /// can be re-measured against a longer rollout horizon rather than
    /// re-derived from scratch — the result depends on a settler not paying
    /// back before a branch is scored, which is a statement about the window.
    ///
    /// **Off by default.** In `assess()` an explicitly targeted
    /// seat normally asks "can this lane still afford to expand first?" before
    /// pursuing its target. The Religion arm is the sole exception: a targeted
    /// seat with no religion yet goes straight to `GrandStrategy::Religion`,
    /// skipping the expansion test entirely.
    ///
    /// Measured with `commit_curve` on the shipped genome, 40 maps at 4p 60×38:
    /// a seat committed to Religion at turn 0 finishes on **1.68 cities** and
    /// wins **15.0%**; committed at turn 60 it reaches 2.48 cities and 30.0%;
    /// the adaptive control reaches **4.10 cities** and 27.5%. Committing to
    /// this lane does not produce an agent that plays religion well, it
    /// produces an agent that never expands.
    ///
    /// **This is not only about targeted play.** `StrategicAi` projects every
    /// macro-search branch by calling `retarget`, so each religion branch is
    /// simulated by a seat that stops expanding — a systematic mis-projection
    /// of the lane this engine converts best. The macro search is the one
    /// component here that has ever won Elo, so a fidelity defect in it is
    /// worth more than the arm it sits in suggests. Compare
    /// `continue_from_plan`, which was worth +37 Elo for the same class of
    /// reason: the counterfactual was simulating the wrong thing.
    pub assigned_religion_may_expand: bool,
    /// Weigh whether a settle site can be held, not only what it yields.
    ///
    /// **Off by default, on measurement.** `settle_value` scores yields,
    /// fresh water and a coastal bonus, then penalises proximity to a rival
    /// major — and explicitly filters barbarians out of that penalty. So a
    /// site beside a barbarian city scores exactly as well as an empty one,
    /// and a site with no friendly city within reach scores as well as one
    /// inside the empire.
    ///
    /// Measured over 16 six-player games: barbarians take **7.0 major cities
    /// per game**, 65% of everything a major loses, at a **median city age of
    /// ten turns**. City-states take none. Only 4% are ever recovered by
    /// their founder and 35% go straight to another major, so a camp launders
    /// cities between empires. Cities held at the end correlate with final
    /// score at r = +0.89 while cities *founded* correlate at r = -0.03:
    /// retention is what the standings turn on, and nothing in the site score
    /// weighs it.
    ///
    /// Mixed-seat A/B, three treated seats against three control in the same
    /// game, seat assignment flipped, 72 games / 432 seats, paired per-game
    /// t-tests: cities lost to barbarians **-0.48** (1.10 -> 0.62,
    /// p < 0.0001) and cities held **+1.09** (p = 0.0001), both replicated on
    /// a fresh seed set. Finishing rank -0.51 (p = 0.0042 pooled) but it
    /// regressed from p = 0.014 to p = 0.137 between halves and the sign test
    /// is only 42-30, so treat the rank effect as weak. **Wins did not move:
    /// 35-37.** Shipped off with the measurement recorded, so the retention
    /// result can be re-derived rather than re-discovered.
    pub defensible_sites: bool,
    /// Tell this empire's governors to want food while it is still short of
    /// its city target.
    ///
    /// `docs/OPENINGS.md` §8 and §11: capital growth gates every settler — a
    /// settler needs `pop >= 2` and consumes one — and the capital gains about
    /// one population per 23 turns, which *is* the city founding interval.
    /// `citizen_strategy` ships wanting production 1.55 against food 1.25, and
    /// reassigning the same tiles toward food raises the capital's food
    /// **surplus** 44–87% at a cost of 18–27% of its production.
    ///
    /// Growth gates the settler; production pays for it. Which wins is not
    /// derivable, so this is an eval arm (`advanced_food_first`) and not a
    /// default. The bias is +0.6 — a moderate shift that puts food just above
    /// production, not the food-10.0 arm that measured the ceiling — and it is
    /// **withdrawn once the empire reaches its city target**, so it buys
    /// expansion tempo rather than permanently detuning the economy.
    pub food_first: f64,
    /// Hold a settler's chosen site across a turn it could not move, instead
    /// of forgetting it.
    ///
    /// `docs/OPENINGS.md` §15: over 17,701 settler-turns the agent ends 27.1%
    /// of them holding no destination at all, and 3.5% holding a different one
    /// than the turn before. The cause is not a re-plan — `settler_step`
    /// discards the target on any turn the unit fails to move, and filters it
    /// out whenever `route_step` is momentarily `None` (a friendly unit in the
    /// way, a zone of control, an unrevealed tile). None of those mean the
    /// site got worse.
    ///
    /// The commitment is **bounded**, which is the whole design: an unbounded
    /// hold would re-create exactly the livelock #492 was merged to fix, a
    /// settler retrying an unreachable site forever. After
    /// `SETTLER_STALL_LIMIT` consecutive turns without moving, the target is
    /// released and the ordinary search runs again.
    pub settler_commit: bool,
    /// Consecutive turns each settler has failed to move, when `settler_commit`
    /// is on. Reset on any successful step.
    settler_stalls: BTreeMap<u32, u32>,
    /// Let more than one settler exist at a time, up to the shortfall against
    /// the city target.
    ///
    /// The settler production gate carries `counts.settlers == 0` on an
    /// `EmpireCounts`, so today at most one settler may exist in the whole
    /// empire. The conjunct beside it already caps cities-plus-settlers at
    /// `desired_cities`, so this one adds no cap — it is purely serialization,
    /// and a four-city empire therefore expands no faster than a one-city one.
    ///
    /// Measured before building (`docs/OPENINGS.md` §6): over 60 maps at 4
    /// players on 32×22 the seat first holds 2/3/4/5/6 cities on turns
    /// 37.0/71.0/89.5/118.7/150.2 — gaps of +34.0/+18.5/+29.2/+31.5 that **do
    /// not shrink as the empire grows**, which is what serialization looks
    /// like and is not what compounding expansion looks like. A seat spends
    /// 60.8 ± 3.8 turns short of its city target with a settler already
    /// walking, against 68.5 ± 4.2 turns short with none.
    ///
    /// This is a *rate* lever and is not the `city_target` sweep in
    /// `docs/GENOME.md`, which is a *target* lever and saturates above six.
    /// Reaching six cities on turn 90 rather than turn 150 compounds those
    /// yields for sixty turns at the same target.
    ///
    /// ⚠ **Measured near-INERT by its own fires-check, and never taken to an
    /// eval.** Over the same 60 maps, turning it on moves the founding cadence
    /// from 37.0/71.0/89.5/118.7/150.2 to 37.6/71.0/89.1/117.6/148.7 and
    /// leaves cities-at-turn-50 at 1.95 either way.
    ///
    /// The mechanism story above was wrong. `counts.settlers == 0` is
    /// redundant on top of engine rules that already bind harder: a settler
    /// requires `pop >= 2` and **consumes a population** on completion
    /// (`Game` at the `settler_no_population` governor check), and successive
    /// settlers cost 80, 110, 140 production. A one- or two-city empire
    /// therefore cannot afford a second settler whether or not the AI permits
    /// one, so lifting the permission buys nothing. The 60.8 ± 3.8 turns a
    /// seat spends short of target with a settler walking are not turns this
    /// clause forbids a second — they are turns the empire could not pay for
    /// one.
    ///
    /// Kept as the `advanced_parallel_settlers` entrant with the null
    /// recorded, on the `advanced_lane_reachable` precedent, so the axis can
    /// be re-measured rather than re-derived if the settler economy changes.
    pub parallel_settlers: bool,
    /// Give every city a strategy: stamp a [`CityDirective`] on each of this
    /// empire's cities every turn, so the citizen governor can see the plan.
    ///
    /// **The gap it closes is structural, not a tuning question.**
    /// `Game::citizen_strategy` decides which tiles a city works from purely
    /// local evidence — the districts standing there, the item in the queue,
    /// the civilization — plus one empire-wide `at_war` boolean and the one
    /// scalar `citizen_food_bias`. So the thousands of tile assignments that
    /// actually produce the empire's yields are blind to the victory lane the
    /// macro search spends its whole budget choosing, and a city on the
    /// frontier of a war reacts exactly like one four hundred tiles behind it.
    /// The engine says so itself beside `citizen_food_bias`: *citizen
    /// assignment is the one city-level decision no player, human or AI, can
    /// currently express*.
    ///
    /// A directive carries three things down, and keeps them on separate axes
    /// on purpose:
    ///
    /// - `emphasis` — the **empire objective**, `plan.strategy` translated
    ///   into yield appetite.
    /// - `role` — the **local optimization**, what this particular city is
    ///   best used for given its own terrain, districts and the empire's
    ///   remaining need for settlers.
    /// - `pressure` — **military awareness**, per city rather than per empire,
    ///   reusing the hostile-over-friendly ratio `threatened_city` already
    ///   measures within six tiles.
    ///
    /// It is deliberately a *scripted* policy that threads existing signals,
    /// not a learned one and not a search: every yield weight it writes is one
    /// a human can read and defend, which is the property that the value-net
    /// arms in this repo lacked when an argmax found the cheapest correlate to
    /// maximise. Off by default and reachable as the `advanced_city_strategy`
    /// entrant, paired against `advanced`.
    ///
    /// ⚠ **The first screen LOST**: 42.5% paired over 120 maps at seed 411000,
    /// Elo-equivalent −53, exact sign p=0.0014 against, terminal score 45.0%
    /// at p=0.0000 resting on all 120 maps. Plan commitment was identical in
    /// both arms (100% adaptive, 0.00 switches), so nothing was rerouted — the
    /// treatment simply built a uniformly smaller empire: cities 2.18 against
    /// 2.76, food 36.9 against 45.5, pop 13.4 against 16.7.
    ///
    /// `city_strategy_emphasis` and `city_strategy_roles` decompose that.
    /// Close the expansion window on whether a settler would pay for itself
    /// rather than on a flat end-of-game reserve.
    ///
    /// See `expansion_pays_back_for`. The motivation is measured, not
    /// aesthetic: #554 showed a free settler while short of the city target
    /// more than doubles the win rate (23.0% to 52.3%, p=0.0000 over 300
    /// games), and #559 localised where the settler is refused — on the
    /// deployment map the shut window is the **sole** blocker on 31.2% of the
    /// city-turns an empire spends short of its own target.
    ///
    /// ⚠ It replaces one constant with two smaller ones. That is a real
    /// objection and the reason this is an entrant rather than a default: what
    /// makes it more than retuning is that the reserve now scales with the
    /// city's actual production rate, so a strong city may still expand late
    /// and a weak one stops earlier than the flat rule allowed.
    ///
    /// Reachable as `advanced_expansion_payback`, paired against `advanced`.
    pub expansion_pays_back: bool,
    /// Where the city-target ramp starts. **3 by default, the shipped value.**
    ///
    /// `assess` computes `desired_cities = (3 + turn / cadence).min(map_capacity).min(6)`
    /// — the empire wants three cities at the opening and adds roughly one per
    /// era. #554's oracle handed a seat settlers up to **six** from the start
    /// and more than doubled its win rate (23.0% to 52.3%, p=0.0000 over 300
    /// games). The gap between those two numbers is this ramp.
    ///
    /// ⚠ **`GENOME.md`'s "`city_target` saturates above six" does not cover
    /// this.** That sweep moved the `city_target` *gene*, and the gene is only
    /// reached through `unwrap_or_else` when there is no plan — a live
    /// `AdvancedAi` reads `plan.desired_cities` and never consults it. The
    /// sweep measured a fallback path, which is why every value above six came
    /// back identical to four decimal places.
    ///
    /// #569 removed the affordability objection: the missing cities cost 0.5%
    /// of everything the empire produces. And on the deployment map the empire
    /// reaches 4.83 of its own 5.00 target, so what limits it is the target,
    /// not its ability to hit one.
    ///
    /// Reachable as `advanced_wide_opening`, paired against `advanced`.
    pub city_target_floor: usize,
    pub city_strategy: bool,
    /// Ablation halves of `city_strategy`, so the loss above can be attributed
    /// rather than guessed at. Each is meaningless unless `city_strategy` is
    /// also on; together they are the full treatment.
    ///
    /// The hypothesis they test: the **emphasis** carries the whole deficit,
    /// because it applies a lane's yield appetite from turn 1 at full strength
    /// in every city, and `docs/OPENINGS.md` establishes that capital growth
    /// gates every settler. Religion — the dominant lane in the screen — raises
    /// faith 0.90 → 1.40, a +56% relative jump, applied exactly when the empire
    /// cannot afford to look away from food. Roles and pressure, by contrast,
    /// only ever ask for food or hammers.
    ///
    /// If the emphasis-only arm reproduces the loss and the roles-only arm does
    /// not, the fix is a phase gate: the empire's objective does not get to
    /// speak until expansion is finished. If instead roles-only carries it, the
    /// role ladder is wrong and the emphasis is exonerated — which would refute
    /// the paragraph above.
    pub city_strategy_emphasis: bool,
    pub city_strategy_roles: bool,
    /// Forbid the two comparative rungs of the role ladder — Forge and
    /// Specialist — while the empire is still short of its city target.
    ///
    /// This is the repair for the defect `role_ladder_census` found, described
    /// on `city_strategy` above. On by default so `advanced_city_strategy`
    /// carries it; `advanced_city_strategy_roles_raw` keeps the measured-worse
    /// behaviour reachable so the 42.1% result stays reproducible.
    pub city_strategy_expansion_first: bool,
    /// Per-rung ablation switches, so the roles half can be attributed
    /// mechanically instead of guessed at a fourth time.
    ///
    /// Three mechanisms have now been proposed for the roles half's 42.1% and
    /// **all three were refuted by measurement**: the lane emphasis (the
    /// emphasis-only arm is a clean null at both 4p/24x16 and the deployment
    /// 6p/74x46), the comparative rungs (`expansion_first` halved Forge and
    /// Specialist from 24.8% of city-turns to 10.3% and moved the result by
    /// 0.4 points, inside noise), and an early Bastion blocking the `pop >= 2`
    /// settler gate (`role_ladder_census` bands Bastion at 9.0% / 2.9% /
    /// **20.8%** / 1.2% across turns 1-39 / 40-79 / 80-139 / 140+, so it is a
    /// late-game phenomenon and misses the settler window entirely).
    ///
    /// Reasoning from the weights has therefore failed three times in a row on
    /// this treatment, which is the signal to stop. Each switch isolates one
    /// rung so a 2-minute paired run can answer what the arithmetic could not.
    /// Let a Bastion halt its own growth, which is what the first three arms
    /// did and is the single mechanism the per-rung bisect convicted. Off.
    pub city_strategy_halt_growth: bool,
    pub city_strategy_bastion: bool,
    pub city_strategy_breadbasket: bool,
    pub city_strategy_comparative: bool,
    pub city_strategy_pressure: bool,
    /// Ignore every by-name civilization signal in the decision layer.
    ///
    /// An **ablation**, not a strategy. `docs/GENOME.md`'s rule is that a null
    /// on selection is uninterpretable without the ceiling beside it: before
    /// asking whether per-civilization openings could be *better*, ask what
    /// the civilization-aware code already there is *worth*. This is that
    /// question, asked the cheapest honest way — by taking it away.
    ///
    /// Six sites, all decision-layer rather than mechanics: the Greece
    /// culture-lane preference and its +45 culture floor, the China +45
    /// science floor, the +55 unique-unit bonus in `tech_value`, and the
    /// `Egypt | China` wonder exemption in `production_value` (twice). It
    /// deliberately does **not** touch which unique unit or district a
    /// civilization may build — that is mechanics, and ablating it would
    /// measure the uniques rather than the decisions about them.
    ///
    /// Reachable as the `advanced_civ_blind` entrant. A large cost means the
    /// civilization-aware code carries real weight and better per-
    /// civilization play could exist. A small cost bounds the layer, the way
    /// deleting the opening book bounded that one at −0.003.
    pub civ_blind: bool,

    /// Whether this empire reacts at all to a rival closing on a victory.
    ///
    /// `true` — the default and the shipped behaviour — lets `victory_denial`
    /// name the rival nearest a win and hand back a counter-strategy, and lets
    /// `urgent_victory_threat` waive the ordinary war-readiness checks against
    /// a terminal clock. `false` makes both silent: the empire still fights,
    /// still expands, still races, but never because somebody else is about to
    /// win.
    ///
    /// Reachable as the `advanced_blind_to_leaders` entrant. It exists because
    /// `leader_census` measured the layer as a near-perfect *predictor* and no
    /// deterrent at all — 83–86% of every empire it ever names goes on to win,
    /// against a 17–25% base rate, and 79–82% even when war followed the
    /// alarm. Paired against `advanced`, this is what the whole counter-leader
    /// response is worth. A null bounds it at nothing, the way the goal layer
    /// was bounded; a real cost says the response works and only its timing is
    /// wrong.
    pub deny_leaders: bool,

    /// Whether the empire will open an **ancient rush**: pick the nearest
    /// weak neighbour before the walls go up, march a small stack to their
    /// capital, and declare only once it is already adjacent.
    ///
    /// Every number this lane uses was measured on this engine by
    /// `rush_census` (12 six-player 74x46 games, seed 900000) rather than
    /// carried over from Civ 6 intuition, because the two disagree sharply:
    ///
    /// - **No capital anywhere carries a wall before turn 80** (0% at turns
    ///   20-60, 8.3% at 80), and no empire holds `masonry` at turn 50. The
    ///   walled-city problem that dominates siege design simply does not
    ///   exist inside this window.
    /// - **Capitals sit at `city_strength` 17.2 at turn 50 with a mean
    ///   garrison of 0.7** — they are, on average, empty. A Monte Carlo over
    ///   the engine's own `damage`/`city_strength` formulas puts two warriors
    ///   at 100% capture against that profile and four at 100% even when the
    ///   defender pulls its whole field army home.
    /// - The nearest rival capital is a median 13 tiles away (p90 17), which
    ///   is 9 and 12 turns of marching. That march, not the army, is the
    ///   binding cost.
    ///
    /// The timing rule is the point of the lane. The defender only prioritises
    /// `walls` once `threatened` fires, and `threatened_city` requires hostile
    /// units within 6 tiles *while already at war*. Walls cost 80 production
    /// against an early city's handful per turn. So a declaration issued from
    /// an already-adjacent stack cannot be answered, while the same
    /// declaration issued at marching distance hands the victim ten turns of
    /// warning. `advanced` cannot express this at all: `assess` withholds
    /// `Conquest` until turn 55 for all but five hardcoded civilizations, and
    /// `advanced_war_declaration` carries a hard `turn < 35` floor — both
    /// after the window this lane plays in.
    ///
    /// Reachable as `advanced_rush`. Paired against `advanced` it isolates the
    /// early-aggression lane and nothing else.
    pub early_rush: bool,
    /// Restrict `early_rush` to rivals a starting land melee unit can actually
    /// reach. The reachable set is frozen once all living major capitals have
    /// been founded, before this treatment can change movement, research,
    /// production, diplomacy, or war.
    ///
    /// Reachable as `advanced_rush_connected`; see `docs/RUSH.md` section 9.
    pub route_connected_rush: bool,
    rush_route_targets: Option<BTreeSet<usize>>,

    /// Whether a Science or Expansion threat is answered by racing the leader
    /// in that lane instead of by declaring on them.
    ///
    /// Four of the seven races already answer themselves. The two that answer
    /// with an army are the two the deployment-scale census argues against:
    /// at 60x38 an empire at war with one or two rivals wins 4.4% and 10.7%
    /// of its seats against a 16.7% base rate, and the shipped response
    /// already costs terminal score (44 map-directions to 65, sign p=0.055)
    /// without buying a win. Reachable as `advanced_counter_in_lane`. It keeps
    /// the alarm and changes only what the alarm asks for, so paired against
    /// `advanced` it isolates the response's *shape* from its existence --
    /// which `advanced_blind_to_leaders` already bounds from the other side.
    pub counter_in_lane: bool,

    /// Whether a Science or Expansion threat is simply not reacted to.
    ///
    /// `counter_in_lane` changes two things at once: it stops the empire
    /// declaring on a science or expansion leader, *and* it puts the empire in
    /// that leader's lane. If the first alone carries the effect then the
    /// mechanism is "stop paying for a war that takes nothing", and the lane
    /// is decoration; if the second is needed then it really is a race. This
    /// repo has published four mechanism stories it had to retract, so the
    /// decomposition is built before either story is told.
    ///
    /// Reachable as `advanced_counter_stand_down`. The other four races are
    /// answered exactly as they are today.
    pub counter_stand_down: bool,

    /// Whether the score race is read as a margin over the field instead of as
    /// a clock.
    ///
    /// The shipped term fires only in the last quarter of the game, so at the
    /// deployment map size — where most games are decided on score at the turn
    /// limit — every leader trips it at the same turn regardless of how far
    /// ahead they are. `docs/COUNTERING_LEADERS.md` measures score as the only
    /// instrument that predicts a winner at an actionable lead, so this reads
    /// the margin: 78 at 20% ahead of the next empire, 100 at 50% ahead, from
    /// the first turn an early game has enough history to mean anything.
    ///
    /// Reachable as `advanced_early_score_alarm`, and as
    /// `advanced_early_score_build` paired with [`Self::counter_in_lane`] so
    /// the earlier alarm asks for a build rather than a war. Every
    /// response-side change in that document measured null, so this is the
    /// instrument change those nulls point at — and it is entirely possible
    /// that an earlier alarm feeding a response worth zero is also worth zero.
    pub early_score_alarm: bool,

    /// Whether the player-targeted World Congress resolutions aim at the empire
    /// closest to a victory rather than at the empire with the most Diplomatic
    /// Victory Points.
    ///
    /// Every leader-targeting term in [`Self::congress_choice`] resolves its
    /// target as `diplomatic_leader`. `congress_census` reads what that target
    /// is worth: over congress sessions of decided games the DVP leader is the
    /// eventual winner **24.8%** of the time at 4p (base rate 25.0%) and
    /// **14.4%** at the 6p exhibition profile (base rate 16.7%) — at or below
    /// chance on both. The score leader is the winner 61% of the time on both.
    ///
    /// That matters more here than anywhere else the same mistake appears,
    /// because the Congress is the **only** counter in this game that is not
    /// paid for in development: [`Game::resolve_congress`] refunds a losing
    /// vote in full and a right-outcome/wrong-target vote at half. The whole
    /// war-shaped counter axis in `docs/COUNTERING_LEADERS.md` measured null
    /// with its cost showing up as terminal score; a ballot has no such cost.
    ///
    /// Three resolutions carry a real, targeted penalty, and this points all
    /// three at the empire [`Self::victory_denial`] already names:
    /// `trade_policy` B (a total trade embargo), `migration_treaty` B (−20%
    /// growth, which today scores **0.0 against any rival**, so the penalty
    /// can never be aimed at anybody), and `border_control_treaty` B (no tile
    /// annexation from border growth).
    ///
    /// `world_leader` is deliberately left aiming at the diplomatic leader:
    /// its ±2 moves Diplomatic Victory Points and nothing else, and the census
    /// finds that veto already lands 95–98.5% of the time with no diplomatic
    /// victory in 40 games. There is no headroom there to take.
    ///
    /// Reachable as `advanced_congress_counter`.
    pub congress_counter_leader: bool,

    /// Whether a ballot cast *against* the empire closest to a victory is
    /// backed with bought votes.
    ///
    /// `take_turn` weights every ballot by the voter's *own* plan — three votes
    /// on the Diplomacy plan holding 30 Favor, otherwise one — and never by
    /// what is at stake. Favor has no sink but votes and deals, and the census
    /// finds rivals holding enough for a third vote on 289 of 326 ballots at
    /// the exhibition profile while buying one **zero** times.
    ///
    /// Kept apart from [`Self::congress_counter_leader`] because that flag
    /// changes *where* the counter points and this one changes *how hard* it
    /// pushes; a combined arm cannot say which half did the work, and this repo
    /// has had to retract four mechanism stories told off combined arms.
    ///
    /// Reachable as `advanced_congress_votes`, and as
    /// `advanced_congress_counter_hard` with both flags set.
    pub congress_counter_votes: bool,
}

impl Default for AdvancedAi {
    fn default() -> Self {
        Self::new()
    }
}

impl AdvancedAi {
    pub fn new() -> AdvancedAi {
        Self::configured(BasicAi::new(), true, None)
    }

    /// Where this agent tells an observer what it is doing.
    ///
    /// There is one journal per agent, not one per layer: the baseline inside
    /// `base` writes to this same log, so a civilization's turn reads as a
    /// single account of its reasoning instead of two that have to be
    /// interleaved. Off unless a spectator attached one.
    fn journal(&self) -> &Journal {
        &self.base.journal
    }

    /// The tile this settler is currently marching to, if it holds one.
    ///
    /// Read-only, for instruments. `docs/OPENINGS.md` §14 measured that a
    /// settler walks 2.32x its straight-line distance; whether that is a bad
    /// path or a changing destination cannot be told from the outside without
    /// this, and the two want completely different repairs.
    pub fn settler_target(&self, uid: u32) -> Option<Pos> {
        self.settler_targets.get(&uid).copied()
    }

    pub fn targeting(target: VictoryTarget) -> AdvancedAi {
        Self::configured(BasicAi::new(), true, Some(target))
    }

    /// Frozen control for measuring future strategic changes against the
    /// first promoted hierarchical agent rather than only against BasicAi.
    pub fn legacy() -> AdvancedAi {
        Self::configured(BasicAi::new(), false, None)
    }

    fn configured(
        base: BasicAi,
        victory_planning: bool,
        victory_target: Option<VictoryTarget>,
    ) -> AdvancedAi {
        AdvancedAi {
            base,
            plan: None,
            settler_targets: BTreeMap::new(),
            builder_targets: BTreeMap::new(),
            major_war_since: None,
            last_campaign_progress: 0,
            last_city_count: 0,
            peace_until: 0,
            victory_planning,
            victory_target,
            census: StrategyCensus::default(),
            forced_target_player: None,
            force_groups: Vec::new(),
            force_groups_dirty: false,
            scoped_relief_hold: false,
            refuse_unreachable_lanes: false,
            prophet_before_opportunism: false,
            settler_price: 1.0,
            preempt_margin: 1.0,
            assigned_religion_may_expand: false,
            defensible_sites: false,
            food_first: 0.0,
            settler_commit: false,
            settler_stalls: BTreeMap::new(),
            parallel_settlers: false,
            expansion_pays_back: false,
            city_target_floor: 3,
            city_strategy: false,
            city_strategy_emphasis: true,
            city_strategy_roles: true,
            city_strategy_expansion_first: true,
            city_strategy_halt_growth: false,
            city_strategy_bastion: true,
            city_strategy_breadbasket: true,
            city_strategy_comparative: true,
            city_strategy_pressure: true,
            civ_blind: false,
            deny_leaders: true,
            early_rush: false,
            route_connected_rush: false,
            rush_route_targets: None,
            counter_in_lane: false,
            counter_stand_down: false,
            early_score_alarm: false,
            congress_counter_leader: false,
            congress_counter_votes: false,
        }
    }

    pub fn with_weights(weights: Weights) -> AdvancedAi {
        Self::configured(BasicAi::with_weights(weights), true, None)
    }

    pub fn with_weights_and_target(weights: Weights, target: VictoryTarget) -> AdvancedAi {
        Self::configured(BasicAi::with_weights(weights), true, Some(target))
    }

    /// Redirect an existing agent at a new explicit victory target without
    /// discarding campaign memory; the strategic plan re-assesses on the
    /// next turn. Used by the rollout-driven `StrategicAi`.
    pub fn retarget(&mut self, target: VictoryTarget) {
        self.victory_target = Some(target);
        self.forced_target_player = None;
        self.plan = None;
    }

    /// Commit to a victory lane while preserving the rival that made an
    /// urgent counter-campaign necessary.  Generic explicit targets remain
    /// free to choose their best opponent; this narrow form is for planners
    /// that have already identified the civilization about to end the game.
    pub fn retarget_against(&mut self, target: VictoryTarget, rival: usize) {
        self.victory_target = Some(target);
        self.forced_target_player = Some(rival);
        self.plan = None;
    }

    /// Swap the strategy genome of a running agent without discarding
    /// campaign, settler, builder or unit-role memory — the same contract as
    /// `retarget`, one level down. `retarget` changes *what* the agent is
    /// playing for; this changes *how* it plays, which is the second free
    /// variable a rollout planner can search over. The strategic plan is
    /// dropped so the next turn re-assesses under the new genome.
    pub fn reweight(&mut self, weights: Weights) {
        self.base.w = weights;
        self.plan = None;
    }

    /// The genome this agent is currently playing.
    pub fn weights(&self) -> &Weights {
        &self.base.w
    }

    /// Return a previously targeted agent to its adaptive victory planner
    /// without discarding campaign and unit-role memory. StrategicAi uses this
    /// when an explicit lane no longer beats the parent policy in rollout.
    pub fn adapt(&mut self) {
        self.victory_target = None;
        self.forced_target_player = None;
        self.plan = None;
    }

    pub fn fleet(g: &Game) -> Vec<AdvancedAi> {
        g.players.iter().map(|_| AdvancedAi::new()).collect()
    }

    pub fn fleet_targeting(g: &Game, target: VictoryTarget) -> Vec<AdvancedAi> {
        g.players
            .iter()
            .map(|_| AdvancedAi::targeting(target))
            .collect()
    }

    pub fn fleet_weighted(g: &Game, weights: &Weights) -> Vec<AdvancedAi> {
        g.players
            .iter()
            .map(|p| {
                if p.is_minor || p.is_barbarian {
                    AdvancedAi::new()
                } else {
                    AdvancedAi::with_weights(weights.clone())
                }
            })
            .collect()
    }

    pub fn current_plan(&self) -> Option<&StrategicPlan> {
        self.plan.as_ref()
    }

    /// Does any city of `pid` currently see somewhere to send a settler?
    ///
    /// This mirrors, exactly, the site test inside `production_value`'s settler
    /// arm — the near ring first, then the whole map once Shipbuilding is in.
    /// It exists because that arm refuses a settler for five different reasons
    /// and then, having passed all five, can still lose the production
    /// argument; "no site" and "out-competed" are different defects with
    /// different repairs and no external probe can tell them apart.
    /// `expansion_funnel` uses it for precisely that split.
    ///
    /// Diagnostic only: nothing in the agent's own decision path calls it.
    pub fn any_settle_site(&self, g: &Game, pid: usize) -> bool {
        g.player_city_ids(pid).into_iter().any(|cid| {
            let Some(city) = g.cities.get(&cid) else {
                return false;
            };
            self.best_settle_site(g, pid, city.pos, 11).is_some()
                || (g.players[pid].techs.contains(&crate::name!("shipbuilding"))
                    && self
                        .best_settle_site(g, pid, city.pos, g.map.width + g.map.height)
                        .is_some())
        })
    }

    pub fn victory_target(&self) -> Option<VictoryTarget> {
        self.victory_target
    }

    /// Rival explicitly pinned by an urgent counter-campaign, if any.
    /// StrategicAi uses this to refresh the objective when a different
    /// civilization becomes the immediate victory threat in the same lane.
    pub fn forced_target_player(&self) -> Option<usize> {
        self.forced_target_player
    }

    /// How many turns this agent spent on each grand strategy.
    pub fn strategy_census(&self) -> StrategyCensus {
        self.census
    }

    fn active_victory_target(&self, g: &Game) -> Option<VictoryTarget> {
        self.victory_target
            .filter(|target| g.victory_conditions.is_enabled(target.as_str()))
    }

    fn victory_strategy_enabled(g: &Game, strategy: GrandStrategy) -> bool {
        match strategy {
            GrandStrategy::Science => g.victory_conditions.science,
            GrandStrategy::Culture => g.victory_conditions.culture,
            GrandStrategy::Religion => g.victory_conditions.religious,
            GrandStrategy::Diplomacy => g.victory_conditions.diplomatic,
            GrandStrategy::Conquest => g.victory_conditions.domination,
            GrandStrategy::Expansion => g.victory_conditions.score,
            GrandStrategy::Recovery => false,
        }
    }

    /// Last set of force orders produced for this agent. This is useful to
    /// observers, evaluators, and tests; orders are rebuilt at every war turn.
    pub fn force_groups(&self) -> &[ForceGroup] {
        &self.force_groups
    }

    pub fn strategy_weights(&self) -> &Weights {
        &self.base.w
    }

    pub fn coordinates_forces(&self) -> bool {
        self.victory_planning
    }

    fn observe_campaign(&mut self, g: &Game, pid: usize) {
        let cities = g.player_city_ids(pid).len();
        if cities > self.last_city_count {
            self.last_campaign_progress = g.turn;
        }
        self.last_city_count = cities;
        let major_war = g.players.iter().any(|p| {
            p.id != pid && p.alive && !p.is_minor && !p.is_barbarian && g.is_at_war(pid, p.id)
        });
        if major_war {
            self.major_war_since.get_or_insert(g.turn);
        } else {
            self.major_war_since = None;
        }
    }

    fn plan_stale(&self, g: &Game, pid: usize) -> bool {
        let Some(plan) = &self.plan else { return true };
        let unavailable_victory_plan = matches!(
            plan.strategy,
            GrandStrategy::Science
                | GrandStrategy::Culture
                | GrandStrategy::Religion
                | GrandStrategy::Diplomacy
        ) && !Self::victory_strategy_enabled(g, plan.strategy);
        let useful_religious_opening = plan.strategy == GrandStrategy::Religion
            && self.religious_opening_viable(g, pid);
        if unavailable_victory_plan && !useful_religious_opening {
            return true;
        }
        // A rush is re-read every turn. The five-turn cadence is right for a
        // plan measured in eras, but the rush's whole decision — "is the stack
        // staged and can it finish?" — becomes true on one specific turn, and
        // waiting up to four more to notice spends them out of a window that
        // shuts. Measured: early campaigns declare at a median turn 36 and
        // kill 14 turns later, so the target lands at 50 rather than 54 on
        // this alone.
        let cadence = if plan.rush { 1 } else { 5 };
        if g.turn.saturating_sub(plan.assessed_turn) >= cadence {
            return true;
        }
        if let Some(target) = plan.target_player {
            if !g.players.get(target).map(|p| p.alive).unwrap_or(false) {
                return true;
            }
            let emergency_target = g
                .emergency_objective(pid)
                .is_some_and(|objective| objective.target == target);
            if !g.is_at_war(pid, target)
                && !emergency_target
                && !self.campaign_target_legal(g, pid, target)
            {
                return true;
            }
        }
        if let Some(forced) = self.forced_target_player {
            if !g.players.get(forced).map(|player| player.alive).unwrap_or(false)
                || (plan.target_player != Some(forced)
                    && self.campaign_target_legal(g, pid, forced))
            {
                return true;
            }
        }
        if let Some(cid) = plan.target_city {
            if !g.cities.get(&cid).map(|c| c.owner != pid).unwrap_or(false) {
                return true;
            }
        }
        // The five-turn planning horizon keeps economic choices stable, but
        // wars and victory races are interrupts rather than ordinary inputs.
        // Waiting four more turns after a surprise attack or a rival's final
        // launch can make the eventual plan irrelevant.
        let major_wars: Vec<usize> = g
            .players
            .iter()
            .filter(|player| {
                player.id != pid
                    && player.alive
                    && !player.is_minor
                    && !player.is_barbarian
                    && g.is_at_war(pid, player.id)
            })
            .map(|player| player.id)
            .collect();
        if !major_wars.is_empty()
            && !plan
                .target_player
                .is_some_and(|target| major_wars.contains(&target))
        {
            return true;
        }
        if plan.threatened_city != self.threatened_city(g, pid) {
            return true;
        }
        if let Some((rival, counter)) = self.victory_denial(g, pid) {
            let expects_hostile_target = self.campaign_target_legal(g, pid, rival);
            if (expects_hostile_target && plan.target_player != Some(rival))
                || (!expects_hostile_target && plan.target_player == Some(rival))
                || (major_wars.is_empty() && plan.strategy != counter)
            {
                return true;
            }
        }
        false
    }

    /// Local military pressure on one city: hostile military strength within
    /// six tiles over the friendly strength answering it, city defenses
    /// included. Zero when no hostile unit is in reach.
    ///
    /// This is the number `threatened_city` has always computed and then
    /// discarded for every city but the worst one. Naming it lets the same
    /// evidence reach the city's own decisions — what it builds and what its
    /// citizens work — instead of only the empire-wide recovery alarm.
    fn city_pressure(g: &Game, pid: usize, cid: u32) -> f64 {
        let Some(city) = g.cities.get(&cid) else {
            return 0.0;
        };
        let hostile: f64 = g
            .units
            .values()
            .filter(|unit| unit.owner != pid && g.is_at_war(pid, unit.owner))
            .filter(|unit| g.wdist(city.pos, unit.pos) <= 6)
            .filter(|unit| g.rules.units[unit.kind].class == "military")
            .map(|unit| crate::game::effective_strength(g.unit_strength(unit, false), unit.hp))
            .sum();
        if hostile <= 0.0 {
            return 0.0;
        }
        let friendly = g.city_strength(cid)
            + g.units
                .values()
                .filter(|unit| unit.owner == pid && g.wdist(city.pos, unit.pos) <= 6)
                .filter(|unit| g.rules.units[unit.kind].class == "military")
                .map(|unit| crate::game::effective_strength(g.unit_strength(unit, true), unit.hp))
                .sum::<f64>();
        hostile / friendly.max(1.0)
    }

    /// The empire's objective translated into the citizen governor's own
    /// vocabulary: how much extra appetite this lane asks for, per yield.
    ///
    /// Every number here is a *tilt*, not a takeover. The shipped weights are
    /// food 1.25 / production 1.55 / science 1.30 / culture 1.20 / gold 0.85 /
    /// faith 0.90, so a lane yield gaining ~0.5 moves it up roughly a third
    /// and never reorders the whole vector — a Science empire still builds
    /// settlers and still feeds itself.
    fn lane_emphasis(strategy: GrandStrategy) -> Yields {
        let mut ys = Yields::default();
        match strategy {
            // Growth is the lane. Hammers still matter because a settler is
            // paid for in production even though it is gated on food.
            GrandStrategy::Expansion => {
                ys.food = 0.35;
                ys.production = 0.20;
            }
            GrandStrategy::Science => {
                ys.science = 0.50;
                ys.production = 0.15;
            }
            GrandStrategy::Culture => {
                ys.culture = 0.50;
                ys.gold = 0.10;
            }
            GrandStrategy::Religion => {
                ys.faith = 0.50;
                ys.food = 0.15;
            }
            // Envoys and city-state suzerainty are bought, so the diplomatic
            // lane is a Gold lane at the tile level.
            GrandStrategy::Diplomacy => {
                ys.gold = 0.45;
                ys.production = 0.15;
            }
            // An army is production and the population that replaces it is
            // food; nothing else on the sheet wins a war.
            GrandStrategy::Conquest => {
                ys.production = 0.55;
                ys.food = 0.15;
            }
            GrandStrategy::Recovery => {
                ys.production = 0.45;
                ys.food = 0.20;
            }
        }
        ys
    }

    /// Write one [`CityDirective`] per owned city, rebuilding the map from
    /// scratch so a captured or razed city cannot leave a stale entry behind.
    ///
    /// The role choice is ranked, and the order is the argument: a city being
    /// shot at is a Bastion whatever else it is good at; an empire short of
    /// its city target puts its best food city on settlers before it optimises
    /// anything else, because expansion compounds and specialization does not;
    /// only then does the local yield mix get to speak.
    ///
    /// ⚠ **`expansion_first` is not a refinement of that rule — it is the
    /// repair of a defect the census found in it.** The comparative rungs are
    /// scale-free: they type a city by whether a yield stands `ROLE_MARGIN`
    /// above its *empire's own mean*, and with two cities the mean sits
    /// between them, so one of them is typed whatever the terrain actually
    /// says. `role_ladder_census` measured the result over 1668 city-turns —
    /// 45.5% of city-turns typed, Forge alone 17.7% and Specialist 7.1% — in
    /// empires averaging 2.14 cities. So the doc comment on `ROLE_MARGIN`
    /// claiming an empire of interchangeable cities "correctly gets no roles
    /// at all" was **false at exactly the empire size where the game is
    /// decided**, and the city most often typed a Forge is the capital, which
    /// is the settler pump whose growth `docs/OPENINGS.md` shows gates every
    /// settler.
    fn stamp_city_directives(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        let cities = g.player_city_ids(pid);
        if cities.is_empty() {
            if let Some(seat) = g.players.get_mut(pid) {
                seat.city_directives.clear();
            }
            return;
        }
        // While the empire is still short of its city target, only the two
        // rungs that cannot cost expansion may fire: Bastion, which answers a
        // real threat, and Breadbasket, which feeds the settler. Forge and
        // Specialist both bid citizens away from food, and a city founded
        // sixty turns earlier compounds for the rest of the game where a
        // marginally better yield mix does not.
        let expanding = self.city_strategy_expansion_first && cities.len() < plan.desired_cities;
        let emphasis = if self.city_strategy_emphasis {
            Self::lane_emphasis(plan.strategy)
        } else {
            Yields::default()
        };
        let short = cities.len() < plan.desired_cities;

        let mut totals = Yields::default();
        for cid in &cities {
            totals.add(g.city_yields(*cid));
        }
        let count = cities.len() as f64;
        let mean_food = totals.food / count;
        let mean_production = totals.production / count;
        let mean_specialty = (totals.science + totals.culture + totals.faith + totals.gold) / count;

        let mut directives = BTreeMap::new();
        for cid in cities {
            let pressure = Self::city_pressure(g, pid, cid);
            let ys = g.city_yields(cid);
            let specialty = ys.science + ys.culture + ys.faith + ys.gold;
            // `BASTION_PRESSURE` is the same 0.45 `threatened_city` treats as
            // a locally competitive hostile force, so the city's own governor
            // and the empire's recovery alarm agree on what a threat is.
            let bastion = pressure >= BASTION_PRESSURE || plan.threatened_city == Some(cid);
            let role = if !self.city_strategy_roles {
                CityRole::Balanced
            } else if bastion && self.city_strategy_bastion {
                CityRole::Bastion
            } else if short && self.city_strategy_breadbasket && ys.food >= mean_food * ROLE_MARGIN
            {
                CityRole::Breadbasket
            } else if expanding || !self.city_strategy_comparative {
                CityRole::Balanced
            } else if ys.production >= mean_production * ROLE_MARGIN {
                CityRole::Forge
            } else if specialty >= mean_specialty * ROLE_MARGIN {
                CityRole::Specialist
            } else {
                CityRole::Balanced
            };
            let pressure = if self.city_strategy_roles && self.city_strategy_pressure {
                pressure
            } else {
                0.0
            };
            directives.insert(
                cid,
                CityDirective {
                    emphasis,
                    role,
                    pressure,
                    // Measured harmful in isolation (43.8%, Elo -44,
                    // p=0.0107). Reachable only through the frozen controls.
                    halt_growth: self.city_strategy_halt_growth
                        && role == CityRole::Bastion,
                },
            );
        }
        if let Some(seat) = g.players.get_mut(pid) {
            seat.city_directives = directives;
        }
    }

    fn threatened_city(&self, g: &Game, pid: usize) -> Option<u32> {
        g.player_city_ids(pid)
            .into_iter()
            .filter_map(|cid| {
                let city = &g.cities[&cid];
                let danger = Self::city_pressure(g, pid, cid);
                if danger <= 0.0 {
                    return None;
                }
                let recently_hit =
                    city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 3;
                let wall_max = g.city_max_wall_hp(city);
                let damaged = city.hp < 200 || city.wall_hp < wall_max;
                let breached = city.hp < 160
                    || (wall_max > 0 && city.wall_hp.saturating_mul(2) < wall_max);
                // A scout or losing skirmisher in the outer city radius is a
                // tactical contact, not an empire-wide emergency. Recovery is
                // reserved for a locally competitive force or a damaged city
                // whose remaining defenders cannot safely absorb another hit.
                let critical = danger >= 0.90
                    || (danger >= 0.45 && (breached || (recently_hit && damaged)));
                critical.then_some((
                    danger,
                    (200 - city.hp).max(0) + (wall_max - city.wall_hp).max(0),
                    cid,
                ))
            })
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| left.1.cmp(&right.1))
                    .then_with(|| right.2.cmp(&left.2))
            })
            .map(|(_, _, cid)| cid)
    }

    fn religious_opening_rank(g: &Game, pid: usize) -> Option<(u8, f64, f64)> {
        let player = &g.players[pid];
        if !player.alive
            || player.is_minor
            || player.is_barbarian
            || player.religion.is_some()
            || player.prophet_pending
            || g.player_city_ids(pid).len() < 2
        {
            return None;
        }
        let city_ids = g.player_city_ids(pid);
        let has_holy_site = city_ids
            .iter()
            .any(|cid| g.cities[cid].districts.contains_key(crate::name!("holy_site")));
        let holy_site_planned = city_ids.iter().any(|cid| {
            matches!(
                g.cities[cid].queue.first(),
                Some(Item::District { district, .. }) if district == "holy_site"
            )
        });
        let best_site = city_ids
            .iter()
            .flat_map(|cid| g.district_sites(*cid, crate::name!("holy_site")))
            .map(|pos| g.district_yields(crate::name!("holy_site"), pos).faith)
            .max_by(f64::total_cmp);
        if !has_holy_site && !holy_site_planned && best_site.is_none() {
            return None;
        }
        // Once an empire has paid toward the race, keep that commitment ahead
        // of an uninvested late entrant. The remaining comparisons select the
        // best available Holy Site and faith economy instead of requiring the
        // unusually rare +3 adjacency that previously left most maps with a
        // single founder.
        let commitment = if has_holy_site {
            4
        } else if holy_site_planned {
            3
        } else if player.techs.contains(&crate::name!("astrology")) {
            2
        } else if player.research.as_deref() == Some("astrology") {
            1
        } else {
            0
        };
        Some((commitment, best_site.unwrap_or(0.0), player.faith))
    }

    fn religious_opening_viable(&self, g: &Game, pid: usize) -> bool {
        let player = &g.players[pid];
        if player.religion.is_some() {
            return false;
        }
        if player.prophet_pending {
            return true;
        }
        let founded = g.religions_founded();
        let pending = g
            .players
            .iter()
            .filter(|candidate| candidate.prophet_pending)
            .count();
        let claimed = founded + pending;
        if claimed >= g.max_religions()
            || g.turn > if founded > 0 { 180 } else { 120 }
            || Self::religious_opening_rank(g, pid).is_none()
        {
            return false;
        }

        // Prophet slots are a global race. Let exactly the best uncommitted
        // contenders pursue the slots that remain, while still allowing a
        // newly founded rival religion to trigger a genuine counter-race.
        let open_slots = g.max_religions() - claimed;
        let mut contenders: Vec<_> = g
            .players
            .iter()
            .filter_map(|candidate| {
                Self::religious_opening_rank(g, candidate.id).map(|rank| (candidate.id, rank))
            })
            .collect();
        contenders.sort_by(|(left_id, left), (right_id, right)| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| right.1.total_cmp(&left.1))
                .then_with(|| right.2.total_cmp(&left.2))
                .then_with(|| left_id.cmp(right_id))
        });
        contenders
            .into_iter()
            .take(open_slots)
            .any(|(contender, _)| contender == pid)
    }

    fn rocketry_readiness(&self, g: &Game, pid: usize) -> i32 {
        let player = &g.players[pid];
        let rocketry_path: Vec<_> = g
            .rules
            .techs
            .keys()
            .filter(|tech| self.tech_leads_to(g, tech, "rocketry"))
            .collect();
        let completed = rocketry_path
            .iter()
            .filter(|tech| player.techs.contains(&Name::new(tech.as_str())))
            .count();
        25 + (40 * completed / rocketry_path.len().max(1)) as i32
    }

    fn diplomatic_science_backup(&self, g: &Game, pid: usize, plan: &StrategicPlan) -> bool {
        self.victory_target.is_none()
            && g.victory_conditions.science
            && plan.strategy == GrandStrategy::Diplomacy
            && g.turn >= g.standard_duration(220)
            && self.rocketry_readiness(g, pid) >= 45
    }

    fn religious_conversion_tally(&self, g: &Game, pid: usize) -> (usize, usize) {
        let living_majors: Vec<usize> = g
            .players
            .iter()
            .filter(|player| player.alive && !player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .collect();
        let converted = g.players[pid].religion.as_ref().map_or(0, |religion| {
            living_majors
                .iter()
                .filter(|other| {
                    let cities = g.player_city_ids(**other);
                    let following = cities
                        .iter()
                        .filter(|city| g.city_religion(&g.cities[city]) == Some(religion.as_str()))
                        .count();
                    !cities.is_empty() && following * 2 > cities.len()
                })
                .count()
        });
        (converted, living_majors.len())
    }

    /// Whether a Science victory can still be finished before the game stops.
    ///
    /// A Science win needs the tech tree and then the four-stage launch, and
    /// `docs/AI_GUIDE.md` records unassisted science victories landing on
    /// turns 1021 and 940. The stock Standard budget is 500. So on a normal
    /// game the lane is not merely difficult, it is arithmetically out of
    /// reach — and `ablate --mode best-lane` measured exactly that: a seat
    /// committed to Science from turn one won **0 of 50**, as did Culture,
    /// Domination and Score, while committed Religion won 29 and the adaptive
    /// agent won 14.
    ///
    /// That matters because `victory_focus` is an argmax over per-lane
    /// progress and Science is the only lane with an unearned floor: it opens
    /// at 25 and climbs to 55 on tech count alone, which every empire
    /// accumulates whatever it is playing for. Religion scores 0 until its
    /// opening is viable. So the argmax leans toward the one lane that cannot
    /// finish, and keeps leaning as the game goes on.
    ///
    /// The estimate deliberately uses the empire's own achieved rate rather
    /// than a table: an empire researching quickly on a fast speed setting
    /// genuinely may finish, and should not be talked out of it by a constant.
    /// Before any tech is in, nothing is claimed — an empire cannot be judged
    /// on a rate it has not had the chance to set.
    fn science_reachable(&self, g: &Game, pid: usize) -> bool {
        let researched = g.players[pid].techs.len();
        let total = g.rules.techs.len();
        if researched == 0 || researched >= total || g.turn == 0 {
            return true;
        }
        let remaining = (total - researched) as u64;
        // Turns per tech achieved so far, kept in integer arithmetic so the
        // estimate is exactly reproducible across platforms.
        let eta_research = g.turn as u64 * remaining / researched as u64;
        // The launch chain still has to be built and run after the last tech.
        let launch = g.standard_duration(60) as u64;
        let budget = g.max_turns.saturating_sub(g.turn) as u64;
        eta_research.saturating_add(launch) <= budget
    }

    /// Lanes this empire could still finish, applied to the adaptive planner
    /// only.
    ///
    /// An explicitly targeted agent keeps its target whatever this says:
    /// `victory_eval` asks for a named victory and must be free to spend as
    /// many turns as that takes.
    fn lane_reachable(&self, g: &Game, pid: usize, strategy: GrandStrategy) -> bool {
        if !self.refuse_unreachable_lanes {
            return true;
        }
        match strategy {
            GrandStrategy::Science => self.science_reachable(g, pid),
            _ => true,
        }
    }

    fn victory_focus(&self, g: &Game, pid: usize) -> VictoryFocus {
        if let Some(target) = self.active_victory_target(g) {
            return VictoryFocus {
                strategy: target.strategy(),
                progress: 100,
            };
        }
        if !self.victory_planning {
            let preferred = if !self.civ_blind && g.players[pid].civ == "Greece" {
                GrandStrategy::Culture
            } else {
                GrandStrategy::Science
            };
            let strategy = [
                preferred,
                GrandStrategy::Science,
                GrandStrategy::Culture,
                GrandStrategy::Religion,
                GrandStrategy::Diplomacy,
                GrandStrategy::Conquest,
                GrandStrategy::Expansion,
            ]
            .into_iter()
            .find(|strategy| Self::victory_strategy_enabled(g, *strategy))
            .unwrap_or(GrandStrategy::Science);
            return VictoryFocus {
                strategy,
                progress: 25,
            };
        }
        let player = &g.players[pid];
        let living_majors: Vec<usize> = g
            .players
            .iter()
            .filter(|p| p.alive && !p.is_minor && !p.is_barbarian)
            .map(|p| p.id)
            .collect();

        // A science race starts long before the first launch.  Treating every
        // pre-space empire as exactly 25% complete made a strong researcher
        // abandon science as soon as even modest tourism appeared.  The tech
        // tree is public victory-screen information and gives the planner a
        // smooth signal until the discrete space-race milestones take over.
        let tech_progress =
            25 + (30 * player.techs.len() / g.rules.techs.len().max(1)).min(30) as i32;
        let project_progress = player.science_projects.len().min(4) as i32 * 18;
        let travel_progress = if player.science_projects.contains("exoplanet_expedition") {
            (player.exoplanet_distance * 100.0 / 50.0).clamp(0.0, 100.0) as i32
        } else {
            0
        };
        // A launch program cannot raise project progress until the AI has
        // already chosen Science long enough to unlock Rocketry and build a
        // Spaceport. Count progress along that prerequisite path so adaptive
        // agents can make the initial commitment instead of remaining stuck
        // at the old 25% floor forever.
        let readiness = self.rocketry_readiness(g, pid);
        let science = tech_progress
            .max(readiness)
            .max(project_progress)
            .max(travel_progress)
            .max((!self.civ_blind && player.civ == "China") as i32 * 45);

        let culture_target = living_majors
            .iter()
            .filter(|other| **other != pid)
            .map(|other| g.domestic_tourists(*other))
            .max()
            .unwrap_or(1)
            .max(1);
        let culture = ((100 * g.foreign_tourists(pid) / culture_target).clamp(0, 100)) as i32;

        let (converted, living_religious_rivals) = self.religious_conversion_tally(g, pid);
        let religion = if player.religion.is_some() {
            // Founding a religion normally converts the founder's own small
            // empire first. That is table stakes, not progress against a
            // Religious Victory: counting it made every founder jump from the
            // 40-point commitment floor to 55 in a four-player game before it
            // had converted a single rival. Measure expansion into foreign
            // civilizations here; rival threat scoring below still counts all
            // living majors because the actual victory rule does too.
            let religion = player.religion.as_deref().unwrap();
            let own_cities = g.player_city_ids(pid);
            let own_following = own_cities
                .iter()
                .filter(|city| g.city_religion(&g.cities[city]) == Some(religion))
                .count();
            let own_converted = !own_cities.is_empty() && own_following * 2 > own_cities.len();
            let foreign_converted = converted.saturating_sub(usize::from(own_converted));
            let foreign_rivals = living_religious_rivals.saturating_sub(1);
            40 + (60 * foreign_converted / foreign_rivals.max(1)) as i32
        } else if self.religious_opening_viable(g, pid) {
            if g.religions_founded() > 0 {
                55
            } else {
                46
            }
        } else {
            0
        };

        let suzerain = g
            .players
            .iter()
            .filter(|minor| {
                minor.alive
                    && minor.is_minor
                    && !minor.is_barbarian
                    && g.suzerain_of(minor.id) == Some(pid)
            })
            .count() as i64;
        let diplomacy = (player.dvp * 5 + suzerain * 6).clamp(0, 100) as i32;

        // A lane that cannot finish inside the remaining turns scores zero
        // rather than its raw progress. Zero rather than a discount because
        // the question is not how far along the empire is, it is whether the
        // finish line arrives before the game ends.
        let science = match self.lane_reachable(g, pid, GrandStrategy::Science) {
            true => science,
            false => 0,
        };
        let candidates = [
            VictoryFocus {
                strategy: GrandStrategy::Science,
                progress: science,
            },
            VictoryFocus {
                strategy: GrandStrategy::Culture,
                progress: culture
                    .max((!self.civ_blind && player.civ == "Greece") as i32 * 45),
            },
            VictoryFocus {
                strategy: GrandStrategy::Religion,
                progress: religion,
            },
            VictoryFocus {
                strategy: GrandStrategy::Diplomacy,
                progress: diplomacy,
            },
            VictoryFocus {
                strategy: GrandStrategy::Conquest,
                progress: 0,
            },
            VictoryFocus {
                strategy: GrandStrategy::Expansion,
                progress: 0,
            },
        ];
        let mut enabled = candidates
            .into_iter()
            .filter(|candidate| Self::victory_strategy_enabled(g, candidate.strategy));
        let mut best = enabled.next().unwrap_or(VictoryFocus {
            strategy: GrandStrategy::Science,
            progress: science,
        });
        for candidate in enabled {
            if candidate.progress > best.progress {
                best = candidate;
            }
        }
        // The scan seeds on the first enabled candidate and only moves on a
        // strict improvement, and Science is first in the table. Zeroing an
        // unreachable Science therefore is not enough on its own: when every
        // lane scores zero the argmax still returns it. Fall through to the
        // first reachable lane instead, so refusing a lane actually refuses
        // it.
        if best.progress == 0 && !self.lane_reachable(g, pid, best.strategy) {
            if let Some(fallback) = candidates.into_iter().find(|candidate| {
                Self::victory_strategy_enabled(g, candidate.strategy)
                    && self.lane_reachable(g, pid, candidate.strategy)
            }) {
                best = fallback;
            }
        }
        best
    }

    /// Public victory-screen information distilled into a single urgency
    /// signal. Strong opponents must be judged by how close they are to ending
    /// the game, not only by how cheap their nearest city looks to capture.
    fn rival_victory_pressure(&self, g: &Game, pid: usize) -> VictoryFocus {
        self.rival_victory_pressure_with_culture(g, pid, None)
    }

    fn rival_victory_pressure_with_culture(
        &self,
        g: &Game,
        pid: usize,
        culture_pressure: Option<i32>,
    ) -> VictoryFocus {
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

        let science = if player.science_projects.contains("exoplanet_expedition") {
            // The final expedition is an irreversible endgame commitment.  A
            // defender needs time to raise, route, and deploy a counterforce,
            // so its launch itself must cross the generic denial threshold;
            // waiting for the first six light-years discarded that reaction
            // window while the rival was already on the victory clock.
            78 + (22.0 * player.exoplanet_distance / 50.0).clamp(0.0, 22.0) as i32
        } else if player.science_projects.contains("launch_mars_colony") {
            65
        } else if player.science_projects.contains("launch_moon_landing") {
            45
        } else if player.science_projects.contains("launch_earth_satellite") {
            25
        } else {
            0
        };

        let culture = culture_pressure.unwrap_or_else(|| {
            let culture_target = living_majors
                .iter()
                .filter(|other| **other != pid)
                .map(|other| g.domestic_tourists(*other))
                .max()
                .unwrap_or(1)
                .max(1);
            (100 * g.foreign_tourists(pid) / culture_target).clamp(0, 100) as i32
        });

        let (converted, living_religious_rivals) = self.religious_conversion_tally(g, pid);
        let religion = if player.religion.is_some() {
            (100 * converted / living_religious_rivals.max(1)) as i32
        } else {
            0
        };
        let diplomacy = (player.dvp * 5).clamp(0, 100) as i32;

        let foreign_capitals = starting_majors
            .iter()
            .filter(|owner| **owner != pid)
            .count();
        let controlled_capitals = g
            .cities
            .values()
            .filter(|city| city.is_capital && city.original_owner != pid && city.owner == pid)
            .count();
        let domination = (100 * controlled_capitals)
            .checked_div(foreign_capitals)
            .unwrap_or(0) as i32;

        // The shipped score term is a clock, not an observation: it fires only
        // in the last quarter of the game, so at the deployment map size --
        // where most games are decided on score at the turn limit -- the alarm
        // it raises arrives at turn 300 of 400 for every leader alike,
        // regardless of how far ahead they are.
        //
        // The census says score is the one instrument that predicts: at the
        // deployment profile the score leader is the eventual winner 62% of
        // the time **200 turns out** against a 16.7% base rate, and settles on
        // them a median 135 turns before the end, while `victory_threat` sits
        // at or below the base rate at four of five leads. `early_score_alarm`
        // reads the margin instead of the clock -- 78 at 20% ahead of the next
        // empire, 100 at 50% ahead -- from the moment an early game has
        // enough history to mean anything.
        let score = if self.early_score_alarm && g.turn >= g.standard_duration(60) {
            let mine = g.score(pid);
            let best_rival = living_majors
                .iter()
                .filter(|candidate| **candidate != pid)
                .map(|candidate| g.score(*candidate))
                .max()
                .unwrap_or(0)
                .max(1);
            let margin = mine as f64 / best_rival as f64 - 1.0;
            if margin <= 0.0 {
                0
            } else if margin < 0.20 {
                (78.0 * margin / 0.20) as i32
            } else {
                (78.0 + 22.0 * ((margin - 0.20) / 0.30).clamp(0.0, 1.0)) as i32
            }
        } else if g.max_turns > 0
            && g.turn.saturating_mul(4) >= g.max_turns.saturating_mul(3)
            && living_majors
                .iter()
                .map(|candidate| g.score(*candidate))
                .max()
                == Some(g.score(pid))
        {
            (40 + 60 * g.turn.min(g.max_turns) / g.max_turns) as i32
        } else {
            0
        };

        [
            VictoryFocus {
                strategy: GrandStrategy::Science,
                progress: science,
            },
            VictoryFocus {
                strategy: GrandStrategy::Culture,
                progress: culture,
            },
            VictoryFocus {
                strategy: GrandStrategy::Religion,
                progress: religion,
            },
            VictoryFocus {
                strategy: GrandStrategy::Diplomacy,
                progress: diplomacy,
            },
            VictoryFocus {
                strategy: GrandStrategy::Conquest,
                progress: domination,
            },
            VictoryFocus {
                strategy: GrandStrategy::Expansion,
                progress: score,
            },
        ]
        .into_iter()
        .filter(|focus| Self::victory_strategy_enabled(g, focus.strategy))
        .max_by_key(|focus| focus.progress)
        .unwrap_or(VictoryFocus {
            strategy: GrandStrategy::Recovery,
            progress: 0,
        })
    }

    /// Compute every living rival's culture-race pressure in one table.
    ///
    /// A single `rival_victory_pressure` calculation asks for every rival's
    /// domestic tourists, and `victory_denial` asks for that pressure for
    /// every rival. Repeating those nested scans made the denial pass cubic in
    /// civilization count. Domestic and foreign tourist totals are public
    /// table state, so calculate each once and reuse them across the pass.
    fn rival_culture_pressures(&self, g: &Game) -> BTreeMap<usize, i32> {
        let living_majors: Vec<usize> = g
            .players
            .iter()
            .filter(|player| player.alive && !player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .collect();
        let domestic: BTreeMap<usize, i64> = living_majors
            .iter()
            .map(|pid| (*pid, g.domestic_tourists(*pid)))
            .collect();
        living_majors
            .iter()
            .map(|pid| {
                let target = living_majors
                    .iter()
                    .filter(|other| *other != pid)
                    .map(|other| domestic[other])
                    .max()
                    .unwrap_or(1)
                    .max(1);
                (
                    *pid,
                    (100 * g.foreign_tourists(*pid) / target).clamp(0, 100) as i32,
                )
            })
            .collect()
    }

    fn victory_denial(&self, g: &Game, pid: usize) -> Option<(usize, GrandStrategy)> {
        if !self.deny_leaders || self.active_victory_target(g).is_some() {
            return None;
        }
        let culture_pressures = self.rival_culture_pressures(g);
        self.victory_denial_with_culture_pressures(g, pid, &culture_pressures)
    }

    fn victory_denial_with_culture_pressures(
        &self,
        g: &Game,
        pid: usize,
        culture_pressures: &BTreeMap<usize, i32>,
    ) -> Option<(usize, GrandStrategy)> {
        let own_progress = self.victory_focus(g, pid).progress;
        let (rival, pressure) = g
            .players
            .iter()
            .filter(|player| {
                player.id != pid && player.alive && !player.is_minor && !player.is_barbarian
            })
            .map(|player| {
                (
                    player.id,
                    self.rival_victory_pressure_with_culture(
                        g,
                        player.id,
                        culture_pressures.get(&player.id).copied(),
                    ),
                )
            })
            .max_by(|left, right| {
                left.1
                    .progress
                    .cmp(&right.1.progress)
                    .then_with(|| right.0.cmp(&left.0))
            })?;
        // Religious progress advances in whole-civilization jumps, and a
        // defender needs time to produce and route religious counters. Start
        // reacting with two holdouts left when the rival also leads our own
        // race, then treat one remaining holdout as an unconditional match
        // point: a slower "close" victory must not suppress that interrupt.
        if pressure.strategy == GrandStrategy::Religion {
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
            if pressure.progress < early_warning
                || (pressure.progress < match_point && pressure.progress < own_progress + 15)
            {
                return None;
            }
        } else if pressure.progress < 78 || pressure.progress < own_progress + 15 {
            return None;
        }
        // Four of the seven races answer themselves — a culture threat is met
        // with culture, a religious one with religion. The two that answer with
        // an army are Science and Expansion, and those are the two the
        // deployment-scale census argues against: at 60x38 and 74x46 an empire
        // fighting one or two rivals wins 4.4% and 10.7% of the time against a
        // 16.7% base rate, and the shipped response already costs terminal
        // score (44 maps to 65, p=0.055) without buying a win. Racing the
        // leader in their own lane keeps the reaction and drops the war.
        // The decomposition arm: react to the other four races unchanged and
        // to these two not at all, so the effect of dropping the war can be
        // read apart from the effect of adopting the lane.
        if self.counter_stand_down
            && matches!(
                pressure.strategy,
                GrandStrategy::Science | GrandStrategy::Expansion
            )
        {
            return None;
        }
        let counter = match pressure.strategy {
            GrandStrategy::Science if self.counter_in_lane => GrandStrategy::Science,
            GrandStrategy::Science => GrandStrategy::Conquest,
            GrandStrategy::Culture => GrandStrategy::Culture,
            GrandStrategy::Religion if g.players[pid].religion.is_some() => GrandStrategy::Religion,
            GrandStrategy::Religion => GrandStrategy::Conquest,
            GrandStrategy::Diplomacy => GrandStrategy::Diplomacy,
            GrandStrategy::Conquest => GrandStrategy::Recovery,
            GrandStrategy::Expansion if self.counter_in_lane => GrandStrategy::Expansion,
            GrandStrategy::Expansion => GrandStrategy::Conquest,
            GrandStrategy::Recovery => GrandStrategy::Recovery,
        };
        Some((rival, counter))
    }

    /// Terminal clocks require action before an ordinary Formal War countdown
    /// or a comfortable force ratio is available. Keep this predicate shared
    /// between declaration timing and campaign readiness so either response
    /// cannot silently become more permissive than the other.
    fn urgent_victory_threat(&self, g: &Game, target: usize) -> bool {
        if !self.deny_leaders {
            return false;
        }
        let pressure = self.rival_victory_pressure(g, target);
        let living_majors = g
            .players
            .iter()
            .filter(|player| player.alive && !player.is_minor && !player.is_barbarian)
            .count()
            .max(1) as i32;
        let religious_match_point = 100 * living_majors.saturating_sub(1) / living_majors;
        pressure.progress >= 90
            || (pressure.strategy == GrandStrategy::Science && pressure.progress >= 78)
            || (pressure.strategy == GrandStrategy::Religion
                && pressure.progress >= religious_match_point)
    }

    /// Diagnostic seam: what this planner believes `target`'s best race is,
    /// and how far along it reads. These are the exact numbers the denial
    /// layer gates on, exposed so a census can compare them against
    /// [`Game::victory_threat`] instead of re-deriving the formula — a second
    /// implementation is how a HUD and an AI end up disagreeing about who is
    /// about to win. Reads nothing from `self`, so any planner may ask.
    pub fn rival_pressure(&self, g: &Game, target: usize) -> (GrandStrategy, i32) {
        let focus = self.rival_victory_pressure(g, target);
        (focus.strategy, focus.progress)
    }

    /// Diagnostic seam: the rival this empire would move against right now and
    /// the counter-strategy it would adopt, or `None` when nobody clears the
    /// bar. Same call `replan_needed` and `assess` make.
    pub fn denial_target(&self, g: &Game, pid: usize) -> Option<(usize, GrandStrategy)> {
        self.victory_denial(g, pid)
    }

    /// Diagnostic seam: whether `target`'s clock is short enough to skip the
    /// ordinary war-readiness checks.
    pub fn denial_is_urgent(&self, g: &Game, target: usize) -> bool {
        self.urgent_victory_threat(g, target)
    }

    fn assess(&self, g: &Game, pid: usize) -> StrategicPlan {
        let cities = g.player_city_ids(pid);
        let my_power = g.military_power(pid);
        let major_rivals: Vec<usize> = g
            .players
            .iter()
            .filter(|p| p.id != pid && p.alive && !p.is_minor && !p.is_barbarian)
            .map(|p| p.id)
            .collect();
        // City-states follow their Suzerain into wars and can also be attacked
        // directly. Once hostilities exist they are real campaign actors, not
        // an uncoordinated side task for whichever unit happens to be nearby.
        let wartime_rivals: Vec<usize> = g
            .players
            .iter()
            .filter(|p| p.id != pid && p.alive && !p.is_barbarian && g.is_at_war(pid, p.id))
            .map(|p| p.id)
            .collect();
        let at_war = !wartime_rivals.is_empty();
        let strongest_rival = major_rivals
            .iter()
            .map(|o| g.military_power(*o))
            .fold(0.0_f64, f64::max);
        let weakest_rival = major_rivals
            .iter()
            .map(|o| g.military_power(*o))
            .fold(f64::INFINITY, f64::min);

        let threatened_city = self.threatened_city(g, pid);

        let land = g
            .map
            .tiles
            .values()
            .filter(|t| g.rules.is_passable(t) && !g.rules.is_water(t))
            .count();
        let map_capacity = (2 + land / 55).clamp(3, 9);
        // Expansion must compound before it pays back. Add roughly one city
        // per era instead of continuously raising the target and starving a
        // young empire of districts, buildings, and population growth. Scale
        // the cadence with game speed; the old fixed turn-175 cutoff expired
        // before the five-city target even became active on Standard speed.
        let city_cadence = g.standard_duration(90).max(1) as usize;
        let desired_cities = (self.city_target_floor + g.turn as usize / city_cadence)
            .min(map_capacity)
            .min(6);
        let mut expansion_origins: Vec<Pos> = cities.iter().map(|cid| g.cities[cid].pos).collect();
        if expansion_origins.is_empty() {
            expansion_origins.extend(
                g.player_unit_ids(pid)
                    .into_iter()
                    .filter(|uid| g.units[uid].kind == "settler")
                    .map(|uid| g.units[&uid].pos),
            );
        }
        let has_site = expansion_origins.iter().any(|pos| {
            self.best_settle_site(g, pid, *pos, 10).is_some()
                || (g.players[pid].techs.contains(&crate::name!("shipbuilding"))
                    && self
                        .best_settle_site(g, pid, *pos, g.map.width + g.map.height)
                        .is_some())
        });

        let military_civ = matches!(
            g.players[pid].civ.as_str(),
            "Sumeria" | "Aztec" | "Nubia" | "Scythia" | "Byzantium"
        );
        let basil_tagma_timing = g.has_ability(pid, "taxis")
            && (g.players[pid].religion.is_some()
                || g.players[pid].civics.contains(&crate::name!("divine_right")));
        // The ancient rush is a *window*, not a preference, so it is decided
        // beside the other timing arm rather than among the victory lanes.
        let rush_victim = self.early_rush_victim(g, pid);
        let victory = self.victory_focus(g, pid);
        // Target selection needs the same public culture-race totals as
        // victory denial. Build them once for the assessment instead of
        // repeating a whole-world tourism scan for every sort comparison.
        let active_victory_target = self.active_victory_target(g);
        let rival_culture_pressures = self.rival_culture_pressures(g);
        let denial = if active_victory_target.is_some() {
            None
        } else {
            self.victory_denial_with_culture_pressures(g, pid, &rival_culture_pressures)
        };
        let emergency_objective = g.emergency_objective(pid).cloned();
        // Each arm carries the reason it fired. The strings are static and
        // cost nothing to build; they exist so the spectator's reasoning log
        // can say which of these tests the empire's whole plan turned on
        // instead of only naming the strategy that came out.
        let (strategy, because) = if at_war
            && (threatened_city.is_some() || my_power * 1.25 < strongest_rival)
        {
            (GrandStrategy::Recovery, "at war and losing ground at home")
        } else if emergency_objective.is_some() {
            (GrandStrategy::Conquest, "an emergency objective is standing")
        } else if basil_tagma_timing {
            (GrandStrategy::Conquest, "Tagma timing is live")
        } else if rush_victim.is_some() {
            (
                GrandStrategy::Conquest,
                "a neighbour is inside the ancient window and cannot wall in time",
            )
        } else if let Some(target) = active_victory_target {
            // The assigned-Religion arm is the only one that does not first ask
            // whether the empire can afford to expand. Measured, that costs the
            // whole empire: a seat committed to Religion from turn 0 finishes on
            // **1.68 cities** against the adaptive agent's **4.10**.
            let may_expand_first = self.assigned_religion_may_expand
                && cities.len() < desired_cities
                && has_site
                && g.turn < g.standard_duration(175);
            if target == VictoryTarget::Religion
                && g.players[pid].religion.is_none()
                && !may_expand_first
            {
                (GrandStrategy::Religion, "the religion lane still needs a religion")
            } else if cities.len() < desired_cities && has_site && g.turn < g.standard_duration(175)
            {
                (GrandStrategy::Expansion, "the assigned lane can still afford to expand first")
            } else {
                (target.strategy(), "following the assigned victory lane")
            }
        } else if let Some((_, counter)) = denial {
            (counter, "countering a rival close to winning")
        } else if at_war {
            (GrandStrategy::Conquest, "already at war")
        } else if self.prophet_before_opportunism && self.religious_opening_viable(g, pid) {
            // Same arm as the one below, tested one step earlier. See
            // `prophet_before_opportunism` for why the order is contested.
            (GrandStrategy::Religion, "a Prophet is a finite race worth entering now")
        } else if (g.turn >= 55 && cities.len() >= 2 && my_power > weakest_rival * 1.80 + 20.0)
            || (military_civ
                && g.turn >= 35
                && cities.len() >= 2
                && my_power >= strongest_rival * 1.10)
        {
            (GrandStrategy::Conquest, "strong enough to take what a neighbour has")
        } else if self.religious_opening_viable(g, pid) {
            // A Prophet is a finite global race, not an economic goal that can
            // wait until the generic city target is complete. Religious
            // production only occupies one city, so the baseline governor can
            // continue settlers and development in the rest of the empire.
            // Keep that commitment independent of the generic progress race:
            // improving a contender's science readiness must not make it
            // abandon a nearly earned, globally limited Prophet.
            (GrandStrategy::Religion, "a Prophet is a finite race worth entering now")
        } else if victory.progress >= 65 {
            (victory.strategy, "already well down its best victory lane")
        } else if cities.len() < desired_cities && has_site && Self::expansion_window_open(g) {
            (GrandStrategy::Expansion, "short of cities with land still open")
        } else {
            (victory.strategy, "its best available victory lane")
        };
        think!(self.journal(), Strategy, Strategy,
               "Grand strategy: {}", strategy.as_str();
               "{because} — {} cities of {desired_cities} wanted, power {my_power:.0} \
                against the strongest rival's {strongest_rival:.0}; \
                best lane {} at {}% progress",
               cities.len(), victory.strategy.as_str(), victory.progress);
        if let Some(city) = threatened_city.and_then(|id| g.cities.get(&id)) {
            think!(self.journal(), Strategy, Strategy,
                   "{} is under threat", city.name;
                   "the plan is written around defending it"; city.pos);
        }

        // Finish wars already in progress before selecting the next major
        // rival. In particular, this gives hostile city-states an explicit
        // city objective that the force-group planner can actually consume.
        let forced_target = self.forced_target_player.filter(|target| {
            g.players.get(*target).map(|player| player.alive).unwrap_or(false)
                && self.campaign_target_legal(g, pid, *target)
        });
        let target_player = if let Some(emergency) = &emergency_objective {
            Some(emergency.target)
        } else if wartime_rivals.is_empty() {
            // The rush already chose, on nearness and weakness, and the
            // generic value sort would happily re-aim the column at a richer
            // rival two weeks' march away.
            rush_victim.map(|(target, _)| target).or_else(|| {
            forced_target.or_else(|| {
                denial
                    .filter(|(rival, _)| self.campaign_target_legal(g, pid, *rival))
                    .map(|(rival, _)| rival)
                    .or_else(|| {
                        let mut candidates: Vec<_> = major_rivals
                            .iter()
                            .copied()
                            .filter(|rival| self.campaign_target_legal(g, pid, *rival))
                            .collect();
                        if strategy == GrandStrategy::Conquest {
                            candidates.extend(
                                g.players
                                    .iter()
                                    .filter(|player| player.is_minor)
                                    .filter(|player| {
                                        self.campaign_target_legal(g, pid, player.id)
                                    })
                                    .map(|player| player.id),
                            );
                        }
                        candidates
                            .into_iter()
                            .map(|rival| {
                                (
                                    rival,
                                    self.campaign_target_value_with_culture(
                                        g,
                                        pid,
                                        rival,
                                        rival_culture_pressures.get(&rival).copied(),
                                    ),
                                )
                            })
                            .min_by(|a, b| {
                                a.1.partial_cmp(&b.1)
                                    .unwrap()
                                    .then(a.0.cmp(&b.0))
                            })
                            .map(|(rival, _)| rival)
                    })
            })
            })
        } else {
            wartime_rivals
                .iter()
                .copied()
                .map(|rival| {
                    (
                        rival,
                        self.rival_value_with_culture(
                            g,
                            pid,
                            rival,
                            rival_culture_pressures.get(&rival).copied(),
                        ),
                    )
                })
                .min_by(|a, b| {
                    a.1.partial_cmp(&b.1)
                        .unwrap()
                        .then(a.0.cmp(&b.0))
                })
                .map(|(rival, _)| rival)
        };
        let target_city = emergency_objective
            .map(|emergency| emergency.city)
            // The rush aims at the capital and nothing else. A capital is the
            // one city whose loss can end a small neighbour outright, it is
            // where `city_strength`'s palace +3 is paid for by having the
            // whole empire's defence in one place, and the generic
            // `campaign_city_value` sort would otherwise send the column at
            // whichever border town scored best.
            .or_else(|| {
                rush_victim.filter(|(target, _)| target_player == Some(*target))
                    .map(|(_, capital)| capital)
            })
            .or_else(|| {
                target_player.and_then(|target| {
                    g.cities
                        .values()
                        .filter(|c| c.owner == target)
                        .min_by(|left, right| {
                            self.campaign_city_value(g, pid, left, strategy)
                                .total_cmp(&self.campaign_city_value(g, pid, right, strategy))
                                .then_with(|| left.id.cmp(&right.id))
                        })
                        .map(|c| c.id)
                })
            });

        if self.journal().wants(crate::reasoning::Level::Strategy) {
            match target_player.and_then(|id| g.players.get(id)) {
                Some(rival) => {
                    let objective = target_city
                        .and_then(|id| g.cities.get(&id))
                        .map(|city| format!("first objective {}", city.name))
                        .unwrap_or_else(|| "no city objective yet".to_string());
                    let at_war_now = if g.is_at_war(pid, rival.id) {
                        "already at war"
                    } else if wartime_rivals.is_empty() {
                        "not yet at war"
                    } else {
                        "chosen from the wars already running"
                    };
                    think!(self.journal(), Strategy, Strategy,
                           "Campaign aimed at {}", rival.civ;
                           "{at_war_now}; {objective}");
                }
                None => {
                    think!(self.journal(), Strategy, Strategy, "No campaign target";
                           "no rival is both reachable and worth opening against");
                }
            }
        }

        StrategicPlan {
            strategy,
            target_player,
            target_city,
            threatened_city,
            desired_cities,
            assessed_turn: g.turn,
            // Only a plan that actually aims at the victim is a rush. If
            // something later in target selection re-aimed the campaign, the
            // production bonus must not follow it.
            rush: rush_victim.is_some_and(|(victim, _)| target_player == Some(victim)),
        }
    }

    /// Whether a settler built *here, now* would still pay for itself.
    ///
    /// `expansion_window_open` reserves a flat `standard_duration(50)` at the
    /// end of every game for every city, whatever that city can actually do.
    /// A city making 20 production a turn builds a settler in four turns; one
    /// making three takes twenty-seven. A single reserve is wrong for both, and
    /// `expansion_funnel_blocker_census` measures the cost of being wrong in
    /// the strict direction: on the 6p/74x46 map the exhibition serves, the
    /// shut window is the **sole** blocker on 310 of 993 city-turns spent short
    /// of the empire's own city target — 31.2% of them.
    ///
    /// This asks the question the reserve was standing in for. Time enough to
    /// build the settler at this city's real production rate, walk it out, and
    /// then hold the ground long enough to return more than it cost.
    fn expansion_pays_back_for(&self, g: &Game, pid: usize, cid: u32) -> bool {
        let remaining = g.max_turns.saturating_sub(g.turn) as f64;
        let production = g.city_yields(cid).production.max(1.0);
        let build = g.item_remaining_cost_for_city(
            pid,
            cid,
            &Item::Unit {
                unit: "settler".into(),
            },
        ) / production;
        remaining > build + g.standard_duration(SETTLE_LAG + SETTLE_PAYBACK) as f64
    }

    fn expansion_window_open(g: &Game) -> bool {
        let payback_window = g.standard_duration(300);
        let endgame_reserve = g.standard_duration(50);
        let deadline = payback_window.min(g.max_turns.saturating_sub(endgame_reserve));
        g.turn < deadline
    }

    /// Lower is a more attractive rival: nearby, weak empires with valuable
    /// cities are preferable to distant low-power distractions.
    #[cfg(test)]
    fn rival_value(&self, g: &Game, pid: usize, other: usize) -> f64 {
        self.rival_value_with_culture(g, pid, other, None)
    }

    fn rival_value_with_culture(
        &self,
        g: &Game,
        pid: usize,
        other: usize,
        culture_pressure: Option<i32>,
    ) -> f64 {
        let mine = g.player_city_ids(pid);
        let theirs = g.player_city_ids(other);
        let distance = mine
            .iter()
            .flat_map(|a| {
                theirs
                    .iter()
                    .map(move |b| g.wdist(g.cities[a].pos, g.cities[b].pos))
            })
            .min()
            .unwrap_or(40) as f64;
        let victory_pressure = self
            .rival_victory_pressure_with_culture(g, other, culture_pressure)
            .progress as f64;
        distance * 7.0 + g.military_power(other) * 1.5
            - g.score(other) as f64 * 0.35
            - victory_pressure * 2.4
    }

    /// Campaign value extends the major-rival heuristic to city-states.
    /// Conquering a city-state is strategically possible, but it burns every
    /// invested Envoy and permanently removes a potential Suzerain bonus, so
    /// a nearby minor should displace a major target only when it is a clearly
    /// cheaper objective. A city-state that can be secured immediately with
    /// free Envoys is treated as an ally to win, not territory to destroy.
    fn campaign_target_legal(&self, g: &Game, pid: usize, other: usize) -> bool {
        let Some(player) = g.players.get(other) else {
            return false;
        };
        if other == pid || !player.alive || player.is_barbarian {
            return false;
        }

        // Preserve an already active war even if a loaded legacy position has
        // contradictory diplomacy. Outside war, relationship commitments are
        // hard legality masks, never soft terms in the positional score.
        if g.is_at_war(pid, other) {
            return true;
        }
        if g.are_friends(pid, other) || g.alliance_with(pid, other).is_some() {
            return false;
        }
        if player.is_minor {
            let Some(suzerain) = g.suzerain_of(other) else {
                return true;
            };
            if suzerain == pid
                || g.are_friends(pid, suzerain)
                || g.alliance_with(pid, suzerain).is_some()
            {
                return false;
            }
        }
        true
    }

    #[cfg(test)]
    fn campaign_target_value(&self, g: &Game, pid: usize, other: usize) -> f64 {
        self.campaign_target_value_with_culture(g, pid, other, None)
    }

    fn campaign_target_value_with_culture(
        &self,
        g: &Game,
        pid: usize,
        other: usize,
        culture_pressure: Option<i32>,
    ) -> f64 {
        let mut value = self.rival_value_with_culture(g, pid, other, culture_pressure);
        if !g.players[other].is_minor {
            // A leader marches on the civilizations their agenda disdains
            // before the ones it respects. Lower is a more attractive target,
            // so approval raises the bar and contempt lowers it. The weight
            // is deliberately smaller than distance, which still decides most
            // campaigns: an agenda colours the choice, it does not make it.
            return value + g.agenda_opinion(pid, other) * 2.0;
        }

        let mine = g.envoys_at(pid, other);
        value += 90.0 + mine as f64 * 45.0;
        let rival_envoys = g
            .players
            .iter()
            .filter(|player| !player.is_minor && !player.is_barbarian && player.id != pid)
            .map(|player| g.envoys_at(player.id, other))
            .max()
            .unwrap_or(0);
        let needed = (3_i64.max(rival_envoys + 1) - mine).max(1);
        if g.players[pid].envoys_free >= needed {
            value += 180.0;
        }
        if let Some(suzerain) = g.suzerain_of(other).filter(|suzerain| *suzerain != pid) {
            value += 40.0 + g.military_power(suzerain) * 0.25;
        }
        value += match g.cs_type(&g.players[other].civ) {
            "militaristic" => 55.0,
            "industrial" => 35.0,
            "scientific" | "cultural" | "religious" => 25.0,
            _ => 15.0,
        };
        value
    }

    fn yield_value(&self, yields: Yields, strategy: GrandStrategy) -> f64 {
        let (food, prod, gold, science, culture, faith) = match strategy {
            GrandStrategy::Expansion => (2.0, 2.2, 0.9, 1.2, 1.2, 0.5),
            GrandStrategy::Science => (1.4, 2.0, 1.0, 4.2, 1.2, 0.4),
            GrandStrategy::Culture => (1.4, 1.8, 1.0, 1.3, 4.2, 0.8),
            GrandStrategy::Religion => (1.4, 1.8, 0.9, 1.1, 1.5, 4.5),
            GrandStrategy::Diplomacy => (1.4, 1.7, 2.2, 1.2, 2.8, 0.7),
            GrandStrategy::Conquest => (1.2, 2.8, 1.4, 1.7, 0.8, 0.3),
            GrandStrategy::Recovery => (1.6, 3.2, 1.5, 1.0, 0.8, 0.3),
        };
        yields.food * food
            + yields.production * prod
            + yields.gold * gold
            + yields.science * science
            + yields.culture * culture
            + yields.faith * faith
    }

    /// Evaluate a repeatable district project as a bounded race move. The
    /// ongoing conversion is valued over the actual build horizon, while the
    /// completion award is priced against the live global Great Person race.
    /// This is deliberately analogous to an engine's passed-pawn extension:
    /// a project receives a large tempo bonus only when completing it crosses
    /// a concrete threshold, rather than from a fixed name-based preference.
    fn district_project_value(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        project: &str,
        plan: &StrategicPlan,
    ) -> f64 {
        let spec = &g.rules.projects[project];
        let city = &g.cities[&cid];
        let production = g.city_yields(cid).production.max(1.0);
        let item = Item::Project {
            project: Name::new(project),
        };
        let turns = g.item_remaining_cost_for_city(pid, cid, &item) / production;
        let threatened = plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4);
        let denominator = 7.0 + turns.max(1.0);

        let mut ongoing = Yields::default();
        for (kind, percent) in &spec.ongoing_yields {
            let amount = production * percent / 100.0;
            match kind.as_str() {
                "food" => ongoing.food += amount,
                "production" => ongoing.production += amount,
                "gold" => ongoing.gold += amount,
                "science" => ongoing.science += amount,
                "culture" => ongoing.culture += amount,
                "faith" => ongoing.faith += amount,
                _ => {}
            }
        }
        let horizon = turns.clamp(1.0, 16.0);
        let mut value = self.yield_value(ongoing, plan.strategy) * horizon * 4.0;

        for (kind, award) in g.project_completion_gpp_awards(pid, cid, project) {
            // Patronage outcome B can set this class's completion award to
            // zero. Ongoing yield conversion may still justify the project,
            // but a disabled class has no race tempo to extend.
            if award <= f64::EPSILON {
                continue;
            }
            let mut affinity: f64 = match (plan.strategy, kind.as_str()) {
                (GrandStrategy::Science, "scientist") => 2.5,
                (GrandStrategy::Culture, "writer" | "artist" | "musician") => 2.6,
                (GrandStrategy::Religion, "prophet") if g.players[pid].religion.is_none() => 2.8,
                (GrandStrategy::Diplomacy, "merchant") => 2.0,
                (GrandStrategy::Conquest, "general" | "admiral") => 2.3,
                (GrandStrategy::Expansion | GrandStrategy::Recovery, "engineer" | "merchant") => {
                    1.8
                }
                (GrandStrategy::Science | GrandStrategy::Culture, "engineer") => 1.6,
                (_, "prophet") if g.players[pid].religion.is_some() => 0.15,
                _ => 0.85,
            };
            let work = match kind.as_str() {
                "writer" => Some("writing"),
                "artist" => Some("art"),
                "musician" => Some("music"),
                _ => None,
            };
            if work.is_some_and(|work| !g.can_house_additional_great_work(pid, work)) {
                affinity *= 0.20;
            }

            let cost = g.gp_cost(pid, &kind).max(1.0);
            let mine = g.players[pid].gpp.get(&kind).copied().unwrap_or(0.0);
            let rival = g
                .players
                .iter()
                .filter(|player| {
                    player.id != pid && player.alive && !player.is_minor && !player.is_barbarian
                })
                .map(|player| player.gpp.get(&kind).copied().unwrap_or(0.0))
                .fold(0.0_f64, f64::max);
            let useful = award.min((cost - mine).max(0.0));
            value += useful * (5.0 + 5.0 * affinity);
            value += (rival / cost).clamp(0.0, 1.0) * 150.0 * affinity;
            if mine + award + f64::EPSILON >= cost && mine < cost {
                value += 620.0 * affinity;
            }
            if rival > mine && mine + award > rival {
                value += 240.0 * affinity;
            }
        }

        if spec.full_power_while_active {
            let deficit = (g.city_power_demand(city) - g.city_power_supply(city)).max(0.0);
            value += deficit * 55.0 * denominator;
        }

        if project == "bread_and_circuses" {
            let loyalty_need = (100.0 - city.loyalty).max(0.0);
            let nearby_foreign_pressure = g
                .cities
                .values()
                .filter(|other| {
                    other.owner != pid
                        && !g.players[other.owner].is_barbarian
                        && g.alliance_with(pid, other.owner)
                            .is_none_or(|alliance| alliance.kind != "cultural")
                })
                .filter_map(|other| {
                    let distance = g.wdist(city.pos, other.pos);
                    (distance <= 9)
                        .then_some(other.pop.max(1) as f64 * (10 - distance) as f64 / 10.0)
                })
                .sum::<f64>();
            if loyalty_need < 5.0 && nearby_foreign_pressure < 2.0 {
                value -= 260.0;
            } else {
                value += loyalty_need * 8.0
                    + nearby_foreign_pressure * horizon * 7.0
                    + spec
                        .effects
                        .get("completion_loyalty")
                        .copied()
                        .unwrap_or(0.0)
                        * 7.0;
            }
        }

        // A project may exploit completed infrastructure, but should not
        // indefinitely postpone the first building in the district that
        // enables it. This is the economic equivalent of a quiet-move pruning
        // guard: search the forcing race only after basic development exists.
        if let Some(district) = spec.district {
            let family = g.district_family(district);
            let has_family_building = city.buildings.iter().any(|building| {
                g.rules.buildings[building]
                    .district
                    .is_some_and(|built| g.district_family(built) == family)
            });
            let family_has_building = g.rules.buildings.values().any(|building| {
                building.buildable
                    && building
                        .district
                        .is_some_and(|built| g.district_family(built) == family)
            });
            if family_has_building && !has_family_building {
                value -= 420.0;
            }
        }
        if threatened {
            value -= 360.0;
        }
        value
    }

    fn product_layout_value(&self, g: &Game, pid: usize, strategy: GrandStrategy) -> f64 {
        let _memo = g.query_memo();
        g.player_city_ids(pid)
            .into_iter()
            .map(|city_id| {
                let city = &g.cities[&city_id];
                let mut value = self.yield_value(g.city_yields(city_id), strategy);
                // Housing beyond +3 no longer changes the immediate growth
                // rate. Valuing only the useful band sends Salt Products to
                // constrained cities instead of accumulating them in a city
                // that already has abundant headroom.
                let headroom = (g.city_housing(city) - city.pop as f64).clamp(-2.0, 3.0);
                value += headroom * 18.0;
                let active_salt = city
                    .products
                    .iter()
                    .take(g.product_capacity(city))
                    .filter(|product| product.as_str() == "salt")
                    .count() as f64;
                value += active_salt * city.pop.max(1) as f64 * 2.5;
                value
            })
            .sum()
    }

    /// Products are movable economic Great Works. Search every legal move on
    /// a cloned position and make one only when it strictly improves the
    /// strategy-sensitive empire evaluation; the strict threshold prevents a
    /// free relocation from oscillating between equivalent slots.
    fn advanced_products(&self, g: &mut Game, pid: usize, strategy: GrandStrategy) {
        let candidates: BTreeSet<(u32, u32, Name)> = g
            .legal_actions_within(pid, ActionFamilies::PRODUCTS)
            .into_iter()
            .filter_map(|action| match action {
                Action::MoveProduct { from, to, product } => Some((from, to, product)),
                _ => None,
            })
            .collect();
        let baseline = self.product_layout_value(g, pid, strategy);
        let mut best: Option<(f64, u32, u32, Name)> = None;
        for (from, to, product) in candidates {
            let action = Action::MoveProduct {
                from,
                to,
                product: Name::new(&product),
            };
            let mut next = g.clone();
            if next.apply(pid, &action).is_err() {
                continue;
            }
            let value = self.product_layout_value(&next, pid, strategy);
            let replace = best.as_ref().is_none_or(|current| {
                value > current.0 + 1e-9
                    || ((value - current.0).abs() <= 1e-9
                        && (to, from, product.as_str())
                            < (current.2, current.1, current.3.as_str()))
            });
            if replace {
                best = Some((value, from, to, product));
            }
        }
        let Some((value, from, to, product)) = best else {
            return;
        };
        if value <= baseline + 0.01 {
            return;
        }
        let _ = g.apply(pid, &Action::MoveProduct { from, to, product: Name::new(&product) });
    }

    fn advanced_research(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        // Explicit evaluator targets and the adaptive live plan must drive the
        // same prerequisite search.  Previously only `victory_target` enabled
        // milestone routing, so a normal spectator AI could correctly assess
        // Science or Culture yet wander through generic unlocks indefinitely.
        let objective = self
            .victory_target
            .map(VictoryTarget::strategy)
            .unwrap_or(plan.strategy);
        if g.players[pid].research.is_none() {
            let available = g.available_techs(pid);
            let science_commitment = objective == GrandStrategy::Science
                || self.diplomatic_science_backup(g, pid, plan);
            let forced_goal = match objective {
                _ if g.has_ability(pid, "taxis")
                    && g.players[pid].religion.is_none()
                    && !g.players[pid].techs.contains(&crate::name!("astrology")) =>
                {
                    Some("astrology")
                }
                // An ancient rush rides. `rush_census` measures **0% of
                // empires holding `horseback_riding` at turn 50**, so the
                // stack is warriors — and a warrior attacking the measured
                // capital takes 28 damage a blow and dies on its fourth,
                // having dealt 134 of the 200 needed. That is the whole
                // reason a war declared at turn 30 does not take a city until
                // turn 53: the stack trades itself, and the next one has to be
                // built and marched all over again.
                //
                // A horseman is strength 36 for 80 production against a
                // warrior's 20 for 40 — it takes 15 a blow instead of 28, so
                // it survives the siege rather than paying for it — and it has
                // **4 movement against 2**, which halves the nine-to-twelve
                // turn march that is the lane's other binding cost. At 195
                // science it is also cheaper to reach than iron working's 225.
                _ if plan.rush && !g.players[pid].techs.contains(&crate::name!("horseback_riding")) => {
                    Some("horseback_riding")
                }
                _ if science_commitment => [
                    "rocketry",
                    "satellites",
                    "nanotechnology",
                    "smart_materials",
                    "offworld_mission",
                ]
                .into_iter()
                .find(|tech| !g.players[pid].techs.contains(&Name::new(tech))),
                GrandStrategy::Culture => ["printing", "radio", "computers"]
                    .into_iter()
                    .find(|tech| !g.players[pid].techs.contains(&Name::new(tech))),
                GrandStrategy::Diplomacy if !g.players[pid].techs.contains(&crate::name!("seasteads")) => {
                    Some("seasteads")
                }
                GrandStrategy::Religion if !g.players[pid].techs.contains(&crate::name!("astrology")) => {
                    Some("astrology")
                }
                _ => None,
            };
            let goal_pick = forced_goal.and_then(|goal| {
                available
                    .iter()
                    .filter(|tech| self.tech_leads_to(g, tech, goal))
                    .min_by(|a, b| {
                        g.rules.techs[*a]
                            .cost
                            .partial_cmp(&g.rules.techs[*b].cost)
                            .unwrap()
                            .then(a.cmp(b))
                    })
                    .cloned()
            });
            let pick = goal_pick.clone().or_else(|| {
                available
                    .iter()
                    .max_by(|a, b| {
                        self.tech_value(g, pid, a, plan.strategy)
                            .partial_cmp(&self.tech_value(g, pid, b, plan.strategy))
                            .unwrap()
                            .then_with(|| b.cmp(a))
                    })
                    .cloned()
            });
            if let Some(tech) = pick {
                if self.journal().wants(crate::reasoning::Level::Decision) {
                    let why = match (forced_goal, &goal_pick) {
                        (Some(goal), Some(_)) => {
                            format!("the cheapest step toward {}, which {} needs",
                                    plain(goal), objective.as_str())
                        }
                        _ => {
                            let runner_up = available
                                .iter()
                                .filter(|other| **other != tech)
                                .max_by(|a, b| {
                                    self.tech_value(g, pid, a, plan.strategy)
                                        .partial_cmp(&self.tech_value(g, pid, b, plan.strategy))
                                        .unwrap()
                                        .then_with(|| b.cmp(a))
                                })
                                .map(|other| {
                                    format!("ahead of {} at {:.0}", plain(other),
                                            self.tech_value(g, pid, other, plan.strategy))
                                })
                                .unwrap_or_else(|| "with nothing else on offer".to_string());
                            format!("worth {:.0} to the {} plan, {runner_up}",
                                    self.tech_value(g, pid, &tech, plan.strategy),
                                    plan.strategy.as_str())
                        }
                    };
                    think!(self.journal(), Research, Decision, "Researching {}", plain(&tech); "{why}");
                }
                let _ = g.apply(pid, &Action::Research { tech: Name::new(&tech) });
            }
        }
        if g.players[pid].civic.is_none() {
            let available = g.available_civics(pid);
            let forced_goal = match objective {
                _ if g.has_ability(pid, "taxis")
                    && !g.players[pid].civics.contains(&crate::name!("divine_right")) =>
                {
                    Some("divine_right")
                }
                GrandStrategy::Culture => [
                    "humanism",
                    "conservation",
                    "professional_sports",
                    "cultural_heritage",
                    "space_race",
                    "environmentalism",
                    "social_media",
                ]
                .into_iter()
                .find(|civic| !g.players[pid].civics.contains(&Name::new(civic))),
                GrandStrategy::Science if !g.players[pid].civics.contains(&crate::name!("space_race")) => {
                    Some("space_race")
                }
                GrandStrategy::Diplomacy
                    if !g.players[pid].civics.contains(&crate::name!("global_warming_mitigation")) =>
                {
                    Some("global_warming_mitigation")
                }
                GrandStrategy::Religion if !g.players[pid].civics.contains(&crate::name!("theology")) => {
                    Some("theology")
                }
                _ => None,
            };
            let goal_pick = forced_goal.and_then(|goal| {
                available
                    .iter()
                    .filter(|civic| self.civic_leads_to(g, civic, goal))
                    .min_by(|a, b| {
                        g.rules.civics[*a]
                            .cost
                            .partial_cmp(&g.rules.civics[*b].cost)
                            .unwrap()
                            .then(a.cmp(b))
                    })
                    .cloned()
            });
            let pick = goal_pick.clone().or_else(|| {
                available
                    .iter()
                    .max_by(|a, b| {
                        self.civic_value(g, pid, a, plan.strategy)
                            .partial_cmp(&self.civic_value(g, pid, b, plan.strategy))
                            .unwrap()
                            .then_with(|| b.cmp(a))
                    })
                    .cloned()
            });
            if let Some(civic) = pick {
                if self.journal().wants(crate::reasoning::Level::Decision) {
                    let why = match (forced_goal, &goal_pick) {
                        (Some(goal), Some(_)) => {
                            format!("the cheapest step toward {}, which {} needs",
                                    plain(goal), objective.as_str())
                        }
                        _ => {
                            let runner_up = available
                                .iter()
                                .filter(|other| **other != civic)
                                .max_by(|a, b| {
                                    self.civic_value(g, pid, a, plan.strategy)
                                        .partial_cmp(&self.civic_value(g, pid, b, plan.strategy))
                                        .unwrap()
                                        .then_with(|| b.cmp(a))
                                })
                                .map(|other| {
                                    format!("ahead of {} at {:.0}", plain(other),
                                            self.civic_value(g, pid, other, plan.strategy))
                                })
                                .unwrap_or_else(|| "with nothing else on offer".to_string());
                            format!("worth {:.0} to the {} plan, {runner_up}",
                                    self.civic_value(g, pid, &civic, plan.strategy),
                                    plan.strategy.as_str())
                        }
                    };
                    think!(self.journal(), Research, Decision, "Adopting the {} civic", plain(&civic);
                           "{why}");
                }
                let _ = g.apply(pid, &Action::Civic { civic: Name::new(&civic) });
            }
        }
    }

    fn tech_leads_to(&self, g: &Game, candidate: &str, target: &str) -> bool {
        candidate == target
            || g.rules
                .tech_ancestors
                .get(target)
                .is_some_and(|ancestors| ancestors.contains(candidate))
    }

    fn civic_leads_to(&self, g: &Game, candidate: &str, target: &str) -> bool {
        candidate == target
            || g.rules
                .civic_ancestors
                .get(target)
                .is_some_and(|ancestors| ancestors.contains(candidate))
    }

    fn advanced_secret_society(&self, g: &mut Game, pid: usize, strategy: GrandStrategy) {
        if !g.game_mode("secret_societies")
            || g.players[pid].secret_society.is_some()
            || !g.players[pid].civics.contains(&crate::name!("code_of_laws"))
        {
            return;
        }
        let long_term = self
            .victory_target
            .map(VictoryTarget::strategy)
            .unwrap_or(strategy);
        let society = match long_term {
            GrandStrategy::Science => "hermetic_order",
            GrandStrategy::Culture | GrandStrategy::Religion => "voidsingers",
            GrandStrategy::Diplomacy
            | GrandStrategy::Conquest
            | GrandStrategy::Expansion
            | GrandStrategy::Recovery => "owls_of_minerva",
        };
        let _ = g.apply(
            pid,
            &Action::ChooseSecretSociety {
                society: Name::new(society),
            },
        );
    }

    fn strategic_government(&self, g: &mut Game, pid: usize, strategy: GrandStrategy) {
        let objective = self
            .victory_target
            .map(VictoryTarget::strategy)
            .unwrap_or(strategy);
        let unlocked = |government: &str| {
            g.rules.governments.get(government).is_some_and(|spec| {
                spec.civic
                    .as_ref()
                    .is_none_or(|civic| g.players[pid].civics.contains(civic))
            })
        };

        // Matching the leading Culture defender removes the full -40%
        // Gathering Storm penalty between distinct Tier 3/4 governments.
        // Lower-tier governments have zero intolerance and do not justify
        // giving up the stronger late-game government effects.
        let culture_match = (objective == GrandStrategy::Culture)
            .then(|| {
                g.players
                    .iter()
                    .filter(|rival| {
                        rival.id != pid && rival.alive && !rival.is_minor && !rival.is_barbarian
                    })
                    .max_by_key(|rival| (g.domestic_tourists(rival.id), rival.id))
                    .and_then(|rival| rival.government.clone())
            })
            .flatten()
            .filter(|government| {
                matches!(
                    government.as_str(),
                    "communism"
                        | "democracy"
                        | "fascism"
                        | "corporate_libertarianism"
                        | "digital_democracy"
                        | "synthetic_technocracy"
                ) && unlocked(government)
            });

        let faith_mobilization =
            matches!(strategy, GrandStrategy::Conquest | GrandStrategy::Recovery)
                && g.players[pid].faith >= 600.0
                && unlocked("theocracy");
        let priorities: &[&str] = match objective {
            GrandStrategy::Culture | GrandStrategy::Diplomacy => &[
                "digital_democracy",
                "democracy",
                "merchant_republic",
                "monarchy",
                "theocracy",
                "classical_republic",
                "chiefdom",
            ],
            GrandStrategy::Science => &[
                "synthetic_technocracy",
                "communism",
                "democracy",
                "merchant_republic",
                "monarchy",
                "theocracy",
                "classical_republic",
                "chiefdom",
            ],
            GrandStrategy::Conquest if faith_mobilization => &[
                "theocracy",
                "corporate_libertarianism",
                "fascism",
                "communism",
                "monarchy",
                "merchant_republic",
                "oligarchy",
                "chiefdom",
            ],
            GrandStrategy::Conquest => &[
                "corporate_libertarianism",
                "fascism",
                "communism",
                "monarchy",
                "merchant_republic",
                "theocracy",
                "oligarchy",
                "chiefdom",
            ],
            GrandStrategy::Religion => &[
                "theocracy",
                "monarchy",
                "merchant_republic",
                "classical_republic",
                "chiefdom",
            ],
            GrandStrategy::Expansion => &[
                "corporate_libertarianism",
                "communism",
                "merchant_republic",
                "monarchy",
                "theocracy",
                "classical_republic",
                "chiefdom",
            ],
            GrandStrategy::Recovery if faith_mobilization => &[
                "theocracy",
                "digital_democracy",
                "democracy",
                "communism",
                "merchant_republic",
                "monarchy",
                "classical_republic",
                "chiefdom",
            ],
            GrandStrategy::Recovery => &[
                "digital_democracy",
                "democracy",
                "communism",
                "merchant_republic",
                "monarchy",
                "theocracy",
                "classical_republic",
                "chiefdom",
            ],
        };
        let choice = culture_match.or_else(|| {
            priorities
                .iter()
                .copied()
                .find(|government| unlocked(government))
                .map(str::to_string)
        });
        if let Some(government) = choice
            .filter(|government| g.players[pid].government.as_deref() != Some(government.as_str()))
        {
            // Returning to any previously used government costs two complete
            // turns of Anarchy. An adaptive plan can legitimately change its
            // mind as a victory race moves, but a lateral return between (for
            // example) Democracy and Fascism must not zero every empire yield
            // on alternating turns. Only pay that recurring cost for a real
            // policy-capacity upgrade; first-time governments remain free.
            let policy_capacity = |name: &str| {
                g.rules.governments.get(name).map_or(0, |spec| {
                    spec.slots.military
                        + spec.slots.economic
                        + spec.slots.diplomatic
                        + spec.slots.wildcard
                })
            };
            let returning = g.players[pid].past_governments.contains(&government);
            let current_capacity = g.players[pid]
                .government
                .as_deref()
                .map_or(0, policy_capacity);
            let choice_capacity = policy_capacity(&government);
            // A newly tried government is free, but dropping from a mature
            // eight-slot government to a six-slot faith or military stopgap
            // invites an expensive return as soon as the adaptive plan moves
            // again. Never give up policy capacity; among equal-capacity
            // governments a first adoption remains free, while a repeat is
            // still blocked by the Anarchy guard below.
            if choice_capacity < current_capacity
                || (returning && choice_capacity == current_capacity)
            {
                think!(self.journal(), Government, Detail,
                       "Staying under {}",
                       plain(g.players[pid].government.as_deref().unwrap_or("no government"));
                       "{} offers {choice_capacity} policy slots against the \
                        current {current_capacity}{}, and two turns of Anarchy is not \
                        worth paying for that",
                       plain(&government),
                       if returning { " and has been run before" } else { "" });
                return;
            }
            think!(self.journal(), Government, Decision, "Changing government to {}", plain(&government);
                   "{choice_capacity} policy slots against {current_capacity} now; \
                    the {} plan wants it",
                   objective.as_str());
            let _ = g.apply(pid, &Action::Government { government: Name::new(&government) });
        }
    }

    /// Whether replacing one active card with another can fit in the current
    /// typed and wildcard policy slots. This mirrors the engine's set-level
    /// seating rule before either action is applied: probing by actually
    /// unslotting a card writes a real action even when the candidate then
    /// fails and the old card is restored.
    fn policy_swap_fits(g: &Game, pid: usize, current: &Name, candidate: &str) -> bool {
        let slots = g.gov_slots(pid);
        let (mut military, mut economic, mut diplomatic, mut wildcard) =
            (0i64, 0i64, 0i64, 0i64);
        let mut count = |card: &str| match g.rules.policies[card].slot.as_str() {
            "military" => military += 1,
            "economic" => economic += 1,
            "diplomatic" => diplomatic += 1,
            _ => wildcard += 1,
        };
        for card in g.players[pid]
            .policies
            .iter()
            .filter(|card| *card != current)
        {
            count(card);
        }
        count(candidate);
        let overflow = (military - slots.military).max(0)
            + (economic - slots.economic).max(0)
            + (diplomatic - slots.diplomatic).max(0);
        overflow + wildcard <= slots.wildcard
    }

    /// Reassess policy cards as the civic tree advances instead of treating
    /// the first cards which filled a government as permanent.  Each plan has
    /// a complete late-game portfolio, while temporary Dark Age cards are
    /// admitted only when their explicit downside is safe for the live empire.
    /// Typed cards preferentially replace cards of their own type so wildcard
    /// capacity remains useful.
    fn strategic_policies(&self, g: &mut Game, pid: usize, strategy: GrandStrategy) {
        let objective = self
            .victory_target
            .map(VictoryTarget::strategy)
            .unwrap_or(strategy);

        // A successor card removes its predecessor from the policy menu.  An
        // already slotted predecessor used to survive forever, which is how
        // future governments reached the archive still running Ilkum and
        // Colonization.  Retire those cards before choosing the new portfolio.
        let obsolete: HashSet<Name> = g
            .rules
            .policies
            .values()
            .filter(|policy| {
                policy
                    .civic
                    .as_ref()
                    .is_none_or(|civic| g.players[pid].civics.contains(civic))
            })
            .filter_map(|policy| policy.replaces.clone())
            .collect();
        let obsolete_active: Vec<Name> = g.players[pid]
            .policies
            .iter()
            .filter(|card| obsolete.contains(&Name::new(card.as_str())))
            .cloned()
            .collect();
        for card in obsolete_active {
            think!(self.journal(), Policies, Decision, "Retiring {}", plain(&card);
                   "a successor card has replaced it on the menu");
            let _ = g.apply(pid, &Action::UnslotPolicy { policy: card });
        }

        let mut desired: Vec<&str> = match objective {
            GrandStrategy::Science => vec![
                "integrated_space_cell",
                "future_victory_science",
                "five_year_plan",
                "rationalism",
                "nobel_prize",
                "international_space_agency",
                "ecommerce",
                "market_economy",
                "new_deal",
                "levee_en_masse",
                "military_research",
                "cryptography",
                "future_counter_science",
            ],
            GrandStrategy::Culture => vec![
                "future_victory_culture",
                "heritage_tourism",
                "satellite_broadcasts",
                "online_communities",
                "collective_activism",
                "sports_media",
                "grand_opera",
                "symphonies",
                "ecommerce",
                "new_deal",
                "gunboat_diplomacy",
                "cryptography",
                "communications_office",
                "levee_en_masse",
            ],
            GrandStrategy::Religion => vec![
                "religious_orders",
                "simultaneum",
                "scripture",
                "revelation",
                "wars_of_religion",
                "collectivization",
                "new_deal",
                "ecommerce",
                "levee_en_masse",
                "gunboat_diplomacy",
                "cryptography",
                "wisselbanken",
                "raj",
            ],
            GrandStrategy::Diplomacy => vec![
                "future_victory_diplomatic",
                "gunboat_diplomacy",
                "charismatic_leader",
                "raj",
                "merchant_confederation",
                "diplomatic_league",
                "containment",
                "collective_activism",
                "international_space_agency",
                "cryptography",
                "communications_office",
                "wisselbanken",
                "ecommerce",
                "new_deal",
                "levee_en_masse",
            ],
            GrandStrategy::Conquest => vec![
                "future_victory_domination",
                "future_counter_domination",
                "military_first",
                "strategic_air_force",
                "lightning_warfare",
                "total_war",
                "propaganda",
                "levee_en_masse",
                "force_modernization",
                "after_action_reports",
                "logistics",
                "five_year_plan",
                "ecommerce",
                "gunboat_diplomacy",
                "cryptography",
                "new_deal",
            ],
            GrandStrategy::Expansion => vec![
                "expropriation",
                "public_works",
                "five_year_plan",
                "economic_union",
                "collectivization",
                "ecommerce",
                "market_economy",
                "new_deal",
                "colonial_taxes",
                "colonial_offices",
                "gunboat_diplomacy",
                "levee_en_masse",
                "logistics",
                "rationalism",
                "cryptography",
            ],
            GrandStrategy::Recovery => vec![
                "new_deal",
                "liberalism",
                "civil_prestige",
                "collectivization",
                "five_year_plan",
                "ecommerce",
                "market_economy",
                "wisselbanken",
                "raj",
                "gunboat_diplomacy",
                "cryptography",
                "communications_office",
                "levee_en_masse",
                "logistics",
                "public_works",
            ],
        };
        if g.has_ability(pid, "taxis") && objective == GrandStrategy::Conquest {
            desired.splice(0..0, ["chivalry", "maneuver", "raid", "conscription"]);
        }

        let city_ids = g.player_city_ids(pid);
        let unit_ids = g.player_unit_ids(pid);
        let settlers = unit_ids
            .iter()
            .filter(|unit| g.units[unit].kind == "settler")
            .count();
        let city_goal = self
            .plan
            .as_ref()
            .map(|plan| plan.desired_cities)
            .unwrap_or_else(|| self.base.w.city_target.ceil().max(1.0) as usize);
        let expansion_complete = strategy != GrandStrategy::Expansion
            && settlers == 0
            && !city_ids.is_empty()
            && city_ids.len() >= city_goal;
        let at_war = g
            .at_war
            .iter()
            .any(|(first, second)| *first == pid || *second == pid);
        let military = unit_ids
            .iter()
            .filter(|unit| g.rules.units[g.units[unit].kind].class == "military")
            .count();
        let elite_active = g.players[pid].policies.contains(&crate::name!("elite_forces"));
        let elite_affordable = if elite_active {
            g.players[pid].gold_per_turn >= 5.0
        } else {
            g.players[pid].gold_per_turn >= military as f64 * 2.0 + 5.0
        };
        let robber_active = g.players[pid].policies.contains(&crate::name!("robber_barons"));
        let robber_pays = city_ids.iter().any(|city| {
            let city = &g.cities[city];
            city.buildings.iter().any(|building| {
                matches!(building.as_str(), "stock_exchange" | "factory")
                    && !city.pillaged_buildings.contains(building)
            })
        });
        let robber_safe = robber_pays
            && city_ids.iter().all(|city| {
                g.city_amenity_surplus(&g.cities[city]) >= if robber_active { 0 } else { 2 }
            });
        let holy_site_cities = city_ids
            .iter()
            .filter(|city| g.city_has_district_family(&g.cities[city], crate::name!("holy_site")))
            .count();

        let mut temporary = Vec::new();
        match objective {
            GrandStrategy::Science if holy_site_cities * 2 >= city_ids.len().max(1) => {
                temporary.push("monasticism");
            }
            GrandStrategy::Religion if g.players[pid].religion.is_some() => {
                temporary.push("inquisition");
            }
            GrandStrategy::Conquest if at_war => {
                temporary.push("twilight_valor");
                if elite_affordable {
                    temporary.push("elite_forces");
                }
            }
            _ => {}
        }
        if expansion_complete
            && matches!(
                objective,
                GrandStrategy::Science
                    | GrandStrategy::Culture
                    | GrandStrategy::Diplomacy
                    | GrandStrategy::Recovery
            )
        {
            temporary.push("isolationism");
        }
        if robber_safe
            && matches!(
                objective,
                GrandStrategy::Science
                    | GrandStrategy::Culture
                    | GrandStrategy::Expansion
                    | GrandStrategy::Recovery
            )
        {
            temporary.push("robber_barons");
        }
        temporary.extend(desired);
        desired = temporary;
        desired.retain(|card| {
            g.rules.policies[*card].offered(&g.players[pid].age, g.world_era)
        });

        // If circumstances changed, remove a downside-bearing Dark Age card
        // immediately.  Most importantly, Isolationism can never coexist with
        // a live settler or an Expansion plan.
        let desired_set: HashSet<&str> = desired.iter().copied().collect();
        let unsafe_dark_cards: Vec<Name> = g.players[pid]
            .policies
            .iter()
            .filter(|card| {
                g.rules.policies[card].dark_age
                    && !desired_set.contains(card.as_str())
            })
            .cloned()
            .collect();
        for card in unsafe_dark_cards {
            think!(self.journal(), Policies, Decision, "Dropping the Dark Age card {}", plain(&card);
                   "its downside no longer suits the {} plan", objective.as_str());
            let _ = g.apply(pid, &Action::UnslotPolicy { policy: card });
        }

        // The portfolio itself is a decision, and the order is the whole of
        // the reasoning behind every slot below: an observer who can see the
        // ranking can tell a card that was skipped from one that never made
        // the list.
        if self.journal().wants(crate::reasoning::Level::Detail) {
            let held: Vec<&str> = desired
                .iter()
                .copied()
                .filter(|card| g.players[pid].policies.contains(&Name::new(card)))
                .collect();
            think!(self.journal(), Policies, Detail,
                   "Policy portfolio for the {} plan", objective.as_str();
                   "wants, in order: {}; already slotted: {}",
                   desired.iter().map(|card| plain(card)).collect::<Vec<_>>().join(", "),
                   if held.is_empty() {
                       "none".to_string()
                   } else {
                       held.iter().map(|card| plain(card)).collect::<Vec<_>>().join(", ")
                   });
        }

        let wanted = desired.len();
        let available: HashSet<Name> = g.available_policies(pid).into_iter().collect();
        for (rank, card) in desired.iter().copied().enumerate() {
            if g.players[pid].policies.contains(&Name::new(card))
                || !available.contains(&Name::new(card))
            {
                continue;
            }
            if g.apply(
                pid,
                &Action::SlotPolicy {
                    policy: Name::new(card),
                },
            )
            .is_ok()
            {
                think!(self.journal(), Policies, Decision, "Slotted {}", plain(card);
                       "priority {} of {wanted} for the {} plan, into a free slot",
                       rank + 1, objective.as_str());
                continue;
            }

            let slot = g.rules.policies[card].slot.clone();
            let mut replaceable: Vec<Name> = g.players[pid]
                .policies
                .iter()
                .filter(|current| !desired_set.contains(current.as_str()))
                .filter(|current| Self::policy_swap_fits(g, pid, current, card))
                .cloned()
                .collect();
            replaceable.sort_by(|first, second| {
                let key = |current: &str| {
                    let policy = &g.rules.policies[current];
                    let era = policy
                        .civic
                        .as_ref()
                        .and_then(|civic| g.rules.civics.get(civic))
                        .map_or(0, |civic| civic.era);
                    (usize::from(policy.slot != slot), era)
                };
                key(first).cmp(&key(second)).then(first.cmp(second))
            });
            for current in replaceable {
                let _ = g.apply(
                    pid,
                    &Action::UnslotPolicy {
                        policy: current.clone(),
                    },
                );
                if g.apply(
                    pid,
                    &Action::SlotPolicy {
                        policy: Name::new(card),
                    },
                )
                .is_ok()
                {
                    think!(self.journal(), Policies, Decision,
                           "Slotted {} over {}", plain(card), plain(&current);
                           "priority {} of {wanted} for the {} plan; {} was the oldest \
                            card the plan does not want",
                           rank + 1, objective.as_str(), plain(&current));
                    break;
                }
                // A type mismatch can make one particular swap invalid. Put
                // the old card back before trying another candidate so policy
                // reassessment can never silently empty a government.
                let _ = g.apply(pid, &Action::SlotPolicy { policy: current });
            }
        }
    }

    fn tech_value(&self, g: &Game, pid: usize, tech: &str, strategy: GrandStrategy) -> f64 {
        let spec = &g.rules.techs[tech];
        let mut value = if g.players[pid].boosted_techs.contains(&Name::new(tech)) {
            28.0
        } else {
            0.0
        };
        for (name, unit) in &g.rules.units {
            if unit.tech.as_deref() == Some(tech)
                && unit
                    .unique_to
                    .as_ref()
                    .is_none_or(|c| c == &g.players[pid].civ)
            {
                let power = unit.strength.max(unit.ranged_attack_strength());
                value += if strategy == GrandStrategy::Conquest {
                    power * 3.2
                } else {
                    power * 1.1
                };
                if !self.civ_blind
                    && g.rules.civs[&g.players[pid].civ].unique_unit.as_deref() == Some(name)
                {
                    value += 55.0;
                }
            }
        }
        // A node that unlocks the successor of units already on the map buys
        // an upgrade for every one of them, not merely the option to train
        // something new. Counted over a whole game, the commonest reason an
        // upgrade was unavailable was a node its owner had simply never taken
        // — Archery, Iron Working, Machinery — while empires holding ten to
        // thirty technologies still fielded the Slingers and Warriors those
        // nodes would have retired.
        let stranded: f64 = g
            .units
            .values()
            .filter(|unit| unit.owner == pid)
            .filter_map(|unit| {
                let held = &g.rules.units[unit.kind];
                let successor = g.rules.units.get(held.upgrade_to.as_deref()?)?;
                (successor.tech.as_deref() == Some(tech)).then(|| {
                    (successor.strength.max(successor.ranged_attack_strength())
                        - held.strength.max(held.ranged_attack_strength()))
                    .max(0.0)
                })
            })
            .sum();
        // Capped so a large stale army nudges the order rather than dictating
        // it: at the ceiling this is worth about as much as an embarkation
        // prerequisite, which is the largest bonus already in this function.
        value += (stranded
            * if strategy == GrandStrategy::Conquest {
                2.4
            } else {
                1.4
            })
        .min(150.0);
        for building in g
            .rules
            .buildings
            .values()
            .filter(|b| b.tech.as_deref() == Some(tech))
        {
            value += self.yield_value(building.yields, strategy) * 14.0
                + building.housing * 12.0
                + building.amenity * 18.0;
        }
        for district in g
            .rules
            .districts
            .values()
            .filter(|d| d.tech.as_deref() == Some(tech))
        {
            value += self.yield_value(district.yields, strategy) * 18.0
                + district.defense * 1.5
                + district.amenity * 18.0;
        }
        for project in g
            .rules
            .projects
            .values()
            .filter(|p| p.tech.as_deref() == Some(tech))
        {
            value += if strategy == GrandStrategy::Science {
                if project.repeatable {
                    120.0
                } else {
                    260.0
                }
            } else if project.repeatable {
                25.0
            } else {
                65.0
            };
        }
        for improvement in g
            .rules
            .improvements
            .values()
            .filter(|i| i.tech.as_deref() == Some(tech))
        {
            value += self.yield_value(improvement.yields, strategy) * 10.0 + 18.0;
        }
        if strategy == GrandStrategy::Religion && tech == "astrology" {
            value += 95.0;
        }
        if let Some(goal) = BasicAi::water_research_goal(g, pid) {
            if self.tech_leads_to(g, tech, goal) {
                // Embarkation and ocean access change which parts of the map
                // are strategically reachable, so their prerequisites must
                // compete with ordinary yield unlocks rather than wait for a
                // naval unit to happen to win a generic score comparison.
                value += match goal {
                    "sailing" => 190.0,
                    "shipbuilding" => 230.0,
                    "celestial_navigation" => 150.0,
                    "cartography" => 210.0,
                    "square_rigging" | "steam_power" | "refining" | "electricity"
                    | "combined_arms" | "lasers" | "telecommunications" => 185.0,
                    _ => 0.0,
                };
            }
        }
        if strategy == GrandStrategy::Science {
            let milestone = if !g.players[pid]
                .science_projects
                .contains("launch_earth_satellite")
            {
                "rocketry"
            } else if !g.players[pid]
                .science_projects
                .contains("launch_moon_landing")
            {
                "satellites"
            } else if !g.players[pid]
                .science_projects
                .contains("launch_mars_colony")
            {
                "nanotechnology"
            } else if !g.players[pid]
                .science_projects
                .contains("exoplanet_expedition")
            {
                "smart_materials"
            } else {
                "offworld_mission"
            };
            if self.tech_leads_to(g, tech, milestone) {
                value += if self.victory_target == Some(VictoryTarget::Science) {
                    900.0
                } else {
                    260.0
                };
            }
        }
        // One-step lookahead prevents cheap prerequisites from being ignored.
        for (future, child) in &g.rules.techs {
            if child.requires.iter().any(|r| r == tech) {
                let unlocks = g
                    .rules
                    .units
                    .values()
                    .filter(|u| u.tech.as_deref() == Some(future))
                    .count()
                    + g.rules
                        .buildings
                        .values()
                        .filter(|b| b.tech.as_deref() == Some(future))
                        .count()
                    + g.rules
                        .districts
                        .values()
                        .filter(|d| d.tech.as_deref() == Some(future))
                        .count()
                    + g.rules
                        .projects
                        .values()
                        .filter(|p| p.tech.as_deref() == Some(future))
                        .count();
                value += unlocks as f64 * 8.0;
            }
        }
        // Discount by opportunity cost so a flashy late-era unlock does not
        // stall several cheaper advances. Square root still lets a genuinely
        // transformative breakthrough win the comparison.
        (value + 35.0) / spec.cost.max(10.0).sqrt()
    }

    fn civic_value(&self, g: &Game, pid: usize, civic: &str, strategy: GrandStrategy) -> f64 {
        let spec = &g.rules.civics[civic];
        let mut value = if g.players[pid].boosted_civics.contains(&Name::new(civic)) {
            28.0
        } else {
            0.0
        };
        for building in g
            .rules
            .buildings
            .values()
            .filter(|b| b.civic.as_deref() == Some(civic))
        {
            value += self.yield_value(building.yields, strategy) * 15.0
                + building.housing * 12.0
                + building.amenity * 18.0;
        }
        for district in g
            .rules
            .districts
            .values()
            .filter(|d| d.civic.as_deref() == Some(civic))
        {
            value += self.yield_value(district.yields, strategy) * 18.0 + district.amenity * 18.0;
        }
        value += g
            .rules
            .governments
            .values()
            .filter(|gov| gov.civic.as_deref() == Some(civic))
            .map(|gov| {
                let slots = gov.slots.military
                    + gov.slots.economic
                    + gov.slots.diplomatic
                    + gov.slots.wildcard;
                45.0 + slots as f64 * 18.0
            })
            .sum::<f64>();
        value += g
            .rules
            .policies
            .values()
            .filter(|p| p.civic.as_deref() == Some(civic))
            .count() as f64
            * 13.0;
        if strategy == GrandStrategy::Expansion && matches!(civic, "early_empire" | "foreign_trade")
        {
            value += 45.0;
        }
        if strategy == GrandStrategy::Culture && civic == "drama_poetry" {
            value += 60.0;
        }
        if strategy == GrandStrategy::Diplomacy
            && matches!(civic, "political_philosophy" | "civil_service" | "guilds")
        {
            value += 60.0;
        }
        if strategy == GrandStrategy::Religion && civic == "theology" {
            value += 120.0;
        }
        value += match civic {
            "foreign_trade" | "craftsmanship" => 25.0,
            "early_empire" | "state_workforce" => 38.0,
            "political_philosophy" => 70.0,
            // Culture infrastructure is a prerequisite for every strategy,
            // not only a culture-victory plan.
            "drama_poetry" => 55.0,
            _ => 0.0,
        };
        (value + 32.0) / spec.cost.max(10.0).sqrt()
    }

    fn incoming_deal_value(
        &self,
        g: &Game,
        pid: usize,
        deal: &DiplomaticDeal,
        plan: &StrategicPlan,
    ) -> f64 {
        let partner = deal.from;
        let my_power = g.military_power(pid);
        let partner_power = g.military_power(partner);
        let grievance = g.players[pid]
            .grievances
            .get(&partner)
            .copied()
            .unwrap_or(0.0);
        let fatigued = self.major_war_since.is_some_and(|started| {
            g.turn.saturating_sub(started) >= 24
                && g.turn.saturating_sub(self.last_campaign_progress) >= 12
        });
        let denied_partner = plan.target_player == Some(partner)
            && (plan.strategy == GrandStrategy::Conquest
                || g.is_at_war(pid, partner)
                || self.rival_victory_pressure(g, partner).progress >= 78);

        let mut value = deal.give_gold - deal.request_gold;
        if deal.peace {
            value += if my_power < partner_power * 0.85 || fatigued {
                320.0
            } else if denied_partner {
                // Recovery is a temporary battlefield posture, not an order
                // to abandon the campaign. A locally threatened city can put
                // an overwhelmingly stronger attacker into Recovery for one
                // assessment window; keep refusing its active target's white
                // peace until the army is actually outmatched or fatigued.
                -260.0
            } else if plan.strategy == GrandStrategy::Recovery {
                320.0
            } else {
                35.0
            };
        } else if denied_partner {
            return -1_000.0;
        }
        if deal.open_borders {
            value += match plan.strategy {
                GrandStrategy::Culture => 70.0,
                GrandStrategy::Conquest => 45.0,
                _ => 25.0,
            };
        }
        if deal.friendship {
            value += if plan.strategy == GrandStrategy::Diplomacy {
                80.0
            } else {
                40.0
            };
        }
        if let Some(alliance) = deal.alliance.as_deref() {
            value += match (plan.strategy, alliance) {
                (GrandStrategy::Science, "research")
                | (GrandStrategy::Culture, "cultural")
                | (GrandStrategy::Religion, "religious")
                | (GrandStrategy::Conquest | GrandStrategy::Recovery, "military")
                | (GrandStrategy::Expansion | GrandStrategy::Diplomacy, "economic") => 150.0,
                (GrandStrategy::Diplomacy, _) => 110.0,
                _ => 55.0,
            };
        }
        value - grievance * 0.8
    }

    fn strategic_bilateral_trade(
        &self,
        g: &mut Game,
        pid: usize,
        excluded_partner: Option<usize>,
        strategy: GrandStrategy,
    ) {
        let objective = self
            .victory_target
            .map(VictoryTarget::strategy)
            .unwrap_or(strategy);
        if objective == GrandStrategy::Culture && g.turn % 6 == pid as u32 % 6 {
            let best = g
                .quick_deals(pid)
                .into_iter()
                .filter(|deal| Some(deal.partner) != excluded_partner)
                .filter(|deal| {
                    deal.item == "open_borders"
                        && deal.direction == "buy"
                        && deal.my_value >= 2.0
                        && deal.partner_value >= 2.0
                })
                .max_by(|left, right| {
                    g.domestic_tourists(left.partner)
                        .cmp(&g.domestic_tourists(right.partner))
                        .then_with(|| {
                            left.my_value
                                .min(left.partner_value)
                                .partial_cmp(&right.my_value.min(right.partner_value))
                                .unwrap()
                        })
                        .then_with(|| right.partner.cmp(&left.partner))
                });
            if let Some(deal) = best {
                if g.apply(
                    pid,
                    &Action::Trade {
                        player: deal.partner,
                        offer: Box::new(deal.offer),
                        request: Box::new(deal.request),
                    },
                )
                .is_ok()
                {
                    return;
                }
            }

            let best = g
                .quick_deals(pid)
                .into_iter()
                .filter(|deal| Some(deal.partner) != excluded_partner)
                .filter(|deal| {
                    deal.category == "great_work"
                        && deal.direction == "buy"
                        && deal.my_value >= 2.0
                        && deal.partner_value >= 2.0
                })
                .max_by(|left, right| {
                    left.my_value
                        .min(left.partner_value)
                        .partial_cmp(&right.my_value.min(right.partner_value))
                        .unwrap()
                        .then_with(|| right.partner.cmp(&left.partner))
                        .then_with(|| right.item.cmp(&left.item))
                });
            if let Some(deal) = best {
                if g.apply(
                    pid,
                    &Action::Trade {
                        player: deal.partner,
                        offer: Box::new(deal.offer),
                        request: Box::new(deal.request),
                    },
                )
                .is_ok()
                {
                    return;
                }
            }

            // A Culture objective preserves its own Great Works. If neither
            // the strategically useful Open Borders direction nor a housed
            // purchase is available, it may still take the best ordinary
            // mutually beneficial quote.
            let best = g
                .quick_deals(pid)
                .into_iter()
                .filter(|deal| Some(deal.partner) != excluded_partner)
                .filter(|deal| {
                    !(deal.category == "great_work" && deal.direction == "sell")
                        && deal.my_value >= 2.0
                        && deal.partner_value >= 2.0
                })
                .max_by(|left, right| {
                    left.my_value
                        .min(left.partner_value)
                        .partial_cmp(&right.my_value.min(right.partner_value))
                        .unwrap()
                });
            if let Some(deal) = best {
                let _ = g.apply(
                    pid,
                    &Action::Trade {
                        player: deal.partner,
                        offer: Box::new(deal.offer),
                        request: Box::new(deal.request),
                    },
                );
            }
            return;
        }
        self.base
            .bilateral_trade_excluding(g, pid, excluded_partner);
    }

    fn propose_strategic_alliance(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
        denied_partner: Option<usize>,
    ) {
        if g.turn % 12 != pid as u32 % 12 || !g.players[pid].civics.contains(&crate::name!("civil_service")) {
            return;
        }
        let kind = match plan.strategy {
            GrandStrategy::Science => "research",
            GrandStrategy::Culture => "cultural",
            GrandStrategy::Religion => "religious",
            GrandStrategy::Conquest | GrandStrategy::Recovery => "military",
            GrandStrategy::Expansion | GrandStrategy::Diplomacy => "economic",
        };
        if kind == "research" && g.tree_effect(pid, "research_agreements") <= 0.0 {
            return;
        }
        if g.players[pid]
            .alliances
            .values()
            .any(|alliance| alliance.ends > g.turn && alliance.kind == kind)
        {
            return;
        }
        let pending_with = |partner: usize| {
            g.pending_deals.iter().any(|deal| {
                deal.expires >= g.turn
                    && ((deal.from == pid && deal.to == partner)
                        || (deal.from == partner && deal.to == pid))
            })
        };
        let partner = g
            .players
            .iter()
            .filter(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && Some(other.id) != denied_partner
                    && !g.is_at_war(pid, other.id)
                    && other.civics.contains(&crate::name!("civil_service"))
                    && g.alliance_with(pid, other.id).is_none()
                    && !pending_with(other.id)
                    && (kind != "research" || g.tree_effect(other.id, "research_agreements") > 0.0)
                    && !other
                        .alliances
                        .values()
                        .any(|alliance| alliance.ends > g.turn && alliance.kind == kind)
                    && g.players[pid]
                        .grievances
                        .get(&other.id)
                        .copied()
                        .unwrap_or(0.0)
                        < 75.0
                    && self.rival_victory_pressure(g, other.id).progress < 82
            })
            .max_by(|left, right| {
                let score = |other: usize| {
                    let friendship = if g.are_friends(pid, other) {
                        180.0
                    } else {
                        0.0
                    };
                    let connected = if g.routes.iter().any(|route| {
                        route.ends > g.turn
                            && ((route.owner == pid
                                && g.cities
                                    .get(&route.dest)
                                    .is_some_and(|destination| destination.owner == other))
                                || (route.owner == other
                                    && g.cities
                                        .get(&route.dest)
                                        .is_some_and(|destination| destination.owner == pid)))
                    }) {
                        70.0
                    } else {
                        0.0
                    };
                    let complement = match kind {
                        "research" => {
                            g.players[other]
                                .techs
                                .difference(&g.players[pid].techs)
                                .count() as f64
                                * 4.0
                        }
                        "cultural" => g.tourism_per_turn(other).min(300.0) * 0.15,
                        "economic" => {
                            g.players
                                .iter()
                                .filter(|minor| minor.alive && minor.is_minor)
                                .filter(|minor| g.suzerain_of(minor.id) == Some(other))
                                .count() as f64
                                * 35.0
                        }
                        "military" => g.military_power(other).min(250.0) * 0.25,
                        "religious" => g.players[other].religion.is_some() as usize as f64 * 45.0,
                        _ => 0.0,
                    };
                    friendship + connected + complement
                        - g.players[pid]
                            .grievances
                            .get(&other)
                            .copied()
                            .unwrap_or(0.0)
                };
                score(left.id)
                    .partial_cmp(&score(right.id))
                    .unwrap()
                    .then_with(|| right.id.cmp(&left.id))
            })
            .map(|other| other.id);
        if let Some(partner) = partner {
            let _ = g.apply(
                pid,
                &Action::ProposeDeal {
                    player: partner,
                    give_gold: 0.0,
                    request_gold: 0.0,
                    open_borders: g.players[pid].civics.contains(&crate::name!("early_empire")),
                    friendship: true,
                    peace: false,
                    alliance: Some(kind.to_string()),
                },
            );
        }
    }

    fn congress_choice(
        &self,
        g: &Game,
        pid: usize,
        resolution: &CongressResolution,
        strategy: GrandStrategy,
    ) -> Option<String> {
        if let Some(proposal) = g.emergency_proposal_for_resolution(&resolution.id) {
            if proposal.target == pid {
                return Some("B:oppose".to_string());
            }
            if !proposal.eligible.contains(&pid) {
                return None;
            }
            let grievance = g.players[pid]
                .grievances
                .get(&proposal.target)
                .copied()
                .unwrap_or(0.0);
            let threat = self.rival_victory_pressure(g, proposal.target).progress;
            let affordable_war =
                g.military_power(pid) * 1.75 + 20.0 >= g.military_power(proposal.target);
            let support = proposal.kind == "city_state"
                || strategy == GrandStrategy::Diplomacy
                || grievance >= 25.0
                || threat >= 55
                || affordable_war;
            return Some(if support { "A:support" } else { "B:oppose" }.to_string());
        }
        // Legacy saves encoded only a target. Preserve their old strategic
        // behavior while new sessions use explicit `A:target`/`B:target`
        // ballots.
        if resolution
            .choices
            .iter()
            .all(|choice| !choice.contains(':'))
        {
            let own = pid.to_string();
            return match resolution.id.as_str() {
                "world_leader" | "international_aid" if strategy == GrandStrategy::Diplomacy => {
                    resolution
                        .choices
                        .iter()
                        .find(|choice| **choice == own)
                        .cloned()
                }
                "world_leader" | "international_aid" => resolution
                    .choices
                    .iter()
                    .filter_map(|choice| {
                        choice.parse::<usize>().ok().map(|target| (choice, target))
                    })
                    .min_by_key(|(_, target)| (g.players[*target].dvp, *target))
                    .map(|(choice, _)| choice.clone()),
                "world_fair" if strategy == GrandStrategy::Culture => resolution
                    .choices
                    .iter()
                    .find(|choice| **choice == own)
                    .cloned(),
                "world_fair" => resolution
                    .choices
                    .iter()
                    .filter_map(|choice| {
                        choice.parse::<usize>().ok().map(|target| (choice, target))
                    })
                    .max_by(|left, right| {
                        g.players[left.1]
                            .culture_lifetime
                            .partial_cmp(&g.players[right.1].culture_lifetime)
                            .unwrap_or(std::cmp::Ordering::Equal)
                            .then_with(|| right.1.cmp(&left.1))
                    })
                    .map(|(choice, _)| choice.clone()),
                _ => resolution.choices.first().cloned(),
            };
        }

        let diplomatic_leader = g
            .players
            .iter()
            .filter(|player| player.alive && !player.is_minor && !player.is_barbarian)
            .max_by_key(|player| (player.dvp, std::cmp::Reverse(player.id)))
            .map(|player| player.id);
        // Who a targeted penalty is pointed at. `world_leader` keeps aiming at
        // the diplomatic leader because its +/-2 moves Diplomatic Victory
        // Points and nothing else; the resolutions that cost an empire real
        // yields aim at whoever is actually about to win. See
        // [`Self::congress_counter_leader`] for what the census measured.
        let denied = self
            .congress_counter_leader
            .then(|| self.victory_denial(g, pid).map(|(rival, _)| rival))
            .flatten();
        let counter_target = denied.or(diplomatic_leader);
        let preferred_district = match strategy {
            GrandStrategy::Science => "campus",
            GrandStrategy::Culture => "theater_square",
            GrandStrategy::Religion => "holy_site",
            GrandStrategy::Conquest | GrandStrategy::Recovery => "encampment",
            GrandStrategy::Diplomacy => "diplomatic_quarter",
            GrandStrategy::Expansion => "commercial_hub",
        };
        let preferred_person = match strategy {
            GrandStrategy::Science => "scientist",
            GrandStrategy::Culture => "artist",
            GrandStrategy::Religion => "prophet",
            GrandStrategy::Conquest | GrandStrategy::Recovery => "general",
            GrandStrategy::Diplomacy | GrandStrategy::Expansion => "merchant",
        };
        let preferred_work = match strategy {
            GrandStrategy::Culture => "art",
            GrandStrategy::Religion => "relic",
            _ => "writing",
        };
        let observed = |choice: &str| {
            resolution
                .ballots
                .values()
                .filter(|(cast, _)| cast == choice)
                .map(|(_, votes)| *votes as f64)
                .sum::<f64>()
        };

        resolution.choices.iter().cloned().max_by(|left, right| {
            let score = |choice: &str| {
                let (outcome, target) = Game::congress_choice_parts(choice);
                let target_player = target.parse::<usize>().ok();
                let base = match resolution.id.as_str() {
                    "world_leader" => match (outcome, target_player) {
                        ("A", Some(target))
                            if target == pid && strategy == GrandStrategy::Diplomacy =>
                        {
                            1_000.0
                        }
                        ("B", Some(target))
                            if Some(target) == diplomatic_leader && target != pid =>
                        {
                            900.0
                        }
                        ("A", Some(target)) => 100.0 - 12.0 * g.players[target].dvp as f64,
                        ("B", Some(target)) => 20.0 + 18.0 * g.players[target].dvp as f64,
                        _ => 0.0,
                    },
                    "mercenary_companies" => match (outcome, target) {
                        ("B", "production") => 340.0,
                        ("B", "gold") => 180.0,
                        ("B", "faith") if strategy == GrandStrategy::Religion => 230.0,
                        ("B", "faith") => 90.0,
                        ("A", _) => -120.0,
                        _ => 0.0,
                    },
                    "luxury_policy" => {
                        let own = g.resource_access_count(pid, target) as f64;
                        let rival = g
                            .players
                            .iter()
                            .filter(|player| player.id != pid)
                            .map(|player| g.resource_access_count(player.id, target))
                            .max()
                            .unwrap_or(0) as f64;
                        if outcome == "A" {
                            own * 75.0
                        } else {
                            rival * 55.0 - own * 85.0
                        }
                    }
                    "trade_policy" => match target_player {
                        // Outcome B embargoes every inter-civ trade route the
                        // target has. Against an empire about to win that is
                        // worth more than a trade slot of our own, which is all
                        // outcome A on ourselves buys.
                        Some(target)
                            if outcome == "B"
                                && Some(target) == denied
                                && target != pid =>
                        {
                            420.0
                        }
                        Some(target) if outcome == "A" && target == pid => 260.0,
                        Some(target)
                            if outcome == "B"
                                && Some(target) == counter_target
                                && target != pid =>
                        {
                            150.0
                        }
                        Some(target)
                            if outcome == "A" && g.alliance_with(pid, target).is_some() =>
                        {
                            120.0
                        }
                        _ => 10.0,
                    },
                    "world_religion" => {
                        let mine = g.players[pid].religion.as_deref() == Some(target);
                        if outcome == "A" && mine {
                            320.0
                        } else if outcome == "B" && !mine {
                            150.0
                        } else {
                            0.0
                        }
                    }
                    "urban_development_treaty" => {
                        if outcome == "A" && target == preferred_district {
                            280.0
                        } else if outcome == "A" {
                            80.0
                        } else {
                            -80.0
                        }
                    }
                    "patronage" => {
                        if outcome == "A" && target == preferred_person {
                            280.0
                        } else if outcome == "A" {
                            70.0
                        } else {
                            -100.0
                        }
                    }
                    "military_advisory" => {
                        let own = g
                            .units
                            .values()
                            .filter(|unit| {
                                unit.owner == pid
                                    && g.rules.units[unit.kind].promotion_class == target
                            })
                            .count() as f64;
                        let rival = g
                            .units
                            .values()
                            .filter(|unit| {
                                unit.owner != pid
                                    && g.rules.units[unit.kind].promotion_class == target
                            })
                            .count() as f64;
                        if outcome == "A" {
                            own * 45.0 - rival * 10.0
                        } else {
                            rival * 35.0 - own * 50.0
                        }
                    }
                    "migration_treaty" => match (outcome, target_player) {
                        // Outcome B costs its target 20% growth and pushes its
                        // loyalty. Shipped, it scores 0.0 against every rival,
                        // so the penalty can never be aimed at anybody.
                        ("B", Some(target)) if Some(target) == denied && target != pid => 300.0,
                        ("A", Some(target))
                            if target == pid && strategy == GrandStrategy::Expansion =>
                        {
                            220.0
                        }
                        ("B", Some(target)) if target == pid => 140.0,
                        ("A", Some(target)) if target != pid => 35.0,
                        _ => 0.0,
                    },
                    "public_relations" => match (outcome, target_player) {
                        ("B", Some(target)) if target == pid => 230.0,
                        ("A", Some(target))
                            if Some(target) == counter_target && target != pid =>
                        {
                            150.0
                        }
                        _ => 0.0,
                    },
                    "heritage_organization" => {
                        if outcome == "A" && target == preferred_work {
                            300.0
                        } else if outcome == "A" {
                            90.0
                        } else {
                            -120.0
                        }
                    }
                    "arms_control" => match target_player {
                        Some(target) => {
                            let inventory = |player: usize| {
                                [
                                    "project_effect:nuclear_devices",
                                    "project_effect:thermonuclear_devices",
                                ]
                                .into_iter()
                                .map(|key| {
                                    g.players[player]
                                        .counters
                                        .get(key)
                                        .copied()
                                        .unwrap_or(0)
                                        .max(0)
                                })
                                .sum::<i64>() as f64
                            };
                            let mine = inventory(pid);
                            let theirs = inventory(target);
                            let major_inventories: Vec<f64> = g
                                .players
                                .iter()
                                .filter(|player| {
                                    player.alive && !player.is_minor && !player.is_barbarian
                                })
                                .map(|player| inventory(player.id))
                                .collect();
                            let world_total = major_inventories.iter().sum::<f64>();
                            let equalized_total =
                                theirs * major_inventories.len().max(1) as f64;
                            let disarmament = world_total - equalized_total;
                            match outcome {
                                // Outcome A copies the target's stockpile to every
                                // major. Peaceful strategies therefore nominate the
                                // smallest arsenal, not the nuclear leader.
                                "A" if strategy != GrandStrategy::Conquest => {
                                    180.0 + 55.0 * disarmament
                                        - 75.0 * (-disarmament).max(0.0)
                                }
                                "A" => 20.0 + 35.0 * (theirs - mine)
                                    - 45.0 * (equalized_total - world_total).max(0.0),
                                "B" if target != pid => {
                                    let aggression = if matches!(
                                        strategy,
                                        GrandStrategy::Conquest | GrandStrategy::Recovery
                                    ) {
                                        100.0
                                    } else {
                                        55.0
                                    };
                                    90.0 + aggression * theirs
                                }
                                "B" => -500.0,
                                _ => 0.0,
                            }
                        }
                        None => 0.0,
                    },
                    "world_ideology" => {
                        let mine = g.players[pid].government.as_deref() == Some(target);
                        let rival_users = g
                            .players
                            .iter()
                            .filter(|player| {
                                player.id != pid
                                    && player.alive
                                    && !player.is_minor
                                    && !player.is_barbarian
                                    && player.government.as_deref() == Some(target)
                            })
                            .count() as f64;
                        match outcome {
                            "A" if mine => 320.0,
                            "A" => 30.0,
                            "B" if mine => -260.0,
                            "B" => 90.0 + 70.0 * rival_users,
                            _ => 0.0,
                        }
                    }
                    "border_control_treaty" => match (outcome, target_player) {
                        // Outcome B stops the target annexing tiles by border
                        // growth. Shipped, it is aimed by raw territory -- the
                        // one leader-targeting term in this table that already
                        // uses something other than Diplomatic Victory Points,
                        // and still not the empire about to win.
                        ("B", Some(target)) if Some(target) == denied && target != pid => 400.0,
                        ("A", Some(target)) if target == pid => 300.0,
                        ("B", Some(target)) if target == pid => -240.0,
                        ("B", Some(target)) => {
                            let territory = g
                                .player_city_ids(target)
                                .into_iter()
                                .map(|city| g.cities[&city].owned_tiles.len())
                                .sum::<usize>() as f64;
                            80.0 + territory
                        }
                        _ => 20.0,
                    },
                    "public_works_program" => {
                        let queued = |owner: usize| {
                            g.cities
                                .values()
                                .filter(|city| city.owner == owner)
                                .filter(|city| {
                                    city.queue.iter().any(|item| {
                                        matches!(item, Item::Project { project } if project == target)
                                    })
                                })
                                .count() as f64
                        };
                        let own_queued = queued(pid);
                        let rival_queued = g
                            .players
                            .iter()
                            .filter(|player| {
                                player.id != pid
                                    && player.alive
                                    && !player.is_minor
                                    && !player.is_barbarian
                            })
                            .map(|player| queued(player.id))
                            .sum::<f64>();
                        let aligned = match strategy {
                            GrandStrategy::Science => {
                                target.contains("launch_")
                                    || target.contains("laser_station")
                                    || target == "exoplanet_expedition"
                            }
                            GrandStrategy::Conquest | GrandStrategy::Recovery => {
                                target.contains("nuclear")
                                    || matches!(target, "manhattan_project" | "operation_ivy")
                            }
                            GrandStrategy::Diplomacy => target == "carbon_recapture",
                            _ => false,
                        };
                        match (outcome, aligned) {
                            ("A", true) => 300.0 + 160.0 * own_queued,
                            ("A", false) => 65.0 + 180.0 * own_queued,
                            ("B", true) => {
                                -220.0 + 140.0 * rival_queued - 180.0 * own_queued
                            }
                            ("B", false) => {
                                25.0 + 140.0 * rival_queued - 180.0 * own_queued
                            }
                            _ => 0.0,
                        }
                    }
                    "global_energy_treaty" => {
                        let queued = |owner: usize| {
                            g.cities
                                .values()
                                .filter(|city| city.owner == owner)
                                .filter(|city| {
                                    city.queue.iter().any(|item| {
                                        matches!(item, Item::Building { building } if building == target)
                                    })
                                })
                                .count() as f64
                        };
                        let own_queued = queued(pid);
                        let rival_queued = g
                            .players
                            .iter()
                            .filter(|player| {
                                player.id != pid
                                    && player.alive
                                    && !player.is_minor
                                    && !player.is_barbarian
                            })
                            .map(|player| queued(player.id))
                            .sum::<f64>();
                        let preferred = match strategy {
                            GrandStrategy::Science | GrandStrategy::Diplomacy => {
                                "nuclear_power_plant"
                            }
                            GrandStrategy::Conquest | GrandStrategy::Recovery => "coal_power_plant",
                            _ => "oil_power_plant",
                        };
                        match (outcome, target) {
                            ("A", candidate) if candidate == preferred => {
                                270.0 + 160.0 * own_queued
                            }
                            ("A", _) => 90.0 + 160.0 * own_queued,
                            ("B", "coal_power_plant") if strategy == GrandStrategy::Diplomacy => {
                                180.0 + 120.0 * rival_queued - 180.0 * own_queued
                            }
                            ("B", candidate) if candidate == preferred => {
                                -180.0 + 120.0 * rival_queued - 180.0 * own_queued
                            }
                            ("B", _) => 35.0 + 120.0 * rival_queued - 180.0 * own_queued,
                            _ => 0.0,
                        }
                    }
                    "deforestation_treaty" => {
                        let owned_copies = |owner: usize| {
                            g.cities
                                .values()
                                .filter(|city| city.owner == owner)
                                .flat_map(|city| city.owned_tiles.iter())
                                .filter(|position| {
                                    g.map.tiles[*position].feature.as_deref() == Some(target)
                                })
                                .count() as f64
                        };
                        let own_copies = owned_copies(pid);
                        let rival_copies = g
                            .players
                            .iter()
                            .filter(|player| {
                                player.id != pid
                                    && player.alive
                                    && !player.is_minor
                                    && !player.is_barbarian
                            })
                            .map(|player| owned_copies(player.id))
                            .sum::<f64>();
                        match outcome {
                            "A" => {
                                65.0
                                    + 35.0 * own_copies
                                    + if strategy == GrandStrategy::Expansion {
                                        90.0
                                    } else {
                                        0.0
                                    }
                            }
                            "B" if strategy == GrandStrategy::Culture && target == "forest" => {
                                165.0 + 5.0 * (own_copies + rival_copies)
                            }
                            "B" => 20.0 + 5.0 * rival_copies - 20.0 * own_copies,
                            _ => 0.0,
                        }
                    }
                    _ => 0.0,
                };
                base + observed(choice) * 35.0
            };
            score(left)
                .partial_cmp(&score(right))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| right.cmp(left))
        })
    }

    /// Prefer an available low-Grievance casus belli. If none is ready, a
    /// major rival is denounced and the campaign waits for Formal War rather
    /// than opening with a Surprise War. The sole exception is a rival already
    /// on the brink of victory, where five setup turns can lose the game.
    /// City-states cannot be denounced and therefore remain direct targets.
    fn preferred_war_opening(&self, g: &Game, pid: usize, target: usize) -> Option<Action> {
        let legal = g.legal_actions_within(pid, ActionFamilies::CORE);
        let casus_belli = legal
            .iter()
            .filter_map(|action| match action {
                Action::DeclareWarWithCasusBelli {
                    player,
                    casus_belli,
                } if *player == target => {
                    let grievance_cost = if casus_belli == "formal_war" { 100 } else { 50 };
                    Some((grievance_cost, casus_belli, action))
                }
                _ => None,
            })
            .min_by_key(|(cost, name, _)| (*cost, *name))
            .map(|(_, _, action)| action.clone());
        if casus_belli.is_some() {
            return casus_belli;
        }

        let surprise = legal.iter().find_map(|action| match action {
            Action::DeclareWar { player } if *player == target => Some(action.clone()),
            _ => None,
        });
        if g.players[target].is_minor {
            return surprise;
        }

        // A final exoplanet launch and a religious match point are both
        // irreversible victory clocks. They already interrupt strategic
        // planning before 90%, so waiting five turns for a Formal War here
        // would make the counter-campaign start after the game can end.
        let urgent = self.urgent_victory_threat(g, target);
        let denounced = g.players[pid]
            .denounced_until
            .get(&target)
            .is_some_and(|until| *until > g.turn);
        if !urgent && !denounced {
            return legal.iter().find_map(|action| match action {
                Action::Denounce { player } if *player == target => Some(action.clone()),
                _ => None,
            });
        }
        if urgent {
            surprise
        } else {
            // The denouncement is active but its five-turn preparation period
            // has not elapsed, so preserve the army and wait for Formal War.
            None
        }
    }

    /// A peacetime tile from which a ground force can begin the selected
    /// campaign without trespassing through the target's borders. Keeping the
    /// ring several tiles outside the city leaves room for different combat
    /// roles to assemble while keeping the army close enough to exploit the
    /// opening turns of the war.
    fn campaign_staging_position(
        &self,
        g: &Game,
        pid: usize,
        target: usize,
        uid: u32,
        objective: Pos,
        position: Pos,
    ) -> bool {
        let Some(tile) = g.map.get(position) else {
            return false;
        };
        let distance = g.wdist(position, objective);
        if !(3..=5).contains(&distance)
            || g.rules.is_water(tile)
            || g.city_at(position).is_some()
            || !g.unit_can_traverse(uid, position)
        {
            return false;
        }
        let territory = tile
            .owner_city
            .and_then(|city| g.cities.get(&city))
            .map(|city| city.owner);
        territory != Some(target)
            && territory.is_none_or(|owner| {
                owner == pid || g.has_open_borders(pid, owner)
            })
    }

    fn staged_campaign_units(
        &self,
        g: &Game,
        pid: usize,
        target: usize,
        objective: Pos,
    ) -> Vec<u32> {
        g.player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let unit = &g.units[uid];
                let spec = &g.rules.units[unit.kind];
                spec.class == "military"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && (spec.is_melee_capable() || spec.has_ranged_attack())
                    && unit.hp as f64 > self.base.w.withdraw_hp
                    && self.campaign_staging_position(
                        g,
                        pid,
                        target,
                        *uid,
                        objective,
                        unit.pos,
                    )
            })
            .collect()
    }

    /// Freeze the rivals the connected-rush treatment may ever target.
    ///
    /// Waiting until every living major has founded its capital prevents seat
    /// order from deciding which rivals exist. Returning whether the freeze
    /// happened lets the caller reassess immediately: the same turn is the
    /// treatment's first opportunity to differ from ordinary `advanced`.
    fn freeze_rush_route_targets(&mut self, g: &Game, pid: usize) -> bool {
        if !self.early_rush || !self.route_connected_rush || self.rush_route_targets.is_some() {
            return false;
        }

        let Some(majors) = g
            .players
            .iter()
            .filter(|player| player.alive && !player.is_minor && !player.is_barbarian)
            .map(|player| {
                g.player_city_ids(player.id)
                    .into_iter()
                    .find(|city| g.cities[city].is_capital)
                    .map(|city| (player.id, g.cities[&city].pos))
            })
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };

        let land_melee: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|unit| {
                let spec = &g.rules.units[g.units[unit].kind];
                spec.class == "military"
                    && spec.is_melee_capable()
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
            })
            .collect();
        let reachable = majors
            .into_iter()
            .filter(|(target, _)| *target != pid)
            .filter(|(_, capital)| {
                land_melee.iter().any(|unit| {
                    g.wdist(g.units[unit].pos, *capital) <= RUSH_STAGING_RANGE
                        || g.route_step(*unit, *capital, RUSH_STAGING_RANGE).is_some()
                })
            })
            .map(|(target, _)| target)
            .collect();
        self.rush_route_targets = Some(reachable);
        true
    }

    /// Global power answers whether a war is affordable; this answers whether
    /// the army is actually in position to prosecute it. At least one melee
    /// unit is mandatory because ranged and siege units cannot capture a city.
    /// The neighbour an ancient rush should open on, and their capital.
    ///
    /// Returns `None` unless every measured precondition holds, because the
    /// whole value of the lane is that it plays a window rather than a
    /// preference. See `early_rush` for where each number comes from.
    fn early_rush_victim(&self, g: &Game, pid: usize) -> Option<(usize, u32)> {
        if !self.early_rush {
            return None;
        }
        // A war already running is not re-opened every turn, and dropping the
        // lane at `RUSH_WINDOW_CLOSES` mid-siege abandons the campaign with
        // the stack on the ring: melee adjacent to the objective fell to zero
        // at turn 80 for exactly this reason. The window governs **opening** a
        // rush; finishing one is governed by the war.
        let already_committed = g.players.iter().any(|player| {
            player.id != pid
                && player.alive
                && !player.is_minor
                && !player.is_barbarian
                && g.is_at_war(pid, player.id)
        });
        if g.turn >= RUSH_WINDOW_CLOSES && !already_committed {
            return None;
        }
        let mine: Vec<Pos> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .collect();
        if mine.is_empty() {
            return None;
        }
        let my_power = g.military_power(pid);
        g.players
            .iter()
            .filter(|player| {
                player.id != pid && player.alive && !player.is_minor && !player.is_barbarian
            })
            .filter(|player| {
                !self.route_connected_rush
                    || self
                        .rush_route_targets
                        .as_ref()
                        .is_some_and(|targets| targets.contains(&player.id))
            })
            .filter(|player| self.campaign_target_legal(g, pid, player.id))
            // Not stronger than us. The stack is what makes the rush work, so
            // this is a floor against opening on somebody who can answer it,
            // not the 1.32-plus-12 superiority `advanced` waits for.
            //
            // It is a test for *opening* a war, not for continuing one. Once
            // the war is running, losing this test would switch the whole lane
            // off mid-campaign — dropping the army floor and handing the
            // column's objective back to the empire-global `threatened_city` —
            // which is the campaign abandoning itself at exactly the moment
            // the victim starts fighting back.
            // ⚠ Tightened to `* 0.85` — "genuinely weaker, not merely
            // comparable" — and measured clearly worse: early wars fell 33 to
            // 14, the median declaration slipped turn 32 to 47, and kills
            // slipped 47 to 70. Early empires are *all* near-parity because
            // they all field one or two units, so a superiority test does not
            // select weak victims, it just postpones the rush out of its own
            // window. What makes the rush work is the staged stack against an
            // unwalled capital, which is what the readiness gate checks.
            .filter(|player| {
                g.is_at_war(pid, player.id)
                    || g.military_power(player.id) <= my_power * 1.15 + 5.0
            })
            .filter_map(|player| {
                // The capital while they have one, otherwise whatever is
                // left. Wiping a neighbour means taking *every* city, and a
                // rush that stops the moment the palace falls leaves a
                // one-city rump alive — which is the difference between a
                // capture and an elimination.
                let capital = g
                    .player_city_ids(player.id)
                    .into_iter()
                    .find(|cid| g.cities[cid].is_capital)
                    .or_else(|| {
                        g.player_city_ids(player.id)
                            .into_iter()
                            .min_by_key(|cid| {
                                let pos = g.cities[cid].pos;
                                (mine.iter().map(|own| g.wdist(*own, pos)).min().unwrap_or(i32::MAX), *cid)
                            })
                    })?;
                let city = &g.cities[&capital];
                // The window is defined by the walls, so test the walls rather
                // than trusting the turn number. `rush_census` reports 0%
                // walled capitals through turn 60, but a modded ruleset, a
                // faster speed, or a defender who reacted would all show up
                // here and close the lane honestly.
                if city
                    .buildings
                    .iter()
                    .any(|b| g.rules.buildings[b].outer_defense > 0)
                {
                    return None;
                }
                let reach = mine.iter().map(|pos| g.wdist(*pos, city.pos)).min()?;
                (reach <= RUSH_REACH).then_some((player.id, capital, reach))
            })
            // Nearest first: the march is the binding cost, not the siege.
            // Break ties on the weaker army, then on id so the choice is
            // deterministic across a mirrored pair.
            .min_by(|a, b| {
                a.2.cmp(&b.2)
                    .then(
                        g.military_power(a.0)
                            .total_cmp(&g.military_power(b.0)),
                    )
                    .then(a.0.cmp(&b.0))
            })
            .map(|(target, capital, _)| (target, capital))
    }

    /// Whether the stack standing off the victim's capital is the size the
    /// engine's own combat math says takes it.
    ///
    /// A Monte Carlo over `damage`, `effective_strength` and `city_strength`
    /// against the measured turn-50 capital (strength 17.2, mean garrison 0.7,
    /// no walls) puts two melee units at 100% and one at 0%. Four is the
    /// figure that still reads 100% when the defender pulls its entire field
    /// army home — the case this lane cannot rule out, since it declares
    /// three tiles from the victim's capital.
    fn early_rush_stack_ready(&self, g: &Game, pid: usize, target: usize, cid: u32) -> bool {
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        let objective = city.pos;
        let units = self.staged_campaign_units(g, pid, target, objective);
        let takers: Vec<u32> = units
            .into_iter()
            .filter(|uid| g.rules.units[g.units[uid].kind].is_melee_capable())
            .collect();
        if takers.len() < RUSH_STACK {
            return false;
        }
        // Counting takers is not the same as being able to finish, and the
        // difference cost this lane twenty turns. War was declared at turn 30
        // on two warriors and the first city did not fall until turn 54: a
        // warrior against the measured capital takes 28 damage a blow and dies
        // on its fourth, having dealt 134 of the 200 needed. The stack traded
        // itself and the next one had to be built and marched all over again.
        //
        // So ask the engine's own combat math whether this force can take the
        // city *before it dies*, using the same `damage` curve the fight will
        // use. `30 * exp((att - def) / 25)` per blow each way, `100 / incoming`
        // blows survived.
        let defense = g.city_strength(cid);
        let deliverable: f64 = takers
            .iter()
            .filter_map(|uid| g.units.get(uid))
            .map(|unit| {
                let attack = crate::game::effective_strength(
                    g.unit_strength(unit, false),
                    unit.hp,
                );
                let out = 30.0 * ((attack - defense) / 25.0).exp();
                let incoming = (30.0 * ((defense - attack) / 25.0).exp()).max(1.0);
                let blows = (unit.hp as f64 / incoming).floor().max(1.0);
                out * blows
            })
            .sum();
        // The city's pool, plus a turn of the 20 HP/turn it regenerates
        // whenever the ring is not fully sealed.
        deliverable >= city.hp as f64 + 20.0
    }

    fn campaign_staged_for_war(
        &self,
        g: &Game,
        pid: usize,
        target: usize,
        objective: Pos,
        committed_domination: bool,
    ) -> bool {
        let units = self.staged_campaign_units(g, pid, target, objective);
        let has_capturer = units.iter().any(|uid| {
            g.rules.units[g.units[uid].kind].is_melee_capable()
        });
        let ratio = self.local_strength_ratio(g, &units, &[target], objective);
        let formation_ready = units.len() >= 3 || (units.len() >= 2 && ratio >= 1.60);
        let minimum_ratio = if committed_domination { 0.90 } else { 1.05 };
        formation_ready && has_capturer && ratio + 1e-9 >= minimum_ratio
    }

    /// Drive one melee unit of an ancient rush directly at the objective
    /// capital, bypassing the force-group heuristics entirely.
    ///
    /// Four separate attempts to make those heuristics conduct a siege made
    /// things measurably worse (see the rejected notes on `focus_target` and
    /// on the `relieving`/`Muster` postures). They are tuned for a field
    /// campaign between comparable armies, and a rush is not that: it is four
    /// melee units against an unwalled capital holding a garrison of 0.7,
    /// inside a window that shuts.
    ///
    /// The routine is the Monte Carlo's own recipe, in order:
    ///
    /// 1. **Walk into a depleted city.** An ordinary ranged attack cannot take
    ///    a city below 1 HP, so a city at 0 was opened by a Bombard and is
    ///    standing open for whoever steps in.
    /// 2. **Attack from the ring** if already on it.
    /// 3. **Take a free ring tile.** Every melee unit on the ring both adds a
    ///    blow and helps seal the siege, and an unsealed city heals 20 HP a
    ///    turn — which is what actually defeated this lane before: the stack
    ///    reached the 3-5 tile staging ring at full strength (measured max 4)
    ///    while the city's own ring never held more than two.
    ///
    /// Returns `None` when this unit is not part of a live rush, leaving the
    /// ordinary wartime behaviour untouched.
    fn rush_siege_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
    ) -> Option<bool> {
        if !plan.rush {
            return None;
        }
        let target = plan.target_player?;
        if !g.is_at_war(pid, target) {
            return None;
        }
        let cid = plan.target_city?;
        let city = g.cities.get(&cid).filter(|city| city.owner == target)?;
        let objective = city.pos;
        let depleted = city.hp <= 0;
        let spec = &g.rules.units[g.units.get(&uid)?.kind];
        if !spec.is_melee_capable() || matches!(spec.domain.as_deref(), Some("sea" | "air")) {
            return None;
        }
        let here = g.units[&uid].pos;

        if g.wdist(here, objective) <= 1 {
            if depleted && g.can_move(uid, objective) {
                // The city is open. Walking in *is* the capture.
                return Some(g.apply(pid, &Action::Move { unit: uid, to: objective }).is_ok());
            }
            let attacked = g
                .apply(
                    pid,
                    &Action::Attack {
                        unit: uid,
                        target: objective,
                    },
                )
                .is_ok();
            if attacked {
                return Some(true);
            }
            // Out of moves this turn: hold the ring rather than give it up.
            // A vacated ring tile is 20 HP a turn handed back.
            return Some(self.base.fortify_or_stop(g, pid, uid));
        }

        // Close on a free ring tile — but *which* ring tile decides whether
        // this is a siege or a queue.
        //
        // `district_under_siege` needs every passable neighbour of the city
        // occupied or covered by our zone of control, and a city that is not
        // besieged heals 20 HP a turn. A ZOC unit covers its own ring tile
        // plus both ring-neighbours, three of six, so **two units placed three
        // apart seal the ring and two units side by side do not**. Routing to
        // the nearest free tile bunches them, which is why an unsealed capital
        // took 23 turns to fall against a Monte Carlo estimate of three.
        //
        // So prefer the free tile furthest from the ones we already hold, and
        // only then the one we can reach.
        let held: Vec<Pos> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|other| *other != uid)
            .filter_map(|other| g.units.get(&other))
            .filter(|other| g.rules.units[other.kind].is_melee_capable())
            .map(|other| other.pos)
            .filter(|pos| g.wdist(*pos, objective) <= 1)
            .collect();
        let mut ring: Vec<Pos> = g
            .wdisk(objective, 1)
            .into_iter()
            .filter(|pos| {
                *pos != objective
                    && g.unit_can_traverse(uid, *pos)
                    && g.units_at(*pos).iter().all(|other| {
                        g.units.get(other).is_some_and(|other| other.owner != pid)
                    })
            })
            .collect();
        if ring.is_empty() {
            return None;
        }
        // Spread first, then nearness to us, then position for determinism.
        ring.sort_by_key(|pos| {
            let spread = held
                .iter()
                .map(|other| g.wdist(*pos, *other))
                .min()
                .unwrap_or(i32::MAX);
            (std::cmp::Reverse(spread), g.wdist(here, *pos), pos.0, pos.1)
        });
        // Walk the preference order: the best tile may be unroutable this
        // turn, and giving up on it would leave the unit standing still.
        for goal in ring.iter().take(3) {
            let goals: HashSet<Pos> = std::iter::once(*goal).collect();
            if let Some(next) = g
                .route_step_to_any(uid, &goals)
                .filter(|pos| g.can_move(uid, *pos))
            {
                return Some(g.apply(pid, &Action::Move { unit: uid, to: next }).is_ok());
            }
        }
        None
    }

    /// Redirect an otherwise idle field unit to the active conquest front.
    /// Returning `Some` means the campaign owns this unit's peacetime order,
    /// including holding a completed staging position; `None` leaves ordinary
    /// patrol, exploration, and naval-escort behavior unchanged.
    fn campaign_staging_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
    ) -> Option<bool> {
        if plan.strategy != GrandStrategy::Conquest {
            return None;
        }
        let target = plan.target_player?;
        let objective = plan
            .target_city
            .and_then(|city| g.cities.get(&city))
            .filter(|city| city.owner == target)
            .map(|city| city.pos)?;
        if !self.campaign_target_legal(g, pid, target) || g.is_at_war(pid, target) {
            return None;
        }
        let unit = &g.units[&uid];
        let spec = &g.rules.units[unit.kind];
        if !matches!(spec.class.as_str(), "military" | "support")
            || matches!(spec.domain.as_deref(), Some("sea" | "air"))
            || (BasicAi::unit_doctrine(g, uid) == UnitDoctrine::Recon
                && self.base.has_exploration_target(g, pid, uid))
        {
            return None;
        }
        if self.campaign_staging_position(g, pid, target, uid, objective, unit.pos) {
            return Some(self.base.fortify_or_stop(g, pid, uid));
        }

        let current = unit.pos;
        let goals: HashSet<Pos> = {
            let _memo = g.query_memo();
            g.wdisk(objective, 5)
                .into_iter()
                .filter(|position| {
                    self.campaign_staging_position(g, pid, target, uid, objective, *position)
                        && g.units_at(*position).is_empty()
                })
                .collect()
        };
        let Some(next) = g
            .route_step_to_any(uid, &goals)
            .filter(|position| g.can_move(uid, *position))
        else {
            return None;
        };
        // Do not use an Open Borders shortcut through the intended victim.
        // The next turn's route search will find a lawful way around it.
        let next_territory = g.map.tiles[&next]
            .owner_city
            .and_then(|city| g.cities.get(&city))
            .map(|city| city.owner);
        if next_territory == Some(target) {
            return Some(self.base.fortify_or_stop(g, pid, uid));
        }
        debug_assert_ne!(next, current);
        Some(g.apply(pid, &Action::Move { unit: uid, to: next }).is_ok())
    }

    fn advanced_diplomacy(&mut self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        crate::ai::choose_dedications(g, pid, self.base.w.dedication_choice);
        let incoming: Vec<u32> = g
            .pending_deals
            .iter()
            .filter(|deal| deal.to == pid && deal.expires >= g.turn)
            .map(|deal| deal.id)
            .collect();
        for deal_id in incoming {
            let Some((accept, peace, worth, from)) = g
                .pending_deals
                .iter()
                .find(|deal| deal.id == deal_id)
                .map(|deal| {
                    let worth = self.incoming_deal_value(g, pid, deal, plan);
                    (worth >= 0.0, deal.peace, worth, deal.from)
                })
            else {
                continue;
            };
            if self.journal().wants(crate::reasoning::Level::Decision) {
                let who = g
                    .players
                    .get(from)
                    .map(|player| player.civ.clone())
                    .unwrap_or_else(|| format!("player {from}"));
                let kind = if peace { "peace offer" } else { "deal" };
                think!(self.journal(), Diplomacy, Decision,
                       "{} {who}'s {kind}", if accept { "Accepting" } else { "Refusing" };
                       "worth {worth:+.0} to the {} plan", plan.strategy.as_str());
            }
            let action = if accept {
                Action::AcceptDeal { deal: deal_id }
            } else {
                Action::RejectDeal { deal: deal_id }
            };
            if g.apply(pid, &action).is_ok() && accept && peace {
                // An accepted peace offer is the negotiated equivalent of
                // MakePeace: remember the stand-down so this AI does not
                // redeclare as soon as the mandatory treaty expires.
                self.peace_until = g.turn.saturating_add(30);
                self.major_war_since = None;
            }
        }
        if let Some(session) = g.congress.clone() {
            for resolution in session.resolutions {
                if resolution.ballots.contains_key(&pid) {
                    continue;
                }
                // In an explicit victory evaluation every major shares the
                // same objective. Civ VI awards a Diplomatic Victory Point for
                // predicting any winning resolution, including International
                // Aid, so repeated participation can end a healthy science or
                // culture race with the wrong victory. Explicit non-diplomatic
                // targets abstain; adaptive agents still participate normally.
                if self.victory_target.is_some()
                    && self.victory_target != Some(VictoryTarget::Diplomacy)
                    && g.emergency_proposal_for_resolution(&resolution.id)
                        .is_none()
                {
                    continue;
                }
                if let Some(choice) = self.congress_choice(g, pid, &resolution, plan.strategy) {
                    // A ballot aimed at the empire closest to a victory is
                    // backed with everything the treasury can spare, because a
                    // losing vote is refunded in full and a right-outcome,
                    // wrong-target one at half -- an opposition that fails
                    // costs nothing. Shipped, weight keys off the voter's own
                    // plan and never off the stakes.
                    let counters_the_leader = self.congress_counter_votes
                        && self.victory_denial(g, pid).is_some_and(|(rival, _)| {
                            let (outcome, target) = Game::congress_choice_parts(&choice);
                            // Naming the rival is not opposing it: outcome A on
                            // most of these table entries is the ballot that
                            // *helps* its target. Only the shapes that carry a
                            // penalty are worth paying for.
                            target == rival.to_string()
                                && match resolution.id.as_str() {
                                    "public_relations" => outcome == "A",
                                    _ => outcome == "B",
                                }
                        });
                    let votes = if (plan.strategy == GrandStrategy::Diplomacy
                        || counters_the_leader)
                        && g.players[pid].diplomatic_favor >= 30.0
                    {
                        3
                    } else {
                        1
                    };
                    think!(self.journal(), Diplomacy, Decision,
                           "Voting {} on {}", plain(&choice), plain(&resolution.id);
                           "{votes} vote{} behind it, on the {} plan",
                           if votes == 1 { "" } else { "s" }, plan.strategy.as_str());
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
        }
        let denied_partner = plan.target_player.filter(|target| {
            plan.strategy == GrandStrategy::Conquest
                || g.is_at_war(pid, *target)
                || self.rival_victory_pressure(g, *target).progress >= 78
        });
        self.strategic_bilateral_trade(g, pid, denied_partner, plan.strategy);
        self.propose_strategic_alliance(g, pid, plan, denied_partner);
        let my_power = g.military_power(pid);
        let rivals: Vec<usize> = g
            .players
            .iter()
            .filter(|p| p.id != pid && p.alive && !p.is_barbarian)
            .map(|p| p.id)
            .collect();
        for other in &rivals {
            let fatigued = self.major_war_since.is_some_and(|started| {
                g.turn.saturating_sub(started) >= 24
                    && g.turn.saturating_sub(self.last_campaign_progress) >= 12
            });
            let peace_pending = g.pending_deals.iter().any(|deal| {
                deal.peace
                    && ((deal.from == pid && deal.to == *other)
                        || (deal.from == *other && deal.to == pid))
                    && deal.expires >= g.turn
            });
            if g.is_at_war(pid, *other)
                && !g.emergency_war_pair(pid, *other)
                && !g.players[*other].is_minor
                && !peace_pending
                && (my_power < g.military_power(*other) * 0.62
                    || (plan.strategy == GrandStrategy::Recovery
                        && plan.target_player != Some(*other))
                    || (fatigued && g.player_city_ids(*other).len() > 1))
            {
                if self.journal().wants(crate::reasoning::Level::Decision) {
                    let their_power = g.military_power(*other);
                    let because = if my_power < their_power * 0.62 {
                        "outmatched"
                    } else if plan.strategy == GrandStrategy::Recovery {
                        "this is not the war the recovery plan is fighting"
                    } else {
                        "the war has stalled"
                    };
                    think!(self.journal(), Diplomacy, Decision,
                           "Offering peace to {}", g.players[*other].civ;
                           "{because}: {my_power:.0} power against their {their_power:.0}");
                }
                // Peace between majors is bilateral. The former direct
                // MakePeace let an outmatched defender terminate a winning
                // invasion on the first legal turn, even when the conqueror
                // valued the campaign. Keep fighting until the recipient's
                // normal deal valuation accepts this offer.
                let _ = g.apply(
                    pid,
                    &Action::ProposeDeal {
                        player: *other,
                        give_gold: 0.0,
                        request_gold: 0.0,
                        open_borders: false,
                        friendship: false,
                        peace: true,
                        alliance: None,
                    },
                );
            }
        }
        let major_wars = rivals
            .iter()
            .filter(|o| !g.players[**o].is_minor && g.is_at_war(pid, **o))
            .count();
        if major_wars > 0
            && matches!(
                plan.strategy,
                GrandStrategy::Conquest | GrandStrategy::Recovery
            )
        {
            self.base.levy_city_state_military(g, pid, true);
        }
        let Some(target) = plan.target_player else {
            return;
        };
        let emergency_target = g
            .emergency_objective(pid)
            .is_some_and(|objective| objective.target == target);
        // An ancient rush is the same decision taken earlier and on smaller
        // numbers, so it waives the two gates that are calendar rather than
        // condition — the turn-35 floor and the second city — and keeps every
        // gate that is about the war itself. It is still subject to
        // `close_enough`, to a staged stack, and to the peace deadline.
        let rushing = self
            .early_rush_victim(g, pid)
            .is_some_and(|(victim, _)| victim == target);
        if plan.strategy != GrandStrategy::Conquest
            || major_wars > 0
            || (!rushing && g.turn < 35)
            || g.turn < self.peace_until
            || (!rushing && g.player_city_ids(pid).len() < 2)
            || g.is_at_war(pid, target)
            || (!emergency_target && !self.campaign_target_legal(g, pid, target))
        {
            return;
        }
        let target_power = g.military_power(target);
        let close_enough = plan
            .target_city
            .and_then(|cid| g.cities.get(&cid))
            .is_some_and(|target_city| {
                g.player_city_ids(pid)
                    .iter()
                    .any(|cid| g.wdist(g.cities[cid].pos, target_city.pos) <= 18)
            });
        let committed_domination = self.victory_target == Some(VictoryTarget::Domination);
        // An army that has reached the enemy border is the only practical
        // answer to a rival's terminal clock.  Keep the normal power margin
        // for elective wars, but do not discard a staged denial force merely
        // because it is outnumbered: when the threat is already at the same
        // threshold that authorizes a Surprise War, waiting for superiority
        // guarantees that the rival gets the final uncontested turns.
        let urgent_denial = self.urgent_victory_threat(g, target);
        // `my_power > target_power * 1.32 + 12` is an empire-wide comparison,
        // and at turn 40 both empires are three or four units, so the `+ 12`
        // alone can outweigh the whole ratio. What decides an ancient siege is
        // not the empires' totals but how many takers are standing at the
        // objective, which is exactly what `early_rush_stack_ready` counts.
        let ready = urgent_denial
            || if rushing {
                plan.target_city
                    .and_then(|city| g.cities.get(&city))
                    .is_some_and(|city| self.early_rush_stack_ready(g, pid, target, city.id))
            } else if committed_domination {
                my_power >= target_power * 0.85 && my_power >= 30.0
            } else {
                my_power > target_power * 1.32 + 12.0
            };
        let staged = plan
            .target_city
            .and_then(|city| g.cities.get(&city))
            .is_some_and(|city| {
                self.campaign_staged_for_war(
                    g,
                    pid,
                    target,
                    city.pos,
                    // A staged rush stack is already at the objective and has
                    // counted its own takers, so hold it to the domination
                    // ratio rather than the elective-war one.
                    committed_domination || rushing,
                )
            });
        if close_enough && ready && staged {
            if let Some(action) = self.preferred_war_opening(g, pid, target) {
                if self.journal().wants(crate::reasoning::Level::Strategy) {
                    let casus = match &action {
                        Action::DeclareWarWithCasusBelli { casus_belli, .. } => {
                            format!(" under a {} casus belli", plain(casus_belli))
                        }
                        _ => String::new(),
                    };
                    think!(self.journal(), Military, Strategy,
                           "Declaring war on {}", g.players[target].civ;
                           "{my_power:.0} power against their {target_power:.0}, the army is \
                            staged within reach of the first objective{casus}{}",
                           if urgent_denial {
                               ", and they are close enough to winning that waiting loses it"
                           } else {
                               ""
                           });
                }
                let _ = g.apply(pid, &action);
            }
        } else if self.journal().wants(crate::reasoning::Level::Detail) {
            // Not opening a war is a decision too, and the observer cannot see
            // it any other way: an army that sits still for thirty turns looks
            // identical to one with no plan.
            let blocker = if !close_enough {
                "no city of theirs is within 18 tiles of one of mine"
            } else if !ready {
                "the army is not strong enough yet"
            } else {
                "the army has not finished staging"
            };
            think!(self.journal(), Military, Detail,
                   "Holding off war with {}", g.players[target].civ;
                   "{blocker}; {my_power:.0} power against their {target_power:.0}");
        }
    }

    fn advanced_envoys(
        &self,
        g: &mut Game,
        pid: usize,
        strategy: GrandStrategy,
        denied_rival: Option<usize>,
    ) {
        while g.players[pid].envoys_free > 0 {
            let target = g
                .players
                .iter()
                .filter(|minor| {
                    minor.alive
                        && minor.is_minor
                        && !minor.is_barbarian
                        && !g.is_at_war(pid, minor.id)
                })
                .map(|minor| {
                    let mine = g.envoys_at(pid, minor.id);
                    let rival = g
                        .players
                        .iter()
                        .filter(|p| !p.is_minor && !p.is_barbarian && p.id != pid)
                        .map(|p| g.envoys_at(p.id, minor.id))
                        .max()
                        .unwrap_or(0);
                    let needed = (3_i64.max(rival + 1) - mine).max(1);
                    let kind = g.cs_type(&minor.civ);
                    let alignment = match (strategy, kind) {
                        (GrandStrategy::Science, "scientific") => 10,
                        (GrandStrategy::Culture, "cultural") => 10,
                        (GrandStrategy::Religion, "religious") => 12,
                        (GrandStrategy::Diplomacy, _) => 10,
                        (GrandStrategy::Conquest, "militaristic") => 10,
                        (GrandStrategy::Expansion, "trade") => 8,
                        (_, "trade") => 4,
                        _ => 2,
                    };
                    let unique_alignment = match (strategy, minor.civ.as_str()) {
                        (GrandStrategy::Science, "Geneva") => 14,
                        (GrandStrategy::Science | GrandStrategy::Conquest, "Hattusa") => 11,
                        (GrandStrategy::Science | GrandStrategy::Culture, "Stockholm") => 10,
                        (GrandStrategy::Conquest, "Kabul") => 14,
                        (GrandStrategy::Conquest | GrandStrategy::Expansion, "Carthage") => 10,
                        (GrandStrategy::Expansion | GrandStrategy::Recovery, "Mohenjo-Daro") => 11,
                        (GrandStrategy::Religion, "Yerevan") => 15,
                        (GrandStrategy::Religion | GrandStrategy::Culture, "Kandy") => 12,
                        (GrandStrategy::Expansion | GrandStrategy::Recovery, "Zanzibar") => 11,
                        (_, "Zanzibar") if g.players[pid].civ == "Aztec" => 12,
                        (
                            GrandStrategy::Science
                            | GrandStrategy::Culture
                            | GrandStrategy::Conquest
                            | GrandStrategy::Expansion,
                            "Auckland",
                        ) => 9,
                        (
                            GrandStrategy::Religion
                            | GrandStrategy::Conquest
                            | GrandStrategy::Recovery,
                            "Valletta",
                        ) => 13,
                        (GrandStrategy::Culture, "Vilnius") => 14,
                        (_, "Stockholm" | "Zanzibar" | "Auckland" | "Valletta") => 5,
                        _ => 2,
                    };
                    let already_secure = g.suzerain_of(minor.id) == Some(pid) && mine > rival + 1;
                    let shared_from_partner = g.suzerain_of(minor.id).is_some_and(|leader| {
                        leader != pid
                            && g.alliance_with(pid, leader).is_some_and(|alliance| {
                                alliance.kind == "economic" && alliance.level >= 3
                            })
                    });
                    let type_bonus_value = g
                        .next_envoy_type_bonus(pid, minor.id)
                        .map(|(envoys, yields)| {
                            (self.yield_value(yields, strategy) * 14.0 / envoys as f64).round()
                                as i64
                        })
                        .unwrap_or(0);
                    let denial = denied_rival
                        .is_some_and(|leader| g.suzerain_of(minor.id) == Some(leader))
                        as i64
                        * 140;
                    let score = (alignment + unique_alignment) * 10 + type_bonus_value + denial
                        - needed * 7
                        - already_secure as i64 * 80
                        - shared_from_partner as i64 * 300;
                    (
                        score,
                        std::cmp::Reverse(needed),
                        std::cmp::Reverse(minor.id),
                        minor.id,
                    )
                })
                .max()
                .map(|(score, _, _, id)| (id, score));
            let Some((target, score)) = target else { break };
            if self.journal().wants(crate::reasoning::Level::Decision) {
                let mine = g.envoys_at(pid, target) + 1;
                let suzerain = g
                    .suzerain_of(target)
                    .and_then(|leader| g.players.get(leader))
                    .map(|leader| leader.civ.clone());
                let standing = match &suzerain {
                    Some(civ) if *civ == g.players[pid].civ => "already its Suzerain".to_string(),
                    Some(civ) => format!("{civ} holds it"),
                    None => "nobody is its Suzerain".to_string(),
                };
                think!(self.journal(), Diplomacy, Decision,
                       "Sending an envoy to {}", g.players[target].civ;
                       "a {} city-state worth {score} to the {} plan; {standing}, \
                        this makes {mine} envoy{}",
                       g.cs_type(&g.players[target].civ), strategy.as_str(),
                       if mine == 1 { "" } else { "s" });
            }
            if g.apply(pid, &Action::SendEnvoy { player: target }).is_err() {
                break;
            }
        }
    }

    /// Buy out a close Great Person race only when the person advances the
    /// active plan and the purchase leaves a useful operating reserve. Normal
    /// GPP recruitment is automatic at turn start; this phase is deliberately
    /// limited to one tempo purchase per turn.
    fn advanced_great_people(&self, g: &mut Game, pid: usize, strategy: GrandStrategy) {
        let city_count = g.player_city_ids(pid).len() as f64;
        let gold_reserve = 150.0 + 50.0 * city_count;
        let faith_reserve = match strategy {
            GrandStrategy::Religion => 250.0,
            GrandStrategy::Culture if g.players[pid].civics.contains(&crate::name!("cold_war")) => 700.0,
            _ => 100.0,
        };
        let mut candidates = Vec::new();
        for kind in [
            "scientist",
            "engineer",
            "writer",
            "artist",
            "musician",
            "merchant",
            "general",
            "admiral",
            "prophet",
        ] {
            let Some((_, person)) = g.current_great_person(kind) else {
                continue;
            };
            if !g.can_activate_current_great_person(pid, kind) {
                continue;
            }
            let points = g.players[pid].gpp.get(kind).copied().unwrap_or(0.0);
            let cost = g.gp_cost(pid, kind);
            let missing = (cost - points).max(0.0);
            if missing <= f64::EPSILON {
                continue;
            }
            let affinity = match (strategy, kind) {
                (GrandStrategy::Science, "scientist")
                | (GrandStrategy::Culture, "writer" | "artist" | "musician")
                | (GrandStrategy::Diplomacy, "merchant")
                | (GrandStrategy::Conquest, "general" | "admiral") => 500.0,
                (GrandStrategy::Religion, "prophet") if g.players[pid].religion.is_none() => 650.0,
                (GrandStrategy::Expansion | GrandStrategy::Recovery, "engineer" | "merchant")
                | (GrandStrategy::Science | GrandStrategy::Culture, "engineer") => 300.0,
                (_, "prophet") if g.players[pid].religion.is_some() => -1_000.0,
                _ => 100.0,
            };
            let close_fraction = missing / cost.max(1.0);
            let limit = if affinity >= 500.0 { 0.40 } else { 0.15 };
            if affinity < 0.0 || close_fraction > limit {
                continue;
            }
            let effect_value = person.effects.values().sum::<f64>() * 12.0;
            for (currency, bank, reserve) in [
                ("gold", g.players[pid].gold, gold_reserve),
                ("faith", g.players[pid].faith, faith_reserve),
            ] {
                let Some(price) = g.great_person_patronage_price(pid, kind, currency) else {
                    continue;
                };
                if bank + f64::EPSILON < price + reserve {
                    continue;
                }
                let opportunity = price / (bank - reserve).max(1.0);
                let score = (affinity + effect_value) * (1.0 - opportunity.min(0.95));
                candidates.push((
                    score,
                    std::cmp::Reverse((kind.to_string(), currency.to_string())),
                    Action::PatronizeGreatPerson {
                        kind: kind.to_string(),
                        currency: currency.to_string(),
                    },
                ));
            }
        }
        if let Some((score, _, action)) = candidates.into_iter().max_by(|left, right| {
            left.0
                .partial_cmp(&right.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.1.cmp(&right.1))
        }) {
            if let Action::PatronizeGreatPerson { kind, currency } = &action {
                let price = g
                    .great_person_patronage_price(pid, kind, currency)
                    .unwrap_or(0.0);
                think!(self.journal(), Economy, Decision, "Buying out the {} race", plain(kind);
                       "{price:.0} {currency}, worth {score:.0} to the {} plan",
                       strategy.as_str());
            }
            let _ = g.apply(pid, &action);
        }
    }

    /// Convert a deep treasury into immediate tempo gains. Candidate
    /// units, buildings, and Governor-enabled districts reuse the strategic
    /// production evaluator, but are scored at their undiscounted positional
    /// value because a purchase completes now. A strategy-sensitive reserve
    /// protects Great Person patronage, Great Work deals, upgrades, and
    /// emergency reinforcement instead of treating all affordable actions as
    /// equally spendable. Late empires can earn more Gold each turn than one
    /// purchase consumes, so buy a bounded series and recompute needs after
    /// every item instead of carrying an ever-growing inert treasury.
    fn advanced_gold_spending(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) -> bool {
        let city_count = g.player_city_ids(pid).len();
        let reserve = match plan.strategy {
            GrandStrategy::Diplomacy | GrandStrategy::Culture => {
                300.0 + 75.0 * city_count as f64
            }
            GrandStrategy::Expansion => 250.0 + 75.0 * city_count as f64,
            GrandStrategy::Science => 250.0 + 50.0 * city_count as f64,
            GrandStrategy::Religion => 150.0 + 50.0 * city_count as f64,
            GrandStrategy::Conquest | GrandStrategy::Recovery => {
                75.0 + 25.0 * city_count as f64
            }
        };
        let purchase_limit = city_count.clamp(1, 4);
        let unit_purchase_limit = if g.players[pid].gold > reserve + 1_000.0 {
            2
        } else {
            1
        };
        let mut purchased = false;
        let mut purchased_units = 0;
        for _ in 0..purchase_limit {
            let bank = g.players[pid].gold;
            let counts = self.counts(g, pid);
            let mut candidates = Vec::new();
            let mut plot_options = Vec::new();
            // Every candidate asks its city for the same yields, twice — once
            // here and once inside `production_value`. The guard borrows the
            // game immutably, so those answers cannot go stale before it is
            // dropped below.
            let memo = g.query_memo();
            for action in g.legal_actions_within(
                pid,
                ActionFamilies::PURCHASES | ActionFamilies::EMPIRE,
            ) {
                if let Action::BuyPlot {
                    city: _,
                    pos,
                    cost,
                } = &action
                {
                    if bank + f64::EPSILON < reserve + 200.0 + cost {
                        continue;
                    }
                    let tile = &g.map.tiles[pos];
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
                    let yields = self
                        .yield_value(g.rules.tile_yields(&visible_tile), plan.strategy)
                        * 24.0;
                    let resource = resource
                        .map(|spec| match spec.class.as_str() {
                            "luxury" => 260.0,
                            "strategic" => 230.0,
                            "bonus" => 70.0,
                            _ => 0.0,
                        })
                        .unwrap_or(0.0);
                    let wonder = tile
                        .feature
                        .as_ref()
                        .and_then(|name| g.rules.features.get(name))
                        .is_some_and(|feature| feature.natural_wonder) as u8 as f64
                        * 320.0;
                    let base_score = yields + resource + wonder - cost * 0.70;
                    // Use adjacency as a cheap shortlist signal. Exact site
                    // legality and full production value are evaluated below
                    // for only the strongest four plots, avoiding a full game
                    // clone for every border hex in a large empire.
                    let adjacency_hint = g
                        .rules
                        .districts
                        .iter()
                        .filter(|(_, spec)| {
                            spec.buildable
                                && spec
                                    .unique_to
                                    .as_ref()
                                    .is_none_or(|civ| civ == &g.players[pid].civ)
                                && spec
                                    .tech
                                    .as_ref()
                                    .is_none_or(|tech| g.players[pid].techs.contains(tech))
                                && spec
                                    .civic
                                    .as_ref()
                                    .is_none_or(|civic| g.players[pid].civics.contains(civic))
                        })
                        .map(|(district, _)| {
                            self.yield_value(g.district_yields(district, *pos), plan.strategy)
                        })
                        .fold(0.0, f64::max)
                        * 10.0;
                    plot_options.push((
                        base_score + adjacency_hint,
                        base_score,
                        std::cmp::Reverse(format!("{action:?}")),
                        action,
                    ));
                    continue;
                }
                let (city, item, currency) = match &action {
                    Action::Buy {
                        city,
                        unit,
                        formation,
                        currency,
                    } => (
                        *city,
                        if *formation == 0 {
                            Item::Unit { unit: unit.clone() }
                        } else {
                            Item::Formation {
                                unit: unit.clone(),
                                formation: *formation,
                            }
                        },
                        currency.as_str(),
                    ),
                    Action::BuyBuilding {
                        city,
                        building,
                        currency,
                    } => (
                        *city,
                        Item::Building {
                            building: building.clone(),
                        },
                        currency.as_str(),
                    ),
                    Action::BuyDistrict {
                        city,
                        district,
                        pos,
                        currency,
                    } => (
                        *city,
                        Item::District {
                            district: district.clone(),
                            pos: *pos,
                        },
                        currency.as_str(),
                    ),
                    _ => continue,
                };
                if currency != "gold" {
                    continue;
                }
                if purchased_units >= unit_purchase_limit && matches!(&item, Item::Unit { .. }) {
                    continue;
                }
                let production = g.city_yields(city).production.max(1.0);
                let turns = g.item_remaining_cost_for_city(pid, city, &item) / production;
                let production_score = self.production_value(g, pid, city, &item, plan, &counts);
                if production_score <= -1_000.0 {
                    continue;
                }
                // Long production time can make even a strategically
                // redundant unit clear the combined purchase score below.
                // Require the underlying need itself to be meaningful before
                // converting a deep treasury into more bodies on the map.
                if matches!(&item, Item::Unit { .. }) && production_score < 120.0 {
                    continue;
                }
                let mut after = g.clone();
                if after.apply(pid, &action).is_err() {
                    continue;
                }
                let cost = (bank - after.players[pid].gold).max(0.0);
                if after.players[pid].gold + f64::EPSILON < reserve {
                    continue;
                }
                let positional = production_score * (7.0 + turns.max(1.0));
                let score = positional + turns.clamp(0.0, 20.0) * 6.0 - cost * 0.30;
                if score >= 120.0 {
                    candidates.push((score, std::cmp::Reverse(format!("{action:?}")), action));
                }
            }
            // A plot is a surplus purchase. Concrete units, buildings and
            // Governor districts already proved an immediate strategic need,
            // so they keep priority whenever one clears the score floor.
            if candidates.is_empty() {
                plot_options.sort_by(|left, right| {
                    right
                        .0
                        .total_cmp(&left.0)
                        .then_with(|| left.2.cmp(&right.2))
                });
                plot_options.truncate(4);
                for (_, base_score, _, action) in plot_options {
                    let Action::BuyPlot { city, pos, .. } = &action else {
                        unreachable!("plot shortlist contains only BuyPlot actions")
                    };
                    let mut after = g.clone();
                    if after.apply(pid, &action).is_err()
                        || after.players[pid].gold + f64::EPSILON < reserve + 200.0
                    {
                        continue;
                    }
                    // Buying the right hex can be valuable even before a
                    // Citizen works it: ownership may expose a district or
                    // Wonder site.
                    let site_value = after
                        .producible_items(pid, *city)
                        .into_iter()
                        .filter(|item| match item {
                            Item::District { pos: site, .. } | Item::Wonder { pos: site, .. } => {
                                site == pos
                            }
                            _ => false,
                        })
                        .map(|item| {
                            self.production_value(&after, pid, *city, &item, plan, &counts)
                        })
                        .fold(0.0, f64::max)
                        .max(0.0)
                        * 0.35;
                    let score = base_score + site_value;
                    if score >= 120.0 {
                        candidates.push((
                            score,
                            std::cmp::Reverse(format!("{action:?}")),
                            action,
                        ));
                    }
                }
            }
            drop(memo);
            let best = candidates.into_iter().max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| left.1.cmp(&right.1))
            });
            let Some((score, _, action)) = best else { break };
            let is_unit = matches!(action, Action::Buy { .. });
            let before = g.players[pid].gold;
            if g.apply(pid, &action).is_err() {
                break;
            }
            if self.journal().wants(crate::reasoning::Level::Decision) {
                let spent = (before - g.players[pid].gold).max(0.0);
                let (city, what) = match &action {
                    Action::Buy { city, unit, formation, .. } => (
                        Some(*city),
                        if *formation == 0 {
                            plain(unit)
                        } else {
                            format!("a formation of {}", plain(unit))
                        },
                    ),
                    Action::BuyBuilding { city, building, .. } => (Some(*city), plain(building)),
                    Action::BuyDistrict { city, district, .. } => {
                        (Some(*city), format!("a {} district", plain(district)))
                    }
                    Action::BuyPlot { city, pos, .. } => {
                        (Some(*city), format!("the tile at {pos:?}"))
                    }
                    _ => (None, "something".to_string()),
                };
                let where_ = city
                    .and_then(|id| g.cities.get(&id))
                    .map(|city| city.name.clone())
                    .unwrap_or_else(|| "the empire".to_string());
                think!(self.journal(), Economy, Decision, "Buying {what} for {where_}";
                       "{spent:.0} Gold, worth {score:.0} to the {} plan; {:.0} left \
                        above a reserve of {reserve:.0}",
                       plan.strategy.as_str(), g.players[pid].gold);
            }
            purchased = true;
            purchased_units += is_unit as usize;
        }
        purchased
    }

    fn counts(&self, g: &Game, pid: usize) -> EmpireCounts {
        let mut counts = EmpireCounts::default();
        for uid in g.player_unit_ids(pid) {
            counts.add_unit(g, &g.units[&uid].kind);
        }
        for cid in g.player_city_ids(pid) {
            if let Some(item) = g.cities[&cid].queue.first() {
                counts.add_item(g, item);
            }
        }
        counts
    }

    fn religious_production(&self, g: &mut Game, pid: usize) {
        let city_ids = g.player_city_ids(pid);
        let has_holy_site = city_ids
            .iter()
            .any(|cid| g.cities[cid].districts.contains_key(crate::name!("holy_site")));
        if !has_holy_site {
            let holy_site_planned = city_ids.iter().any(|cid| {
                matches!(
                    g.cities[cid].queue.first(),
                    Some(Item::District { district, .. }) if district == "holy_site"
                )
            });
            if holy_site_planned {
                return;
            }
            let mut best: Option<(f64, u32, Pos)> = None;
            for cid in &city_ids {
                if !g.cities[cid].queue.is_empty() {
                    continue;
                }
                for item in g.producible_items(pid, *cid) {
                    let Item::District { district, pos } = item else {
                        continue;
                    };
                    if district != "holy_site" {
                        continue;
                    }
                    let faith = g.district_yields(crate::name!("holy_site"), pos).faith;
                    if best
                        .map(|old| {
                            faith > old.0 || (faith == old.0 && (*cid, pos) > (old.1, old.2))
                        })
                        .unwrap_or(true)
                    {
                        best = Some((faith, *cid, pos));
                    }
                }
            }
            if let Some((_, city, pos)) = best {
                let _ = g.apply(
                    pid,
                    &Action::Produce {
                        city,
                        item: Item::District {
                            district: crate::name!("holy_site"),
                            pos,
                        },
                    },
                );
            }
            return;
        }

        let religion_unfounded = g.players[pid].religion.is_none();
        let prophet_slot_open = g.religions_founded()
            + g.players
                .iter()
                .filter(|player| player.prophet_pending)
                .count()
            < g.max_religions();
        if religion_unfounded && !g.players[pid].prophet_pending && prophet_slot_open {
            let shrine_planned = city_ids.iter().any(|cid| {
                matches!(
                    g.cities[cid].queue.first(),
                    Some(Item::Building { building }) if building == "shrine"
                )
            });
            let has_shrine = city_ids.iter().any(|cid| {
                g.cities[cid]
                    .buildings
                    .iter()
                    .any(|building| building == "shrine")
            });
            if !has_shrine && g.religions_founded() == 0 {
                if shrine_planned {
                    return;
                }
                for cid in &city_ids {
                    let item = Item::Building {
                        building: crate::name!("shrine"),
                    };
                    if g.cities[cid].queue.is_empty()
                        && g.cities[cid].districts.contains_key(crate::name!("holy_site"))
                        && g.can_produce(pid, *cid, &item)
                    {
                        let _ = g.apply(pid, &Action::Produce { city: *cid, item });
                        return;
                    }
                }
                return;
            }

            let prayers = Item::Project {
                project: crate::name!("holy_site_prayers"),
            };
            let prayer_city = {
                let _memo = g.query_memo();
                city_ids
                    .iter()
                    .filter(|cid| {
                        g.cities[cid].queue.is_empty()
                            && g.cities[cid].districts.contains_key(crate::name!("holy_site"))
                            && g.can_produce(pid, **cid, &prayers)
                    })
                    .max_by(|left, right| {
                        g.city_yields(**left)
                            .production
                            .total_cmp(&g.city_yields(**right).production)
                            .then_with(|| right.cmp(left))
                    })
                    .copied()
            };
            if let Some(city) = prayer_city {
                let _ = g.apply(
                    pid,
                    &Action::Produce {
                        city,
                        item: prayers,
                    },
                );
            }
            return;
        }

        for building in ["shrine", "temple"] {
            for cid in &city_ids {
                let item = Item::Building {
                    building: Name::new(building),
                };
                if g.cities[cid].queue.is_empty()
                    && g.cities[cid].districts.contains_key(crate::name!("holy_site"))
                    && g.can_produce(pid, *cid, &item)
                {
                    let _ = g.apply(pid, &Action::Produce { city: *cid, item });
                    return;
                }
            }
        }
    }

    /// A rival founder's religion holding the majority in one of our cities.
    /// The religious victory requires every living major, so home
    /// reconversion alone denies it — this is the trigger for the
    /// cross-strategy defense below.
    fn home_conversion_threat(&self, g: &Game, pid: usize) -> Option<String> {
        let own = g.players[pid].religion.as_deref();
        let rival_faith = |religion: &str| {
            g.players.iter().any(|o| {
                o.id != pid && o.alive && !o.is_minor && o.religion.as_deref() == Some(religion)
            })
        };
        for cid in g.player_city_ids(pid) {
            let city = &g.cities[&cid];
            // React while the conversion is still in progress: waiting for a
            // flipped majority loses the pressure race outright. Any rival
            // faith at 60% of the city's strongest pressure is a live threat.
            let top = city.pressure.values().fold(0.0f64, |a, b| a.max(*b));
            if top <= 0.0 {
                continue;
            }
            for (religion, pressure) in &city.pressure {
                if Some(religion.as_str()) == own || *pressure + 1e-9 < top * 0.6 {
                    continue;
                }
                if rival_faith(religion) {
                    return Some(religion.clone());
                }
            }
        }
        None
    }

    /// Home religious defense for civilizations whose grand strategy is NOT
    /// religion. Founders reuse the emergency spending path; everyone else
    /// buys Missionaries of an adopted non-threat majority faith, which the
    /// engine now assigns from the purchase city (stock rule).
    fn religious_defense(&self, g: &mut Game, pid: usize, threat: &str) {
        if g.players[pid].religion.is_some() {
            self.religious_spending(g, pid, false);
            return;
        }
        let defenders = g
            .units
            .values()
            .filter(|unit| unit.owner == pid && unit.kind == "missionary")
            .count();
        if defenders >= 2 {
            return;
        }
        for cid in g.player_city_ids(pid) {
            let Some(majority) = g.city_religion(&g.cities[&cid]) else {
                continue;
            };
            if majority == threat {
                continue;
            }
            if g.apply(
                pid,
                &Action::Buy {
                    city: cid,
                    unit: crate::name!("missionary"),
                    formation: 0,
                    currency: "faith".to_string(),
                },
            )
            .is_ok()
            {
                return;
            }
        }
    }

    fn city_needs_religious_support(
        g: &Game,
        pid: usize,
        city: &crate::game::City,
        religion: &str,
    ) -> bool {
        if city.owner != pid {
            return false;
        }
        let own = city.pressure.get(religion).copied().unwrap_or(0.0);
        let rival = city
            .pressure
            .iter()
            .filter(|(faith, _)| faith.as_str() != religion)
            .map(|(_, pressure)| *pressure)
            .fold(0.0_f64, f64::max);
        g.city_religion(city) != Some(religion) || (rival > 0.0 && rival * 2.0 >= own)
    }

    /// Keep a founded religion's small field corps useful after the adaptive
    /// planner changes its primary victory strategy. Previously that switch
    /// made charged Missionaries stop at home and let thousands of Faith sit
    /// idle. A large surplus may start a secondary campaign, while an active
    /// spreader keeps it moving after the initial purchase lowers the bank.
    fn religious_offensive_posture(&self, g: &Game, pid: usize, strategy: GrandStrategy) -> bool {
        if strategy == GrandStrategy::Religion {
            return true;
        }
        if !g.victory_conditions.religious {
            return false;
        }
        let Some(religion) = g.players[pid].religion.as_deref() else {
            return false;
        };
        let foreign_target = g.cities.values().any(|city| {
            city.owner != pid
                && g.players[city.owner].alive
                && !g.players[city.owner].is_minor
                && !g.players[city.owner].is_barbarian
                && !g.is_at_war(pid, city.owner)
                && g.city_religion(city) != Some(religion)
        });
        if !foreign_target {
            return false;
        }
        let active_campaign = g.units.values().any(|unit| {
            unit.owner == pid
                && unit.religion.as_deref() == Some(religion)
                && unit.charges > 0
                && g.rules.units[unit.kind].religious_spread > 0.0
        });
        active_campaign || g.players[pid].faith >= g.game_speed.scale(2_000.0)
    }

    fn religious_spending(&self, g: &mut Game, pid: usize, offensive: bool) {
        self.religious_spending_with_reserve(g, pid, offensive, 80.0);
    }

    fn religious_spending_with_reserve(
        &self,
        g: &mut Game,
        pid: usize,
        offensive: bool,
        ordinary_reserve: f64,
    ) {
        let Some(religion) = g.players[pid].religion.clone() else {
            return;
        };
        let match_point_defense = self
            .victory_denial(g, pid)
            .is_some_and(|(_, counter)| counter == GrandStrategy::Religion);
        let count_units = |kind: &str| {
            g.units
                .values()
                .filter(|unit| unit.owner == pid && unit.kind == kind)
                .count()
        };
        let missionaries = count_units("missionary");
        let apostles = count_units("apostle");
        let gurus = count_units("guru");
        let inquisitors = count_units("inquisitor");
        let defensive_targets = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| Self::city_needs_religious_support(g, pid, &g.cities[cid], &religion))
            .count();
        let home_under_pressure = defensive_targets > 0;
        let inquisition_launched = g.players[pid]
            .counters
            .get("inquisition")
            .copied()
            .unwrap_or(0)
            > 0;
        let spread_targets = defensive_targets
            + usize::from(offensive)
                * g.cities
                    .values()
                    .filter(|city| {
                        city.owner != pid
                            && !g.is_at_war(pid, city.owner)
                            && g.city_religion(city) != Some(religion.as_str())
                    })
                    .count();
        // A small circulating corps is enough: every Missionary has several
        // spreads, and replacements can be bought as charges are consumed.
        // Scaling gently with live targets preserves a religious push without
        // allowing one faith purchase every turn to fill the map.
        let missionary_cap = if spread_targets == 0 {
            0
        } else if offensive {
            (2 + spread_targets.div_ceil(4)).min(6)
        } else {
            (1 + defensive_targets.div_ceil(2)).min(2)
        };
        let apostle_cap = if offensive { 2 } else { 0 };
        let guru_cap = usize::from(offensive && apostles > 0);
        let inquisitor_cap = if home_under_pressure && inquisition_launched {
            2
        } else {
            0
        };
        let priorities: &[&str] = if home_under_pressure
            && inquisition_launched
            && inquisitors < 2
        {
            &["inquisitor", "apostle", "missionary", "guru"]
        } else if !offensive {
            &["missionary", "inquisitor"]
        } else if apostles < 2 {
            &["apostle", "missionary", "guru"]
        } else if gurus < 1 {
            &["guru", "apostle", "missionary"]
        } else {
            &["missionary", "apostle", "guru"]
        };
        for unit in priorities {
            let cap = match *unit {
                "missionary" => missionary_cap,
                "apostle" => apostle_cap,
                "guru" => guru_cap,
                "inquisitor" => inquisitor_cap,
                _ => 0,
            };
            let current = match *unit {
                "missionary" => missionaries,
                "apostle" => apostles,
                "guru" => gurus,
                "inquisitor" => inquisitors,
                _ => 0,
            };
            if current >= cap {
                continue;
            }
            let Some(spec) = g.rules.units.get(*unit) else {
                continue;
            };
            let price = spec.cost * 2.0;
            // The ordinary buffer is useful while safely building toward a
            // victory, but it must not block the last affordable defender at
            // match point or when one of our cities is already losing its
            // religious majority.
            let reserve = if match_point_defense || home_under_pressure {
                0.0
            } else {
                ordinary_reserve
            };
            if g.players[pid].faith + f64::EPSILON < price + reserve {
                continue;
            }
            let cities = g.player_city_ids(pid);
            for cid in cities {
                // Religious units inherit the purchase city's majority.  A
                // converted Holy Site must never make the defender spend its
                // Faith strengthening the runaway rival religion.
                if g.city_religion(&g.cities[&cid]) != Some(religion.as_str()) {
                    continue;
                }
                if g.apply(
                    pid,
                    &Action::Buy {
                        city: cid,
                        unit: Name::new(*unit),
                        formation: 0,
                        currency: "faith".to_string(),
                    },
                )
                .is_ok()
                {
                    return;
                }
            }
        }
    }

    fn culture_spending(&self, g: &mut Game, pid: usize) {
        let active_naturalists = g
            .units
            .values()
            .filter(|unit| unit.owner == pid && unit.kind == "naturalist")
            .count();
        if active_naturalists == 0
            && !g.national_park_sites(pid).is_empty()
            && g.players[pid].faith + f64::EPSILON >= g.naturalist_purchase_cost(pid)
        {
            for city in g.player_city_ids(pid) {
                if g.apply(
                    pid,
                    &Action::Buy {
                        city,
                        unit: crate::name!("naturalist"),
                        formation: 0,
                        currency: "faith".to_string(),
                    },
                )
                .is_ok()
                {
                    return;
                }
            }
        }
        let active_bands = g
            .units
            .values()
            .filter(|unit| unit.owner == pid && unit.kind == "rock_band")
            .count();
        if active_bands >= 2
            || !g.players[pid].civics.contains(&crate::name!("cold_war"))
            || g.players[pid].faith + f64::EPSILON < g.rules.units["rock_band"].cost
        {
            return;
        }
        for city in g.player_city_ids(pid) {
            if g.apply(
                pid,
                &Action::Buy {
                    city,
                    unit: crate::name!("rock_band"),
                    formation: 0,
                    currency: "faith".to_string(),
                },
            )
            .is_ok()
            {
                return;
            }
        }
    }

    fn faith_building_spending(&self, g: &mut Game, pid: usize, strategy: GrandStrategy) {
        let reserve = match strategy {
            GrandStrategy::Religion => 180.0,
            GrandStrategy::Culture if !g.national_park_sites(pid).is_empty() => {
                g.naturalist_purchase_cost(pid)
            }
            GrandStrategy::Culture if g.players[pid].civics.contains(&crate::name!("cold_war")) => 700.0,
            _ => 80.0,
        };
        let best = g
            .legal_actions_within(pid, ActionFamilies::PURCHASES | ActionFamilies::EMPIRE)
            .into_iter()
            .filter_map(|action| match &action {
                Action::BuyBuilding {
                    city,
                    building,
                    currency,
                } if currency == "faith" => {
                    let spec = &g.rules.buildings[building];
                    let cost = g.building_faith_purchase_cost(pid, *city, building)?;
                    if g.players[pid].faith + f64::EPSILON < cost + reserve {
                        return None;
                    }
                    let worship = spec.worship_belief.is_some() as i32;
                    let defensive_value = match strategy {
                        GrandStrategy::Conquest | GrandStrategy::Recovery => spec.outer_defense * 2,
                        _ => spec.outer_defense,
                    };
                    let score = (self.yield_value(spec.yields, strategy) * 25.0) as i32
                        + (spec.housing * 35.0 + spec.amenity * 50.0) as i32
                        + spec.great_work_slots.values().sum::<i32>() * 60
                        + spec.trade_route_capacity * 100
                        + defensive_value
                        + worship * 220
                        - (cost * 0.05) as i32;
                    Some((score, std::cmp::Reverse((*city, building.clone())), action))
                }
                _ => None,
            })
            .max_by_key(|(score, key, _)| (*score, key.clone()));
        if let Some((_, _, action)) = best {
            let _ = g.apply(pid, &action);
        }
    }

    fn governor_priority(strategy: GrandStrategy) -> &'static [&'static str] {
        match strategy {
            GrandStrategy::Expansion => &[
                "magnus", "pingala", "liang", "reyna", "victor", "moksha", "amani",
            ],
            GrandStrategy::Science => &[
                "pingala", "magnus", "reyna", "liang", "victor", "moksha", "amani",
            ],
            GrandStrategy::Culture => &[
                "pingala", "reyna", "liang", "magnus", "moksha", "victor", "amani",
            ],
            GrandStrategy::Religion => &[
                "moksha", "pingala", "magnus", "amani", "liang", "victor", "reyna",
            ],
            GrandStrategy::Diplomacy => &[
                "amani", "pingala", "reyna", "magnus", "liang", "victor", "moksha",
            ],
            GrandStrategy::Conquest => &[
                "victor", "magnus", "pingala", "liang", "reyna", "moksha", "amani",
            ],
            GrandStrategy::Recovery => &[
                "victor", "reyna", "magnus", "pingala", "liang", "moksha", "amani",
            ],
        }
    }

    fn governor_promotion_priority(
        strategy: GrandStrategy,
        governor: &str,
    ) -> &'static [&'static str] {
        match governor {
            "pingala" if strategy == GrandStrategy::Culture => &[
                "connoisseur",
                "researcher",
                "grants",
                "curator",
                "space_initiative",
            ],
            "pingala" => &[
                "researcher",
                "connoisseur",
                "grants",
                "space_initiative",
                "curator",
            ],
            "magnus" => &[
                "provision",
                "surplus_logistics",
                "black_marketeer",
                "industrialist",
                "vertical_integration",
            ],
            "liang" => &[
                "zoning_commissioner",
                "aquaculture",
                "reinforced_materials",
                "water_works",
                "parks_and_recreation",
            ],
            "reyna" => &[
                "harbormaster",
                "forestry_management",
                "tax_collector",
                "contractor",
                "renewable_subsidizer",
            ],
            "victor" => &[
                "garrison_commander",
                "defense_logistics",
                "embrasure",
                "air_defense_initiative",
                "arms_race_proponent",
            ],
            "moksha" => &[
                "grand_inquisitor",
                "laying_on_of_hands",
                "citadel_of_god",
                "patron_saint",
                "divine_architect",
            ],
            "amani" => &[
                "emissary",
                "affluence",
                "local_informants",
                "foreign_investor",
                "puppeteer",
            ],
            _ => &[],
        }
    }

    fn best_governor_city(
        &self,
        g: &Game,
        pid: usize,
        governor: &str,
        plan: &StrategicPlan,
    ) -> Option<u32> {
        let occupied: BTreeSet<u32> = g.players[pid]
            .governor_roster
            .values()
            .filter_map(|state| state.city)
            .collect();
        let mut candidates: Vec<u32> = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|city| !occupied.contains(city))
            .collect();
        if governor == "amani" {
            candidates.extend(
                g.players
                    .iter()
                    .filter(|player| {
                        player.alive
                            && player.is_minor
                            && !player.is_barbarian
                            && !g.is_at_war(pid, player.id)
                    })
                    .flat_map(|player| g.player_city_ids(player.id))
                    .filter(|city| !occupied.contains(city)),
            );
        }
        // `max_by` re-evaluates both sides of every comparison, so each city's
        // yields would otherwise be recomputed a logarithmic number of times.
        let _memo = g.query_memo();
        candidates.into_iter().max_by(|left, right| {
            let value = |city_id: u32| {
                let city = &g.cities[&city_id];
                let yields = g.city_yields(city_id);
                let own = city.owner == pid;
                let commercial = city.districts.keys().any(|district| {
                    matches!(g.district_family(*district).as_str(), "commercial_hub" | "harbor")
                }) as i32 as f64;
                let holy = city
                    .districts
                    .keys()
                    .any(|district| g.district_family(*district) == "holy_site")
                    as i32 as f64;
                let base = if own {
                    (100.0 - city.loyalty).max(0.0) * 2.0
                } else {
                    0.0
                };
                base + match governor {
                    "pingala" => {
                        city.pop as f64 * 14.0 + yields.science * 9.0 + yields.culture * 9.0
                    }
                    "magnus" => {
                        city.pop as f64 * 5.0
                            + yields.food * 5.0
                            + yields.production * 11.0
                            + matches!(
                                city.queue.first(),
                                Some(Item::Unit { unit }) if unit == "settler"
                            ) as i32 as f64
                                * 180.0
                    }
                    "liang" => yields.production * 10.0 + city.owned_tiles.len() as f64 * 2.0,
                    "reyna" => city.pop as f64 * 8.0 + yields.gold * 13.0 + commercial * 150.0,
                    "victor" => {
                        plan.threatened_city.is_some_and(|target| target == city_id) as i32 as f64
                            * 600.0
                            + city.wall_hp.max(0) as f64
                            + city.pop as f64 * 5.0
                    }
                    "moksha" => {
                        yields.faith * 14.0
                            + holy * 180.0
                            + (g.players[pid].holy_city == Some(city_id)) as i32 as f64 * 220.0
                    }
                    "amani" if !own => 600.0 + g.envoys_at(pid, city.owner) as f64 * 55.0,
                    "amani" => (100.0 - city.loyalty).max(0.0) * 5.0,
                    _ => 0.0,
                }
            };
            value(*left)
                .partial_cmp(&value(*right))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| right.cmp(left))
        })
    }

    fn preferred_governor_promotion(
        &self,
        g: &Game,
        pid: usize,
        strategy: GrandStrategy,
        governor: &str,
    ) -> Option<String> {
        let available = g.available_governor_promotions(pid, governor);
        Self::governor_promotion_priority(strategy, governor)
            .iter()
            .find(|promotion| available.iter().any(|candidate| candidate == **promotion))
            .map(|promotion| (*promotion).to_string())
    }

    fn strategic_governors(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        let priority = Self::governor_priority(plan.strategy);
        while g.governor_titles_available(pid) > 0 {
            // Strategy can change every assessment window, but Governor
            // Titles arrive much more slowly. Finish the earliest incumbent's
            // two-promotion foundation before adapting the roster, otherwise
            // transient wars or victory races recreate the old dilution bug.
            let primary_name = g.players[pid]
                .governor_roster
                .iter()
                .filter(|(_, state)| state.promotions.len() < 2)
                .min_by_key(|(name, state)| (state.assigned_turn, name.as_str()))
                .map(|(name, _)| name.clone())
                .unwrap_or_else(|| Name::new(priority[0]));
            let primary = primary_name.as_str();
            if !g.players[pid].governor_roster.contains_key(primary) {
                if let Some(city) = self.best_governor_city(g, pid, primary, plan) {
                    if g.apply(
                        pid,
                        &Action::AppointGovernor {
                            governor: Name::new(primary),
                            city,
                        },
                    )
                    .is_ok()
                    {
                        continue;
                    }
                }
            }

            let primary_promotions = g.players[pid]
                .governor_roster
                .get(primary)
                .map(|state| state.promotions.len())
                .unwrap_or(0);
            if primary_promotions < 2 {
                if let Some(promotion) =
                    self.preferred_governor_promotion(g, pid, plan.strategy, primary)
                {
                    if g.apply(
                        pid,
                        &Action::PromoteGovernor {
                            governor: Name::new(primary),
                            promotion: Name::new(&promotion),
                        },
                    )
                    .is_ok()
                    {
                        continue;
                    }
                }
            }

            // After both tier-one promotions are online, establish one
            // complementary governor before completing the primary's tree.
            if g.players[pid].governor_roster.len() < 2 {
                if let Some((governor, city)) = priority.iter().skip(1).find_map(|governor| {
                    (!g.players[pid].governor_roster.contains_key(*governor))
                        .then(|| {
                            self.best_governor_city(g, pid, governor, plan)
                                .map(|city| ((*governor).to_string(), city))
                        })
                        .flatten()
                }) {
                    if g.apply(pid, &Action::AppointGovernor { governor: Name::new(&governor), city })
                        .is_ok()
                    {
                        continue;
                    }
                }
            }

            if let Some(promotion) =
                self.preferred_governor_promotion(g, pid, plan.strategy, primary)
            {
                if g.apply(
                    pid,
                    &Action::PromoteGovernor {
                        governor: Name::new(primary),
                        promotion: Name::new(&promotion),
                    },
                )
                .is_ok()
                {
                    continue;
                }
            }

            // Add a third regional anchor before investing deeply in the
            // complementary governor. Further titles finish existing trees;
            // only then does the roster widen again.
            if g.players[pid].governor_roster.len() < 3 {
                if let Some((governor, city)) = priority.iter().skip(1).find_map(|governor| {
                    (!g.players[pid].governor_roster.contains_key(*governor))
                        .then(|| {
                            self.best_governor_city(g, pid, governor, plan)
                                .map(|city| ((*governor).to_string(), city))
                        })
                        .flatten()
                }) {
                    if g.apply(pid, &Action::AppointGovernor { governor: Name::new(&governor), city })
                        .is_ok()
                    {
                        continue;
                    }
                }
            }

            let next_promotion = priority.iter().find_map(|governor| {
                self.preferred_governor_promotion(g, pid, plan.strategy, governor)
                    .map(|promotion| ((*governor).to_string(), promotion))
            });
            if let Some((governor, promotion)) = next_promotion {
                if g.apply(
                    pid,
                    &Action::PromoteGovernor {
                        governor: Name::new(&governor),
                        promotion: Name::new(&promotion),
                    },
                )
                .is_ok()
                {
                    continue;
                }
            }

            let appointment = priority.iter().find_map(|governor| {
                (!g.players[pid].governor_roster.contains_key(*governor))
                    .then(|| {
                        self.best_governor_city(g, pid, governor, plan)
                            .map(|city| ((*governor).to_string(), city))
                    })
                    .flatten()
            });
            let Some((governor, city)) = appointment else {
                break;
            };
            let where_ = g
                .cities
                .get(&city)
                .map(|city| city.name.clone())
                .unwrap_or_else(|| format!("city {city}"));
            if g.apply(
                pid,
                &Action::AppointGovernor {
                    governor: Name::new(&governor),
                    city,
                },
            )
            .is_err()
            {
                break;
            }
            think!(self.journal(), Government, Decision, "Posting {} to {where_}", plain(&governor);
                   "the city the {} plan gets most from", plan.strategy.as_str());
        }

    }

    /// A faith-rich empire countering a military or religious victory threat
    /// should convert that otherwise stranded treasury into defenders once
    /// Theocracy (or another legal faith-purchase source) makes them available.
    fn military_faith_spending(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) -> bool {
        if !matches!(
            plan.strategy,
            GrandStrategy::Conquest | GrandStrategy::Recovery
        ) || g.players[pid].faith < 600.0
        {
            return false;
        }
        let bank = g.players[pid].faith;
        let reserve = 180.0;
        let counts = self.counts(g, pid);
        let mut candidates = Vec::new();
        let memo = g.query_memo();
        for action in
            g.legal_actions_within(pid, ActionFamilies::PURCHASES | ActionFamilies::EMPIRE)
        {
            let Action::Buy {
                city,
                unit,
                formation,
                currency,
            } = &action
            else {
                continue;
            };
            if currency != "faith" || g.rules.units[unit].class != "military" {
                continue;
            }
            let mut after = g.clone();
            if after.apply(pid, &action).is_err() || after.players[pid].faith < reserve {
                continue;
            }
            let cost = (bank - after.players[pid].faith).max(0.0);
            let spec = &g.rules.units[unit];
            let combat = spec
                .strength
                .max(spec.ranged_strength)
                .max(spec.bombard_strength)
                + match *formation {
                    1 => 10.0,
                    2.. => 17.0,
                    _ => 0.0,
                };
            let strategic = self
                .production_value(
                    g,
                    pid,
                    *city,
                    &if *formation == 0 {
                        Item::Unit { unit: unit.clone() }
                    } else {
                        Item::Formation {
                            unit: unit.clone(),
                            formation: *formation,
                        }
                    },
                    plan,
                    &counts,
                )
                .max(0.0);
            let score = strategic + combat * 12.0 - cost * 0.25;
            candidates.push((score, std::cmp::Reverse((*city, unit.clone())), action));
        }
        drop(memo);
        candidates
            .into_iter()
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| left.1.cmp(&right.1))
            })
            .is_some_and(|(_, _, action)| g.apply(pid, &action).is_ok())
    }

    fn science_production(&self, g: &mut Game, pid: usize) {
        let completed = g.players[pid].science_projects.clone();
        let project = if !completed.contains("launch_earth_satellite") {
            "launch_earth_satellite"
        } else if !completed.contains("launch_moon_landing") {
            "launch_moon_landing"
        } else if !completed.contains("launch_mars_colony") {
            "launch_mars_colony"
        } else if !completed.contains("exoplanet_expedition") {
            "exoplanet_expedition"
        } else {
            "lagrange_laser_station"
        };
        let project_item = Item::Project {
            project: Name::new(project),
        };
        let parallel_project = matches!(
            project,
            "lagrange_laser_station" | "terrestrial_laser_station"
        );
        let already_queued = !parallel_project
            && g.player_city_ids(pid).iter().any(|cid| {
                matches!(
                    g.cities[cid].queue.first(),
                    Some(Item::Project { project: queued }) if queued == project
                )
            });
        if !already_queued {
            let project_city = {
                let _memo = g.query_memo();
                g.player_city_ids(pid)
                    .into_iter()
                    .filter(|cid| {
                        g.cities[cid].districts.contains_key(crate::name!("spaceport"))
                            && g.can_produce(pid, *cid, &project_item)
                            && !matches!(
                                g.cities[cid].queue.first(),
                                Some(Item::Project { project: queued }) if queued == project
                            )
                            && (self.victory_target == Some(VictoryTarget::Science)
                                || g.cities[cid].queue.is_empty())
                    })
                    .max_by(|a, b| {
                        g.city_yields(*a)
                            .production
                            .partial_cmp(&g.city_yields(*b).production)
                            .unwrap_or(std::cmp::Ordering::Equal)
                            .then_with(|| b.cmp(a))
                    })
            };
            if let Some(city) = project_city {
                let _ = g.apply(
                    pid,
                    &Action::Produce {
                        city,
                        item: project_item,
                    },
                );
                return;
            }
        }

        let city_ids = g.player_city_ids(pid);
        let built_spaceports = city_ids
            .iter()
            .filter(|cid| g.cities[cid].districts.contains_key(crate::name!("spaceport")))
            .count();
        let queued_spaceports = city_ids
            .iter()
            .filter(|cid| {
                matches!(
                    g.cities[cid].queue.first(),
                    Some(Item::District { district, .. }) if district == "spaceport"
                )
            })
            .count();
        // One launch site is enough for the sequential opening missions. A
        // second can prepare Mars while the first launches, and up to three
        // let the post-Exoplanet laser race run in parallel. Separate cities
        // matter; duplicate Spaceports in one production queue do not.
        let desired_spaceports = if self.victory_target == Some(VictoryTarget::Science) {
            if completed.contains("launch_mars_colony") {
                3
            } else if completed.contains("launch_moon_landing") {
                2
            } else {
                1
            }
        } else {
            1
        }
        .min(city_ids.len());
        if built_spaceports + queued_spaceports >= desired_spaceports {
            return;
        }
        let mut best: Option<(f64, u32, Pos)> = None;
        for cid in city_ids {
            if g.cities[&cid].districts.contains_key(crate::name!("spaceport"))
                || matches!(
                    g.cities[&cid].queue.first(),
                    Some(Item::District { district, .. }) if district == "spaceport"
                )
            {
                continue;
            }
            if self.victory_target != Some(VictoryTarget::Science)
                && !g.cities[&cid].queue.is_empty()
            {
                continue;
            }
            for item in g.producible_items(pid, cid) {
                let Item::District { district, pos } = item else {
                    continue;
                };
                if district != "spaceport" {
                    continue;
                }
                let production = g.city_yields(cid).production;
                if best
                    .map(|old| {
                        production > old.0 || (production == old.0 && (cid, pos) < (old.1, old.2))
                    })
                    .unwrap_or(true)
                {
                    best = Some((production, cid, pos));
                }
            }
        }
        if let Some((_, city, pos)) = best {
            let _ = g.apply(
                pid,
                &Action::Produce {
                    city,
                    item: Item::District {
                        district: crate::name!("spaceport"),
                        pos,
                    },
                },
            );
        }
    }

    fn advanced_spies(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        let ids: Vec<u32> = g
            .spies
            .values()
            .filter(|spy| spy.owner == pid)
            .map(|spy| spy.id)
            .collect();
        let infiltrated_cities: BTreeSet<u32> = g
            .spies
            .values()
            .filter(|spy| spy.owner != pid && spy.captured_by.is_none())
            .filter_map(|spy| {
                spy.city
                    .filter(|city| g.cities.get(city).is_some_and(|city| city.owner == pid))
            })
            .collect();
        let home_defender = ids
            .iter()
            .copied()
            .filter(|spy| {
                g.spies[spy].city.is_some_and(|city| {
                    // A spy outlives the city it was posted to: that city can
                    // be razed while the agent is still assigned there, and
                    // indexing the map for it took the whole server down with
                    // it. The infiltration scan above already reads this the
                    // safe way.
                    g.cities.get(&city).is_some_and(|home| {
                        home.owner == pid
                            && (home.districts.contains_key(crate::name!("spaceport"))
                                || infiltrated_cities.contains(&city))
                    })
                })
            })
            .min();
        for spy_id in ids {
            let legal = g.legal_spy_actions(pid, spy_id);
            if legal.is_empty() {
                continue;
            }
            let promotion_priority: &[&str] = match plan.strategy {
                GrandStrategy::Science => &[
                    "technologist",
                    "rocket_scientist",
                    "disguise",
                    "linguist",
                    "quartermaster",
                ],
                GrandStrategy::Culture => &[
                    "cat_burglar",
                    "con_artist",
                    "disguise",
                    "linguist",
                    "surveillance",
                ],
                GrandStrategy::Diplomacy => &[
                    "smear_campaign",
                    "polygraph",
                    "quartermaster",
                    "seduction",
                    "disguise",
                ],
                GrandStrategy::Conquest => &[
                    "license_to_kill",
                    "demolitions",
                    "guerrilla_leader",
                    "covert_action",
                    "ace_driver",
                ],
                _ => &[
                    "quartermaster",
                    "seduction",
                    "con_artist",
                    "technologist",
                    "linguist",
                ],
            };
            if let Some(action) = promotion_priority
                .iter()
                .find_map(|wanted| {
                    legal.iter().find(|action| {
                        matches!(action, Action::PromoteSpy { promotion, .. } if promotion == *wanted)
                    })
                })
                .or_else(|| {
                    legal
                        .iter()
                        .find(|action| matches!(action, Action::PromoteSpy { .. }))
                })
            {
                let _ = g.apply(pid, action);
                continue;
            }
            let current_city = g.spies.get(&spy_id).and_then(|spy| spy.city);
            if Some(spy_id) == home_defender
                && matches!(
                    plan.strategy,
                    GrandStrategy::Science | GrandStrategy::Recovery
                )
                && current_city.is_some_and(|city| g.cities[&city].owner == pid)
            {
                let spaceport = current_city.and_then(|city| {
                    g.cities[&city]
                        .districts
                        .iter()
                        .find_map(|(district, position)| {
                            (g.district_family(*district) == "spaceport").then_some(*position)
                        })
                });
                if let Some(action) = legal
                    .iter()
                    .filter(|action| {
                        matches!(action, Action::SpyMission { mission, .. } if mission == "counterspy")
                    })
                    .max_by_key(|action| match action {
                        Action::SpyMission { target, .. } => {
                            (Some(*target) == spaceport, std::cmp::Reverse(*target))
                        }
                        _ => unreachable!(),
                    })
                {
                    let _ = g.apply(pid, action);
                    continue;
                }
            }
            let offensive = current_city
                .and_then(|city| g.cities.get(&city))
                .is_some_and(|city| city.owner != pid);
            if offensive {
                if g.spies[&spy_id].level < 2 {
                    if let Some(action) = legal.iter().find(|action| {
                        matches!(action, Action::SpyMission { mission, .. } if mission == "gain_sources")
                    }) {
                        let _ = g.apply(pid, action);
                        continue;
                    }
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
                        if matches!(mission.as_str(), "gain_sources" | "counterspy") {
                            return None;
                        }
                        let city = current_city?;
                        let defender = g.cities[&city].owner;
                        let active = crate::game::SpyMission {
                            kind: mission.clone(),
                            city,
                            target: *target,
                            started: g.turn,
                            ends: g.turn,
                        };
                        let strategic = match (plan.strategy, mission.as_str()) {
                            (GrandStrategy::Science, "steal_tech_boost") => 320.0,
                            (GrandStrategy::Science, "disrupt_rocketry") => 290.0,
                            (GrandStrategy::Culture, "great_work_heist") => 340.0,
                            (GrandStrategy::Culture, "siphon_funds") => 135.0,
                            (GrandStrategy::Diplomacy, "fabricate_scandal") => 330.0,
                            (GrandStrategy::Diplomacy, "listening_post") => 185.0,
                            (GrandStrategy::Conquest, "neutralize_governor") => 310.0,
                            (GrandStrategy::Conquest, "sabotage_production") => 260.0,
                            (GrandStrategy::Conquest, "recruit_partisans") => 245.0,
                            (GrandStrategy::Conquest, "foment_unrest") => 230.0,
                            (GrandStrategy::Conquest, "breach_dam") => 210.0,
                            (_, "siphon_funds") => 150.0,
                            (_, "steal_tech_boost") => 145.0,
                            (_, "great_work_heist") => 135.0,
                            (_, "neutralize_governor") => 125.0,
                            (_, "fabricate_scandal") => 120.0,
                            (_, "listening_post") => 75.0,
                            _ => 100.0,
                        } + if plan.target_player == Some(defender) {
                            90.0
                        } else {
                            0.0
                        };
                        Some((
                            strategic * g.spy_success_chance(*spy, &active),
                            mission,
                            action,
                        ))
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
                    Action::AssignSpy { city, .. } if g.cities[city].owner != pid => {
                        let target = &g.cities[city];
                        let strategic = if plan.target_player == Some(target.owner) {
                            180
                        } else {
                            0
                        } + match plan.strategy {
                            GrandStrategy::Science => {
                                i32::from(target.districts.contains_key(crate::name!("campus"))) * 90
                                    + i32::from(target.districts.contains_key(crate::name!("spaceport"))) * 150
                            }
                            GrandStrategy::Culture => {
                                i32::from(target.districts.contains_key(crate::name!("theater_square"))) * 140
                            }
                            GrandStrategy::Diplomacy => {
                                i32::from(g.players[target.owner].is_minor) * 180
                            }
                            GrandStrategy::Conquest => {
                                i32::from(g.city_can_strike(target)) * 35
                                    + i32::from(g.players[target.owner].governors.contains(city))
                                        * 120
                            }
                            _ => i32::from(target.districts.contains_key(crate::name!("commercial_hub"))) * 70,
                        };
                        Some((
                            strategic
                                + target.pop * 8
                                + target.districts.len() as i32 * 14
                                + target.wonders.len() as i32 * 24,
                            std::cmp::Reverse(*city),
                            action,
                        ))
                    }
                    _ => None,
                })
                .max_by_key(|(score, city, _)| (*score, *city))
                .map(|(_, _, action)| action);
            if let Some(action) = assignment {
                let _ = g.apply(pid, action);
            }
        }
    }

    fn support_unit_value(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        unit: &str,
        plan: &StrategicPlan,
        counts: &EmpireCounts,
    ) -> f64 {
        let spec = &g.rules.units[unit];
        if spec.class != "support" || unit == "military_engineer" {
            return -10_000.0;
        }
        if spec.anti_air_strength > 0.0 {
            let hostile_aircraft = g
                .units
                .values()
                .filter(|candidate| {
                    candidate.owner != pid
                        && g.is_at_war(pid, candidate.owner)
                        && g.rules.units[candidate.kind].domain.as_deref() == Some("air")
                })
                .count();
            let desired = hostile_aircraft.min(g.player_city_ids(pid).len().div_ceil(2).max(1));
            if desired == 0 || counts.air_defense >= desired {
                return -10_000.0;
            }
            let best_available = g
                .rules
                .units
                .iter()
                .filter(|(name, candidate)| {
                    candidate.class == "support"
                        && candidate.anti_air_strength > 0.0
                        && g.can_produce(
                            pid,
                            cid,
                            &Item::Unit {
                                unit: (*name).clone(),
                            },
                        )
                })
                .map(|(_, candidate)| candidate.anti_air_strength)
                .fold(0.0_f64, f64::max);
            if spec.anti_air_strength + 5.0 < best_available {
                return -2_000.0;
            }
            return 340.0
                + spec.anti_air_strength * 3.0
                + hostile_aircraft.min(4) as f64 * 65.0
                + desired.saturating_sub(counts.air_defense) as f64 * 90.0;
        }
        let land_military = counts
            .military
            .saturating_sub(counts.naval + counts.aircraft);
        let field_support = counts
            .support
            .saturating_sub(counts.military_engineers + counts.air_defense);
        let desired_support = if land_military >= 8 {
            2
        } else if land_military >= 3 {
            1
        } else {
            0
        };
        if field_support >= desired_support {
            return -10_000.0;
        }

        let existing_kinds: Vec<&str> = g
            .units
            .values()
            .filter(|candidate| candidate.owner == pid)
            .map(|candidate| candidate.kind.as_str())
            .chain(
                g.cities
                    .values()
                    .filter(|city| city.owner == pid)
                    .filter_map(|city| match city.queue.first() {
                        Some(Item::Unit { unit }) => Some(unit.as_str()),
                        _ => None,
                    }),
            )
            .collect();
        let has_capability = |effect: &str| {
            existing_kinds.iter().any(|kind| {
                g.rules.units[*kind]
                    .effects
                    .get(effect)
                    .is_some_and(|amount| *amount > 0.0)
            })
        };
        let is_breach = matches!(unit, "battering_ram" | "siege_tower");
        if (spec
            .effects
            .get("adjacent_siege_range")
            .copied()
            .unwrap_or(0.0)
            > 0.0
            && has_capability("adjacent_siege_range"))
            || (spec.effects.get("adjacent_heal").copied().unwrap_or(0.0) > 0.0
                && has_capability("adjacent_heal"))
            || (is_breach
                && existing_kinds
                    .iter()
                    .any(|kind| matches!(*kind, "battering_ram" | "siege_tower")))
        {
            return -10_000.0;
        }

        let target_cities: Vec<_> = plan
            .target_city
            .and_then(|city| g.cities.get(&city))
            .into_iter()
            .chain(g.cities.values().filter(|city| {
                city.owner != pid
                    && g.is_at_war(pid, city.owner)
                    && plan.target_city != Some(city.id)
            }))
            .collect();
        let breach_value = if is_breach {
            target_cities
                .iter()
                .filter(|city| !g.players[city.owner].techs.contains(&crate::name!("steel")))
                .map(|city| {
                    let wall_levels = city
                        .buildings
                        .iter()
                        .filter(|building| g.rules.buildings[building].outer_defense > 0)
                        .count();
                    match unit {
                        "battering_ram" if wall_levels == 1 => 760.0,
                        "siege_tower" if (1..=2).contains(&wall_levels) => 800.0,
                        _ => 0.0,
                    }
                })
                .fold(0.0_f64, f64::max)
        } else {
            0.0
        };
        let siege_range = spec
            .effects
            .get("adjacent_siege_range")
            .copied()
            .unwrap_or(0.0);
        let siege_bombard = spec
            .effects
            .get("adjacent_siege_bombard")
            .copied()
            .unwrap_or(0.0);
        let siege_value = if counts.siege > 0 {
            siege_range * 470.0 + siege_bombard * 38.0
        } else {
            0.0
        };
        let wounded = g
            .units
            .values()
            .filter(|candidate| {
                candidate.owner == pid
                    && candidate.hp < 100
                    && g.rules.units[candidate.kind].class == "military"
            })
            .count() as f64;
        let heal = spec.effects.get("adjacent_heal").copied().unwrap_or(0.0);
        let movement = spec
            .effects
            .get("adjacent_movement")
            .copied()
            .unwrap_or(0.0);
        let logistics_value = if heal > 0.0 {
            heal * 12.0 + wounded.min(4.0) * 85.0 + movement * 210.0
        } else {
            0.0
        };
        let value = breach_value.max(siege_value).max(logistics_value);
        if value > 0.0 {
            value
                + if plan.strategy == GrandStrategy::Conquest {
                    140.0
                } else {
                    0.0
                }
        } else {
            -10_000.0
        }
    }

    /// The adaptive agent normally delegates routine city queues to the
    /// lightweight governor. Reserve at most one empty queue per turn for a
    /// support capability that the active campaign and army can actually use.
    fn advanced_support_production(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if self.base.book_pos < 4
            || !g
                .players
                .iter()
                .any(|other| other.id != pid && g.is_at_war(pid, other.id))
        {
            return;
        }
        let counts = self.counts(g, pid);
        let best: Option<(f64, u32, String)> = {
            let _memo = g.query_memo();
            let mut best = None;
            for city in g
                .cities
                .values()
                .filter(|city| city.owner == pid && city.queue.is_empty())
            {
                for item in g.producible_items(pid, city.id) {
                    let Item::Unit { unit } = item else { continue };
                    if g.rules.units[&unit].class != "support"
                        || unit == "military_engineer"
                    {
                        continue;
                    }
                    let value = self.production_value(
                        g,
                        pid,
                        city.id,
                        &Item::Unit { unit: unit.clone() },
                        plan,
                        &counts,
                    );
                    if best.as_ref().is_none_or(
                        |(old, old_city, old_unit): &(f64, u32, String)| {
                            value > *old + 1e-9
                                || ((value - *old).abs() < 1e-9
                                    && (city.id, unit.as_str())
                                        < (*old_city, old_unit.as_str()))
                        },
                    ) {
                        best = Some((value, city.id, unit.to_string()));
                    }
                }
            }
            best
        };
        let Some((value, city, unit)) = best else {
            return;
        };
        if value > 0.0 {
            let _ = g.apply(
                pid,
                &Action::Produce {
                    city,
                    item: Item::Unit { unit: Name::new(&unit) },
                },
            );
        }
    }

    /// A live strategic pivot must reach city queues, not only policies and
    /// unit orders. Pause repeatable economic projects when Conquest or
    /// Recovery has a real land-force gap; item progress remains banked and
    /// can resume after the emergency. One-off and victory projects are never
    /// interrupted here.
    fn redirect_repeatable_projects_for_force_gap(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !matches!(
            plan.strategy,
            GrandStrategy::Conquest | GrandStrategy::Recovery
        ) {
            return;
        }
        let city_ids = g.player_city_ids(pid);
        let desired_land = 2 * city_ids.len();
        for cid in city_ids {
            let counts = self.counts(g, pid);
            let land = counts
                .military
                .saturating_sub(counts.naval + counts.aircraft);
            if land >= desired_land {
                return;
            }
            let Some(Item::Project { project }) = g.cities[&cid].queue.first() else {
                continue;
            };
            let project = project.clone();
            let spec = &g.rules.projects[&project];
            if !spec.repeatable
                || (spec.completion_gpp.is_empty() && spec.ongoing_yields.is_empty())
            {
                continue;
            }
            let best = {
                let _memo = g.query_memo();
                g.producible_items(pid, cid)
                    .into_iter()
                    .filter(|item| {
                        let Item::Unit { unit } = item else {
                            return false;
                        };
                        let unit = &g.rules.units[unit];
                        unit.class == "military"
                            && unit.domain.as_deref() != Some("sea")
                            && unit.domain.as_deref() != Some("air")
                    })
                    .map(|item| {
                        let score = self.production_value(g, pid, cid, &item, plan, &counts);
                        (score, std::cmp::Reverse(format!("{item:?}")), item)
                    })
                    .max_by(|left, right| {
                        left.0
                            .total_cmp(&right.0)
                            .then_with(|| left.1.cmp(&right.1))
                    })
            };
            if let Some((score, _, item)) = best {
                if score > 0.0 {
                    let _ = g.apply(pid, &Action::Produce { city: cid, item });
                }
            }
        }
    }

    fn advanced_production(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        let mut counts = self.counts(g, pid);
        let city_ids = g.player_city_ids(pid);
        for cid in city_ids {
            // What this city is already committed to, and what that is worth
            // *now*. Without preemption a non-empty queue is skipped outright,
            // so `production_value` is only ever consulted on an idle city.
            let committed: Option<(f64, Item)> = g.cities[&cid].queue.first().cloned().map(|item| {
                let value = self.production_value(g, pid, cid, &item, plan, &counts);
                (value, item)
            });
            if committed.is_some() && self.preempt_margin <= 1.0 {
                continue;
            }
            let best: Option<(f64, String, Item)> = {
                let _memo = g.query_memo();
                let mut best = None;
                for item in g.producible_items(pid, cid) {
                    if let Item::Project { project } = &item {
                        let spec = &g.rules.projects[project];
                        let already_queued_elsewhere = !spec.repeatable
                            && g.cities.values().any(|city| {
                                city.owner == pid
                                    && city.id != cid
                                    && matches!(
                                        city.queue.first(),
                                        Some(Item::Project { project: queued }) if queued == project
                                    )
                            });
                        if already_queued_elsewhere {
                            continue;
                        }
                    }
                    let score = self.production_value(g, pid, cid, &item, plan, &counts);
                    let key = format!("{item:?}");
                    let replace = best
                        .as_ref()
                        .map(|(old, old_key, _): &(f64, String, Item)| {
                            score > *old + 1e-9
                                || ((score - *old).abs() < 1e-9 && key < *old_key)
                        })
                        .unwrap_or(true);
                    if replace {
                        best = Some((score, key, item));
                    }
                }
                best
            };
            if let Some((score, _, item)) = best {
                // Switching is close to free in this engine: `City::production_progress`
                // banks a paused build's progress by item key, so an abandoned
                // item resumes where it stopped. The margin is what stops a
                // city oscillating between two nearly equal candidates.
                let displaces_commitment = match &committed {
                    Some((current, current_item)) => {
                        *current_item != item && score > *current * self.preempt_margin
                    }
                    None => true,
                };
                if displaces_commitment
                    && score > -1_000.0
                    && g.apply(
                        pid,
                        &Action::Produce {
                            city: cid,
                            item: item.clone(),
                        },
                    )
                    .is_ok()
                {
                    counts.add_item(g, &item);
                }
            }
        }
    }

    /// Basil II's timing attack: bank Hippodrome production just short of
    /// completion, finish the empire-wide batch when Divine Right unlocks the
    /// Tagma, then exploit Chivalry to build a decisive second wave.
    fn byzantium_tagma_production(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if !g.has_ability(pid, "taxis") {
            return;
        }
        if g.players[pid].religion.is_none() {
            self.religious_production(g, pid);
            return;
        }
        let divine_right = g.players[pid].civics.contains(&crate::name!("divine_right"));
        let city_ids = g.player_city_ids(pid);
        let tagma = Item::Unit {
            unit: crate::name!("tagma"),
        };
        let existing_tagmata = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|unit| g.units[unit].kind == "tagma")
            .count();
        let queued_tagmata = city_ids
            .iter()
            .filter(|city| g.cities[city].queue.first() == Some(&tagma))
            .count();
        let target_tagmata = city_ids.len().saturating_mul(2).saturating_add(2);
        let mut committed_tagmata = existing_tagmata + queued_tagmata;

        for city in city_ids {
            let has_hippodrome = g.city_has_district_family(&g.cities[&city], crate::name!("hippodrome"));
            if !has_hippodrome {
                let hippodrome = g
                    .producible_items(pid, city)
                    .into_iter()
                    .find(|item| {
                        matches!(item, Item::District { district, .. } if district == "hippodrome")
                    });
                if let Some(hippodrome) = hippodrome {
                    let production = g.city_yields(city).production.max(1.0);
                    let remaining = g.item_remaining_cost_for_city(pid, city, &hippodrome);
                    let staged = !divine_right && remaining <= production * 1.15;
                    let building_hippodrome =
                        g.cities[&city].queue.first() == Some(&hippodrome);
                    if divine_right || !staged {
                        if !building_hippodrome {
                            let _ = g.apply(
                                pid,
                                &Action::Produce {
                                    city,
                                    item: hippodrome,
                                },
                            );
                        }
                        continue;
                    }
                    if building_hippodrome {
                        let counts = self.counts(g, pid);
                        let alternate = g
                            .producible_items(pid, city)
                            .into_iter()
                            .filter(|item| {
                                !matches!(
                                    item,
                                    Item::District { district, .. } if district == "hippodrome"
                                )
                            })
                            .map(|item| {
                                let cavalry = matches!(
                                    &item,
                                    Item::Unit { unit }
                                        if g.rules.units[unit].promotion_class == "heavy_cavalry"
                                );
                                let value =
                                    self.production_value(g, pid, city, &item, plan, &counts)
                                        + if cavalry { 500.0 } else { 0.0 };
                                (value, std::cmp::Reverse(format!("{item:?}")), item)
                            })
                            .max_by(|left, right| {
                                left.0
                                    .total_cmp(&right.0)
                                    .then_with(|| left.1.cmp(&right.1))
                            });
                        if let Some((_, _, item)) = alternate {
                            let _ = g.apply(pid, &Action::Produce { city, item });
                        }
                    }
                }
                continue;
            }
            if divine_right
                && committed_tagmata < target_tagmata
                && g.can_produce(pid, city, &tagma)
                && g.cities[&city].queue.first() != Some(&tagma)
                && g
                    .apply(
                        pid,
                        &Action::Produce {
                            city,
                            item: tagma.clone(),
                        },
                    )
                    .is_ok()
            {
                committed_tagmata += 1;
            }
        }
    }

    fn production_value(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
        plan: &StrategicPlan,
        counts: &EmpireCounts,
    ) -> f64 {
        let city = &g.cities[&cid];
        let city_count = g.player_city_ids(pid).len();
        let production = g.city_yields(cid).production.max(1.0);
        let turns = g.item_remaining_cost_for_city(pid, cid, item) / production;
        let remaining_turns = g.max_turns.saturating_sub(g.turn).max(1) as f64;
        let threatened = plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4);
        let desired_military = match plan.strategy {
            GrandStrategy::Conquest => 2 * city_count,
            GrandStrategy::Recovery => 2 * city_count,
            _ => city_count,
        };
        // A rush is fought out of one or two cities, so `2 * city_count` asks
        // for two units when the siege needs four and the census measures it
        // fielding 2.5 melee at turn 50 with 1.1 of them anywhere near the
        // objective. Ask for the stack plus one left at home; the lane shuts
        // itself at `RUSH_WINDOW_CLOSES`, so this cannot become a standing
        // military appetite.
        let desired_military = if plan.rush {
            desired_military.max(RUSH_ARMY)
        } else {
            desired_military
        };
        let raw = match item {
            Item::Unit { unit } if unit == "settler" => {
                let site = self.best_settle_site(g, pid, city.pos, 11).or_else(|| {
                    g.players[pid]
                        .techs
                        
.contains(&crate::name!("shipbuilding"))
                        .then(|| {
                            self.best_settle_site(g, pid, city.pos, g.map.width + g.map.height)
                        })
                        .flatten()
                });
                let expansion_open = if self.expansion_pays_back {
                    self.expansion_pays_back_for(g, pid, cid)
                } else if self.victory_target.is_some() {
                    g.turn < g.standard_duration(175)
                } else {
                    Self::expansion_window_open(g)
                };
                // One settler at a time empire-wide unless the treatment is
                // on. The clause above already caps cities-plus-settlers at
                // the target, so `parallel_settlers` widens the *rate* and
                // never the total: a seat two cities short may walk two.
                let in_flight_allowed = if self.parallel_settlers {
                    plan.desired_cities.saturating_sub(city_count).max(1)
                } else {
                    1
                };
                if city_count + counts.settlers < plan.desired_cities
                    && counts.settlers < in_flight_allowed
                    && city.pop >= 2
                    && expansion_open
                    && site.is_some()
                {
                    (920.0 + site.map(|(_, v)| v * 4.0).unwrap_or(0.0)) * self.settler_price
                } else {
                    -10_000.0
                }
            }
            Item::Unit { unit } if unit == "builder" => {
                let desired = city_count.div_ceil(2).max(1);
                if counts.builders < desired {
                    260.0 + 35.0 * (desired - counts.builders) as f64
                } else {
                    25.0
                }
            }
            Item::Unit { unit } if unit == "trader" => {
                let open_capacity = g
                    .trade_capacity(pid)
                    .saturating_sub(g.active_routes(pid))
                    .max(0) as usize;
                let usable_capacity = open_capacity.min(self.trade_route_opportunity_count(g, pid));
                if counts.traders < usable_capacity {
                    let opportunity = self
                        .best_trade_route_origin(g, pid, city.pos, plan.strategy)
                        .map(|(value, _)| value)
                        .unwrap_or(0.0);
                    280.0
                        + opportunity.max(0.0) * 18.0
                        + usable_capacity.saturating_sub(counts.traders) as f64 * 45.0
                } else {
                    -10_000.0
                }
            }
            Item::Unit { unit } if unit == "spy" => {
                let active = g.spies.values().filter(|spy| spy.owner == pid).count();
                let strategic = match plan.strategy {
                    GrandStrategy::Science | GrandStrategy::Culture => 850.0,
                    GrandStrategy::Diplomacy | GrandStrategy::Conquest => 1_050.0,
                    GrandStrategy::Recovery => 650.0,
                    _ => 500.0,
                };
                if active < g.spy_capacity(pid).max(0) as usize {
                    1_500.0 + strategic + active as f64 * 90.0
                } else {
                    -10_000.0
                }
            }
            Item::Unit { unit } if unit == "missionary" => {
                if self.victory_target.is_some()
                    && self.victory_target != Some(VictoryTarget::Religion)
                {
                    -10_000.0
                } else if g.players[pid].religion.is_some() && counts.missionaries < 2 {
                    150.0
                } else {
                    -10_000.0
                }
            }
            Item::Unit { unit } if unit == "archaeologist" => {
                let active = g
                    .units
                    .values()
                    .any(|unit| unit.owner == pid && unit.kind == "archaeologist");
                let sites = g.excavation_sites(pid).len();
                if plan.strategy == GrandStrategy::Culture && !active && sites > 0 {
                    2_700.0 + sites.min(3) as f64 * 180.0
                } else {
                    -10_000.0
                }
            }
            Item::Unit { unit } if unit == "military_engineer" => {
                let engineering_districts = g
                    .cities
                    .values()
                    .filter(|candidate| candidate.owner == pid)
                    .filter(|candidate| {
                        matches!(
                            candidate.queue.first(),
                            Some(Item::District { district, .. })
                                if matches!(
                                    g.district_family(*district).as_str(),
                                    "aqueduct" | "canal" | "dam"
                                )
                        )
                    })
                    .count();
                if engineering_districts > counts.military_engineers {
                    390.0 + engineering_districts as f64 * 70.0
                } else {
                    -10_000.0
                }
            }
            Item::Formation { unit, formation } => {
                let spec = &g.rules.units[unit];
                let naval = spec.domain.as_deref() == Some("sea");
                let desired = if naval {
                    BasicAi::desired_navy(g, pid)
                } else {
                    desired_military
                };
                let current = if naval {
                    counts.naval
                } else {
                    counts
                        .military
                        .saturating_sub(counts.naval + counts.aircraft)
                };
                let effective_power = spec.strength.max(spec.ranged_attack_strength())
                    + if *formation >= 2 { 17.0 } else { 10.0 };
                effective_power
                    * if current < desired || threatened {
                        4.25
                    } else {
                        0.75
                    }
                    + if threatened { 240.0 } else { 0.0 }
                    + if plan.strategy == GrandStrategy::Conquest {
                        160.0
                    } else {
                        0.0
                    }
            }
            Item::Unit { unit } => {
                let spec = &g.rules.units[unit];
                if spec.class == "military" {
                    let naval = spec.domain.as_deref() == Some("sea");
                    let aircraft = spec.domain.as_deref() == Some("air");
                    let desired_naval = BasicAi::desired_navy(g, pid);
                    let desired_aircraft = if plan.strategy == GrandStrategy::Conquest {
                        city_count.max(1)
                    } else {
                        city_count.div_ceil(2).max(1)
                    };
                    let land_military = counts
                        .military
                        .saturating_sub(counts.naval + counts.aircraft);
                    if naval && !BasicAi::city_is_coastal(g, cid) {
                        return -10_000.0;
                    }
                    let domain_saturated = if naval {
                        counts.naval >= desired_naval
                    } else if aircraft {
                        counts.aircraft >= desired_aircraft
                    } else {
                        land_military >= desired_military
                    };
                    if self.victory_target.is_some()
                        && self.victory_target != Some(VictoryTarget::Domination)
                        && domain_saturated
                        && !threatened
                    {
                        return -2_000.0;
                    }
                    if unit == "scout" && counts.scouts >= 1 {
                        return -2_000.0;
                    }
                    let power = spec.strength.max(spec.ranged_attack_strength());
                    let best_role_power = g
                        .rules
                        .units
                        .iter()
                        .filter(|(name, candidate)| {
                            candidate.class == "military"
                                && candidate.domain == spec.domain
                                && candidate.has_ranged_attack() == spec.has_ranged_attack()
                                && g.can_produce(
                                    pid,
                                    cid,
                                    &Item::Unit {
                                        unit: (*name).clone(),
                                    },
                                )
                        })
                        .map(|(_, candidate)| {
                            candidate.strength.max(candidate.ranged_attack_strength())
                        })
                        .fold(0.0_f64, f64::max);
                    if unit != "scout" && power + 5.0 < best_role_power {
                        return -2_000.0;
                    }
                    let force_gap = if naval {
                        desired_naval.saturating_sub(counts.naval) as f64
                    } else if aircraft {
                        desired_aircraft.saturating_sub(counts.aircraft) as f64
                    } else {
                        desired_military.saturating_sub(land_military) as f64
                    };
                    let role_gap = if force_gap <= 0.0 {
                        0.0
                    } else if naval {
                        match spec.promotion_class.as_str() {
                            "naval_melee" => {
                                (counts.naval_melee <= counts.naval_ranged + counts.naval_raider)
                                    as i32 as f64
                                    * 80.0
                            }
                            "naval_ranged" => {
                                (counts.naval_ranged < counts.naval_melee.max(1)) as i32 as f64
                                    * 65.0
                            }
                            "naval_raider" => {
                                (counts.naval >= 2 && counts.naval_raider == 0) as i32 as f64 * 45.0
                            }
                            "naval_carrier" => {
                                if counts.aircraft > 0 && counts.carriers == 0 {
                                    55.0
                                } else {
                                    -180.0
                                }
                            }
                            _ => 0.0,
                        }
                    } else if aircraft {
                        0.0
                    } else if spec.has_ranged_attack() {
                        (counts.melee > counts.ranged) as i32 as f64 * 55.0
                    } else {
                        (counts.ranged >= counts.melee) as i32 as f64 * 55.0
                    };
                    power * if force_gap > 0.0 { 4.0 } else { 0.65 }
                        + role_gap
                        + force_gap * 58.0
                        + if threatened { 210.0 } else { 0.0 }
                        + if plan.strategy == GrandStrategy::Conquest
                            && counts.military < desired_military + 2
                        {
                            120.0
                        } else {
                            0.0
                        }
                        + if spec.siege && counts.siege == 0 && plan.target_city.is_some() {
                            95.0
                        } else {
                            0.0
                        }
                        // The rush wants melee, cheaply, now. Ranged units are
                        // measured at roughly half a melee unit's damage per
                        // production against a city (a flat -17 attacking one),
                        // they exert no zone of control so they cannot help
                        // seal the siege ring that stops a city healing 20 a
                        // turn, and they can never land the capturing blow.
                        // Siege is worth nothing inside a window in which no
                        // capital has walls.
                        + if plan.rush
                            && !naval
                            && !aircraft
                            && force_gap > 0.0
                            && spec.is_melee_capable()
                        {
                            240.0
                        } else {
                            0.0
                        }
                } else if spec.class == "support" {
                    self.support_unit_value(g, pid, cid, unit, plan, counts)
                } else {
                    20.0
                }
            }
            Item::Building { building } => {
                let spec = &g.rules.buildings[building];
                if self.victory_target.is_some()
                    && self.victory_target != Some(VictoryTarget::Culture)
                    && !spec.great_work_slots.is_empty()
                {
                    return -10_000.0;
                }
                if spec.wonder {
                    let wonder_civ = !self.civ_blind
                        && matches!(g.players[pid].civ.as_str(), "Egypt" | "China");
                    if threatened
                        || city.buildings.len() < 3
                        || turns > remaining_turns * 0.65
                        || (plan.strategy != GrandStrategy::Culture && !wonder_civ)
                    {
                        -10_000.0
                    } else {
                        self.yield_value(spec.yields, plan.strategy) * 35.0
                            + spec.housing * 30.0
                            + spec.amenity * 45.0
                            + if plan.strategy == GrandStrategy::Culture {
                                150.0
                            } else {
                                0.0
                            }
                            + if wonder_civ { 120.0 } else { 0.0 }
                    }
                } else {
                    let housing_need = (city.pop as f64 + 1.0 - g.city_housing(city)).max(0.0);
                    let amenity_need = (-g.city_amenity_surplus(city)).max(0) as f64;
                    let great_work_slots =
                        spec.great_work_slots.values().sum::<i32>().max(0) as f64;
                    let cultural_gpp = ["writer", "artist", "musician"]
                        .into_iter()
                        .map(|kind| spec.great_person_points.get(kind).copied().unwrap_or(0.0))
                        .sum::<f64>();
                    self.yield_value(spec.yields, plan.strategy) * 42.0
                        + spec.housing * (22.0 + housing_need * 18.0)
                        + spec.amenity * (30.0 + amenity_need * 22.0)
                        + great_work_slots
                            * if plan.strategy == GrandStrategy::Culture {
                                180.0
                            } else {
                                25.0
                            }
                        + cultural_gpp
                            * if plan.strategy == GrandStrategy::Culture {
                                140.0
                            } else {
                                10.0
                            }
                        + spec.effects.get("tourism").copied().unwrap_or(0.0) * 80.0
                        + if building == "monument" && g.turn < 120 {
                            240.0
                        } else {
                            0.0
                        }
                        + if building == "granary" && city.pop as f64 + 1.0 >= g.city_housing(city)
                        {
                            180.0
                        } else {
                            0.0
                        }
                        + if building.contains("walls") && threatened {
                            320.0
                        } else {
                            0.0
                        }
                }
            }
            Item::District { district, pos } => {
                let spec = &g.rules.districts[district];
                let family = g.district_family(*district);
                if family == "spaceport" && city.districts.contains_key(crate::name!("spaceport")) {
                    // Multiple Spaceports are rules-legal, but one city can
                    // execute only one project at a time. Put additional
                    // launch sites in other cities for actual parallelism.
                    return -10_000.0;
                }
                let district_count = g
                    .cities
                    .values()
                    .filter(|candidate| {
                        candidate.owner == pid
                            && candidate
                                .districts
                                .keys()
                                .any(|built| g.district_family(*built) == family)
                    })
                    .count();
                let balanced_core = if district_count * 2 < city_count {
                    match family.as_str() {
                        "campus" | "theater_square" | "commercial_hub" => 130.0,
                        "harbor" | "industrial_zone" => 90.0,
                        _ => 0.0,
                    }
                } else {
                    0.0
                };

                // Evaluate the rules engine's actual post-construction
                // housing rather than duplicating Aqueduct water rules or
                // the appeal bands used by Neighborhoods and Preserves.
                let mut developed = city.clone();
                developed.districts.insert(Name::new(district), *pos);
                let housing_gain = (g.city_housing(&developed) - g.city_housing(city)).max(0.0);
                let housing_need = (city.pop as f64 + 2.0 - g.city_housing(city)).max(0.0);
                let amenity_gain = g.district_amenity(district, *pos);
                let amenity_need = (-g.city_amenity_surplus(city)).max(0) as f64;
                let great_people = spec.great_person_points.values().sum::<f64>();
                let relevant_great_people = match plan.strategy {
                    GrandStrategy::Science => spec
                        .great_person_points
                        .get("scientist")
                        .copied()
                        .unwrap_or(0.0),
                    GrandStrategy::Culture => ["writer", "artist", "musician"]
                        .into_iter()
                        .map(|kind| spec.great_person_points.get(kind).copied().unwrap_or(0.0))
                        .sum(),
                    GrandStrategy::Religion => spec
                        .great_person_points
                        .get("prophet")
                        .copied()
                        .unwrap_or(0.0),
                    GrandStrategy::Diplomacy => spec
                        .great_person_points
                        .get("merchant")
                        .copied()
                        .unwrap_or(0.0),
                    GrandStrategy::Conquest => ["general", "admiral"]
                        .into_iter()
                        .map(|kind| spec.great_person_points.get(kind).copied().unwrap_or(0.0))
                        .sum(),
                    GrandStrategy::Expansion | GrandStrategy::Recovery => spec
                        .great_person_points
                        .get("engineer")
                        .copied()
                        .unwrap_or(0.0),
                };
                let effects = &spec.effects;
                let effect_value = effects.get("governor_titles").copied().unwrap_or(0.0) * 520.0
                    + effects.get("envoys").copied().unwrap_or(0.0)
                        * if plan.strategy == GrandStrategy::Diplomacy {
                            300.0
                        } else {
                            170.0
                        }
                    + effects
                        .get("envoy_if_adjacent_city_center")
                        .copied()
                        .unwrap_or(0.0)
                        * if g.wdist(city.pos, *pos) == 1 {
                            if plan.strategy == GrandStrategy::Diplomacy {
                                300.0
                            } else {
                                170.0
                            }
                        } else {
                            0.0
                        }
                    + effects.get("spy_defense_levels").copied().unwrap_or(0.0) * 75.0
                    + effects.get("flood_protection").copied().unwrap_or(0.0) * 160.0
                    + effects.get("drought_protection").copied().unwrap_or(0.0) * 55.0
                    + effects.get("culture_bomb").copied().unwrap_or(0.0) * 85.0
                    + effects.get("naval_passage").copied().unwrap_or(0.0)
                        * if plan.strategy == GrandStrategy::Conquest {
                            150.0
                        } else {
                            75.0
                        }
                    + effects
                        .get("gold_faith_purchase_discount_pct")
                        .copied()
                        .unwrap_or(0.0)
                        * 8.0
                    + effects
                        .get("corps_army_discount_pct")
                        .copied()
                        .unwrap_or(0.0)
                        * if plan.strategy == GrandStrategy::Conquest {
                            8.0
                        } else {
                            2.0
                        }
                    + effects.get("free_heavy_cavalry").copied().unwrap_or(0.0)
                        * if plan.strategy == GrandStrategy::Conquest {
                            380.0
                        } else {
                            180.0
                        }
                    + effects
                        .get("naval_settler_production_pct")
                        .copied()
                        .unwrap_or(0.0)
                        * if plan.strategy == GrandStrategy::Expansion {
                            4.0
                        } else {
                            1.5
                        }
                    + effects.get("naval_heal_full").copied().unwrap_or(0.0) * 90.0
                    + effects.get("naval_movement").copied().unwrap_or(0.0) * 130.0
                    + effects
                        .get("foreign_continent_loyalty")
                        .copied()
                        .unwrap_or(0.0)
                        * 22.0
                    + effects.get("tourism_after_flight").copied().unwrap_or(0.0)
                        * if plan.strategy == GrandStrategy::Culture {
                            180.0
                        } else {
                            35.0
                        }
                    + effects
                        .get("border_growth_on_great_person")
                        .copied()
                        .unwrap_or(0.0)
                        * 90.0
                    + effects.get("unlock_apprenticeship").copied().unwrap_or(0.0) * 120.0;

                let strategic_family = match (plan.strategy, family.as_str()) {
                    (GrandStrategy::Science, "spaceport") if district_count == 0 => 3_000.0,
                    (GrandStrategy::Science, "spaceport") => 250.0,
                    (GrandStrategy::Science, "campus") => 170.0,
                    (GrandStrategy::Science, "industrial_zone") => 150.0,
                    (GrandStrategy::Religion, "holy_site") => 210.0,
                    (GrandStrategy::Culture, "theater_square") => 850.0,
                    (GrandStrategy::Culture, "preserve") => 210.0,
                    (GrandStrategy::Diplomacy, "diplomatic_quarter") => 360.0,
                    (GrandStrategy::Diplomacy, "commercial_hub") => 150.0,
                    (GrandStrategy::Diplomacy, "harbor") => 130.0,
                    (GrandStrategy::Diplomacy, "theater_square") => 100.0,
                    (GrandStrategy::Conquest, "encampment") => 170.0,
                    (GrandStrategy::Conquest, "aerodrome") => 280.0,
                    (GrandStrategy::Conquest, "harbor") => 150.0,
                    (GrandStrategy::Conquest, "industrial_zone") => 160.0,
                    (GrandStrategy::Conquest, "canal") => 120.0,
                    (GrandStrategy::Recovery, "industrial_zone") => 190.0,
                    (GrandStrategy::Recovery, "dam") => 180.0,
                    (GrandStrategy::Recovery, "aqueduct") => 120.0,
                    (GrandStrategy::Expansion, "commercial_hub" | "harbor") => 90.0,
                    (GrandStrategy::Expansion, "aqueduct" | "neighborhood") => 110.0,
                    _ => 0.0,
                };
                let first_copy = match family.as_str() {
                    "government_plaza" if district_count == 0 => 420.0,
                    "diplomatic_quarter" if district_count == 0 => 180.0,
                    "aerodrome" if district_count == 0 && counts.aircraft > 0 => 260.0,
                    _ => 0.0,
                };
                let development_penalty = if spec.specialty
                    && !city.districts.is_empty()
                    && city.buildings.len() <= city.districts.len()
                {
                    -120.0
                } else {
                    0.0
                };
                self.yield_value(g.district_yields(district, *pos), plan.strategy) * 60.0
                    + self.yield_value(spec.citizen_yields, plan.strategy) * 24.0
                    + spec.defense * if threatened { 5.0 } else { 1.5 }
                    + housing_gain * (32.0 + housing_need * 18.0)
                    + amenity_gain * (55.0 + amenity_need * 35.0)
                    + spec.loyalty * if city.loyalty < 76.0 { 22.0 } else { 7.0 }
                    + spec.air_slots.max(0) as f64
                        * if plan.strategy == GrandStrategy::Conquest || counts.aircraft > 0 {
                            95.0
                        } else {
                            25.0
                        }
                    + spec.appeal
                        * if plan.strategy == GrandStrategy::Culture {
                            35.0
                        } else {
                            8.0
                        }
                    + great_people * 30.0
                    + relevant_great_people * 85.0
                    + balanced_core
                    + strategic_family
                    + first_copy
                    + effect_value
                    + development_penalty
            }
            Item::Repair { repair, .. } => {
                if repair == "district" {
                    1_500.0 + if threatened { 300.0 } else { 0.0 }
                } else {
                    1_050.0 + if threatened { 180.0 } else { 0.0 }
                }
            }
            Item::Wonder { wonder, .. } => {
                let spec = &g.rules.wonders[wonder];
                let wonder_civ = !self.civ_blind
                    && matches!(g.players[pid].civ.as_str(), "Egypt" | "China");
                let already_queued = g.cities.values().any(|other| {
                    matches!(
                        other.queue.first(),
                        Some(Item::Wonder { wonder: queued, .. }) if queued == wonder
                    )
                });
                if already_queued
                    || threatened
                    || city.buildings.len() < 2
                    || turns > remaining_turns * 0.65
                    || (plan.strategy != GrandStrategy::Culture
                        && self.victory_target != Some(VictoryTarget::Score)
                        && (!wonder_civ || self.victory_target.is_some()))
                {
                    -10_000.0
                } else {
                    self.yield_value(spec.yields, plan.strategy) * 45.0
                        + spec.housing * 30.0
                        + spec.amenity * 50.0
                        + spec.great_work_slots.values().sum::<i32>() as f64 * 40.0
                        + spec.great_person_points.values().sum::<f64>() * 18.0
                        + if plan.strategy == GrandStrategy::Culture {
                            320.0
                        } else if self.victory_target == Some(VictoryTarget::Score) {
                            180.0
                        } else {
                            0.0
                        }
                        + if wonder_civ { 120.0 } else { 0.0 }
                }
            }
            Item::Project { project } => {
                let space_race = matches!(
                    project.as_str(),
                    "launch_earth_satellite"
                        | "launch_moon_landing"
                        | "launch_mars_colony"
                        | "exoplanet_expedition"
                        | "lagrange_laser_station"
                        | "terrestrial_laser_station"
                );
                if (space_race
                    && self.victory_target.is_some()
                    && self.victory_target != Some(VictoryTarget::Science))
                    || turns > remaining_turns * 0.8
                {
                    -10_000.0
                } else {
                    let completed = g.players[pid].science_projects.len() as f64;
                    let spec = &g.rules.projects[project];
                    match project.as_str() {
                        "repair_outer_defenses" => {
                            let missing = (g.city_max_wall_hp(city) - city.wall_hp).max(0);
                            900.0 + missing as f64 * 12.0 + if threatened { 1_500.0 } else { 0.0 }
                        }
                        "repair_encampment" => {
                            let missing = (100 - city.encampment_hp).max(0)
                                + (g.city_max_wall_hp(city) - city.encampment_wall_hp).max(0);
                            700.0 + missing as f64 * 10.0 + if threatened { 1_150.0 } else { 0.0 }
                        }
                        "recommission_reactor" => {
                            if city.reactor_age <= 12 {
                                -10_000.0
                            } else {
                                // Maintenance becomes urgent as the reactor's
                                // per-turn accident risk compounds. A fresh
                                // plant must never monopolize production just
                                // because this is a repeatable project.
                                500.0 + (city.reactor_age - 10) as f64 * 75.0
                            }
                        }
                        "convert_reactor_to_coal"
                        | "convert_reactor_to_oil"
                        | "convert_reactor_to_uranium" => {
                            let (resource, stock_value, clean_value) = match project.as_str() {
                                "convert_reactor_to_coal" => ("coal", 18.0, -110.0),
                                "convert_reactor_to_oil" => ("oil", 20.0, -55.0),
                                _ => ("uranium", 55.0, 130.0),
                            };
                            450.0
                                + g.strategic_stockpile(pid, Name::new(resource)).min(50.0) * stock_value
                                + g.climate_phase as f64 * clean_value
                        }
                        "carbon_recapture" => {
                            if g.global_co2_emissions() <= f64::EPSILON
                                && plan.strategy != GrandStrategy::Diplomacy
                            {
                                -10_000.0
                            } else {
                                450.0
                                    + g.climate_phase as f64 * 260.0
                                    + (g.players[pid].co2_emissions.max(0.0) / 500.0).min(800.0)
                                    + if plan.strategy == GrandStrategy::Diplomacy {
                                        900.0
                                    } else {
                                        0.0
                                    }
                            }
                        }
                        "manhattan_project" | "operation_ivy" => {
                            if plan.strategy == GrandStrategy::Conquest {
                                2_200.0
                            } else {
                                350.0
                            }
                        }
                        "build_nuclear_device" | "build_thermonuclear_device" => {
                            if plan.strategy == GrandStrategy::Conquest {
                                2_600.0
                            } else if plan.target_player.is_some() {
                                850.0
                            } else {
                                250.0
                            }
                        }
                        _ if space_race => {
                            3_300.0
                                + completed * 220.0
                                + if plan.strategy == GrandStrategy::Science {
                                    650.0
                                } else {
                                    0.0
                                }
                        }
                        _ if !spec.completion_gpp.is_empty()
                            || !spec.ongoing_yields.is_empty()
                            || spec.full_power_while_active
                            || project == "bread_and_circuses" =>
                        {
                            self.district_project_value(g, pid, cid, project, plan)
                        }
                        // Scenario and future projects without an understood
                        // economic effect remain legal, but cannot crowd out
                        // infrastructure solely because they are repeatable.
                        _ => 180.0,
                    }
                }
            }
            Item::Product { product } => {
                let existing = g
                    .cities
                    .values()
                    .filter(|other| other.owner == pid)
                    .flat_map(|other| other.products.iter())
                    .filter(|existing| *existing == product)
                    .count() as f64;
                let strategic = match (plan.strategy, product.as_str()) {
                    (GrandStrategy::Culture, "silk" | "wine") => 2_000.0,
                    (GrandStrategy::Expansion | GrandStrategy::Recovery, "salt") => 1_650.0,
                    (GrandStrategy::Diplomacy, _) => 900.0,
                    _ => 600.0,
                };
                1_600.0 + strategic - existing * 280.0
            }
        };
        if raw <= -9_999.0 {
            return raw;
        }
        if turns > remaining_turns + 1.0 {
            return -1_500.0;
        }
        let completion_discount = if turns > remaining_turns * 0.6 {
            0.25
        } else {
            1.0
        };
        completion_discount * raw / (7.0 + turns.max(1.0))
    }

    fn settle_value(&self, g: &Game, pid: usize, pos: Pos) -> f64 {
        let tile = &g.map.tiles[&pos];
        let mut value = 0.0;
        for p in g.wdisk(pos, 2) {
            let Some(t) = g.map.get(p) else { continue };
            if t.owner_city.is_some() && p != pos {
                continue;
            }
            let y = g.rules.tile_yields(t);
            let ring_discount = if g.wdist(pos, p) <= 1 { 1.0 } else { 0.45 };
            value += ring_discount
                * (y.food * 2.0
                    + y.production * 2.2
                    + y.gold * 0.7
                    + y.science * 1.2
                    + y.culture * 1.2
                    + y.faith * 0.4);
            if let Some(resource) = &t.resource {
                value += match g.rules.resources[resource].class.as_str() {
                    "luxury" => 5.0,
                    "strategic" => 4.0,
                    _ => 1.5,
                } * ring_discount;
            }
        }
        let fresh = tile.has_river()
            || g.nbrs(pos).iter().any(|p| {
                g.map
                    .get(*p)
                    .is_some_and(|t| t.feature.as_deref() == Some("oasis"))
            });
        let coastal = g
            .nbrs(pos)
            .iter()
            .any(|p| g.map.get(*p).is_some_and(|t| g.rules.is_water(t)));
        value += if fresh {
            14.0
        } else if coastal {
            6.0
        } else {
            -5.0
        };
        let enemy_distance = g
            .cities
            .values()
            .filter(|c| c.owner != pid && !g.players[c.owner].is_barbarian)
            .map(|c| g.wdist(pos, c.pos))
            .min()
            .unwrap_or(20);
        if enemy_distance < 6 {
            value -= (6 - enemy_distance) as f64 * 6.0;
        }
        if self.defensible_sites {
            value += self.defensibility(g, pid, pos);
        }
        value
    }

    /// How well a site can be held, on the same scale `settle_value` uses for
    /// everything else. Never positive: this only ever discounts a site.
    ///
    /// Two terms, for the two things the shipped score is silent about. A camp
    /// or barbarian city nearby is what actually takes new cities, and it is
    /// filtered out of the rival-proximity penalty above. Distance from the
    /// empire's own nearest city decides whether help can arrive at all: the
    /// measured loss lands at city age ten, which is less time than a soldier
    /// needs to cross open ground.
    ///
    /// Six tiles is the same threshold the rival-proximity penalty uses, and
    /// the isolation term is capped so a first city — which has no other city
    /// to be near — cannot be discounted without bound.
    fn defensibility(&self, g: &Game, pid: usize, pos: Pos) -> f64 {
        let camp_distance = g
            .barb_camps
            .keys()
            .chain(
                g.cities
                    .values()
                    .filter(|city| g.players[city.owner].is_barbarian)
                    .map(|city| &city.pos),
            )
            .map(|camp| g.wdist(pos, *camp))
            .min()
            .unwrap_or(20);
        let exposure = match camp_distance < 6 {
            true => (6 - camp_distance) as f64 * 7.0,
            false => 0.0,
        };
        let support_distance = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.wdist(pos, g.cities[&cid].pos))
            .min()
            .unwrap_or(0);
        let isolation = match support_distance > 6 {
            true => (support_distance - 6).min(6) as f64 * 5.0,
            false => 0.0,
        };
        -(exposure + isolation)
    }

    /// Lower is a better operational objective. Unlike a nearest-city rule,
    /// this combines approach geometry, live defenses, staged forces,
    /// occupation pressure, development, and victory-denial value. It is the
    /// campaign analogue of a chess engine's move ordering: forces search the
    /// most forcing and profitable front first rather than the first legal one.
    fn campaign_city_value(
        &self,
        g: &Game,
        pid: usize,
        city: &crate::game::City,
        strategy: GrandStrategy,
    ) -> f64 {
        let core_distance = g
            .player_city_ids(pid)
            .into_iter()
            .map(|mine| g.wdist(g.cities[&mine].pos, city.pos))
            .min()
            .unwrap_or(40);
        let military_units = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|unit| g.rules.units[g.units[unit].kind].class == "military")
            .collect::<Vec<_>>();
        let city_is_coastal = g.nbrs(city.pos).into_iter().any(|position| {
            g.map
                .get(position)
                .is_some_and(|tile| g.rules.is_water(tile))
        });
        let military_distance = military_units
            .iter()
            .filter(|unit| {
                let domain = g.rules.units[g.units[unit].kind].domain.as_deref();
                domain != Some("sea") || city_is_coastal
            })
            .map(|unit| g.wdist(g.units[unit].pos, city.pos))
            .min()
            .unwrap_or(core_distance);
        let has_land_force = military_units.iter().any(|unit| {
            !matches!(
                g.rules.units[g.units[unit].kind].domain.as_deref(),
                Some("sea" | "air")
            )
        });
        let has_naval_force = military_units.iter().any(|unit| {
            g.rules.units[g.units[unit].kind].domain.as_deref() == Some("sea")
        });
        // With no army yet, value prospective land staging rather than
        // declaring every objective sealed. Once forces exist, only count
        // adjacent tiles that the relevant land or naval arm can exploit.
        let plan_land_approach = has_land_force || !has_naval_force;
        let approaches = g
            .nbrs(city.pos)
            .into_iter()
            .filter(|position| {
                g.map.get(*position).is_some_and(|tile| {
                    g.rules.is_passable(tile)
                        && if g.rules.is_water(tile) {
                            has_naval_force
                        } else {
                            plan_land_approach
                        }
                })
            })
            .count();
        let friendly_local: f64 = g
            .units
            .values()
            .filter(|unit| unit.owner == pid && g.wdist(unit.pos, city.pos) <= 7)
            .filter(|unit| g.rules.units[unit.kind].class == "military")
            .map(|unit| crate::game::effective_strength(g.unit_strength(unit, true), unit.hp))
            .sum();
        let hostile_local: f64 = g
            .units
            .values()
            .filter(|unit| unit.owner == city.owner && g.wdist(unit.pos, city.pos) <= 7)
            .filter(|unit| g.rules.units[unit.kind].class == "military")
            .map(|unit| crate::game::effective_strength(g.unit_strength(unit, true), unit.hp))
            .sum();

        let friendly_pressure: f64 = g
            .cities
            .values()
            .filter(|source| source.owner == pid)
            .filter_map(|source| {
                let distance = g.wdist(source.pos, city.pos);
                (distance <= 9).then_some(source.pop.max(1) as f64 * (10 - distance) as f64)
            })
            .sum();
        let hostile_pressure: f64 = g
            .cities
            .values()
            .filter(|source| source.owner == city.owner && source.id != city.id)
            .filter_map(|source| {
                let distance = g.wdist(source.pos, city.pos);
                (distance <= 9).then_some(source.pop.max(1) as f64 * (10 - distance) as f64)
            })
            .sum();
        let occupation_risk = (hostile_pressure - friendly_pressure).max(0.0)
            * if strategy == GrandStrategy::Conquest {
                0.7
            } else {
                1.2
            };
        // An Original Capital cannot be razed, so taking it before the
        // surrounding population is controlled creates a forced keep/flip/
        // recapture cycle. Make the rest of that population the campaign
        // objective first; the cost disappears as soon as the capital can be
        // held, or when taking it would end the war or game outright.
        let unsupported_capture = if Self::should_defer_city_capture(g, pid, city.id) {
            10_000.0
        } else {
            0.0
        };

        let defenses = g.city_strength(city.id) * 1.8
            + city.hp.max(0) as f64 * 0.12
            + city.wall_hp.max(0) as f64 * 0.16;
        let local_balance = (hostile_local - friendly_local).clamp(-250.0, 250.0) * 0.45;
        let approach_cost = (6usize.saturating_sub(approaches)) as f64 * 11.0;
        let development = city.pop.max(1) as f64 * 7.0
            + city.buildings.len() as f64 * 5.0
            + city.districts.len() as f64 * 10.0
            + city.wonders.len() as f64 * 24.0;
        let capital_value = if city.is_capital {
            if strategy == GrandStrategy::Conquest {
                180.0
            } else {
                75.0
            }
        } else {
            0.0
        };
        let science_denial = if city.districts.contains_key(crate::name!("spaceport"))
            && self.rival_victory_pressure(g, city.owner).strategy == GrandStrategy::Science
        {
            110.0
        } else {
            0.0
        };
        let recapture_value = if city.original_owner == pid {
            135.0
        } else {
            0.0
        };
        let liberation_value = if city.original_owner != city.owner
            && city.original_owner != pid
            && g.players
                .get(city.original_owner)
                .is_some_and(|founder| !founder.is_barbarian)
            && strategy == GrandStrategy::Diplomacy
        {
            120.0
        } else {
            0.0
        };

        core_distance as f64 * 7.0
            + military_distance as f64 * 5.0
            + defenses
            + local_balance
            + approach_cost
            + occupation_risk
            + unsupported_capture
            - development
            - capital_value
            - science_denial
            - recapture_value
            - liberation_value
    }

    fn settle_sites(&self, g: &Game, pid: usize, from: Pos, radius: i32) -> Vec<(Pos, f64)> {
        let mut sites = Vec::new();
        let distance_penalty = if radius > 12 { 0.45 } else { 0.9 };
        for pos in g.wdisk(from, radius) {
            let Some(tile) = g.map.get(pos) else { continue };
            if g.rules.is_water(tile)
                || !g.rules.is_passable(tile)
                || g.cities.values().any(|c| g.wdist(c.pos, pos) < 4)
                || tile
                    .owner_city
                    .is_some_and(|cid| g.cities[&cid].owner != pid)
            {
                continue;
            }
            let value =
                self.settle_value(g, pid, pos) - g.wdist(from, pos) as f64 * distance_penalty;
            if value >= 12.0 {
                sites.push((pos, value));
            }
        }
        sites.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
        sites
    }

    fn best_settle_site(&self, g: &Game, pid: usize, from: Pos, radius: i32) -> Option<(Pos, f64)> {
        self.settle_sites(g, pid, from, radius).into_iter().next()
    }

    fn best_reachable_settle_site(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        radius: i32,
    ) -> Option<(Pos, f64)> {
        let from = g.units[&uid].pos;
        let candidates = self.settle_sites(g, pid, from, radius);
        BasicAi::first_reachable_settle_site(g, uid, &candidates)
    }

    fn advanced_settler_step(&mut self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let current = g.units[&uid].pos;
        // Search only the immediate neighborhood for the capital. The target
        // is fixed after the first assessment, preventing a rolling optimum
        // from leading the settler across the map for many compounding turns.
        if g.player_city_ids(pid).is_empty() {
            let cached = self.settler_targets.get(&uid).copied().filter(|target| {
                let Some(tile) = g.map.get(*target) else {
                    return false;
                };
                !g.rules.is_water(tile)
                    && g.rules.is_passable(tile)
                    && !g.cities.values().any(|city| g.wdist(city.pos, *target) < 4)
                    && tile
                        .owner_city
                        .is_none_or(|cid| g.cities[&cid].owner == pid)
                    && (*target == current || g.route_step(uid, *target, 0).is_some())
            });
            if cached.is_none() {
                self.settler_targets.remove(&uid);
            }
            let target = cached.or_else(|| {
                let current_value = self.settle_value(g, pid, current);
                let local = self.best_reachable_settle_site(g, pid, uid, 2);
                let target = if g.can_found_city(uid) {
                    Some(
                        local
                            .filter(|(_, value)| *value > current_value + 3.0)
                            .map(|(pos, _)| pos)
                            .unwrap_or(current),
                    )
                } else {
                    local
                        .or_else(|| {
                            self.best_reachable_settle_site(g, pid, uid, g.map.width + g.map.height)
                        })
                        .or_else(|| {
                            self.base.best_reachable_settle_site(
                                g,
                                pid,
                                uid,
                                g.map.width + g.map.height,
                            )
                        })
                        .map(|(pos, _)| pos)
                };
                if let Some(target) = target {
                    self.settler_targets.insert(uid, target);
                }
                target
            });
            if target == Some(current) && g.can_found_city(uid) {
                self.settler_targets.remove(&uid);
                think!(self.journal(), Expansion, Decision, "Founding the capital at {current:?}";
                       "the site is worth {:.1}", self.settle_value(g, pid, current); current);
                return g.apply(pid, &Action::FoundCity { unit: uid }).is_ok();
            }
            if let Some(target) = target {
                think!(self.journal(), Expansion, Detail,
                       "Walking the first settler toward {target:?}";
                       "worth {:.1} against {:.1} where it stands",
                       self.settle_value(g, pid, target),
                       self.settle_value(g, pid, current); target);
                let moved = self.base.settler_step_toward(g, pid, uid, target);
                if !moved {
                    self.settler_targets.remove(&uid);
                }
                return moved;
            }
            return false;
        }
        let valid_target = self.settler_targets.get(&uid).copied().filter(|target| {
            let Some(tile) = g.map.get(*target) else {
                return false;
            };
            !g.rules.is_water(tile)
                && g.rules.is_passable(tile)
                && !g.cities.values().any(|c| g.wdist(c.pos, *target) < 4)
                && tile
                    .owner_city
                    .is_none_or(|cid| g.cities[&cid].owner == pid)
                // A momentarily unavailable route is not a bad site. Under
                // `settler_commit` the stall counter decides when to give up,
                // not a single blocked turn.
                && (*target == current
                    || g.route_step(uid, *target, 0).is_some()
                    || self.settler_commit)
        });
        let target = valid_target.or_else(|| {
            let local = self.best_reachable_settle_site(g, pid, uid, 8);
            let global = self.best_reachable_settle_site(
                g,
                pid,
                uid,
                g.map.width + g.map.height,
            );
            match (local, global) {
                (Some(local), Some(global)) if global.1 > local.1 + 5.0 => Some(global),
                (Some(local), _) => Some(local),
                (None, global) => global,
            }
            .map(|(pos, _)| {
                self.settler_targets.insert(uid, pos);
                pos
            })
        });
        let Some(target) = target else {
            return self.base.settler_step(g, pid, uid);
        };
        if current == target && g.can_found_city(uid) {
            self.settler_targets.remove(&uid);
            think!(self.journal(), Expansion, Decision, "Founding a city at {current:?}";
                   "the site is worth {:.1}; the empire holds {} cities and wants {}",
                   self.settle_value(g, pid, current),
                   g.player_city_ids(pid).len(),
                   self.plan.as_ref().map_or(0, |plan| plan.desired_cities); current);
            return g.apply(pid, &Action::FoundCity { unit: uid }).is_ok();
        }
        if let Some(escort) = g.units[&uid].linked_to.filter(|peer| {
            g.units.get(peer).is_some_and(|escort| {
                g.rules.units[escort.kind].domain.as_deref() == Some("sea")
            })
        }) {
            if g.wdist(current, target) == 1 {
                return g.apply(pid, &Action::UnlinkUnits { unit: escort }).is_ok();
            }
            return false;
        }
        think!(self.journal(), Expansion, Detail, "Settler marching to {target:?}";
               "{} tiles away, the site is worth {:.1}",
               g.wdist(current, target), self.settle_value(g, pid, target); target);
        let moved = self.base.settler_step_toward(g, pid, uid, target);
        if moved {
            self.settler_stalls.remove(&uid);
        } else if self.settler_commit {
            let stalls = self.settler_stalls.entry(uid).or_insert(0);
            *stalls += 1;
            if *stalls >= SETTLER_STALL_LIMIT {
                self.settler_targets.remove(&uid);
                self.settler_stalls.remove(&uid);
            }
        } else {
            self.settler_targets.remove(&uid);
        }
        moved
    }

    fn improvement_value(
        &self,
        g: &Game,
        pos: Pos,
        improvement: &str,
        strategy: GrandStrategy,
    ) -> f64 {
        let tile = &g.map.tiles[&pos];
        let spec = &g.rules.improvements[improvement];
        let appeal = g.tile_appeal(pos).max(0) as f64;
        let mut yields = spec.yields;
        yields.gold += spec.effects.get("appeal_gold").copied().unwrap_or(0.0) * appeal;
        let mut value = self.yield_value(yields, strategy);
        if strategy == GrandStrategy::Culture {
            // Tourism is cumulative: delaying a resort or national park by
            // dozens of turns loses visitors that cannot be recovered by an
            // equivalent late-game yield. Treat it as a durable strategic
            // yield so builders seek tourist sites as soon as they unlock.
            let tourism = spec.effects.get("tourism").copied().unwrap_or(0.0)
                + spec.effects.get("appeal_tourism").copied().unwrap_or(0.0) * appeal;
            value += tourism * 35.0;
        }
        if let Some(resource) = &tile.resource {
            // Only the improvement that actually works the resource connects
            // it. A Farm on an Iron hill was scoring the same premium as the
            // Mine, so the deposit read as already handled and no builder ever
            // came back for it.
            let worked = spec.resources.iter().any(|entry| entry == resource);
            value += match g.rules.resources[resource].class.as_str() {
                "luxury" => 14.0 * worked as i32 as f64,
                // A strategic deposit is not a yield: it is the empire's only
                // supply of the material every modern unit costs to build and
                // to upgrade into, and the stockpile it feeds is capped at 50,
                // so a second source still earns its Builder charge. Opening
                // one outranks any ordinary tile improvement in the game.
                "strategic" if worked => 30.0,
                "strategic" => 0.0,
                _ => 4.0,
            };
        }
        value
    }

    /// Return only improvements that genuinely upgrade the tile. The game
    /// permits builders to replace an existing improvement, so comparing
    /// candidates in isolation made late-game builders oscillate between a
    /// high-value resort and a lower-value farm on successive turns.
    fn worthwhile_improvements(
        &self,
        g: &Game,
        pid: usize,
        pos: Pos,
        strategy: GrandStrategy,
    ) -> Vec<Name> {
        let current_value = g.map.tiles[&pos]
            .improvement
            .as_deref()
            .map(|improvement| self.improvement_value(g, pos, improvement, strategy))
            .unwrap_or(0.0);
        // Score each candidate once and sort the scores. The comparator used
        // to re-derive both sides of every comparison, so ranking eight
        // improvements valued a tile's appeal and resource close to sixty
        // times instead of eight. Same order, same ties.
        let mut choices: Vec<(f64, Name)> = g
            .valid_improvements(pid, pos)
            .into_iter()
            .filter(|improvement| g.rules.improvements[improvement].builder_buildable)
            .map(|improvement| {
                let value = self.improvement_value(g, pos, &improvement, strategy);
                (value, improvement)
            })
            .filter(|(value, _)| *value > current_value + 0.5)
            .collect();
        choices.sort_by(|(a_value, a), (b_value, b)| {
            b_value.partial_cmp(a_value).unwrap().then(a.cmp(b))
        });
        choices
            .into_iter()
            .map(|(_, improvement)| improvement)
            .collect()
    }

    fn advanced_builder_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        strategy: GrandStrategy,
    ) -> bool {
        let current = g.units[&uid].pos;
        let project = g
            .player_city_ids(pid)
            .into_iter()
            .filter_map(|city| {
                g.project_contribution_target(pid, city)
                    .map(|position| (g.wdist(current, position), position, city))
            })
            .min();
        if let Some((_, position, city)) = project {
            self.builder_targets.remove(&uid);
            if current == position && g.can_contribute_project(pid, uid, city) {
                return g
                    .apply(pid, &Action::ContributeProject { unit: uid, city })
                    .is_ok();
            }
            if self.base.step_toward(g, pid, uid, position) {
                return true;
            }
        }
        let repairable = g.map.get(current).is_some_and(|tile| {
            tile.pillaged
                && tile.improvement.is_some()
                && tile
                    .owner_city
                    .and_then(|city| g.cities.get(&city))
                    .is_some_and(|city| city.owner == pid)
        });
        if repairable {
            self.builder_targets.remove(&uid);
            return g
                .apply(pid, &Action::RepairImprovement { unit: uid })
                .is_ok();
        }
        let here = self.worthwhile_improvements(g, pid, current, strategy);
        if let Some(improvement) = here.first() {
            self.builder_targets.remove(&uid);
            think!(self.journal(), Expansion, Detail,
                   "Building a {} at {current:?}", plain(improvement);
                   "worth {:.1} to the {} plan, best of {} that fit this tile",
                   self.improvement_value(g, current, improvement, strategy),
                   strategy.as_str(), here.len(); current);
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
        let reserved: HashSet<Pos> = self
            .builder_targets
            .iter()
            .filter(|(other, _)| **other != uid && g.units.contains_key(other))
            .map(|(_, pos)| *pos)
            .collect();
        // Reading every tile the empire owns: one memo scope so the
        // empire-wide questions each tile asks are answered once for the whole
        // sweep rather than once per tile. The borrow checker rejects the
        // guard the moment anything in here starts mutating the game.
        let best = {
            let _memo = g.query_memo();
            let current_target = self.builder_targets.get(&uid).copied().filter(|pos| {
                !reserved.contains(pos)
                    && !self
                        .worthwhile_improvements(g, pid, *pos, strategy)
                        .is_empty()
            });
            match current_target {
                Some(pos) => Ok(pos),
                None => {
                    let mut best: Option<(f64, Pos)> = None;
                    for cid in g.player_city_ids(pid) {
                        for pos in &g.cities[&cid].owned_tiles {
                            if reserved.contains(pos) {
                                continue;
                            }
                            for improvement in self.worthwhile_improvements(g, pid, *pos, strategy)
                            {
                                let score = self.improvement_value(g, *pos, &improvement, strategy)
                                    - g.wdist(current, *pos) as f64 * 0.7;
                                if best
                                    .map(|(old, bp)| score > old || (score == old && *pos < bp))
                                    .unwrap_or(true)
                                {
                                    best = Some((score, *pos));
                                }
                            }
                        }
                    }
                    Err(best.map(|(_, pos)| pos))
                }
            }
        };
        let target = match best {
            Ok(pos) => Some(pos),
            Err(found) => found.map(|pos| {
                self.builder_targets.insert(uid, pos);
                pos
            }),
        };
        target.is_some_and(|pos| self.base.step_toward(g, pid, uid, pos))
    }

    fn advanced_trader_step(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        strategy: GrandStrategy,
    ) -> bool {
        let current = g.units[&uid].pos;
        if let Some(origin) = g.city_at(current).filter(|cid| g.cities[cid].owner == pid) {
            if let Some((_, city)) = self.best_trade_route_destination(g, pid, origin, strategy) {
                return g
                    .apply(pid, &Action::TradeRoute { unit: uid, city })
                    .is_ok();
            }
        }
        let Some((_, origin)) = self.best_trade_route_origin(g, pid, current, strategy) else {
            return false;
        };
        self.base.step_toward(g, pid, uid, g.cities[&origin].pos)
    }

    fn best_trade_route_destination(
        &self,
        g: &Game,
        pid: usize,
        origin: u32,
        strategy: GrandStrategy,
    ) -> Option<(f64, u32)> {
        g.cities
            .values()
            .filter(|city| g.can_establish_trade_route(pid, origin, city.id))
            .map(|city| {
                (
                    self.trade_route_destination_value(g, pid, city, strategy),
                    city.id,
                )
            })
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| right.1.cmp(&left.1))
            })
    }

    /// Count destinations, not origin/destination pairs: final-patch Civ VI
    /// permits only one active route to a destination per empire.
    fn trade_route_opportunity_count(&self, g: &Game, pid: usize) -> usize {
        let origins = g.player_city_ids(pid);
        g.cities
            .values()
            .filter(|destination| {
                origins
                    .iter()
                    .any(|origin| g.can_establish_trade_route(pid, *origin, destination.id))
            })
            .count()
    }

    /// Best city in which an idle or newly completed Trader can begin a
    /// legal route. Travel time prevents a slightly richer distant route from
    /// delaying economic output indefinitely.
    fn best_trade_route_origin(
        &self,
        g: &Game,
        pid: usize,
        from: Pos,
        strategy: GrandStrategy,
    ) -> Option<(f64, u32)> {
        g.player_city_ids(pid)
            .into_iter()
            .filter_map(|origin| {
                self.best_trade_route_destination(g, pid, origin, strategy)
                    .map(|(value, _)| {
                        (
                            value - g.wdist(from, g.cities[&origin].pos) as f64 * 1.5,
                            origin,
                        )
                    })
            })
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| right.1.cmp(&left.1))
            })
    }

    fn trade_route_destination_value(
        &self,
        g: &Game,
        pid: usize,
        city: &crate::game::City,
        strategy: GrandStrategy,
    ) -> f64 {
        let mut value = self.yield_value(g.trade_route_yields(pid, city.id), strategy);
        if let Some(alliance) = g.alliance_with(pid, city.owner) {
            let mut yields = Yields::default();
            match alliance.kind.as_str() {
                "research" => yields.science = 2.0,
                "cultural" => yields.culture = 2.0,
                "economic" => yields.gold = 4.0,
                "religious" => yields.faith = 2.0,
                _ => {}
            }
            value += self.yield_value(yields, strategy);
            let already_connected = g.routes.iter().any(|route| {
                route.owner == pid
                    && route.ends > g.turn
                    && g.cities
                        .get(&route.dest)
                        .is_some_and(|destination| destination.owner == city.owner)
            });
            if !already_connected {
                // The first route in each direction accelerates alliance XP;
                // later duplicate routes should compete on their yields.
                value += 45.0;
            }
            if alliance.kind == "cultural" && alliance.level >= 2 {
                value += 18.0;
            }
        }
        let objective = self
            .victory_target
            .map(VictoryTarget::strategy)
            .unwrap_or(strategy);
        // One route unlocks the entire empire's +25% Tourism pressure against
        // that civilization (+75% with Online Communities). Duplicate routes
        // do not stack, so Culture agents connect every rival before
        // optimizing the route's ordinary yields.
        if objective == GrandStrategy::Culture
            && city.owner != pid
            && !g.has_tourism_trade_route(pid, city.owner)
        {
            let modifier = 25.0 + g.policy_effect(pid, "trade_partner_tourism_pct");
            value += 12.0 + g.tourism_per_turn(pid).min(400.0) * modifier / 100.0;
        }
        value
    }

    fn advanced_missionary_step(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        offensive: bool,
    ) -> bool {
        let Some(religion) = g.players[pid].religion.clone() else {
            return false;
        };
        let current = g.units[&uid].pos;
        let mut targets: Vec<(i32, std::cmp::Reverse<u32>, Pos)> = g
            .cities
            .values()
            .filter(|city| {
                Self::city_needs_religious_support(g, pid, city, &religion)
                    || (offensive
                        && city.owner != pid
                        && !g.is_at_war(pid, city.owner)
                        && g.city_religion(city) != Some(religion.as_str()))
            })
            .map(|city| {
                let own_pressure = city.pressure.get(&religion).copied().unwrap_or(0.0);
                let rival_pressure = city
                    .pressure
                    .iter()
                    .filter(|(belief, _)| belief.as_str() != religion)
                    .map(|(_, pressure)| *pressure)
                    .fold(0.0_f64, f64::max);
                let swing = (rival_pressure - own_pressure).clamp(0.0, 500.0) as i32;
                let foreign = (city.owner != pid) as i32;
                let defensive_conversion = (city.owner == pid) as i32 * 170;
                let score = defensive_conversion
                    + foreign * 90
                    + city.pop * 12
                    + city.is_capital as i32 * 18
                    + swing / 10
                    - g.wdist(current, city.pos) * 4;
                (score, std::cmp::Reverse(city.id), city.pos)
            })
            .collect();
        targets.sort_by(|left, right| right.cmp(left));
        for (_, _, target) in targets {
            if g.wdist(current, target) <= 1 {
                return g.apply(pid, &Action::Spread { unit: uid }).is_ok();
            }
            if self.base.step_toward_range(g, pid, uid, target, 1) {
                return true;
            }
        }
        false
    }

    fn advanced_religious_step(&self, g: &mut Game, pid: usize, uid: u32, offensive: bool) -> bool {
        let unit = g.units[&uid].clone();
        let religion = unit
            .religion
            .clone()
            .or_else(|| g.players[pid].religion.clone());
        let legal = g.legal_actions_within(pid, ActionFamilies::UNITS);

        // A lost core city is more urgent than another enhancer or worship
        // belief. Preserve this Apostle until it reaches the Holy City, then
        // launch the inquisition before the rival can close out the match.
        let needs_inquisition = unit.kind == "apostle"
            && unit.religion == g.players[pid].religion
            && g.players[pid]
                .counters
                .get("inquisition")
                .copied()
                .unwrap_or(0)
                == 0
            && religion.as_ref().is_some_and(|faith| {
                g.player_city_ids(pid)
                    .iter()
                    .any(|city| g.city_religion(&g.cities[city]) != Some(faith.as_str()))
            });
        if needs_inquisition {
            if let Some(action) = legal
                .iter()
                .find(|action| matches!(action, Action::LaunchInquisition { unit } if *unit == uid))
                .cloned()
            {
                return g.apply(pid, &action).is_ok();
            }
            if let Some(target) = g.players[pid]
                .holy_city
                .and_then(|city| g.cities.get(&city).map(|city| city.pos))
            {
                return self.base.step_toward(g, pid, uid, target);
            }
        }

        if unit.kind == "apostle" && g.players[pid].religion_beliefs.len() < 4 {
            let objective = if g.has_ability(pid, "taxis") {
                GrandStrategy::Conquest
            } else {
                self.victory_target
                    .map(VictoryTarget::strategy)
                    .unwrap_or(GrandStrategy::Religion)
            };
            let evangelize = legal
                .iter()
                .filter_map(|action| match action {
                    Action::EvangelizeBelief { unit, belief } if *unit == uid => {
                        let score = match (objective, belief.as_str()) {
                            (GrandStrategy::Science, "wat")
                            | (GrandStrategy::Culture, "cathedral")
                            | (GrandStrategy::Diplomacy, "pagoda")
                            | (GrandStrategy::Conquest, "just_war")
                            | (GrandStrategy::Expansion, "religious_colonization")
                            | (GrandStrategy::Religion, "holy_order") => 300,
                            (GrandStrategy::Conquest, "meeting_house")
                            | (GrandStrategy::Expansion, "gurdwara")
                            | (GrandStrategy::Religion, "mosque") => 240,
                            (_, "holy_order" | "mosque" | "wat" | "pagoda") => 180,
                            _ => 100,
                        };
                        Some((score, std::cmp::Reverse(belief.clone()), action.clone()))
                    }
                    _ => None,
                })
                .max_by_key(|(score, belief, _)| (*score, belief.clone()));
            if let Some((_, _, action)) = evangelize {
                return g.apply(pid, &action).is_ok();
            }
        }

        if unit.kind == "guru" {
            if let Some(action) = legal
                .iter()
                .find(|action| matches!(action, Action::HealReligious { unit } if *unit == uid))
                .cloned()
            {
                return g.apply(pid, &action).is_ok();
            }
        }
        if unit.kind == "inquisitor" {
            if let Some(action) = legal
                .iter()
                .find(|action| matches!(action, Action::RemoveHeresy { unit } if *unit == uid))
                .cloned()
            {
                return g.apply(pid, &action).is_ok();
            }
        }

        let theological = legal
            .iter()
            .filter_map(|action| match action {
                Action::TheologicalAttack { unit, target } if *unit == uid => {
                    let defender_hp = g
                        .units_at(*target)
                        .into_iter()
                        .filter(|other| {
                            let other = &g.units[other];
                            g.rules.units[other.kind].class == "religious"
                                && other.religion != religion
                        })
                        .map(|other| g.units[&other].hp)
                        .min()
                        .unwrap_or(100);
                    Some((100 - defender_hp, *target, action.clone()))
                }
                _ => None,
            })
            .max_by_key(|(score, target, _)| (*score, std::cmp::Reverse(*target)));
        if let Some((score, _, action)) = theological {
            if unit.hp >= 55 || score >= 45 {
                return g.apply(pid, &action).is_ok();
            }
        }

        if g.rules.units[unit.kind].religious_spread > 0.0 && unit.charges > 0 {
            return self.advanced_missionary_step(g, pid, uid, offensive);
        }

        let target = g
            .units
            .values()
            .filter(|other| {
                other.owner != pid
                    && g.rules.units[other.kind].class == "religious"
                    && other.religion != religion
            })
            .min_by_key(|other| (g.wdist(unit.pos, other.pos), other.id))
            .map(|other| other.pos)
            .or_else(|| {
                g.players[pid]
                    .holy_city
                    .and_then(|cid| g.cities.get(&cid).map(|city| city.pos))
            });
        target.is_some_and(|target| self.base.step_toward(g, pid, uid, target))
    }

    fn force_domain(g: &Game, uid: u32) -> ForceDomain {
        if g.rules.units[g.units[&uid].kind].domain.as_deref() == Some("sea") {
            ForceDomain::Sea
        } else {
            ForceDomain::Land
        }
    }

    fn force_role(g: &Game, uid: u32) -> ForceRole {
        match BasicAi::unit_doctrine(g, uid) {
            UnitDoctrine::Recon => ForceRole::Recon,
            UnitDoctrine::Assault => ForceRole::Vanguard,
            UnitDoctrine::Mobile => ForceRole::Mobile,
            UnitDoctrine::Ranged => ForceRole::Ranged,
            UnitDoctrine::Siege => ForceRole::Siege,
            UnitDoctrine::Support | UnitDoctrine::Carrier => ForceRole::Support,
            UnitDoctrine::AirDefense | UnitDoctrine::AirStrike => ForceRole::AirStrike,
        }
    }

    /// Reserve one reachable land combat unit for each ungarrisoned occupied
    /// city, weakest-loyalty first. The assignment is recomputed from the
    /// current position before every unit acts, so a completed garrison is
    /// immediately removed from the demand set and cannot attract a second
    /// unit. This is the strategic counterpart to Gathering Storm's -5
    /// occupation Loyalty penalty.
    fn occupation_garrison_target(&self, g: &Game, pid: usize, uid: u32) -> Option<Pos> {
        let mut cities: Vec<_> = g
            .cities
            .values()
            .filter(|city| city.owner == pid)
            .filter(|city| {
                city.occupied_from
                    .is_some_and(|former| g.players.get(former).is_some_and(|p| p.alive))
            })
            .filter(|city| {
                !g.units_at(city.pos).into_iter().any(|unit| {
                    g.units[&unit].owner == pid
                        && g.rules.units[g.units[&unit].kind].class == "military"
                })
            })
            .collect();
        cities.sort_by(|left, right| {
            left.loyalty
                .total_cmp(&right.loyalty)
                .then_with(|| left.id.cmp(&right.id))
        });
        let mut available: BTreeSet<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|unit| {
                let spec = &g.rules.units[g.units[unit].kind];
                spec.class == "military"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && g.units[unit].linked_to.is_none()
            })
            .collect();
        for city in cities {
            let selected = available
                .iter()
                .filter(|unit| {
                    g.units[unit].pos == city.pos || g.route_step(**unit, city.pos, 0).is_some()
                })
                .min_by(|left, right| {
                    let rank = |unit: u32| {
                        (
                            g.wdist(g.units[&unit].pos, city.pos),
                            g.unit_strength(&g.units[&unit], true) as i32,
                            unit,
                        )
                    };
                    rank(**left).cmp(&rank(**right))
                })
                .copied();
            if let Some(selected) = selected {
                available.remove(&selected);
                if selected == uid {
                    return Some(city.pos);
                }
            }
        }
        None
    }

    fn force_anchor(g: &Game, units: &[u32]) -> Pos {
        units
            .iter()
            .map(|uid| {
                let pos = g.units[uid].pos;
                let total: i32 = units
                    .iter()
                    .map(|other| g.wdist(pos, g.units[other].pos))
                    .sum();
                (total, *uid, pos)
            })
            .min()
            .map(|(_, _, pos)| pos)
            .unwrap_or((0, 0))
    }

    fn domain_objective(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        domain: ForceDomain,
        anchor: Pos,
        enemies: &[usize],
    ) -> Pos {
        // An ancient rush keeps its objective. `threatened_city` outranks
        // `target_city` here and is an empire-wide fact, so the turn the
        // victim's counter-raid puts any city of ours under pressure the whole
        // column re-aims — homeward, or at whatever hostile unit is nearest
        // that city. Measured: melee standing adjacent to a rival capital runs
        // at **0.03 per civilization** at turn 50 while 1.01 sits on the
        // staging ring three to five tiles out. The stack marches, declares,
        // and then never closes the last three tiles.
        //
        // Trading a city for their capital is the whole bet of a rush, and it
        // is a bet the census says pays: their capital is unwalled and holds a
        // garrison of 0.7, ours is not the one under threat yet.
        let rush_objective = plan
            .rush
            .then(|| plan.target_city.and_then(|cid| g.cities.get(&cid)))
            .flatten();
        if domain == ForceDomain::Land {
            if let Some(city) = rush_objective {
                return city.pos;
            }
        }
        let threatened_enemy = plan.threatened_city.and_then(|cid| {
            let city = g.cities.get(&cid)?;
            g.units
                .values()
                .filter(|unit| {
                    enemies.contains(&unit.owner)
                        && match domain {
                            ForceDomain::Sea => BasicAi::waterborne(g, unit.id),
                            ForceDomain::Land => !BasicAi::waterborne(g, unit.id),
                        }
                        && g.wdist(city.pos, unit.pos) <= 8
                })
                .min_by_key(|unit| (g.wdist(anchor, unit.pos), unit.id))
                .map(|unit| unit.pos)
        });
        if let Some(pos) = threatened_enemy {
            return pos;
        }

        let planned = plan
            .threatened_city
            .or(plan.target_city)
            .and_then(|cid| g.cities.get(&cid).map(|city| city.pos));
        if domain == ForceDomain::Land {
            return planned
                .or_else(|| self.base.nearest_enemy_from(g, pid, anchor, enemies))
                .unwrap_or(anchor);
        }

        // Fleets interdict hostile ships first. Against a land objective they
        // share the campaign but receive a reachable coastal approach tile.
        if let Some(pos) = g
            .units
            .values()
            .filter(|unit| enemies.contains(&unit.owner) && BasicAi::waterborne(g, unit.id))
            .min_by_key(|unit| (g.wdist(anchor, unit.pos), unit.id))
            .map(|unit| unit.pos)
        {
            return pos;
        }
        // During colonization, a fleet without an immediate contact screens
        // the embarked settler. Once the civilian is linked, its naval leader
        // will carry the pair all the way to the persistent colony objective.
        if let Some(pos) = g
            .units
            .values()
            .filter(|unit| {
                unit.owner == pid
                    && unit.kind == "settler"
                    && g.map
                        .get(unit.pos)
                        .is_some_and(|tile| g.rules.is_water(tile))
            })
            .min_by_key(|unit| (g.wdist(anchor, unit.pos), unit.id))
            .map(|unit| unit.pos)
        {
            return pos;
        }

        let coastal_campaign_city = planned
            .filter(|pos| {
                g.city_at(*pos)
                    .is_some_and(|cid| BasicAi::city_is_coastal(g, cid))
            })
            .or_else(|| {
                g.cities
                    .values()
                    .filter(|city| {
                        enemies.contains(&city.owner) && BasicAi::city_is_coastal(g, city.id)
                    })
                    .min_by_key(|city| (g.wdist(anchor, city.pos), city.id))
                    .map(|city| city.pos)
            });
        coastal_campaign_city
            .and_then(|city_pos| {
                let approach = |radius| {
                    g.wdisk(city_pos, radius)
                        .into_iter()
                        .filter(|pos| {
                            g.map.get(*pos).is_some_and(|tile| {
                                g.rules.is_water(tile)
                                    && g.rules.is_passable(tile)
                                    && (tile.terrain != "ocean"
                                        || g.players[pid].techs.contains(&crate::name!("cartography")))
                            })
                        })
                        .min_by_key(|pos| (g.wdist(anchor, *pos), *pos))
                };
                // Adjacent water lets melee ships capture after ranged ships
                // remove defenses. Radius three is only a fallback for cities
                // behind a narrow land/coast configuration.
                approach(1).or_else(|| approach(3))
            })
            .unwrap_or(anchor)
    }

    fn force_focus_target(
        &self,
        g: &Game,
        units: &[u32],
        enemies: &[usize],
        plan: &StrategicPlan,
    ) -> Option<Pos> {
        let mut targets = BTreeSet::new();
        for uid in units {
            let unit = &g.units[uid];
            let spec = &g.rules.units[unit.kind];
            if spec.class != "military" || (!spec.is_melee_capable() && !spec.has_ranged_attack()) {
                continue;
            }
            let radius = if spec.has_ranged_attack() {
                g.unit_attack_range(*uid).max(1)
            } else {
                1
            };
            for pos in g.wdisk(unit.pos, radius) {
                if pos != unit.pos && self.base.is_enemy_tile(g, pos, enemies) {
                    targets.insert(pos);
                }
            }
        }
        targets.into_iter().max_by(|a, b| {
            let value = |target: Pos| -> f64 {
                let mut score = 0.0;
                let mut attackers = 0;
                for uid in units {
                    let unit = &g.units[uid];
                    let spec = &g.rules.units[unit.kind];
                    if spec.class != "military"
                        || (!spec.is_melee_capable() && !spec.has_ranged_attack())
                    {
                        continue;
                    }
                    let distance = g.wdist(unit.pos, target);
                    let mut exchange = f64::NEG_INFINITY;
                    if spec.has_ranged_attack() && distance <= g.unit_attack_range(*uid) {
                        exchange = exchange.max(self.base.exchange_score(g, *uid, target, true));
                    }
                    if spec.is_melee_capable() && distance == 1 {
                        exchange = exchange.max(self.base.exchange_score(g, *uid, target, false));
                    }
                    if exchange.is_finite() {
                        score += exchange.max(-20.0);
                        attackers += 1;
                    }
                }
                score += attackers as f64 * 8.0;
                if plan
                    .target_city
                    .is_some_and(|cid| g.cities.get(&cid).is_some_and(|city| city.pos == target))
                {
                    score += 35.0;
                }
                if let Some(hp) = g
                    .units_at(target)
                    .iter()
                    .filter_map(|uid| {
                        enemies
                            .contains(&g.units[uid].owner)
                            .then_some(g.units[uid].hp)
                    })
                    .min()
                {
                    score += (100 - hp) as f64 * 0.4;
                }
                score
            };
            value(*a)
                .partial_cmp(&value(*b))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.cmp(a))
        })
    }

    fn local_strength_ratio(
        &self,
        g: &Game,
        units: &[u32],
        enemies: &[usize],
        objective: Pos,
    ) -> f64 {
        let friendly: f64 = units
            .iter()
            .filter_map(|uid| {
                let unit = &g.units[uid];
                (g.rules.units[unit.kind].class == "military").then_some(
                    crate::game::effective_strength(g.unit_strength(unit, true), unit.hp),
                )
            })
            .sum();
        let hostile: f64 = g
            .units
            .values()
            .filter(|unit| enemies.contains(&unit.owner) && g.wdist(unit.pos, objective) <= 6)
            .filter(|unit| g.rules.units[unit.kind].class == "military")
            .map(|unit| crate::game::effective_strength(g.unit_strength(unit, true), unit.hp))
            .sum::<f64>()
            + g.city_at(objective)
                .filter(|city| enemies.contains(&g.cities[city].owner))
                .map(|city| g.city_strength(city))
                .unwrap_or(0.0)
            + g.encampment_at(objective)
                .filter(|city| enemies.contains(&g.cities[city].owner))
                .map(|city| g.encampment_strength(city))
                .unwrap_or(0.0);
        if hostile <= 0.0 {
            3.0
        } else {
            (friendly / hostile).clamp(0.0, 3.0)
        }
    }

    fn rebuild_force_groups(&mut self, g: &Game, pid: usize, plan: &StrategicPlan) {
        self.force_groups.clear();
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
            return;
        }

        let mut remaining: BTreeSet<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let spec = &g.rules.units[g.units[uid].kind];
                // Aircraft receive missions from the air-operations evaluator.
                // Counting them as land units makes a thin ground army appear
                // assembled and locally superior even though aircraft cannot
                // occupy its front, screen siege, or capture the objective.
                let field_unit = matches!(spec.class.as_str(), "military" | "support")
                    && spec.domain.as_deref() != Some("air");
                field_unit
                    && !(BasicAi::unit_doctrine(g, *uid) == UnitDoctrine::Recon
                        && self.base.has_exploration_target(g, pid, *uid))
            })
            .collect();
        let command_radius = self.base.w.command_radius.round().max(1.0) as i32;
        while let Some(seed) = remaining.iter().next().copied() {
            remaining.remove(&seed);
            let domain = Self::force_domain(g, seed);
            let mut units = vec![seed];
            loop {
                let additions: Vec<u32> = remaining
                    .iter()
                    .copied()
                    .filter(|candidate| {
                        Self::force_domain(g, *candidate) == domain
                            && units.iter().any(|member| {
                                g.wdist(g.units[member].pos, g.units[candidate].pos)
                                    <= command_radius
                            })
                    })
                    .collect();
                if additions.is_empty() {
                    break;
                }
                for uid in additions {
                    remaining.remove(&uid);
                    units.push(uid);
                }
            }
            units.sort_unstable();
            let anchor = Self::force_anchor(g, &units);
            let objective = self.domain_objective(g, pid, plan, domain, anchor, &enemies);
            // ⚠ MEASURED AND REJECTED: pinning `focus_target` to the objective
            // city for a rush, on the theory that the first defender met
            // otherwise pulls the column off the capital. It made things
            // worse — blows on cities by turn 60 fell 6.1 to 2.9, first
            // capture slipped turn 65 to 86 — and the city's own ring still
            // never held more than two. A rush that walks past the defenders
            // to stand on the ring is a rush that gets killed on the ring.
            let focus_target = self.force_focus_target(g, &units, &enemies, plan);
            let muster_radius = self.base.w.muster_radius.round().max(1.0) as i32;
            let readiness = units
                .iter()
                .filter(|uid| {
                    g.wdist(g.units[uid].pos, anchor) <= muster_radius
                        && g.units[uid].hp as f64 > self.base.w.withdraw_hp
                })
                .count() as f64
                / units.len().max(1) as f64;
            let local_strength_ratio = self.local_strength_ratio(g, &units, &enemies, objective);
            let average_hp = units.iter().map(|uid| g.units[uid].hp).sum::<i32>() as f64
                / units.len().max(1) as f64;
            let forcing_focus = focus_target.is_some_and(|target| {
                let low_hp_unit = g
                    .units_at(target)
                    .into_iter()
                    .any(|unit| enemies.contains(&g.units[&unit].owner) && g.units[&unit].hp <= 35);
                let capturable_city = g.city_at(target).is_some_and(|city| {
                    enemies.contains(&g.cities[&city].owner)
                        && g.cities[&city].hp <= 40
                        && g.cities[&city].wall_hp <= 0
                        && units.iter().any(|unit| {
                            g.rules.units[g.units[unit].kind].is_melee_capable()
                                && g.wdist(g.units[unit].pos, target) <= 1
                        })
                });
                low_hp_unit || capturable_city
            });
            // `threatened_city` is an empire-wide fact, so every force group
            // in every domain stands still whenever any city anywhere is
            // under pressure — including columns far too distant to affect
            // the siege. `scoped_relief_hold` restricts the hold to groups
            // that could actually arrive. It is off by default: it changes
            // the behaviour as intended and measured no stronger, so the
            // shipped agent keeps the old rule until the constraint it
            // exposes is worth spending on. See the field's documentation.
            // The Engage disjuncts deliberately keep reading the raw flag
            // either way: a group already in contact should press regardless
            // of which city is in trouble.
            let relieving = plan.threatened_city.is_some_and(|city| {
                !self.scoped_relief_hold || Self::can_relieve(g, &units, anchor, city)
            });
            // ⚠ MEASURED AND REJECTED: letting a rush ignore `relieving` and
            // `Muster` — on the theory that a stack sized against one
            // undefended capital should never stand still — made it *worse*.
            // Over the same 12 maps, captures fell 9/12 to 6/12 and the median
            // first capture slipped from turn 79 to 96. The two standing-still
            // postures are load-bearing even for a rush; do not retry this
            // without a different mechanism.
            let posture = if average_hp <= self.base.w.withdraw_hp + 10.0 {
                ForcePosture::Recover
            } else if (focus_target.is_some()
                && (local_strength_ratio >= LOCAL_SUPERIORITY_FLOOR
                    || plan.threatened_city.is_some()
                    || forcing_focus))
                || (units.iter().any(|uid| {
                    g.units.values().any(|enemy| {
                        enemies.contains(&enemy.owner)
                            && g.wdist(g.units[uid].pos, enemy.pos) <= 2
                            && (local_strength_ratio >= LOCAL_SUPERIORITY_FLOOR
                                || plan.threatened_city.is_some()
                                || enemy.hp <= 35)
                    })
                }))
            {
                ForcePosture::Engage
            } else if relieving || local_strength_ratio < LOCAL_SUPERIORITY_FLOOR {
                ForcePosture::Hold
            } else if units.len() > 1 && readiness + 1e-9 < self.base.w.muster_readiness {
                ForcePosture::Muster
            } else {
                ForcePosture::Advance
            };
            self.force_groups.push(ForceGroup {
                id: units[0],
                domain,
                units,
                anchor,
                objective,
                focus_target,
                posture,
                readiness,
                local_strength_ratio,
            });
        }
        self.force_groups.sort_by_key(|group| group.id);
        // What the armies have been told to do. This is the layer between the
        // grand strategy and the individual attacks below it, and it is the
        // one an observer cannot infer from watching units move: a group that
        // holds and a group that has nowhere to go look identical on the map.
        if self.journal().wants(crate::reasoning::Level::Decision) {
            for group in &self.force_groups {
                let held = match group.posture {
                    ForcePosture::Hold if group.local_strength_ratio < LOCAL_SUPERIORITY_FLOOR => {
                        " — too weak locally to advance"
                    }
                    ForcePosture::Hold => " — held back to cover a threat",
                    ForcePosture::Muster => " — still gathering",
                    _ => "",
                };
                think!(self.journal(), Military, Decision,
                       "A {} force of {} will {}",
                       group.domain.as_str(), group.units.len(), group.posture.as_str();
                       "objective {:?}, {:.0}% ready, {:.2} local strength against the \
                        enemy there{held}",
                       group.objective, group.readiness * 100.0, group.local_strength_ratio;
                       group.objective);
            }
        }
        for group in &self.force_groups {
            self.census.count_posture(group.posture);
            if group.posture == ForcePosture::Hold {
                // Attribute the disjunct that actually held the group, not
                // whichever flag happened to be set. A group below the
                // superiority floor holds on its own account whether or not
                // a city is threatened, so counting it as threat-held
                // overstated the threat term: measured against the shipped
                // census it read 61% where the causal share was 34%.
                if group.local_strength_ratio < LOCAL_SUPERIORITY_FLOOR {
                    self.census.hold_weak += 1;
                } else {
                    self.census.hold_threatened += 1;
                }
            }
        }
    }

    /// Whether this force group could plausibly reach `city` before its siege
    /// resolves, and is therefore worth halting to defend it.
    ///
    /// [`AdvancedAi::threatened_city`] scores hostiles within six hexes of the
    /// city, so a group already inside that ring is in the fight. Outside it
    /// the group has to march, and it marches at the pace of its slowest
    /// member — a siege train does not keep up with horse. Allow it the ground
    /// it can cover in [`RELIEF_MARCH_TURNS`] and no more.
    fn can_relieve(g: &Game, units: &[u32], anchor: Pos, city: u32) -> bool {
        let Some(city) = g.cities.get(&city) else {
            return false;
        };
        let pace = units
            .iter()
            .filter_map(|uid| g.units.get(uid))
            .map(|unit| g.rules.units[unit.kind].moves)
            .fold(f64::INFINITY, f64::min);
        let pace = if pace.is_finite() { pace.max(1.0) } else { 1.0 };
        let reach = THREAT_RELIEF_RADIUS + (pace * RELIEF_MARCH_TURNS).round() as i32;
        g.wdist(anchor, city.pos) <= reach
    }

    fn coordinated_tactical_step(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        group: &ForceGroup,
        enemies: &[usize],
        decline_settlers: bool,
    ) -> bool {
        let unit = &g.units[&uid];
        let upos = unit.pos;
        let role = Self::force_role(g, uid);
        let spec = &g.rules.units[unit.kind];
        let target = match group.posture {
            ForcePosture::Muster | ForcePosture::Hold | ForcePosture::Recover => group.anchor,
            ForcePosture::Engage => group.focus_target.unwrap_or(group.objective),
            ForcePosture::Advance => group.objective,
        };
        let preferred_depth = match role {
            ForceRole::Recon => spec.range.max(2),
            ForceRole::Vanguard | ForceRole::Mobile => 1,
            ForceRole::Ranged | ForceRole::Siege => g.unit_attack_range(uid),
            ForceRole::Support => 2,
            ForceRole::AirStrike => spec.range.max(3),
        };
        let vanguard_depth = group
            .units
            .iter()
            .filter(|other| {
                **other != uid
                    && g.units.contains_key(other)
                    && matches!(
                        Self::force_role(g, **other),
                        ForceRole::Vanguard | ForceRole::Mobile
                    )
            })
            .map(|other| g.wdist(g.units[other].pos, target))
            .min();
        let score = |g: &Game, tile: Pos| -> f64 {
            let objective_distance = g.wdist(tile, target);
            let (progress, cohesion, threat_caution, spacing) = match role {
                ForceRole::Recon => (0.55, 0.40, 1.35, 1.25),
                ForceRole::Vanguard => (1.15, 1.00, 1.00, 1.00),
                ForceRole::Mobile => (1.40, 0.65, 0.80, 1.00),
                ForceRole::Ranged => (0.90, 1.10, 1.15, 1.50),
                ForceRole::Siege => (0.80, 1.30, 1.25, 1.70),
                ForceRole::Support => (0.65, 1.50, 1.40, 1.20),
                ForceRole::AirStrike => (1.20, 0.20, 0.75, 0.50),
            };
            let mut value = -self.base.w.objective_progress * progress * objective_distance as f64;
            let nearest_friend = group
                .units
                .iter()
                .filter(|other| **other != uid && g.units.contains_key(other))
                .map(|other| g.wdist(tile, g.units[other].pos))
                .min();
            if let Some(distance) = nearest_friend {
                value -= self.base.w.cohesion * cohesion * (distance - 2).max(0) as f64;
                if distance == 1 {
                    value += self.base.w.mv_support;
                }
            }
            for enemy in g
                .units
                .values()
                .filter(|other| enemies.contains(&other.owner))
            {
                let enemy_spec = &g.rules.units[enemy.kind];
                if enemy_spec.class != "military"
                    || (!enemy_spec.is_melee_capable() && !enemy_spec.has_ranged_attack())
                {
                    continue;
                }
                let radius = if enemy_spec.has_ranged_attack() {
                    g.unit_attack_range(enemy.id).max(1)
                } else {
                    1
                };
                if g.wdist(tile, enemy.pos) <= radius {
                    let attack =
                        crate::game::effective_strength(g.unit_strength(enemy, false), enemy.hp);
                    let defense =
                        crate::game::effective_strength(g.unit_strength(unit, true), unit.hp);
                    value -= self.base.w.mv_threat
                        * threat_caution
                        * 30.0
                        * ((attack - defense) / 25.0).exp();
                }
            }
            if g.wdist(tile, target) <= 5 {
                value -= self.base.w.role_spacing
                    * spacing
                    * (g.wdist(tile, target) - preferred_depth).abs() as f64;
                if matches!(
                    role,
                    ForceRole::Recon | ForceRole::Ranged | ForceRole::Siege | ForceRole::AirStrike
                ) {
                    if let Some(front_depth) = vanguard_depth {
                        value -= self.base.w.screen
                            * (front_depth - g.wdist(tile, target)).max(0) as f64;
                    }
                }
            }
            if group.local_strength_ratio < 1.0 {
                let advance = g.wdist(upos, target) - objective_distance;
                value -= self.base.w.local_superiority
                    * (1.0 - group.local_strength_ratio)
                    * advance.max(0) as f64;
            }
            value + self.base.livelock_penalty(uid, tile)
        };

        let stay = score(g, upos);
        let holding_role_position = g.wdist(upos, target) == preferred_depth;
        let mut best: Option<(f64, Pos)> = None;
        for pos in g.nbrs(upos).into_iter().filter(|pos| {
            g.can_move(uid, *pos)
                && !(decline_settlers
                    && g.units_at(*pos).iter().any(|other| {
                        let other = &g.units[other];
                        other.owner != pid
                            && g.is_at_war(pid, other.owner)
                            && other.kind == "settler"
                    }))
        }) {
            let candidate = score(g, pos);
            if best
                .map(|(old, old_pos)| candidate > old || (candidate == old && pos < old_pos))
                .unwrap_or(true)
            {
                best = Some((candidate, pos));
            }
        }
        if let Some((candidate, pos)) = best {
            let should_move = if holding_role_position {
                candidate > stay + 1e-9
            } else {
                self.base.move_beats_holding(g, uid, candidate, stay)
            };
            if should_move {
                return g.apply(pid, &Action::Move { unit: uid, to: pos }).is_ok();
            }
        }

        let stop_range = if matches!(
            role,
            ForceRole::Recon | ForceRole::Ranged | ForceRole::Siege | ForceRole::AirStrike
        ) {
            preferred_depth
        } else {
            1
        };
        if g.wdist(upos, target) > stop_range {
            if let Some(pos) = g
                .route_step(uid, target, stop_range)
                .filter(|pos| g.can_move(uid, *pos))
                .filter(|pos| {
                    !(decline_settlers
                        && g.units_at(*pos).iter().any(|other| {
                            let other = &g.units[other];
                            other.owner != pid
                                && g.is_at_war(pid, other.owner)
                                && other.kind == "settler"
                        }))
                })
            {
                if self.base.move_beats_holding(g, uid, score(g, pos), stay) {
                    return g.apply(pid, &Action::Move { unit: uid, to: pos }).is_ok();
                }
            }
        }
        self.base.fortify_or_stop(g, pid, uid)
    }

    /// Candidate attacks on one tactical victim. `Game::legal_actions` also
    /// enumerates production, diplomacy, spies, purchases, every movement,
    /// and every other unit target; calling it at each reply-search node made
    /// late-game turns spend most of their time proving irrelevant actions
    /// irrelevant. Build the small target-specific superset here and let
    /// `Game::apply` remain the authoritative legality check.
    fn forcing_attacks_to(
        position: &Game,
        enemy: usize,
        victim_pos: Pos,
        only_unit: Option<u32>,
    ) -> Vec<Action> {
        let mut replies = Vec::new();
        for unit in position.units.values().filter(|unit| {
            unit.owner == enemy && only_unit.is_none_or(|candidate| candidate == unit.id)
        }) {
            if unit.moves_left <= 0.0 || unit.attacks_left <= 0 {
                continue;
            }
            let spec = &position.rules.units[unit.kind];
            let distance = position.wdist(unit.pos, victim_pos);
            if spec.domain.as_deref() == Some("air") {
                if distance <= position.unit_attack_range(unit.id) {
                    replies.push(Action::AirStrike {
                        unit: unit.id,
                        target: victim_pos,
                    });
                }
                continue;
            }
            if spec.class != "military" || position.is_embarked(unit) {
                continue;
            }
            if spec.has_ranged_attack() && distance <= position.unit_attack_range(unit.id) {
                replies.push(Action::Ranged {
                    unit: unit.id,
                    target: victim_pos,
                });
            }
            if spec.is_melee_capable() && distance == 1 {
                replies.push(Action::Attack {
                    unit: unit.id,
                    target: victim_pos,
                });
            }
        }
        if only_unit.is_none() {
            for city in position.cities.values().filter(|city| city.owner == enemy) {
                replies.push(Action::CityStrike {
                    city: city.id,
                    target: victim_pos,
                });
                replies.push(Action::EncampmentStrike {
                    city: city.id,
                    target: victim_pos,
                });
            }
        }
        replies
    }

    fn forcing_reply_line(&self, position: &Game, enemy: usize, victim: u32, depth: usize) -> f64 {
        if depth == 0 || !position.units.contains_key(&victim) {
            return 0.0;
        }
        let victim_hp = position.units[&victim].hp;
        let victim_pos = position.units[&victim].pos;
        let replies = Self::forcing_attacks_to(position, enemy, victim_pos, None);

        let mut reply_branches = Vec::new();
        let mut direct_attackers = BTreeSet::new();
        for reply in replies {
            let reply_unit = match &reply {
                Action::Attack { unit, .. }
                | Action::Ranged { unit, .. }
                | Action::AirStrike { unit, .. } => Some(*unit),
                _ => None,
            };
            direct_attackers.extend(reply_unit);
            let reply_hp =
                reply_unit.and_then(|unit| position.units.get(&unit).map(|candidate| candidate.hp));
            let mut branch = position.clone();
            if branch.apply(enemy, &reply).is_err() {
                continue;
            }
            reply_branches.push((format!("{reply:?}"), branch, reply_unit, reply_hp));
        }

        // A Civ unit can normally move and attack in the same turn. Search
        // only one-step forcing approaches whose resulting position already
        // has a legal attack on the victim. This is the tactical analogue of
        // a check extension: it closes the horizon gap around an exposed
        // capture without admitting every quiet movement into quiescence.
        let mobile_attackers: Vec<u32> = position
            .units
            .values()
            .filter(|unit| unit.owner == enemy && !direct_attackers.contains(&unit.id))
            .filter(|unit| {
                let spec = &position.rules.units[unit.kind];
                spec.class == "military"
                    && spec.domain.as_deref() != Some("air")
                    && position.wdist(unit.pos, victim_pos)
                        <= position.unit_attack_range(unit.id) + 2
            })
            .map(|unit| unit.id)
            .collect();
        for attacker in mobile_attackers {
            let reply_hp = position.units[&attacker].hp;
            for to in position
                .nbrs(position.units[&attacker].pos)
                .into_iter()
                .filter(|to| position.can_move(attacker, *to))
            {
                let movement = Action::Move { unit: attacker, to };
                let mut moved = position.clone();
                if moved.apply(enemy, &movement).is_err() {
                    continue;
                }
                let followups =
                    Self::forcing_attacks_to(&moved, enemy, victim_pos, Some(attacker));
                for followup in followups {
                    let mut branch = moved.clone();
                    if branch.apply(enemy, &followup).is_err() {
                        continue;
                    }
                    reply_branches.push((
                        format!("{movement:?} -> {followup:?}"),
                        branch,
                        Some(attacker),
                        Some(reply_hp),
                    ));
                }
            }
        }

        let mut ordered = Vec::new();
        for (label, branch, reply_unit, reply_hp) in reply_branches {
            let loss = branch
                .units
                .get(&victim)
                .map(|unit| (victim_hp - unit.hp).max(0) as f64)
                .unwrap_or(victim_hp as f64 + 35.0);
            let counter_loss = match (reply_unit, reply_hp) {
                (Some(unit), Some(hp)) => branch
                    .units
                    .get(&unit)
                    .map(|unit| (hp - unit.hp).max(0) as f64)
                    .unwrap_or(hp as f64 + 20.0),
                _ => 0.0,
            };
            ordered.push(((loss - 0.35 * counter_loss).max(0.0), label, branch));
        }

        // Chess-style move ordering keeps the extension bounded: examine all
        // forcing replies at the frontier, but only extend the four strongest
        // captures/checks into another focus-fire action.
        ordered.sort_by(|left, right| {
            right
                .0
                .total_cmp(&left.0)
                .then_with(|| left.1.cmp(&right.1))
        });
        ordered
            .into_iter()
            .take(4)
            .map(|(immediate, _, branch)| {
                immediate + self.forcing_reply_line(&branch, enemy, victim, depth - 1)
            })
            .fold(0.0_f64, f64::max)
    }

    /// Make a candidate battlefield action on a clone and value the exact
    /// result before extending opponent replies. This is the principal-search
    /// half of the tactical evaluator: static exchange remains useful for
    /// cheap move ordering, while the final decision sees the seeded damage
    /// roll, kills, attacker survival, wall damage, district pillage, and an
    /// actual city transfer.
    fn tactical_attack_value(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        action: &Action,
        plan: &StrategicPlan,
    ) -> f64 {
        let target = match action {
            Action::Attack { unit, target }
            | Action::Ranged { unit, target }
            | Action::PriorityTarget { unit, target }
                if *unit == uid =>
            {
                *target
            }
            _ => return f64::NEG_INFINITY,
        };
        let priority_target = matches!(action, Action::PriorityTarget { .. });
        let attacker = &g.units[&uid];
        let attacker_spec = &g.rules.units[attacker.kind];
        let defenders: Vec<(u32, i32, f64, f64, bool, bool)> = g
            .units_at(target)
            .into_iter()
            .filter_map(|unit| {
                let defender = &g.units[&unit];
                let spec = &g.rules.units[defender.kind];
                (defender.owner != pid
                    && g.is_at_war(pid, defender.owner)
                    && if priority_target {
                        spec.class == "support"
                    } else {
                        spec.class == "military"
                    })
                .then_some((
                    unit,
                    defender.hp,
                    g.unit_strength(defender, true),
                    spec.cost,
                    spec.siege,
                    spec.is_melee_capable(),
                ))
            })
            .collect();
        let target_city = (!priority_target)
            .then(|| g.city_at(target))
            .flatten()
            .filter(|city| g.cities[city].owner != pid && g.is_at_war(pid, g.cities[city].owner));
        let target_encampment = target_city
            .is_none()
            .then(|| g.encampment_at(target))
            .flatten();
        let mut after = g.clone();
        if after.apply(pid, action).is_err() {
            return f64::NEG_INFINITY;
        }

        let attacker_loss = match after.units.get(&uid) {
            Some(survivor) => {
                (attacker.hp - survivor.hp).max(0) as f64 * (1.25 + attacker_spec.cost / 800.0)
            }
            None => 230.0 + attacker_spec.cost * 0.65,
        };
        let mut value = -attacker_loss;
        for (unit, hp, strength, cost, siege, captures) in defenders {
            value += match after.units.get(&unit) {
                None => {
                    190.0
                        + cost * 0.45
                        + strength * 1.8
                        + if siege { 65.0 } else { 0.0 }
                        + if captures { 30.0 } else { 0.0 }
                }
                Some(survivor) => {
                    (hp - survivor.hp).max(0) as f64 * (1.0 + strength / 100.0)
                        + if siege { 18.0 } else { 0.0 }
                        + if captures { 6.0 } else { 0.0 }
                }
            };
        }
        if let Some(city) = target_city {
            let before = &g.cities[&city];
            let captured = after
                .cities
                .get(&city)
                .is_some_and(|city| city.owner == pid);
            if captured {
                if Self::should_defer_city_capture(g, pid, city) {
                    return f64::NEG_INFINITY;
                }
                value += 520.0
                    + before.pop.max(1) as f64 * 14.0
                    + before.districts.len() as f64 * 24.0
                    + before.wonders.len() as f64 * 45.0
                    + if before.is_capital { 180.0 } else { 0.0 }
                    + if plan.target_city == Some(city) {
                        100.0
                    } else {
                        0.0
                    };
            } else if let Some(after_city) = after.cities.get(&city) {
                let wall_damage = (before.wall_hp - after_city.wall_hp).max(0) as f64;
                let city_damage = (before.hp - after_city.hp).max(0) as f64;
                let progress = wall_damage * 1.35 + city_damage;
                value += progress
                    + if progress > 0.0 && plan.target_city == Some(city) {
                        35.0
                    } else {
                        0.0
                    };
            }
        } else if let Some(city) = target_encampment {
            let before = &g.cities[&city];
            let after_city = &after.cities[&city];
            value += (before.encampment_wall_hp - after_city.encampment_wall_hp).max(0) as f64
                * 1.35
                + (before.encampment_hp - after_city.encampment_hp).max(0) as f64;
            if !before.encampment_pillaged && after_city.encampment_pillaged {
                value += 180.0;
            }
        }
        value
    }

    /// Bounded quiescence-style reply search for a proposed attack. The
    /// ordinary exchange evaluator accounts for the target's counter-damage;
    /// this extension makes the move on a cloned position, refreshes only the
    /// enemy's forcing combat actions, and prices a two-action focus-fire
    /// sequence. It catches poisoned captures and coordinated ranged kills
    /// without turning every unit decision into an unbounded turn search.
    fn forcing_reply_penalty(&self, g: &Game, pid: usize, uid: u32, action: &Action) -> f64 {
        let mut after = g.clone();
        if after.apply(pid, action).is_err() {
            return 1_000.0;
        }
        if !after.units.contains_key(&uid) {
            return 135.0;
        }
        let enemies: Vec<usize> = after
            .players
            .iter()
            .filter(|player| player.id != pid && player.alive && after.is_at_war(pid, player.id))
            .map(|player| player.id)
            .collect();
        let mut worst_reply = 0.0_f64;

        for enemy in enemies {
            let mut reply_position = after.clone();
            reply_position.current = enemy;
            for unit in reply_position
                .units
                .values_mut()
                .filter(|unit| unit.owner == enemy)
            {
                // Only attacks are searched, so a generous movement budget
                // merely restores next-turn attack availability; it cannot
                // manufacture a move-and-attack line in this one-ply search.
                unit.moves_left = 100.0;
                unit.attacks_left = 1;
                unit.acted = false;
                unit.zoc_stopped = false;
            }
            for city in reply_position
                .cities
                .values_mut()
                .filter(|city| city.owner == enemy)
            {
                city.struck = false;
                city.encampment_struck = false;
            }

            worst_reply = worst_reply.max(self.forcing_reply_line(&reply_position, enemy, uid, 2));
        }
        worst_reply
    }

    /// Evaluate an air strike by making it on a cloned position. This captures
    /// the seeded combat roll, interception damage, wall-vs-city damage, and
    /// kills in one bounded result score instead of ordering targets by their
    /// pre-combat HP alone.
    fn air_strike_value(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        target: Pos,
        plan: &StrategicPlan,
    ) -> f64 {
        let attacker = &g.units[&uid];
        let attacker_spec = &g.rules.units[attacker.kind];
        let target_city = g
            .city_at(target)
            .filter(|city| g.cities[city].owner != pid && g.is_at_war(pid, g.cities[city].owner));
        let target_encampment = target_city
            .is_none()
            .then(|| g.encampment_at(target))
            .flatten();
        let target_unit = (target_city.is_none() && target_encampment.is_none())
            .then(|| {
                g.units_at(target).into_iter().find(|other| {
                    let defender = &g.units[other];
                    defender.owner != pid
                        && g.is_at_war(pid, defender.owner)
                        && g.rules.units[defender.kind].class == "military"
                })
            })
            .flatten();
        let action = Action::AirStrike { unit: uid, target };
        let mut after = g.clone();
        if after.apply(pid, &action).is_err() {
            return f64::NEG_INFINITY;
        }

        let attacker_loss = match after.units.get(&uid) {
            Some(survivor) => {
                (attacker.hp - survivor.hp).max(0) as f64 * (1.4 + attacker_spec.cost / 700.0)
            }
            None => 260.0 + attacker_spec.cost * 0.7,
        };
        let mut value = -attacker_loss;
        if let Some(unit) = target_unit {
            let defender = &g.units[&unit];
            let spec = &g.rules.units[defender.kind];
            let role_value = if spec.siege { 70.0 } else { 0.0 }
                + if spec.is_melee_capable() { 30.0 } else { 0.0 }
                + if spec.domain.as_deref() == Some("air") {
                    85.0
                } else {
                    0.0
                };
            value += match after.units.get(&unit) {
                None => {
                    190.0 + spec.cost * 0.45 + g.unit_strength(defender, true) * 1.8 + role_value
                }
                Some(survivor) => {
                    (defender.hp - survivor.hp).max(0) as f64
                        * (1.0 + g.unit_strength(defender, true) / 100.0)
                        + role_value * 0.25
                }
            };
        } else if let Some(city) = target_city {
            let before = &g.cities[&city];
            let after_city = &after.cities[&city];
            let wall_damage = (before.wall_hp - after_city.wall_hp).max(0) as f64;
            let city_damage = (before.hp - after_city.hp).max(0) as f64;
            let progress = wall_damage * 1.35 + city_damage;
            value += progress;
            if progress > 0.0 && plan.target_city == Some(city) {
                value += 45.0;
            }
            if before.is_capital && progress > 0.0 && plan.strategy == GrandStrategy::Conquest {
                value += 25.0;
            }
        } else if let Some(city) = target_encampment {
            let before = &g.cities[&city];
            let after_city = &after.cities[&city];
            value += (before.encampment_wall_hp - after_city.encampment_wall_hp).max(0) as f64
                * 1.35
                + (before.encampment_hp - after_city.encampment_hp).max(0) as f64;
        }
        value
    }

    /// Evaluate infrastructure bombing on an exact cloned position. Besides
    /// the pillaged layer, this prices interception losses and the operational
    /// disruption from scattering aircraft out of a disabled air base.
    fn air_pillage_value(&self, g: &Game, pid: usize, uid: u32, target: Pos) -> f64 {
        let attacker = &g.units[&uid];
        let attacker_spec = &g.rules.units[attacker.kind];
        let before_tile = &g.map.tiles[&target];
        let before_aircraft: Vec<(u32, f64)> = g
            .units_at(target)
            .into_iter()
            .filter_map(|unit| {
                let candidate = &g.units[&unit];
                (candidate.owner != pid
                    && g.rules.units[candidate.kind].domain.as_deref() == Some("air"))
                .then_some((unit, g.rules.units[candidate.kind].cost))
            })
            .collect();
        let city_id = before_tile.owner_city;
        let before_pillaged_buildings = city_id
            .and_then(|city| g.cities.get(&city))
            .map(|city| city.pillaged_buildings.clone())
            .unwrap_or_default();
        let action = Action::AirPillage { unit: uid, target };
        let mut after = g.clone();
        if after.apply(pid, &action).is_err() {
            return f64::NEG_INFINITY;
        }

        let attacker_loss = match after.units.get(&uid) {
            Some(survivor) => {
                (attacker.hp - survivor.hp).max(0) as f64 * (1.4 + attacker_spec.cost / 700.0)
            }
            None => 260.0 + attacker_spec.cost * 0.7,
        };
        let mut value = -attacker_loss;
        let after_tile = &after.map.tiles[&target];
        if let Some(improvement) = before_tile.improvement.as_deref() {
            if !before_tile.pillaged && after_tile.pillaged {
                value += match improvement {
                    "airstrip" => 185.0,
                    "oil_well" | "offshore_oil_rig" | "mine" | "quarry" => 115.0,
                    "farm" | "fishing_boats" => 65.0,
                    _ => 85.0,
                };
            }
        } else if let Some(district) = before_tile.district {
            if !before_tile.pillaged && after_tile.pillaged {
                value += match g.district_family(district).as_str() {
                    "aerodrome" | "industrial_zone" | "campus" | "spaceport" => 175.0,
                    "commercial_hub" | "harbor" | "holy_site" | "theater_square" => 145.0,
                    _ => 115.0,
                };
            } else if let Some(city) = city_id.and_then(|city| after.cities.get(&city)) {
                value += city
                    .pillaged_buildings
                    .difference(&before_pillaged_buildings)
                    .map(|building| 80.0 + after.rules.buildings[building].cost * 0.32)
                    .sum::<f64>();
            }
        }
        for (aircraft, cost) in before_aircraft {
            value += match after.units.get(&aircraft) {
                None => 150.0 + cost * 0.55,
                Some(unit) if unit.pos != target => 55.0 + cost * 0.08,
                _ => 0.0,
            };
        }
        value
    }

    fn priority_target_value(&self, g: &Game, pid: usize, uid: u32, target: Pos) -> f64 {
        let Some(defender_id) = g.priority_support_target_at(pid, target) else {
            return f64::NEG_INFINITY;
        };
        let attacker = &g.units[&uid];
        let attacker_spec = &g.rules.units[attacker.kind];
        let defender = &g.units[&defender_id];
        let defender_spec = &g.rules.units[defender.kind];
        let mut after = g.clone();
        if after
            .apply(pid, &Action::PriorityTarget { unit: uid, target })
            .is_err()
        {
            return f64::NEG_INFINITY;
        }
        let attacker_loss = match after.units.get(&uid) {
            Some(survivor) => {
                (attacker.hp - survivor.hp).max(0) as f64 * (1.4 + attacker_spec.cost / 700.0)
            }
            None => 260.0 + attacker_spec.cost * 0.7,
        };
        let target_value = match after.units.get(&defender_id) {
            None => 175.0 + defender_spec.cost * 0.55,
            Some(survivor) => {
                (defender.hp - survivor.hp).max(0) as f64 * (1.0 + defender_spec.cost / 500.0)
            }
        };
        target_value
            + if defender_spec.anti_air_strength > 0.0 {
                120.0
            } else if matches!(defender.kind.as_str(), "drone" | "observation_balloon") {
                55.0
            } else if matches!(defender.kind.as_str(), "medic" | "supply_convoy") {
                40.0
            } else {
                0.0
            }
            - attacker_loss
    }

    /// Choose among exact air-strike or air-pillage results, a useful patrol,
    /// and a rebase
    /// that materially improves reach to the active front. Fighters preserve
    /// interception coverage when hostile aircraft threaten the theater;
    /// bombers avoid suicidal missions and reposition when no profitable
    /// strike is available.
    fn advanced_air_action(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
    ) -> Option<Action> {
        let unit = &g.units[&uid];
        let doctrine = BasicAi::unit_doctrine(g, uid);
        let legal = g.legal_doctrine_actions(pid, uid);
        let best_strike = legal
            .iter()
            .filter_map(|action| match action {
                Action::AirStrike { unit, target } if *unit == uid => Some((
                    self.air_strike_value(g, pid, uid, *target, plan),
                    *target,
                    action.clone(),
                )),
                _ => None,
            })
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| right.1.cmp(&left.1))
            });
        let best_pillage = legal
            .iter()
            .filter_map(|action| match action {
                Action::AirPillage { unit, target } if *unit == uid => Some((
                    self.air_pillage_value(g, pid, uid, *target),
                    *target,
                    action.clone(),
                )),
                _ => None,
            })
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| right.1.cmp(&left.1))
            });
        let best_priority = legal
            .iter()
            .filter_map(|action| match action {
                Action::PriorityTarget { unit, target } if *unit == uid => Some((
                    self.priority_target_value(g, pid, uid, *target),
                    *target,
                    action.clone(),
                )),
                _ => None,
            })
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| right.1.cmp(&left.1))
            });
        let best_attack = best_strike
            .clone()
            .into_iter()
            .chain(best_priority.clone())
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| right.1.cmp(&left.1))
            });
        let best_mission =
            best_attack
                .clone()
                .into_iter()
                .chain(best_pillage)
                .max_by(|left, right| {
                    left.0
                        .total_cmp(&right.0)
                        .then_with(|| right.1.cmp(&left.1))
                });

        let objective = match doctrine {
            UnitDoctrine::AirDefense => plan.threatened_city.or(plan.target_city),
            _ => plan.target_city.or(plan.threatened_city),
        }
        .and_then(|city| g.cities.get(&city).map(|city| city.pos))
        .or_else(|| {
            g.units
                .values()
                .filter(|other| other.owner != pid && g.is_at_war(pid, other.owner))
                .min_by_key(|other| (g.wdist(unit.pos, other.pos), other.id))
                .map(|other| other.pos)
        });
        let best_rebase = objective.and_then(|objective| {
            let current_distance = g.wdist(unit.pos, objective);
            legal
                .iter()
                .filter_map(|action| match action {
                    Action::AirRebase { unit, to } if *unit == uid => {
                        let distance = g.wdist(*to, objective);
                        let improvement = current_distance - distance;
                        let reaches = (distance <= g.unit_attack_range(uid)) as i32;
                        Some((
                            improvement as f64 * 18.0 + reaches as f64 * 35.0,
                            *to,
                            action.clone(),
                        ))
                    }
                    _ => None,
                })
                .filter(|(value, _, _)| *value > 0.0)
                .max_by(|left, right| {
                    left.0
                        .total_cmp(&right.0)
                        .then_with(|| right.1.cmp(&left.1))
                })
        });

        if doctrine == UnitDoctrine::AirStrike {
            return best_mission
                .filter(|(value, _, _)| *value > 0.0)
                .map(|(_, _, action)| action)
                .or_else(|| best_rebase.map(|(_, _, action)| action));
        }

        let patrol = legal
            .iter()
            .filter_map(|action| match action {
                Action::AirPatrol {
                    unit: action_unit,
                    to,
                } if *action_unit == uid => {
                    let city_cover = g
                        .cities
                        .values()
                        .filter(|city| city.owner == pid && g.wdist(*to, city.pos) <= 1)
                        .map(|city| {
                            70.0 + city.pop as f64 * 4.0
                                + if Some(city.id) == plan.threatened_city {
                                    90.0
                                } else {
                                    0.0
                                }
                        })
                        .sum::<f64>();
                    let force_cover = g
                        .units
                        .values()
                        .filter(|other| {
                            other.owner == pid
                                && other.id != uid
                                && g.wdist(*to, other.pos) <= 1
                                && g.rules.units[other.kind].class == "military"
                        })
                        .map(|other| g.rules.units[other.kind].cost * 0.035)
                        .sum::<f64>();
                    let objective_distance = objective.map_or(0, |pos| g.wdist(*to, pos));
                    let existing = (unit.air_patrol_pos == Some(*to)) as i32 as f64 * 8.0;
                    Some((
                        city_cover + force_cover + existing - objective_distance as f64 * 2.0,
                        *to,
                        action.clone(),
                    ))
                }
                _ => None,
            })
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| right.1.cmp(&left.1))
            })
            .map(|(_, _, action)| action);
        let hostile_air_threat = g
            .units
            .values()
            .filter(|other| {
                other.owner != pid
                    && g.is_at_war(pid, other.owner)
                    && g.rules.units[other.kind].domain.as_deref() == Some("air")
            })
            .map(|other| {
                let distance = g.wdist(unit.pos, other.pos);
                if distance <= g.air_rebase_range(uid) {
                    80.0 + g.rules.units[other.kind].cost * 0.08
                } else {
                    0.0
                }
            })
            .fold(0.0_f64, f64::max);
        let defended_city = plan.threatened_city.is_some_and(|city| {
            g.cities.get(&city).is_some_and(|city| {
                legal.iter().any(|action| {
                    matches!(action, Action::AirPatrol { unit, to }
                        if *unit == uid && g.wdist(*to, city.pos) <= 1)
                })
            })
        });
        let patrol_value = hostile_air_threat
            + if defended_city { 55.0 } else { 0.0 }
            + if g
                .players
                .iter()
                .any(|other| other.id != pid && g.is_at_war(pid, other.id))
            {
                16.0
            } else {
                5.0
            };
        if let Some((value, _, action)) = best_attack {
            if value > patrol_value {
                return Some(action);
            }
        }
        if let Some((value, _, action)) = best_rebase {
            if value > patrol_value && hostile_air_threat <= 0.0 {
                return Some(action);
            }
        }
        patrol
    }

    /// Condemning a foreign Missionary or Apostle destroys it and pushes our
    /// own Pressure back — the standing military answer to a religious
    /// offensive. Previously this only fired when an enemy religious unit
    /// happened to already share our tile, which almost never happens, so
    /// the counter was effectively dead. Now a military unit will step onto
    /// an adjacent one and condemn it.
    fn condemn_step(&mut self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let condemnable = |game: &Game, at: Pos| -> Option<u32> {
            game.units_at(at).into_iter().find(|target| {
                let target = &game.units[target];
                target.owner != pid
                    && game.is_at_war(pid, target.owner)
                    && game.rules.units[target.kind].class == "religious"
            })
        };
        let here = g.units[&uid].pos;
        if let Some(target_unit) = condemnable(g, here) {
            if g.apply(pid, &Action::CondemnHeretic { unit: uid, target_unit }).is_ok() {
                return true;
            }
        }
        // Only chase intruders around our own territory: a lone unit running
        // down missionaries across the map abandons the campaign.
        let near_home = g
            .cities
            .values()
            .any(|city| city.owner == pid && g.wdist(here, city.pos) <= 6);
        if !near_home {
            return false;
        }
        let mut targets: Vec<Pos> = g
            .nbrs(here)
            .into_iter()
            .filter(|n| condemnable(g, *n).is_some() && g.can_move(uid, *n))
            .collect();
        targets.sort();
        let Some(to) = targets.first().copied() else {
            return false;
        };
        if g.apply(pid, &Action::Move { unit: uid, to }).is_err() {
            return false;
        }
        if let Some(target_unit) = condemnable(g, to) {
            let _ = g.apply(pid, &Action::CondemnHeretic { unit: uid, target_unit });
        }
        true
    }

    #[cfg(test)]
    fn advanced_military_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
    ) -> bool {
        let decline_settlers = self.counts(g, pid).settlers > 0
            || !self.base.has_practical_settle_site(g, pid);
        self.advanced_military_step_with_decline(g, pid, uid, plan, decline_settlers)
    }

    fn advanced_military_step_with_decline(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
        decline_settlers: bool,
    ) -> bool {
        let unit = g.units[&uid].clone();
        let rules = std::sync::Arc::clone(&g.rules);
        let spec = &rules.units[unit.kind];
        let doctrine = BasicAi::unit_doctrine(g, uid);
        let unwanted_settler_adjacent = decline_settlers
            && g.nbrs(unit.pos).into_iter().any(|position| {
                g.units_at(position).iter().any(|other| {
                    let other = &g.units[other];
                    other.owner != pid
                        && g.is_at_war(pid, other.owner)
                        && other.kind == "settler"
                })
            });
        if !unwanted_settler_adjacent
            && self.victory_planning
            && spec.class == "military"
            && self.condemn_step(g, pid, uid)
        {
            return true;
        }
        let holding_threatened_city = plan.threatened_city.is_some_and(|cid| {
            g.cities
                .get(&cid)
                .is_some_and(|city| g.wdist(unit.pos, city.pos) <= 3)
        });
        if !unwanted_settler_adjacent && !holding_threatened_city {
            if let Some(acted) = self.base.healing_step(g, pid, uid) {
                return acted;
            }
        }
        if self
            .base
            .capture_adjacent_civilian(g, pid, uid, decline_settlers)
        {
            return true;
        }
        if matches!(doctrine, UnitDoctrine::AirDefense | UnitDoctrine::AirStrike) {
            let Some(action) = self.advanced_air_action(g, pid, uid, plan) else {
                return false;
            };
            let changes_force_picture = matches!(
                &action,
                Action::AirStrike { .. } | Action::PriorityTarget { .. }
            );
            let acted = g.apply(pid, &action).is_ok();
            self.force_groups_dirty |= acted && changes_force_picture;
            return acted;
        }
        if let Some(action) = self.base.doctrine_action(g, pid, uid) {
            let changes_force_picture = matches!(
                &action,
                Action::Attack { .. }
                    | Action::Ranged { .. }
                    | Action::AirStrike { .. }
                    | Action::PriorityTarget { .. }
            );
            let acted = g.apply(pid, &action).is_ok();
            self.force_groups_dirty |= acted && changes_force_picture;
            return acted;
        }
        if !unwanted_settler_adjacent {
            if let Some(city) = self.occupation_garrison_target(g, pid, uid) {
                if unit.pos != city {
                    return self.base.step_toward(g, pid, uid, city);
                }
                return self.base.fortify_or_stop(g, pid, uid);
            }
        }
        let enemies: Vec<usize> = g
            .players
            .iter()
            .filter(|p| p.id != pid && p.alive && !p.is_barbarian && g.is_at_war(pid, p.id))
            .map(|p| p.id)
            .collect();
        if enemies.is_empty() {
            if spec.domain.as_deref() == Some("sea") {
                if let Some(settler) = unit
                    .linked_to
                    .filter(|peer| g.units.get(peer).is_some_and(|peer| peer.kind == "settler"))
                {
                    if let Some(target) = self.settler_targets.get(&settler).copied() {
                        let approach = BasicAi::naval_approach(g, uid, target).unwrap_or(target);
                        if approach != unit.pos && self.base.step_toward(g, pid, uid, approach) {
                            return true;
                        }
                    }
                    return self.base.fortify_or_stop(g, pid, uid);
                }
            }
            if let Some(acted) = self.campaign_staging_step(g, pid, uid, plan) {
                return acted;
            }
            return self.base.military_step(g, pid, uid);
        }
        // Combat can change occupancy, local power and the best focus target.
        // Movement cannot change the opposing force, so keep the existing
        // orders until an attack actually dirties them. The former
        // unconditional rebuild before every step made a turn with `n` units
        // repeatedly regroup and rescore all `n` after each unit moved.
        if self.victory_planning && self.force_groups_dirty {
            self.rebuild_force_groups(g, pid, plan);
            self.force_groups_dirty = false;
        }
        // A live rush executes its own siege. See `rush_siege_step`: the
        // general force-group heuristics assemble the stack correctly and then
        // will not put it on the city's ring, and four attempts to make them
        // do so each measured worse.
        if let Some(acted) = self.rush_siege_step(g, pid, uid, plan) {
            self.force_groups_dirty = true;
            return acted;
        }
        let group = self
            .force_groups
            .iter()
            .find(|group| group.units.contains(&uid))
            .cloned();

        let radius = if spec.has_ranged_attack() {
            g.unit_attack_range(uid).max(1)
        } else {
            1
        };
        let mut best: Option<(f64, Pos, Action)> = None;
        for pos in g.wdisk(unit.pos, radius) {
            if spec.class != "military" {
                break;
            }
            if pos == unit.pos || !self.base.is_enemy_tile(g, pos, &enemies) {
                continue;
            }
            let unusable_settler = g
                .units_at(pos)
                .iter()
                .any(|oid| g.units[oid].kind == "settler" && decline_settlers);
            if unusable_settler && g.city_at(pos).is_none() {
                continue;
            }
            let distance = g.wdist(unit.pos, pos);
            let mut actions = Vec::with_capacity(2);
            if spec.has_ranged_attack() && distance <= g.unit_attack_range(uid) {
                actions.push(Action::Ranged {
                    unit: uid,
                    target: pos,
                });
            }
            if unit.kind == "spec_ops"
                && distance <= g.unit_attack_range(uid)
                && g.priority_support_target_at(pid, pos).is_some()
            {
                actions.push(Action::PriorityTarget {
                    unit: uid,
                    target: pos,
                });
            }
            if spec.is_melee_capable() && distance == 1 {
                actions.push(Action::Attack {
                    unit: uid,
                    target: pos,
                });
            }
            for action in actions {
                let mut score = self.tactical_attack_value(g, pid, uid, &action, plan)
                    - self.base.attack_threshold(g, uid, pos);
                if plan
                    .target_city
                    .is_some_and(|cid| g.cities.get(&cid).is_some_and(|c| c.pos == pos))
                {
                    score += 28.0;
                }
                if g.units_at(pos).iter().any(|oid| g.units[oid].hp <= 35) {
                    score += 16.0;
                }
                if group.as_ref().and_then(|orders| orders.focus_target) == Some(pos) {
                    score += self.base.w.focus_fire * 10.0;
                }
                if let Some(orders) = &group {
                    score -= self.base.w.local_superiority
                        * (1.0 - orders.local_strength_ratio).max(0.0);
                }
                score -=
                    self.base.w.trade_caution * self.forcing_reply_penalty(g, pid, uid, &action);
                if best
                    .as_ref()
                    .map(|(old, bp, _)| score > *old || (score == *old && pos < *bp))
                    .unwrap_or(true)
                {
                    best = Some((score, pos, action));
                }
            }
        }
        if let Some((score, at, action)) = best {
            let required_margin = if unit.hp < 55 { 12.0 } else { 0.0 };
            if score > required_margin {
                if self.journal().wants(crate::reasoning::Level::Detail) {
                    let verb = match &action {
                        Action::Ranged { .. } => "shells",
                        Action::PriorityTarget { .. } => "targets",
                        _ => "attacks",
                    };
                    let defender = g
                        .city_at(at)
                        .and_then(|cid| g.cities.get(&cid))
                        .map(|city| city.name.clone())
                        .or_else(|| {
                            g.units_at(at)
                                .first()
                                .map(|oid| plain(&g.units[oid].kind))
                        })
                        .unwrap_or_else(|| format!("{at:?}"));
                    let orders = group
                        .as_ref()
                        .map(|orders| {
                            format!("{} group at {:.2} local strength",
                                    orders.posture.as_str(), orders.local_strength_ratio)
                        })
                        .unwrap_or_else(|| "unattached".to_string());
                    think!(self.journal(), Military, Detail,
                           "{} {verb} {defender}", plain(&unit.kind);
                           "worth {score:.0} over a margin of {required_margin:.0}, \
                            on {} health, {orders}", unit.hp; at);
                }
                if g.apply(pid, &action).is_ok() {
                    self.force_groups_dirty = true;
                    return true;
                }
            }
        }

        let linked_settler = (spec.domain.as_deref() == Some("sea"))
            .then_some(unit.linked_to)
            .flatten()
            .filter(|peer| g.units.get(peer).is_some_and(|peer| peer.kind == "settler"));
        let hostile_water_unit = g
            .units
            .values()
            .any(|enemy| enemies.contains(&enemy.owner) && BasicAi::waterborne(g, enemy.id));
        if !hostile_water_unit {
            if let Some(settler) = linked_settler {
                if let Some(target) = self.settler_targets.get(&settler).copied() {
                    let approach = BasicAi::naval_approach(g, uid, target).unwrap_or(target);
                    if approach != unit.pos && self.base.step_toward(g, pid, uid, approach) {
                        return true;
                    }
                }
                return self.base.fortify_or_stop(g, pid, uid);
            }
        }

        if unwanted_settler_adjacent {
            return self.base.fortify_or_stop(g, pid, uid);
        }

        if doctrine == UnitDoctrine::Recon
            && self.base.should_explore(g, pid, uid, true)
            && self.base.explore_step(g, pid, uid)
        {
            return true;
        }

        let defend_target = plan.threatened_city.and_then(|cid| {
            let city = g.cities.get(&cid)?;
            g.units
                .values()
                .filter(|u| enemies.contains(&u.owner) && g.wdist(city.pos, u.pos) <= 7)
                .min_by_key(|u| (g.wdist(unit.pos, u.pos), u.id))
                .map(|u| u.pos)
        });
        let campaign = if spec.domain.as_deref() == Some("sea") {
            defend_target
                .filter(|pos| g.map.get(*pos).is_some_and(|tile| g.rules.is_water(tile)))
                .or_else(|| self.base.nearest_enemy_for_unit(g, pid, uid, &enemies))
        } else {
            defend_target
                .or_else(|| {
                    plan.target_city
                        .and_then(|cid| g.cities.get(&cid).map(|c| c.pos))
                })
                .or_else(|| self.base.nearest_enemy(g, pid, uid, &enemies))
        };
        if let Some(orders) = &group {
            return self.coordinated_tactical_step(
                g,
                pid,
                uid,
                orders,
                &enemies,
                decline_settlers,
            );
        }
        match campaign {
            Some(target) => self
                .base
                .tactical_step(g, pid, uid, target, &enemies, radius),
            // Nothing this unit is willing to fight: explore or garrison
            // rather than shadowing a raider it will never strike.
            None => self.base.peacetime_step(g, pid, uid),
        }
    }

    fn promotion_value(&self, g: &Game, name: &str, strategy: GrandStrategy) -> f64 {
        let promotion = &g.rules.promotions[name];
        let mut value = promotion.tier as f64 * 4.0;
        for (effect, amount) in &promotion.effects {
            let weight = match effect.as_str() {
                "extra_attacks" => 70.0,
                "range" => 55.0,
                "attack_after_move" => 48.0,
                "move_after_attack" => 42.0,
                "heal_anywhere" => 38.0,
                "escort_mobility" => 32.0,
                "zone_of_control" | "camouflage" => 28.0,
                "movement" => 20.0,
                "support_multiplier" | "flanking_multiplier" => 18.0,
                "sight" | "see_through_woods" => 15.0,
                "pillage_cost" | "scale_cliffs" | "amphibious" => 14.0,
                "woods_move_cost" | "hills_move_cost" => 12.0,
                name if name.starts_with("rock_") && name.ends_with("_levels") => 110.0,
                "rock_nature_venue" | "rock_space_venue" | "rock_surf_venue" => 150.0,
                "rock_nearby_tourism_pct" => 6.0,
                "rock_gold_pct" => 2.5,
                "rock_loyalty_loss" => 1.5,
                "rock_convert_city" if strategy == GrandStrategy::Religion => 350.0,
                "rock_convert_city" => 50.0,
                "combat_all" => 4.0,
                name if name.starts_with("attack_")
                    || name.starts_with("ranged_")
                    || name.starts_with("siege_")
                    || name.starts_with("vs_") =>
                {
                    3.5
                }
                name if name.starts_with("defend_") || name.ends_with("_defense") => 3.0,
                _ => 2.0,
            };
            value += weight * amount;
        }
        match strategy {
            GrandStrategy::Conquest => value * 1.18,
            GrandStrategy::Recovery => {
                value
                    + promotion
                        .effects
                        .iter()
                        .filter(|(effect, _)| {
                            effect.starts_with("defend_") || effect.ends_with("_defense")
                        })
                        .map(|(_, amount)| 2.0 * amount)
                        .sum::<f64>()
            }
            _ => value,
        }
    }

    fn advanced_promotions(&self, g: &mut Game, pid: usize, strategy: GrandStrategy) {
        for uid in g.player_unit_ids(pid) {
            let promotions = g.available_promotions(uid);
            let choice = promotions.into_iter().max_by(|a, b| {
                self.promotion_value(g, a, strategy)
                    .partial_cmp(&self.promotion_value(g, b, strategy))
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| b.cmp(a))
            });
            if let Some(promotion) = choice {
                let _ = g.apply(
                    pid,
                    &Action::Promote {
                        unit: uid,
                        promotion: Name::new(&promotion),
                    },
                );
            }
        }
    }

    fn advanced_formations(&self, g: &mut Game, pid: usize) {
        let reserve = (g.player_city_ids(pid).len() + 3).max(5);
        let military: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| g.rules.units[g.units[uid].kind].class == "military")
            .collect();
        let max_combinations = military.len().saturating_sub(reserve);
        let mut pairs = Vec::new();
        for (index, unit) in military.iter().enumerate() {
            for with in &military[index + 1..] {
                let a = &g.units[unit];
                let b = &g.units[with];
                let valid_formation = match (a.formation, b.formation) {
                    (0, 0) => g.players[pid].civics.contains(&crate::name!("nationalism")),
                    (0, 1) | (1, 0) => g.players[pid].civics.contains(&crate::name!("mobilization")),
                    _ => false,
                };
                if a.kind != b.kind
                    || a.linked_to.is_some()
                    || b.linked_to.is_some()
                    || a.moves_left <= 0.0
                    || b.moves_left <= 0.0
                    || g.wdist(a.pos, b.pos) > 1
                    || !valid_formation
                {
                    continue;
                }
                let army = (a.formation.max(b.formation) == 1) as i64;
                let score = army * 100 + a.xp.max(b.xp) + a.hp.max(b.hp) as i64 / 10;
                pairs.push((
                    std::cmp::Reverse(score),
                    (*unit).min(*with),
                    (*unit).max(*with),
                ));
            }
        }
        pairs.sort_unstable();
        let mut used = HashSet::new();
        let mut combined = 0;
        for (_, unit, with) in pairs {
            if combined >= max_combinations || !used.insert(unit) || !used.insert(with) {
                continue;
            }
            if g.apply(pid, &Action::CombineUnits { unit, with }).is_ok() {
                combined += 1;
            }
        }

        let support: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                g.rules.units[g.units[uid].kind].class == "support"
                    && g.units[uid].kind != "military_engineer"
                    && g.units[uid].linked_to.is_none()
            })
            .collect();
        for with in support {
            let pos = g.units[&with].pos;
            let escort = g
                .units_at(pos)
                .into_iter()
                .filter(|unit| {
                    let unit = &g.units[unit];
                    unit.owner == pid
                        && unit.linked_to.is_none()
                        && g.rules.units[unit.kind].class == "military"
                })
                .max_by_key(|unit| {
                    let unit = &g.units[unit];
                    (
                        !g.rules.units[unit.kind].has_ranged_attack(),
                        g.unit_strength(unit, true) as i64,
                        std::cmp::Reverse(unit.id),
                    )
                });
            if let Some(unit) = escort {
                let _ = g.apply(pid, &Action::LinkUnits { unit, with });
            }
        }

        let embarked_settlers: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let unit = &g.units[uid];
                unit.kind == "settler"
                    && unit.linked_to.is_none()
                    && g.map
                        .get(unit.pos)
                        .is_some_and(|tile| g.rules.is_water(tile))
            })
            .collect();
        for with in embarked_settlers {
            let escort = g.units_at(g.units[&with].pos).into_iter().find(|uid| {
                let unit = &g.units[uid];
                unit.owner == pid
                    && unit.linked_to.is_none()
                    && g.rules.units[unit.kind].domain.as_deref() == Some("sea")
            });
            if let Some(unit) = escort {
                let _ = g.apply(pid, &Action::LinkUnits { unit, with });
            }
        }
    }

    fn defensive_strike_value(&self, g: &Game, pid: usize, action: &Action) -> f64 {
        let target = match action {
            Action::CityStrike { target, .. } | Action::EncampmentStrike { target, .. } => *target,
            _ => return f64::NEG_INFINITY,
        };
        let defenders: Vec<(u32, i32, f64, f64, bool, bool)> = g
            .units_at(target)
            .into_iter()
            .filter_map(|unit| {
                let defender = &g.units[&unit];
                let spec = &g.rules.units[defender.kind];
                (defender.owner != pid
                    && g.is_at_war(pid, defender.owner)
                    && spec.class == "military")
                    .then_some((
                        unit,
                        defender.hp,
                        g.unit_strength(defender, true),
                        spec.cost,
                        spec.siege,
                        !spec.has_ranged_attack(),
                    ))
            })
            .collect();
        let mut after = g.clone();
        if after.apply(pid, action).is_err() {
            return f64::NEG_INFINITY;
        }
        defenders
            .into_iter()
            .map(
                |(unit, hp, strength, cost, siege, captures)| match after.units.get(&unit) {
                    None => {
                        180.0
                            + cost * 0.45
                            + strength * 2.0
                            + if siege { 70.0 } else { 0.0 }
                            + if captures { 30.0 } else { 0.0 }
                    }
                    Some(defender) => {
                        (hp - defender.hp).max(0) as f64 * (1.0 + strength / 100.0)
                            + if siege { 25.0 } else { 0.0 }
                            + if captures { 8.0 } else { 0.0 }
                    }
                },
            )
            .sum()
    }

    fn advanced_encampment_strikes(&self, g: &mut Game, pid: usize) {
        let has_ready_encampment = g.player_city_ids(pid).into_iter().any(|cid| {
            let city = &g.cities[&cid];
            city.encampment_hp > 0
                && city.encampment_wall_hp > 0
                && !city.encampment_pillaged
                && !city.encampment_struck
        });
        if !has_ready_encampment {
            return;
        }
        let mut best: BTreeMap<u32, (f64, Pos)> = BTreeMap::new();
        for action in g.legal_actions_within(pid, ActionFamilies::CORE) {
            let Action::EncampmentStrike { city, target } = action else {
                continue;
            };
            let strike = Action::EncampmentStrike { city, target };
            let target_value = self.defensive_strike_value(g, pid, &strike);
            let candidate = (target_value, target);
            if best.get(&city).is_none_or(|old| {
                target_value.total_cmp(&old.0).is_gt()
                    || (target_value.total_cmp(&old.0).is_eq() && target < old.1)
            }) {
                best.insert(city, candidate);
            }
        }
        for (city, (_, target)) in best {
            let _ = g.apply(pid, &Action::EncampmentStrike { city, target });
        }
    }

    /// Fire every available city-center strike, choosing each target from an
    /// exact cloned result. Explicit-victory agents do not run the Basic city
    /// governor after the opening, so this command phase is the authoritative
    /// path for walls (including Victor's extra strike).
    fn advanced_city_strikes(&self, g: &mut Game, pid: usize) {
        loop {
            let candidates: Vec<Action> = g
                .legal_actions_within(pid, ActionFamilies::CORE)
                .into_iter()
                .filter(|action| matches!(action, Action::CityStrike { .. }))
                .collect();
            let best = candidates.into_iter().max_by(|left, right| {
                self.defensive_strike_value(g, pid, left)
                    .total_cmp(&self.defensive_strike_value(g, pid, right))
                    .then_with(|| format!("{right:?}").cmp(&format!("{left:?}")))
            });
            let Some(action) = best else { break };
            if g.apply(pid, &action).is_err() {
                break;
            }
        }
    }

    fn advanced_command_actions(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        self.advanced_city_strikes(g, pid);
        self.advanced_encampment_strikes(g, pid);
        self.advanced_wmd_strikes(g, pid, plan);
        self.advanced_promotions(g, pid, plan.strategy);
        self.advanced_formations(g, pid);
    }

    /// Nuclear doctrine: a Conquest empire at war spends a stockpiled device
    /// on the hardest enemy city in range — the one whose walls and garrison
    /// would cost the most to break conventionally — and never on a blast
    /// that would touch its own cities or units.
    fn advanced_wmd_strikes(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if plan.strategy != GrandStrategy::Conquest {
            return;
        }
        let candidates: Vec<(Action, Pos, bool)> = g
            .legal_actions_within(pid, ActionFamilies::EMPIRE)
            .into_iter()
            .filter_map(|action| match action {
                Action::WmdStrike {
                    target,
                    thermonuclear,
                    ..
                } => Some((action.clone(), target, thermonuclear)),
                _ => None,
            })
            .collect();
        let mut best: Option<(f64, Action)> = None;
        for (action, target, thermonuclear) in candidates {
            let radius = g.rules.wmds[if thermonuclear {
                "thermonuclear_device"
            } else {
                "nuclear_device"
            }]
            .blast_radius;
            let blast = g.wdisk(target, radius);
            let friendly_exposure = blast.iter().any(|position| {
                g.city_at(*position)
                    .is_some_and(|city| g.cities[&city].owner == pid)
                    || g.units_at(*position)
                        .into_iter()
                        .any(|uid| g.units[&uid].owner == pid)
            });
            if friendly_exposure {
                continue;
            }
            let Some(city) = g.city_at(target) else {
                continue;
            };
            let garrison = blast
                .iter()
                .flat_map(|position| g.units_at(*position))
                .filter(|uid| g.is_at_war(pid, g.units[uid].owner))
                .count();
            let city_ref = &g.cities[&city];
            let hardness = g.city_strength(city) + city_ref.wall_hp as f64 / 10.0;
            // A device is worth spending only on a genuinely hard target.
            if hardness < 50.0 && garrison < 3 {
                continue;
            }
            let value = hardness + garrison as f64 * 12.0;
            if best.as_ref().is_none_or(|(current, _)| value > *current) {
                best = Some((value, action));
            }
        }
        if let Some((_, action)) = best {
            let _ = g.apply(pid, &action);
        }
    }

    fn advanced_units(&mut self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        self.base.begin_movement_turn(g, pid);
        if self.victory_planning {
            self.rebuild_force_groups(g, pid, plan);
        } else {
            self.force_groups.clear();
        }
        self.force_groups_dirty = false;
        let religious_offensive = self.religious_offensive_posture(g, pid, plan.strategy);
        // Settlement feasibility is an empire/map question, not a unit
        // question. It used to rescan the known world and recompute empire
        // counts up to eight times for every military unit in the same turn.
        let decline_settlers = self.counts(g, pid).settlers > 0
            || !self.base.has_practical_settle_site(g, pid);
        let mut ids = g.player_unit_ids(pid);
        ids.sort_by_key(|uid| {
            let u = &g.units[uid];
            let spec = &g.rules.units[u.kind];
            let order = match u.kind.as_str() {
                "settler" => 0,
                "builder" => 1,
                "naturalist" => 1,
                "archaeologist" => 1,
                "trader" => 2,
                "missionary" => 3,
                "rock_band" => 3,
                _ if spec.has_ranged_attack() && !spec.siege => 4,
                _ if spec.siege => 5,
                _ => 6,
            };
            (order, *uid)
        });
        for uid in ids {
            let mut took_a_turn = false;
            for _ in 0..8 {
                if !g.units.contains_key(&uid) || g.units[&uid].moves_left <= 0.0 {
                    break;
                }
                let kind = g.units[&uid].kind.clone();
                let class = g.rules.units[kind].class.clone();
                let acted = match kind.as_str() {
                    "settler" => self.advanced_settler_step(g, pid, uid),
                    "builder" => self.advanced_builder_step(g, pid, uid, plan.strategy),
                    "military_engineer" => self.base.military_engineer_step(g, pid, uid),
                    "naturalist" => self.base.naturalist_step(g, pid, uid),
                    "archaeologist" => self.base.archaeologist_step(g, pid, uid),
                    "trader" => self.advanced_trader_step(g, pid, uid, plan.strategy),
                    "missionary" if self.victory_planning => self.advanced_missionary_step(
                        g,
                        pid,
                        uid,
                        religious_offensive,
                    ),
                    "missionary" => self.base.missionary_step(g, pid, uid),
                    "rock_band" => self.base.rock_band_step(g, pid, uid),
                    _ if self.victory_planning && class == "religious" => self
                        .advanced_religious_step(
                            g,
                            pid,
                            uid,
                            religious_offensive,
                        ),
                    _ => self.advanced_military_step_with_decline(
                        g,
                        pid,
                        uid,
                        plan,
                        decline_settlers,
                    ),
                };
                if !acted {
                    break;
                }
                took_a_turn = true;
            }
            if !took_a_turn {
                self.base.hold_stood_down_unit(g, pid, uid);
            }
        }
        self.settler_targets
            .retain(|uid, _| g.units.contains_key(uid));
        self.builder_targets
            .retain(|uid, _| g.units.contains_key(uid));
    }

    /// Evaluate each legal city disposition on a cloned position, then play
    /// the best resulting state. This is the same separation used by strong
    /// chess engines: generate a very small set of forcing candidates, make
    /// each move, and compare the resulting position with strategy-sensitive
    /// terms instead of relying on a single local rule.
    fn resolve_city_dispositions(&mut self, g: &mut Game, pid: usize, strategy: GrandStrategy) {
        loop {
            let candidates: Vec<Action> = g
                .legal_city_disposition_actions(pid)
                .into_iter()
                .filter(|action| {
                    matches!(
                        action,
                        Action::KeepCity { .. }
                            | Action::RazeCity { .. }
                            | Action::LiberateCity { .. }
                    )
                })
                .collect();
            if candidates.is_empty() {
                break;
            }
            let mut best: Option<(f64, Action)> = None;
            for action in candidates {
                let mut next = g.clone();
                if next.apply(pid, &action).is_err() {
                    continue;
                }
                let value = self.city_disposition_value(g, &next, pid, strategy, &action);
                if best.as_ref().is_none_or(|(old, _)| value > *old + 1e-9) {
                    best = Some((value, action));
                }
            }
            let Some((_, action)) = best else { break };
            if g.apply(pid, &action).is_err() {
                break;
            }
        }
    }

    /// Immediate population-pressure component of a captured city's Loyalty
    /// change. Governors, policies, and projects can improve this later, but a
    /// city that will revolt in two or three turns cannot wait for them to be
    /// established. Mirroring the rules engine's pressure equation here lets
    /// the mandatory keep/raze decision price that short horizon explicitly.
    fn population_loyalty_delta(g: &Game, pid: usize, city_id: u32) -> f64 {
        Self::population_loyalty_delta_with_capture(g, pid, city_id, false)
    }

    /// Forecast the pressure immediately after a conquest without cloning the
    /// entire world. The target changes sides and retains the ceiling of 75%
    /// Population, exactly as `Game::transfer_city` will do.
    fn population_loyalty_delta_with_capture(
        g: &Game,
        pid: usize,
        city_id: u32,
        project_capture: bool,
    ) -> f64 {
        let city = &g.cities[&city_id];
        let age_factor = |owner: usize| match g.players[owner].age.as_str() {
            "golden" | "heroic" => 1.5,
            "dark" => 0.5,
            _ => 1.0,
        };
        let mut domestic = 0.0;
        let mut foreign = 0.0;
        for source in g.cities.values() {
            let source_owner = if project_capture && source.id == city_id {
                pid
            } else {
                source.owner
            };
            if g.players[source_owner].is_minor || g.players[source_owner].is_barbarian {
                continue;
            }
            let distance = g.wdist(source.pos, city.pos);
            if distance > 9 {
                continue;
            }
            let source_pop = if project_capture && source.id == city_id {
                ((source.pop * 3 + 3) / 4).max(1)
            } else {
                source.pop
            };
            let mut pressure = source_pop as f64
                * (10 - distance) as f64
                * age_factor(source_owner);
            if source.is_capital && source.original_owner == source_owner {
                pressure += source_pop as f64;
            }
            if source_owner == pid {
                domestic += pressure;
            } else if !g.same_team(pid, source_owner)
                && !g
                    .alliance_with(pid, source_owner)
                    .is_some_and(|alliance| alliance.kind == "cultural")
            {
                foreign += pressure;
            }
        }
        (10.0 * (domestic - foreign) / (domestic.min(foreign) + 0.5)).clamp(-20.0, 20.0)
    }

    /// A city that cannot be razed or liberated should not be captured merely
    /// to hand it back through Loyalty and attack it again. Wait only when the
    /// projected revolt is imminent; eliminating the defender or completing
    /// Domination remains decisive enough to take immediately.
    fn should_defer_city_capture(g: &Game, pid: usize, city_id: u32) -> bool {
        let city = &g.cities[&city_id];
        let razable = !city.is_capital
            && !g.players[city.original_owner].is_minor
            && city.original_owner != pid
            && !g.are_allied(pid, city.original_owner);
        let liberatable = city.original_owner != pid
            && city.owner != city.original_owner
            && g.players
                .get(city.original_owner)
                .is_some_and(|founder| !founder.is_barbarian);
        if razable || liberatable || g.player_city_ids(city.owner).len() <= 1 {
            return false;
        }

        let completes_domination = g
            .players
            .iter()
            .filter(|candidate| {
                !candidate.is_minor
                    && !candidate.is_barbarian
                    && !g.same_team(pid, candidate.id)
            })
            .all(|original_owner| {
                if original_owner.id == pid {
                    return true;
                }
                g.cities
                    .values()
                    .find(|candidate| {
                        candidate.is_capital && candidate.original_owner == original_owner.id
                    })
                    .is_none_or(|capital| capital.id == city_id || capital.owner == pid)
            });
        if completes_domination {
            return false;
        }

        let loyalty_delta =
            Self::population_loyalty_delta_with_capture(g, pid, city_id, true);
        let turns_to_flip = if loyalty_delta < 0.0 {
            50.0 / -loyalty_delta
        } else {
            f64::INFINITY
        };
        loyalty_delta <= -8.0 && turns_to_flip <= 4.0
    }

    fn city_disposition_value(
        &self,
        before: &Game,
        after: &Game,
        pid: usize,
        strategy: GrandStrategy,
        action: &Action,
    ) -> f64 {
        if after.winner == Some(pid) {
            return 1_000_000_000.0;
        }
        if after.winner.is_some() {
            return -1_000_000_000.0;
        }
        let player = &after.players[pid];
        let yield_weights = match strategy {
            GrandStrategy::Science => Yields {
                food: 1.0,
                production: 1.5,
                gold: 0.7,
                science: 2.8,
                culture: 0.8,
                faith: 0.3,
            },
            GrandStrategy::Culture => Yields {
                food: 1.0,
                production: 1.2,
                gold: 0.8,
                science: 0.7,
                culture: 2.8,
                faith: 0.8,
            },
            GrandStrategy::Religion => Yields {
                food: 1.0,
                production: 1.1,
                gold: 0.6,
                science: 0.5,
                culture: 0.8,
                faith: 3.0,
            },
            GrandStrategy::Diplomacy => Yields {
                food: 1.0,
                production: 1.0,
                gold: 1.5,
                science: 0.8,
                culture: 1.0,
                faith: 0.5,
            },
            GrandStrategy::Conquest | GrandStrategy::Recovery => Yields {
                food: 1.1,
                production: 2.3,
                gold: 1.0,
                science: 0.8,
                culture: 0.7,
                faith: 0.3,
            },
            GrandStrategy::Expansion => Yields {
                food: 1.7,
                production: 1.8,
                gold: 0.9,
                science: 0.8,
                culture: 0.8,
                faith: 0.4,
            },
        };
        let weighted = |yields: Yields| {
            yields.food * yield_weights.food
                + yields.production * yield_weights.production
                + yields.gold * yield_weights.gold
                + yields.science * yield_weights.science
                + yields.culture * yield_weights.culture
                + yields.faith * yield_weights.faith
        };
        let economy = after
            .player_city_ids(pid)
            .into_iter()
            .map(|city| weighted(after.city_yields(city)))
            .sum::<f64>();
        let grievances = after
            .players
            .iter()
            .filter(|observer| observer.id != pid)
            .map(|observer| observer.grievances.get(&pid).copied().unwrap_or(0.0))
            .sum::<f64>();
        let grievance_weight = if strategy == GrandStrategy::Diplomacy {
            0.75
        } else {
            0.12
        };
        let favor_weight = if strategy == GrandStrategy::Diplomacy {
            2.5
        } else {
            0.15
        };
        let mut value = after.score(pid) as f64 * 6.0
            + economy * 3.0
            + after.military_power(pid) * 0.3
            + player.gold * 0.02
            + player.faith * 0.02
            + player.diplomatic_favor * favor_weight
            + player.dvp as f64 * 140.0
            - grievances * grievance_weight;

        value += match strategy {
            GrandStrategy::Science => {
                player.techs.len() as f64 * 8.0 + player.science_projects.len() as f64 * 80.0
            }
            GrandStrategy::Culture => {
                player.culture_lifetime * 0.02 + player.tourism_lifetime * 0.06
            }
            GrandStrategy::Religion => player.faith * 0.08,
            GrandStrategy::Diplomacy => {
                after
                    .players
                    .iter()
                    .filter(|minor| {
                        minor.is_minor
                            && !minor.is_barbarian
                            && minor.alive
                            && after.suzerain_of(minor.id) == Some(pid)
                    })
                    .count() as f64
                    * 35.0
            }
            GrandStrategy::Conquest => {
                let capitals = after
                    .cities
                    .values()
                    .filter(|city| {
                        city.owner == pid && city.is_capital && city.original_owner != pid
                    })
                    .count() as f64;
                capitals * 180.0 + after.military_power(pid) * 0.25
            }
            GrandStrategy::Expansion => after.player_city_ids(pid).len() as f64 * 20.0,
            GrandStrategy::Recovery => after.player_city_ids(pid).len() as f64 * 12.0,
        };

        let city_id = match action {
            Action::KeepCity { city }
            | Action::RazeCity { city }
            | Action::LiberateCity { city } => *city,
            _ => return value,
        };
        if let Some(city) = before.cities.get(&city_id) {
            let emergency_objective = before
                .active_emergencies
                .iter()
                .any(|emergency| emergency.city == city_id && emergency.members.contains(&pid));
            if emergency_objective {
                value += if matches!(action, Action::LiberateCity { .. }) {
                    100_000.0
                } else {
                    -100_000.0
                };
            }
            let nearest_core = before
                .cities
                .values()
                .filter(|other| other.owner == pid && other.id != city_id)
                .map(|other| before.wdist(city.pos, other.pos))
                .min()
                .unwrap_or(20);
            let development = city.pop.max(1) as f64 * 6.0
                + city.districts.len() as f64 * 12.0
                + city.wonders.len() as f64 * 35.0;
            let loyalty_delta = Self::population_loyalty_delta(before, pid, city_id);
            let turns_to_flip = if loyalty_delta < 0.0 {
                city.loyalty.max(0.0) / -loyalty_delta
            } else {
                f64::INFINITY
            };
            let disposable = !city.is_capital
                && !before.players[city.original_owner].is_minor
                && city.original_owner != pid
                && !before.are_allied(pid, city.original_owner);
            let imminent_low_value_revolt = development < 35.0 && turns_to_flip <= 4.0;
            let unsupported_revolt = nearest_core > 9 && turns_to_flip <= 8.0;
            let hopeless_occupation = disposable
                && matches!(strategy, GrandStrategy::Conquest | GrandStrategy::Recovery)
                && loyalty_delta <= -8.0
                && (imminent_low_value_revolt || unsupported_revolt);
            match action {
                Action::KeepCity { .. } => {
                    value += development;
                    if nearest_core > 9 && city.loyalty <= 50.0 {
                        value -= (nearest_core - 9) as f64 * 5.0;
                    }
                    if strategy == GrandStrategy::Conquest {
                        value += 35.0;
                    }
                    if hopeless_occupation {
                        value -= 240.0
                            + -loyalty_delta * 18.0
                            + (8.0 - turns_to_flip).max(0.0) * 30.0;
                    }
                }
                Action::RazeCity { .. } => {
                    value -= development * 0.4;
                    if strategy == GrandStrategy::Conquest && nearest_core > 9 && development < 35.0
                    {
                        value += 65.0;
                    }
                    if hopeless_occupation {
                        value += 120.0 + -loyalty_delta * 8.0;
                    }
                }
                Action::LiberateCity { .. } => {
                    if strategy == GrandStrategy::Diplomacy {
                        value += 100.0;
                    }
                    if before.players[city.original_owner].is_minor {
                        value += if strategy == GrandStrategy::Diplomacy {
                            70.0
                        } else {
                            15.0
                        };
                    }
                }
                _ => {}
            }
        }
        value
    }
}

impl Ai for AdvancedAi {
    fn strategy_label(&self) -> Option<&'static str> {
        self.plan.as_ref().map(|plan| plan.strategy.as_str())
    }

    fn plan_report(&self) -> Option<PlanReport> {
        let plan = self.plan.as_ref()?;
        Some(PlanReport {
            strategy: plan.strategy.as_str(),
            victory_target: self.victory_target.map(VictoryTarget::as_str),
            rush: plan.rush,
            target_player: plan.target_player,
            target_city: plan.target_city,
            threatened_city: plan.threatened_city,
            desired_cities: plan.desired_cities,
            assessed_turn: plan.assessed_turn,
            forces: self
                .force_groups
                .iter()
                .map(|group| ForceReport {
                    domain: group.domain.as_str(),
                    posture: group.posture.as_str(),
                    units: group.units.len(),
                    objective: group.objective,
                    readiness: group.readiness,
                    strength_ratio: group.local_strength_ratio,
                })
                .collect(),
        })
    }

    fn take_turn(&mut self, g: &mut Game, pid: usize) {
        // Stamp the context once, for every layer. Nothing below repeats the
        // turn number or the acting civilization.
        self.journal().begin_turn(g.turn, pid);
        g.with_deferred_visibility(|g| self.take_turn_inner(g, pid));
    }

    fn attach_journal(&mut self, journal: Journal) {
        self.base.attach_journal(journal);
    }
}

impl AdvancedAi {
    fn take_turn_inner(&mut self, g: &mut Game, pid: usize) {
        self.base.minor = g.players[pid].is_minor;
        self.base.barb = g.players[pid].is_barbarian;
        let active_victory_target = self.active_victory_target(g);
        self.base.pursue_religion = g.has_ability(pid, "taxis")
            || active_victory_target.is_none()
            || active_victory_target == Some(VictoryTarget::Religion);
        if self.base.minor || self.base.barb {
            self.base.take_turn(g, pid);
            return;
        }
        let rush_routes_frozen = self.freeze_rush_route_targets(g, pid);
        let disposition_strategy = active_victory_target
            .map(VictoryTarget::strategy)
            .unwrap_or_else(|| self.victory_focus(g, pid).strategy);
        self.resolve_city_dispositions(g, pid, disposition_strategy);
        self.observe_campaign(g, pid);
        if rush_routes_frozen || self.plan_stale(g, pid) {
            self.plan = Some(self.assess(g, pid));
        }
        let plan = self.plan.clone().unwrap();
        // Production for a Conquest plan without an assigned victory target
        // runs through `BasicAi::cities`, not `advanced_production`, so the
        // rush has to raise the standing-army floor there or it plans a war it
        // never builds an army for. Rewritten every turn, including back to
        // zero the turn the window shuts.
        self.base.rush_military_floor = if plan.rush { RUSH_ARMY } else { 0 };
        // Before anything spends this turn, tell each city what the empire
        // wants of it. Production, governors and the citizen governor all read
        // the same plan afterwards, so what a city builds and what its
        // citizens work stop being two unrelated decisions.
        if self.city_strategy {
            self.stamp_city_directives(g, pid, &plan);
        }
        if self.food_first != 0.0 {
            // Want food only while short of the target. Past it the extra
            // food buys nothing this treatment is arguing for, and the
            // production it costs is real.
            let short = g.player_city_ids(pid).len() < plan.desired_cities;
            if let Some(seat) = g.players.get_mut(pid) {
                seat.citizen_food_bias = if short { self.food_first } else { 0.0 };
            }
        }
        self.census.count(plan.strategy);
        self.advanced_research(g, pid, &plan);
        if self.victory_planning {
            let denied_rival = plan
                .target_player
                .filter(|target| self.rival_victory_pressure(g, *target).progress >= 78);
            self.advanced_envoys(g, pid, plan.strategy, denied_rival);
            self.advanced_secret_society(g, pid, plan.strategy);
        }
        // Spend Governor Titles against the same strategic plan before the
        // baseline ancillary pass can dilute them across empty cities.
        self.strategic_governors(g, pid, &plan);
        // Keep the mature ancillary systems: governments, policies, beliefs,
        // religions, and envoys. Research is already selected.
        self.base.research_without_government(g, pid);
        self.strategic_government(g, pid, plan.strategy);
        self.base.corporations(g, pid);
        self.advanced_products(g, pid, plan.strategy);
        self.advanced_great_people(g, pid, plan.strategy);
        if self.victory_planning && g.victory_conditions.religious {
            let committed = plan.strategy == GrandStrategy::Religion;
            let offensive = self.religious_offensive_posture(g, pid, plan.strategy);
            // A secondary campaign spends only the bank above a substantial
            // reserve, leaving Culture agents able to buy Naturalists or Rock
            // Bands and every other plan able to react to an emergency.
            let reserve = if committed {
                80.0
            } else {
                g.game_speed.scale(1_200.0)
            };
            self.religious_spending_with_reserve(g, pid, offensive, reserve);
        }
        self.faith_building_spending(g, pid, plan.strategy);
        self.military_faith_spending(g, pid, &plan);
        // Live spectator majors choose an adaptive plan instead of carrying
        // an explicit `victory_target`. Give both modes the same strategic
        // purchase pass; otherwise the adaptive agents are limited to the
        // baseline building/unit buyer and can carry thousands of Gold past
        // an immediately affordable plan-critical district.
        if self.victory_planning {
            self.advanced_gold_spending(g, pid, &plan);
        }
        self.strategic_policies(g, pid, plan.strategy);
        self.advanced_diplomacy(g, pid, &plan);
        self.advanced_spies(g, pid, &plan);
        self.byzantium_tagma_production(g, pid, &plan);

        // Preserve the proven four-build opening before switching every city
        // to utility planning. This also keeps the frozen baseline comparable.
        if self.base.book_pos < 4 {
            self.base.cities(g, pid);
        } else {
            if self.victory_planning {
                self.redirect_repeatable_projects_for_force_gap(g, pid, &plan);
            }
            // Explicit victory-target runs use strategic production directly;
            // otherwise the baseline governor remains the stronger general
            // policy in paired evaluation.
            if self.victory_planning && plan.strategy == GrandStrategy::Religion {
                self.religious_production(g, pid);
            } else if self.victory_planning
                && g.victory_conditions.religious
                && g.players[pid].religion.is_none()
            {
                // Every other strategy still defends its homeland: a rival's
                // religious victory needs a majority in every living major,
                // and before this pass non-religion civilizations never spent
                // a point of Faith resisting conversion.
                if let Some(threat) = self.home_conversion_threat(g, pid) {
                    self.religious_defense(g, pid, &threat);
                }
            }
            if self.victory_planning
                && (plan.strategy == GrandStrategy::Science
                    || self.diplomatic_science_backup(g, pid, &plan))
            {
                self.science_production(g, pid);
            }
            if self.victory_planning && plan.strategy == GrandStrategy::Culture {
                self.culture_spending(g, pid);
            }
            if plan.strategy == GrandStrategy::Recovery || active_victory_target.is_some() {
                self.advanced_production(g, pid, &plan);
            }
            if active_victory_target.is_none() {
                self.advanced_support_production(g, pid, &plan);
                self.base.cities(g, pid);
            }
        }
        if active_victory_target.is_some() {
            let counts = self.counts(g, pid);
            let cities = g.player_city_ids(pid);
            self.base.spend_gold(
                g,
                pid,
                &cities,
                counts.settlers,
                counts.builders,
                counts.traders,
                counts.military,
                counts.melee,
                counts.ranged,
            );
        }
        if self.victory_planning {
            self.advanced_command_actions(g, pid, &plan);
        }
        BasicAi::upgrade_units(g, pid);
        self.advanced_units(g, pid, &plan);
        self.resolve_city_dispositions(g, pid, plan.strategy);
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &Action::EndTurn);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::run_game;
    use crate::game::GovernorState;

    fn found_test_city(game: &mut Game, pid: usize) -> u32 {
        let position = game
            .map
            .tiles
            .values()
            .filter(|tile| {
                game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && tile.district.is_none()
                    && tile.wonder.is_none()
                    && tile.owner_city.is_none()
                    && game.city_at(tile.pos).is_none()
                    && game.units_at(tile.pos).is_empty()
                    && game
                        .cities
                        .values()
                        .all(|city| game.wdist(city.pos, tile.pos) >= 4)
            })
            .map(|tile| tile.pos)
            .next()
            .expect("test map needs another legal city site");
        let settler = game.spawn_test_unit("settler", pid, position);
        game.current = pid;
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
        game.city_at(position).unwrap()
    }

    fn install_ai_test_district(game: &mut Game, city: u32, district: &str) -> Pos {
        let center = game.cities[&city].pos;
        let position = game.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| {
                *position != center
                    && game.map.tiles[position].district.is_none()
                    && game.map.tiles[position].wonder.is_none()
                    && game.map.tiles[position].improvement.is_none()
            })
            .expect("test city has an unused district tile");
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.district = Some(Name::new(district));
        tile.pillaged = false;
        game.cities
            .get_mut(&city)
            .unwrap()
            .districts
            .insert(Name::new(district), position);
        position
    }

    fn install_test_holy_site(game: &mut Game, city: u32) {
        install_ai_test_district(game, city, "holy_site");
        game.cities.get_mut(&city).unwrap().buildings =
            vec![crate::name!("shrine"), crate::name!("temple")];
    }

    fn found_nearby_test_city(game: &mut Game, owner: usize, anchor: Pos) -> u32 {
        let position = game
            .map
            .tiles
            .iter()
            .filter(|(_, tile)| {
                tile.owner_city.is_none()
                    && game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
            })
            .map(|(position, _)| *position)
            .find(|position| {
                (4..=10).contains(&game.wdist(anchor, *position))
                    && game
                        .cities
                        .values()
                        .all(|city| game.wdist(city.pos, *position) >= 4)
                    // Units staged here must be able to walk out: skip sites
                    // ringed by water and mountains.
                    && game
                        .nbrs(*position)
                        .iter()
                        .filter(|neighbour| {
                            game.map.get(**neighbour).is_some_and(|tile| {
                                game.rules.is_passable(tile) && !game.rules.is_water(tile)
                            })
                        })
                        .count()
                        >= 3
            })
            .expect("test map has a nearby city site");
        game.found_city_for(owner, position, None)
    }

    /// A small Pangaea on which the shipped rush has exactly one legal victim
    /// and a starting land melee unit can route to its staging ring.
    fn connected_rush_fixture() -> Game {
        for seed in 560_000..560_064 {
            let mut game = Game::new_full(2, 24, 16, seed, 200, 0, false);
            for pid in 0..2 {
                let settler = game
                    .player_unit_ids(pid)
                    .into_iter()
                    .find(|unit| game.units[unit].kind == "settler")
                    .expect("each major starts with a settler");
                let position = game.units[&settler].pos;
                game.found_city_for(pid, position, None);
                game.remove_unit(settler);
            }
            game.current = 0;
            let mut rush = AdvancedAi::new();
            rush.early_rush = true;
            let Some((target, capital)) = rush.early_rush_victim(&game, 0) else {
                continue;
            };
            if target != 1 {
                continue;
            }
            let objective = game.cities[&capital].pos;
            let connected = game.player_unit_ids(0).into_iter().any(|unit| {
                let spec = &game.rules.units[game.units[&unit].kind];
                spec.class == "military"
                    && spec.is_melee_capable()
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && game.wdist(game.units[&unit].pos, objective) > RUSH_STAGING_RANGE
                    && game
                        .route_step(unit, objective, RUSH_STAGING_RANGE)
                        .is_some()
            });
            if connected {
                return game;
            }
        }
        panic!("the fixed seed window needs one route-connected rush fixture")
    }

    /// Turn the ring immediately outside the selector's staging range into
    /// water. A pre-embarkation land unit then has no route to any tile within
    /// the range, while the capital's geometric distance remains unchanged.
    fn split_capital_from_land_route(game: &mut Game, capital: Pos) {
        let barrier: BTreeSet<Pos> = game
            .wdisk(capital, RUSH_STAGING_RANGE + 1)
            .into_iter()
            .filter(|position| game.wdist(*position, capital) == RUSH_STAGING_RANGE + 1)
            .collect();
        for tile in game.map.tiles.values_mut() {
            if barrier.contains(&tile.pos) {
                tile.terrain = crate::name!("ocean");
                tile.feature = None;
            }
        }
    }

    #[test]
    fn connected_rush_matches_the_unconditional_plan_on_pangaea() {
        let game = connected_rush_fixture();
        let mut unconditional = AdvancedAi::new();
        unconditional.early_rush = true;
        let expected = unconditional.assess(&game, 0);
        assert!(expected.rush, "the fixture must exercise the treatment");

        let mut connected = AdvancedAi::new();
        connected.early_rush = true;
        connected.route_connected_rush = true;
        assert!(connected.freeze_rush_route_targets(&game, 0));
        assert_eq!(connected.assess(&game, 0), expected);
    }

    #[test]
    fn disconnected_rush_stays_on_the_ordinary_plan() {
        let mut game = connected_rush_fixture();
        let capital = game.cities[&game.player_city_ids(1)[0]].pos;
        split_capital_from_land_route(&mut game, capital);

        let mut connected = AdvancedAi::new();
        connected.early_rush = true;
        connected.route_connected_rush = true;
        assert!(connected.freeze_rush_route_targets(&game, 0));
        assert_eq!(connected.rush_route_targets, Some(BTreeSet::new()));

        let ordinary = AdvancedAi::new().assess(&game, 0);
        let selected = connected.assess(&game, 0);
        assert!(!selected.rush);
        assert_eq!(selected, ordinary);
    }

    #[test]
    fn rush_route_targets_are_frozen_after_the_first_complete_capital_state() {
        let mut game = connected_rush_fixture();
        let mut connected = AdvancedAi::new();
        connected.early_rush = true;
        connected.route_connected_rush = true;
        assert!(connected.freeze_rush_route_targets(&game, 0));
        let frozen = connected.rush_route_targets.clone();
        assert_eq!(frozen, Some(BTreeSet::from([1])));

        for unit in game.player_unit_ids(0) {
            game.remove_unit(unit);
        }
        assert!(!connected.freeze_rush_route_targets(&game, 0));
        assert_eq!(connected.rush_route_targets, frozen);
    }

    #[test]
    fn a_committed_connected_rush_finishes_after_the_route_changes() {
        let mut game = connected_rush_fixture();
        let mut connected = AdvancedAi::new();
        connected.early_rush = true;
        connected.route_connected_rush = true;
        assert!(connected.freeze_rush_route_targets(&game, 0));

        let capital = game.cities[&game.player_city_ids(1)[0]].pos;
        split_capital_from_land_route(&mut game, capital);
        game.at_war.insert((0, 1));
        game.turn = RUSH_WINDOW_CLOSES + 1;

        assert_eq!(
            connected
                .early_rush_victim(&game, 0)
                .map(|(target, _)| target),
            Some(1)
        );
        assert!(connected.assess(&game, 0).rush);
    }

    #[test]
    fn conquest_ai_spends_a_device_on_the_hard_city_but_spares_its_own() {
        let mut game = Game::new_full(2, 24, 16, 91_802, 200, 0, false);
        for pid in 0..2 {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.found_city_for(pid, game.units[&settler].pos, None);
            game.remove_unit(settler);
        }
        let target_city = game.player_city_ids(1)[0];
        let target = game.cities[&target_city].pos;
        game.at_war.insert((0, 1));
        game.players[0]
            .counters
            .insert("project_effect:thermonuclear_devices".to_string(), 1);
        game.players[0].explored.insert(target);
        game.cities.get_mut(&target_city).unwrap().wall_hp = 300;
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: Some(target_city),
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: game.turn,
            rush: false,
        };
        let ai = AdvancedAi::targeting(VictoryTarget::Domination);

        // A friendly scout in the blast must hold the launch.
        let radius = game.rules.wmds["thermonuclear_device"].blast_radius;
        let picket_pos = game
            .wdisk(target, radius)
            .into_iter()
            .find(|position| {
                *position != target
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
                    && game.units_at(*position).is_empty()
            })
            .expect("blast ring has an open land tile");
        let picket = game.spawn_test_unit("scout", 0, picket_pos);
        ai.advanced_wmd_strikes(&mut game, 0, &plan);
        assert_eq!(
            game.players[0].counters["project_effect:thermonuclear_devices"],
            1,
            "no launch while a friendly unit stands in the blast"
        );

        game.remove_unit(picket);
        ai.advanced_wmd_strikes(&mut game, 0, &plan);
        assert_eq!(
            game.players[0].counters["project_effect:thermonuclear_devices"],
            0,
            "the hard city draws the device once the blast is clean"
        );
        assert!(game.map.tiles[&target].fallout_until > game.turn);
    }

    fn island_colony_game() -> (Game, Pos, Pos) {
        let mut g = Game::new_full(1, 18, 10, 92, 120, 0, false);
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
            .unwrap();
        assert!(g.wdist(source, target) > 8);
        for tile in g.map.tiles.values_mut() {
            tile.terrain = crate::name!("coast");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
            tile.district = None;
            tile.wonder = None;
            tile.owner_city = None;
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

    #[test]
    fn product_search_concentrates_culture_multipliers_without_cycling() {
        let mut game = Game::new_full(1, 20, 14, 92_101, 120, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let origin = game.player_city_ids(0)[0];
        let target_position = game
            .map
            .tiles
            .keys()
            .copied()
            .filter(|position| {
                game.rules.is_passable(&game.map.tiles[position])
                    && !game.rules.is_water(&game.map.tiles[position])
                    && game.map.tiles[position].owner_city.is_none()
                    && game.wdist(game.cities[&origin].pos, *position) >= 4
            })
            .max_by_key(|position| game.wdist(game.cities[&origin].pos, *position))
            .unwrap();
        let second_settler = game.spawn_test_unit("settler", 0, target_position);
        game.apply(
            0,
            &Action::FoundCity {
                unit: second_settler,
            },
        )
        .unwrap();
        let target = game.city_at(target_position).unwrap();
        install_ai_test_district(&mut game, origin, "commercial_hub");
        install_ai_test_district(&mut game, target, "commercial_hub");
        install_ai_test_district(&mut game, target, "theater_square");
        game.cities
            .get_mut(&origin)
            .unwrap()
            .buildings
            .push(crate::name!("stock_exchange"));
        game.cities
            .get_mut(&origin)
            .unwrap()
            .products
            .push("silk".to_string());
        game.cities.get_mut(&target).unwrap().buildings.extend([
            crate::name!("stock_exchange"),
            crate::name!("monument"),
            crate::name!("amphitheater"),
            crate::name!("broadcast_center"),
        ]);

        // Keep both cities in the same amenity band, so the comparison
        // exercises the culture multipliers rather than the happiness gap.
        game.cities.get_mut(&origin).unwrap().pop = 6;
        game.cities.get_mut(&target).unwrap().pop = 2;
        let ai = AdvancedAi::targeting(VictoryTarget::Culture);
        ai.advanced_products(&mut game, 0, GrandStrategy::Culture);
        assert!(game.cities[&origin].products.is_empty());
        assert_eq!(game.cities[&target].products, vec!["silk"]);

        ai.advanced_products(&mut game, 0, GrandStrategy::Culture);
        assert!(game.cities[&origin].products.is_empty());
        assert_eq!(game.cities[&target].products, vec!["silk"]);
    }

    #[test]
    fn strategic_settler_routes_to_an_island_beyond_the_local_search_radius() {
        let (mut g, source, target) = island_colony_game();
        g.players[0]
            .techs
            .extend([crate::name!("sailing"), crate::name!("shipbuilding")]);
        let settler = g.spawn_test_unit("settler", 0, source);
        let mut ai = AdvancedAi::new();
        assert!(ai.advanced_settler_step(&mut g, 0, settler));
        assert_eq!(ai.settler_targets.get(&settler), Some(&target));
        assert!(g
            .map
            .get(g.units[&settler].pos)
            .is_some_and(|tile| g.rules.is_water(tile)));
    }

    #[test]
    fn capital_settler_retargets_when_its_cached_site_becomes_illegal() {
        let mut game = Game::new_full(2, 30, 18, 9_204, 120, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| game.units[uid].kind == "settler")
            .unwrap();
        let start = game.units[&settler].pos;
        let blocker = game
            .map
            .tiles
            .iter()
            .filter(|(_, tile)| game.rules.is_passable(tile) && !game.rules.is_water(tile))
            .map(|(position, _)| *position)
            .find(|position| game.wdist(start, *position) == 3)
            .expect("test start has a land tile three hexes away");
        game.found_city_for(1, blocker, None);
        game.current = 0;
        assert!(!game.can_found_city(settler));

        let mut ai = AdvancedAi::new();
        ai.settler_targets.insert(settler, start);
        for _ in 0..100 {
            if !game.units.contains_key(&settler) {
                break;
            }
            let unit = game.units.get_mut(&settler).unwrap();
            unit.moves_left = 4.0;
            unit.acted = false;
            assert!(
                ai.advanced_settler_step(&mut game, 0, settler),
                "the capital settler should keep routing to a replacement site"
            );
            assert_ne!(ai.settler_targets.get(&settler), Some(&start));
        }

        assert!(!game.player_city_ids(0).is_empty());
        assert!(!game.units.contains_key(&settler));
    }

    #[test]
    fn fleet_objective_treats_embarked_enemies_as_naval_contacts() {
        let mut g = Game::new_full(2, 24, 16, 93, 80, 0, false);
        let (anchor, contact) = g
            .map
            .tiles
            .iter()
            .filter(|(_, tile)| g.rules.is_water(tile))
            .find_map(|(pos, _)| {
                g.nbrs(*pos)
                    .into_iter()
                    .find(|neighbor| {
                        g.map
                            .get(*neighbor)
                            .is_some_and(|tile| g.rules.is_water(tile))
                    })
                    .map(|neighbor| (*pos, neighbor))
            })
            .expect("map has adjacent water");
        let embarked = g.spawn_test_unit("settler", 1, contact);
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: None,
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: g.turn,
            rush: false,
        };
        let objective =
            AdvancedAi::new().domain_objective(&g, 0, &plan, ForceDomain::Sea, anchor, &[1]);
        assert_eq!(objective, g.units[&embarked].pos);
    }

    #[test]
    fn fleet_uses_an_adjacent_water_approach_for_coastal_city_capture() {
        let mut g = Game::new_full(2, 24, 16, 94, 80, 0, false);
        for pid in 0..2 {
            g.current = pid;
            let settler = g
                .player_unit_ids(pid)
                .into_iter()
                .find(|uid| g.units[uid].kind == "settler")
                .unwrap();
            g.apply(pid, &Action::FoundCity { unit: settler }).unwrap();
        }
        g.current = 0;
        let target_city = g.player_city_ids(1)[0];
        let target = g.cities[&target_city].pos;
        let approach = g.nbrs(target)[0];
        {
            let tile = g.map.tiles.get_mut(&approach).unwrap();
            tile.terrain = crate::name!("coast");
            tile.feature = None;
            tile.hills = false;
        }
        g.players[0].techs.insert(crate::name!("sailing"));
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: Some(target_city),
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: g.turn,
            rush: false,
        };
        let objective =
            AdvancedAi::new().domain_objective(&g, 0, &plan, ForceDomain::Sea, approach, &[1]);
        assert_eq!(g.wdist(objective, target), 1);
        assert!(g
            .map
            .get(objective)
            .is_some_and(|tile| g.rules.is_water(tile)));
    }

    #[test]
    fn every_victory_condition_can_be_forced_for_every_major() {
        let g = Game::new(4, 24, 16, 70, 80, 0);
        for target in VictoryTarget::ALL {
            let mut ais = AdvancedAi::fleet_targeting(&g, target);
            assert_eq!(ais.len(), g.players.len());
            for pid in g
                .players
                .iter()
                .filter(|player| !player.is_minor && !player.is_barbarian)
                .map(|player| player.id)
            {
                let ai = &mut ais[pid];
                assert_eq!(ai.victory_target(), Some(target));
                ai.base.minor = false;
                ai.base.barb = false;
                let plan = ai.assess(&g, pid);
                let expected = if target == VictoryTarget::Religion {
                    GrandStrategy::Religion
                } else {
                    GrandStrategy::Expansion
                };
                assert_eq!(plan.strategy, expected, "player {pid} targeting {target:?}");
            }
        }

        // The public parser accepts both victory nouns and result labels.
        assert_eq!("religious".parse(), Ok(VictoryTarget::Religion));
        assert_eq!("diplomatic".parse(), Ok(VictoryTarget::Diplomacy));
        assert_eq!("conquest".parse(), Ok(VictoryTarget::Domination));
    }

    #[test]
    fn explicit_non_diplomatic_targets_do_not_score_congress_points() {
        use crate::game::{CongressResolution, CongressSession};

        let session = || CongressSession {
            convened: 0,
            closes: 5,
            resolutions: vec![
                CongressResolution {
                    id: "world_leader".to_string(),
                    title: "Diplomatic Victory".to_string(),
                    choices: vec!["0".to_string(), "1".to_string()],
                    ballots: BTreeMap::new(),
                },
                CongressResolution {
                    id: "international_aid".to_string(),
                    title: "International Aid".to_string(),
                    choices: vec!["0".to_string(), "1".to_string()],
                    ballots: BTreeMap::new(),
                },
            ],
        };
        let plan = |strategy| StrategicPlan {
            strategy,
            target_player: Some(1),
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: 0,
            rush: false,
        };

        let mut science_game = Game::new(2, 24, 16, 77, 80, 0);
        science_game.congress = Some(session());
        AdvancedAi::targeting(VictoryTarget::Science).advanced_diplomacy(
            &mut science_game,
            0,
            &plan(GrandStrategy::Science),
        );
        let science_resolutions = &science_game.congress.as_ref().unwrap().resolutions;
        assert!(!science_resolutions[0].ballots.contains_key(&0));
        assert!(!science_resolutions[1].ballots.contains_key(&0));

        let mut diplomacy_game = Game::new(2, 24, 16, 78, 80, 0);
        diplomacy_game.congress = Some(session());
        AdvancedAi::targeting(VictoryTarget::Diplomacy).advanced_diplomacy(
            &mut diplomacy_game,
            0,
            &plan(GrandStrategy::Diplomacy),
        );
        assert!(diplomacy_game.congress.as_ref().unwrap().resolutions[0]
            .ballots
            .contains_key(&0));
    }

    #[test]
    fn congress_strategy_contests_leaders_and_predicts_competitions() {
        let mut game = Game::new_full(3, 24, 16, 780, 200, 0, false);
        game.players[0].dvp = 10;
        game.players[1].dvp = 18;
        game.players[2].dvp = 3;
        game.players[1].culture_lifetime = 2_000.0;
        let ai = AdvancedAi::new();
        let resolution = |id: &str| CongressResolution {
            id: id.to_string(),
            title: id.to_string(),
            choices: vec!["0".to_string(), "1".to_string(), "2".to_string()],
            ballots: BTreeMap::new(),
        };

        assert_eq!(
            ai.congress_choice(
                &game,
                0,
                &resolution("world_leader"),
                GrandStrategy::Expansion,
            ),
            Some("2".to_string())
        );
        assert_eq!(
            ai.congress_choice(
                &game,
                0,
                &resolution("world_leader"),
                GrandStrategy::Diplomacy,
            ),
            Some("0".to_string())
        );
        assert_eq!(
            ai.congress_choice(&game, 0, &resolution("world_fair"), GrandStrategy::Science,),
            Some("1".to_string())
        );
        assert_eq!(
            ai.congress_choice(&game, 0, &resolution("world_fair"), GrandStrategy::Culture,),
            Some("0".to_string())
        );

        let outcome_resolution = |id: &str, targets: &[&str]| CongressResolution {
            id: id.to_string(),
            title: id.to_string(),
            choices: ["A", "B"]
                .into_iter()
                .flat_map(|outcome| {
                    targets
                        .iter()
                        .map(move |target| format!("{outcome}:{target}"))
                })
                .collect(),
            ballots: BTreeMap::new(),
        };
        assert_eq!(
            ai.congress_choice(
                &game,
                0,
                &outcome_resolution("world_leader", &["0", "1", "2"]),
                GrandStrategy::Expansion,
            ),
            Some("B:1".to_string())
        );
        assert_eq!(
            ai.congress_choice(
                &game,
                0,
                &outcome_resolution("world_leader", &["0", "1", "2"]),
                GrandStrategy::Diplomacy,
            ),
            Some("A:0".to_string())
        );
        assert_eq!(
            ai.congress_choice(
                &game,
                0,
                &outcome_resolution("mercenary_companies", &["production", "gold", "faith"]),
                GrandStrategy::Conquest,
            ),
            Some("B:production".to_string())
        );
        assert_eq!(
            ai.congress_choice(
                &game,
                0,
                &outcome_resolution(
                    "urban_development_treaty",
                    &["campus", "theater_square", "holy_site"],
                ),
                GrandStrategy::Science,
            ),
            Some("A:campus".to_string())
        );

        game.players[1]
            .counters
            .insert("project_effect:nuclear_devices".to_string(), 3);
        assert_eq!(
            ai.congress_choice(
                &game,
                0,
                &outcome_resolution("arms_control", &["0", "1", "2"]),
                GrandStrategy::Conquest,
            ),
            Some("B:1".to_string())
        );
        game.players[0]
            .counters
            .insert("project_effect:nuclear_devices".to_string(), 1);
        assert_eq!(
            ai.congress_choice(
                &game,
                0,
                &outcome_resolution("arms_control", &["0", "1", "2"]),
                GrandStrategy::Diplomacy,
            ),
            Some("A:2".to_string())
        );

        game.players[0].government = Some("autocracy".to_string());
        game.players[1].government = Some("democracy".to_string());
        game.players[2].government = Some("democracy".to_string());
        assert_eq!(
            ai.congress_choice(
                &game,
                0,
                &outcome_resolution("world_ideology", &["autocracy", "democracy"]),
                GrandStrategy::Science,
            ),
            Some("A:autocracy".to_string())
        );
        assert_eq!(
            ai.congress_choice(
                &game,
                0,
                &outcome_resolution("border_control_treaty", &["0", "1", "2"]),
                GrandStrategy::Expansion,
            ),
            Some("A:0".to_string())
        );
        assert_eq!(
            ai.congress_choice(
                &game,
                0,
                &outcome_resolution(
                    "public_works_program",
                    &["launch_earth_satellite", "manhattan_project"],
                ),
                GrandStrategy::Science,
            ),
            Some("A:launch_earth_satellite".to_string())
        );
        assert_eq!(
            ai.congress_choice(
                &game,
                0,
                &outcome_resolution(
                    "global_energy_treaty",
                    &["coal_power_plant", "oil_power_plant", "nuclear_power_plant"],
                ),
                GrandStrategy::Science,
            ),
            Some("A:nuclear_power_plant".to_string())
        );
        assert_eq!(
            ai.congress_choice(
                &game,
                0,
                &outcome_resolution("deforestation_treaty", &["forest"]),
                GrandStrategy::Expansion,
            ),
            Some("A:forest".to_string())
        );
    }

    #[test]
    fn strategic_diplomacy_prices_incoming_deals_and_rejects_victory_leaders() {
        let mut game = Game::new_full(2, 24, 16, 781, 300, 0, false);
        let ai = AdvancedAi::new();
        let mut plan = StrategicPlan {
            strategy: GrandStrategy::Expansion,
            target_player: Some(1),
            target_city: None,
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: game.turn,
            rush: false,
        };
        let expires = game.turn + 10;
        let deal = |give_gold, request_gold, friendship, peace| DiplomaticDeal {
            id: 1,
            from: 1,
            to: 0,
            give_gold,
            request_gold,
            open_borders: false,
            friendship,
            peace,
            alliance: None,
            expires,
        };

        assert!(ai.incoming_deal_value(&game, 0, &deal(0.0, 100.0, true, false), &plan) < 0.0);
        assert!(ai.incoming_deal_value(&game, 0, &deal(10.0, 0.0, true, false), &plan) > 0.0);

        plan.strategy = GrandStrategy::Conquest;
        assert!(
            ai.incoming_deal_value(&game, 0, &deal(10.0, 0.0, true, false), &plan) < 0.0,
            "a campaign target must not be protected by a new friendship"
        );
        plan.strategy = GrandStrategy::Expansion;

        game.players[1].science_projects.extend([
            "launch_earth_satellite".to_string(),
            "launch_moon_landing".to_string(),
            "launch_mars_colony".to_string(),
            "exoplanet_expedition".to_string(),
        ]);
        game.players[1].exoplanet_distance = 42.0;
        assert!(ai.incoming_deal_value(&game, 0, &deal(10.0, 0.0, true, false), &plan) < 0.0);

        game.at_war.insert((0, 1));
        plan.strategy = GrandStrategy::Recovery;
        assert!(
            ai.incoming_deal_value(&game, 0, &deal(0.0, 100.0, false, true), &plan) < 0.0,
            "a strong Recovery posture must not abandon its active campaign target"
        );
        plan.target_player = None;
        assert!(ai.incoming_deal_value(&game, 0, &deal(0.0, 100.0, false, true), &plan) > 0.0);
    }

    #[test]
    fn outmatched_major_must_negotiate_peace_with_the_winning_campaign() {
        let mut game = Game::new_full(2, 24, 16, 7_922, 300, 0, false);
        for pid in 0..2 {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.found_city_for(pid, game.units[&settler].pos, None);
            game.remove_unit(settler);
        }
        let staging = game.cities[&game.player_city_ids(0)[0]].pos;
        for _ in 0..3 {
            game.spawn_test_unit("modern_armor", 0, staging);
        }
        game.current = 0;
        game.turn = 60;
        game.apply(0, &Action::DeclareWar { player: 1 })
            .unwrap();
        game.turn = game
            .peace_available_at(0, 1)
            .expect("the new war has a mandatory minimum");
        assert!(game.military_power(1) < game.military_power(0) * 0.62);

        let recovery = StrategicPlan {
            strategy: GrandStrategy::Recovery,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 2,
            assessed_turn: game.turn,
            rush: false,
        };
        let mut defender = AdvancedAi::new();
        defender.major_war_since = Some(60);
        game.current = 1;
        defender.advanced_diplomacy(&mut game, 1, &recovery);

        assert!(
            game.is_at_war(0, 1),
            "an outmatched defender cannot impose white peace unilaterally"
        );
        assert!(game.pending_deals.iter().any(|deal| {
            deal.from == 1 && deal.to == 0 && deal.peace
        }));

        let winning_campaign = StrategicPlan {
            // A threatened home city can temporarily classify even a much
            // stronger attacker as Recovery. Its active campaign target must
            // still be able to refuse the defender's immediate white peace.
            strategy: GrandStrategy::Recovery,
            target_player: Some(1),
            target_city: game.player_city_ids(1).into_iter().next(),
            threatened_city: None,
            desired_cities: 2,
            assessed_turn: game.turn,
            rush: false,
        };
        let mut refused = game.clone();
        let mut conqueror = AdvancedAi::new();
        conqueror.major_war_since = Some(60);
        refused.current = 0;
        conqueror.advanced_diplomacy(&mut refused, 0, &winning_campaign);
        assert!(refused.is_at_war(0, 1));
        assert!(
            refused.pending_deals.iter().all(|deal| !deal.peace),
            "the stronger conquest plan should reject an immediate white peace"
        );

        let mut accepting = AdvancedAi::new();
        accepting.major_war_since = Some(60);
        game.current = 0;
        accepting.advanced_diplomacy(&mut game, 0, &recovery);
        assert!(!game.is_at_war(0, 1));
        assert_eq!(accepting.peace_until, game.turn + 30);
        assert!(accepting.major_war_since.is_none());
    }

    #[test]
    fn advanced_ai_proposes_the_alliance_for_its_victory_plan() {
        let mut game = Game::new_full(3, 24, 16, 782, 300, 0, false);
        game.turn = 12;
        // There is nobody to ally with until the table has been introduced.
        for pid in 0..3 {
            for other in pid + 1..3 {
                game.record_contact(pid, other);
            }
        }
        for player in game.players.iter_mut() {
            player.civics.insert(crate::name!("civil_service"));
            player.techs.insert(crate::name!("scientific_theory"));
        }
        game.players[1].techs.insert(crate::name!("radio"));
        let plan = StrategicPlan {
            strategy: GrandStrategy::Science,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: game.turn,
            rush: false,
        };
        let ai = AdvancedAi::targeting(VictoryTarget::Science);
        assert!(game.legal_actions(0).iter().any(|action| {
            matches!(
                action,
                Action::ProposeDeal {
                    alliance: Some(kind),
                    ..
                } if kind == "research"
            )
        }));
        assert!(ai.rival_victory_pressure(&game, 1).progress < 82);
        ai.propose_strategic_alliance(&mut game, 0, &plan, None);
        let proposal = game
            .pending_deals
            .iter()
            .find(|deal| deal.from == 0)
            .unwrap();
        assert_eq!(proposal.alliance.as_deref(), Some("research"));
        assert!(proposal.friendship);
    }

    #[test]
    fn initial_plan_coordinates_expansion() {
        let g = Game::new(2, 24, 16, 71, 80, 0);
        let ai = AdvancedAi::new();
        let plan = ai.assess(&g, 0);
        assert_eq!(plan.strategy, GrandStrategy::Expansion);
        assert!(plan.desired_cities >= 3);
        assert!(plan.target_player.is_some());
    }

    #[test]
    fn governor_titles_promote_the_primary_before_widening_the_roster() {
        let mut game = Game::new_full(1, 24, 16, 7_111, 200, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        // Three civics that each award a Governor title: one pays for the
        // appointment, two for promotions.
        game.players[0].civics.extend([
            crate::name!("state_workforce"),
            crate::name!("early_empire"),
            crate::name!("guilds"),
        ]);
        let plan = StrategicPlan {
            strategy: GrandStrategy::Science,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };
        let ai = AdvancedAi::new();
        ai.strategic_governors(&mut game, 0, &plan);

        assert_eq!(game.players[0].governor_roster.len(), 1);
        let pingala = &game.players[0].governor_roster["pingala"];
        // Which two tier-one promotions it picks is a weighting detail; the
        // rule under test is that both titles went to the primary governor.
        assert_eq!(pingala.promotions.len(), 2);
        assert_eq!(game.governor_titles_available(0), 0);

        found_test_city(&mut game, 0);
        game.players[0]
            .counters
            .insert("district_governor_titles".to_string(), 1);
        ai.strategic_governors(&mut game, 0, &plan);
        assert_eq!(game.players[0].governor_roster.len(), 2);
        assert!(game.players[0].governor_roster.contains_key("magnus"));
        assert_eq!(
            game.players[0].governor_roster["pingala"].promotions.len(),
            2
        );
    }

    #[test]
    fn governor_path_stays_focused_when_strategy_changes_between_titles() {
        let mut game = Game::new_full(1, 24, 16, 7_112, 200, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        game.players[0]
            .civics
            .insert(crate::name!("state_workforce"));
        let assessed_turn = game.turn;
        let plan = |strategy| StrategicPlan {
            strategy,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn,
            rush: false,
        };
        let ai = AdvancedAi::new();
        ai.strategic_governors(&mut game, 0, &plan(GrandStrategy::Expansion));
        assert!(game.players[0].governor_roster.contains_key("magnus"));

        game.players[0]
            .counters
            .insert("district_governor_titles".to_string(), 1);
        ai.strategic_governors(&mut game, 0, &plan(GrandStrategy::Science));
        game.players[0]
            .counters
            .insert("district_governor_titles".to_string(), 2);
        ai.strategic_governors(&mut game, 0, &plan(GrandStrategy::Conquest));

        assert_eq!(game.players[0].governor_roster.len(), 1);
        assert_eq!(
            game.players[0].governor_roster["magnus"].promotions.len(),
            2
        );

        found_test_city(&mut game, 0);
        game.players[0]
            .counters
            .insert("district_governor_titles".to_string(), 3);
        ai.strategic_governors(&mut game, 0, &plan(GrandStrategy::Science));
        assert!(game.players[0].governor_roster.contains_key("pingala"));
    }

    #[test]
    fn first_governor_matches_the_empire_strategy() {
        for (index, (strategy, expected)) in [
            (GrandStrategy::Expansion, "magnus"),
            (GrandStrategy::Science, "pingala"),
            (GrandStrategy::Culture, "pingala"),
            (GrandStrategy::Religion, "moksha"),
            (GrandStrategy::Diplomacy, "amani"),
            (GrandStrategy::Conquest, "victor"),
            (GrandStrategy::Recovery, "victor"),
        ]
        .into_iter()
        .enumerate()
        {
            let mut game = Game::new_full(1, 18, 10, 7_120 + index as u64, 120, 0, false);
            let settler = game
                .player_unit_ids(0)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
            game.players[0]
                .civics
                .insert(crate::name!("state_workforce"));
            let plan = StrategicPlan {
                strategy,
                target_player: None,
                target_city: None,
                threatened_city: None,
                desired_cities: 3,
                assessed_turn: game.turn,
                rush: false,
            };
            AdvancedAi::new().strategic_governors(&mut game, 0, &plan);
            assert!(
                game.players[0].governor_roster.contains_key(expected),
                "{strategy:?} appointed {:?}",
                game.players[0].governor_roster.keys().collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn expansion_window_reaches_its_six_city_target_before_endgame() {
        let mut game = Game::new_full(1, 30, 18, 7_113, 500, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        for tile in game.map.tiles.values_mut() {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
        }
        let city = game.player_city_ids(0)[0];
        game.cities.get_mut(&city).unwrap().pop = 6;
        game.turn = 270;

        let ai = AdvancedAi::new();
        let plan = ai.assess(&game, 0);
        assert_eq!(plan.desired_cities, 6);
        assert_eq!(plan.strategy, GrandStrategy::Expansion);
        let item = Item::Unit {
            unit: crate::name!("settler"),
        };
        let counts = ai.counts(&game, 0);
        assert!(ai.production_value(&game, 0, city, &item, &plan, &counts) > -9_000.0);

        game.turn = 300;
        assert!(!AdvancedAi::expansion_window_open(&game));
        assert!(ai.production_value(&game, 0, city, &item, &plan, &counts) < -9_000.0);
    }

    #[test]
    fn conquest_can_target_an_exposed_city_state_but_preserves_its_suzerain() {
        let mut game = Game::new_full(2, 30, 18, 711, 300, 1, false);
        for pid in 0..2 {
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
        game.turn = 200;
        let minor = game
            .players
            .iter()
            .find(|player| player.is_minor && !player.is_barbarian)
            .unwrap()
            .id;
        let rival_capital = game.cities[&game.player_city_ids(1)[0]].pos;
        for _ in 0..6 {
            game.spawn_test_unit("giant_death_robot", 1, rival_capital);
        }

        let ai = AdvancedAi::targeting(VictoryTarget::Domination);
        let exposed = ai.assess(&game, 0);
        assert_eq!(exposed.strategy, GrandStrategy::Conquest);
        assert_eq!(exposed.target_player, Some(minor));

        game.players[0].envoys = vec![(minor, 3)];
        assert_eq!(game.suzerain_of(minor), Some(0));
        let allied = ai.assess(&game, 0);
        assert_eq!(allied.target_player, Some(1));
    }

    #[test]
    fn campaign_masks_allied_rivals_and_their_suzerained_city_states() {
        let mut game = Game::new_full(2, 30, 18, 7_112, 300, 1, false);
        for pid in 0..2 {
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
        game.turn = 200;
        let minor = game
            .players
            .iter()
            .find(|player| player.is_minor && !player.is_barbarian)
            .unwrap()
            .id;
        game.players[1].envoys = vec![(minor, 3)];
        assert_eq!(game.suzerain_of(minor), Some(1));

        let alliance = crate::game::AllianceState {
            kind: "military".to_string(),
            points: 0.0,
            level: 1,
            ends: game.turn + 30,
        };
        game.players[0].alliances.insert(1, alliance.clone());
        game.players[1].alliances.insert(0, alliance);

        let ai = AdvancedAi::targeting(VictoryTarget::Domination);
        assert!(!ai.campaign_target_legal(&game, 0, 1));
        assert!(!ai.campaign_target_legal(&game, 0, minor));
        assert_eq!(ai.assess(&game, 0).target_player, None);

        let mut stale_ai = AdvancedAi::targeting(VictoryTarget::Domination);
        stale_ai.plan = Some(StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: game.player_city_ids(1).first().copied(),
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        });
        assert!(
            stale_ai.plan_stale(&game, 0),
            "a new alliance must interrupt a cached hostile plan immediately"
        );

        // A loaded legacy position can contain contradictory relationship
        // state. An actual war remains a forcing objective until peace.
        game.at_war.insert((0, 1));
        assert!(ai.campaign_target_legal(&game, 0, 1));
        assert!(
            ai.assess(&game, 0)
                .target_player
                .is_some_and(|target| target == 1 || target == minor),
            "the suzerain or the city-state that joined its war must remain actionable"
        );
    }

    #[test]
    fn campaign_city_ordering_prefers_a_breach_then_the_domination_capital() {
        let mut game = Game::new_full(2, 30, 18, 7_111, 300, 0, false);
        for pid in 0..2 {
            game.current = pid;
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.apply(pid, &Action::FoundCity { unit: settler })
                .unwrap();
        }
        let own_capital = game.player_city_ids(0)[0];
        let enemy_capital = game.player_city_ids(1)[0];
        let enemy_position = game.cities[&enemy_capital].pos;
        let capital_distance = game.wdist(game.cities[&own_capital].pos, enemy_position);
        let outpost_position = game
            .map
            .tiles
            .iter()
            .filter(|(position, tile)| {
                game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && tile.owner_city.is_none()
                    && game.wdist(enemy_position, **position) >= 9
                    && game
                        .cities
                        .values()
                        .all(|city| game.wdist(city.pos, **position) >= 4)
            })
            .min_by_key(|(position, _)| {
                (
                    (game.wdist(game.cities[&own_capital].pos, **position) - capital_distance)
                        .abs(),
                    **position,
                )
            })
            .map(|(position, _)| *position)
            .expect("test map has a comparable second-city site");
        game.current = 1;
        let settler = game.spawn_test_unit("settler", 1, outpost_position);
        game.apply(1, &Action::FoundCity { unit: settler }).unwrap();
        let enemy_outpost = game
            .player_city_ids(1)
            .into_iter()
            .find(|city| *city != enemy_capital)
            .unwrap();
        for unit in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(unit);
        }

        let fortify = |game: &mut Game, city: u32| {
            let position = {
                let target = game.cities.get_mut(&city).unwrap();
                target.hp = 200;
                target.wall_hp = 400;
                target.buildings.extend([
                    crate::name!("walls"),
                    crate::name!("medieval_walls"),
                    crate::name!("renaissance_walls"),
                ]);
                target.pos
            };
            for _ in 0..3 {
                game.spawn_test_unit("giant_death_robot", 1, position);
            }
        };
        let breach = |game: &mut Game, city: u32| {
            let target = game.cities.get_mut(&city).unwrap();
            target.hp = 25;
            target.wall_hp = 0;
            target.buildings.retain(|building| {
                !matches!(
                    building.as_str(),
                    "walls" | "medieval_walls" | "renaissance_walls"
                )
            });
        };
        let ai = AdvancedAi::targeting(VictoryTarget::Domination);

        fortify(&mut game, enemy_capital);
        breach(&mut game, enemy_outpost);
        let exposed_outpost = ai.campaign_city_value(
            &game,
            0,
            &game.cities[&enemy_outpost],
            GrandStrategy::Conquest,
        );
        let fortified_capital = ai.campaign_city_value(
            &game,
            0,
            &game.cities[&enemy_capital],
            GrandStrategy::Conquest,
        );
        assert!(
            exposed_outpost < fortified_capital,
            "an exposed breach ({exposed_outpost}) should be searched before a fully defended capital ({fortified_capital})"
        );

        for unit in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(unit);
        }
        breach(&mut game, enemy_capital);
        fortify(&mut game, enemy_outpost);
        assert!(
            ai.campaign_city_value(
                &game,
                0,
                &game.cities[&enemy_capital],
                GrandStrategy::Conquest,
            ) < ai.campaign_city_value(
                &game,
                0,
                &game.cities[&enemy_outpost],
                GrandStrategy::Conquest,
            ),
            "once both geometry and defenses favor it, Domination must order the original capital first"
        );
    }

    #[test]
    fn conquest_army_stages_before_diplomacy_opens_the_war() {
        let mut game = Game::new_full(2, 30, 18, 7_114, 300, 0, false);
        // A war has to be declarable, which means the two have met.
        game.record_contact(0, 1);
        for pid in 0..2 {
            game.current = pid;
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.apply(pid, &Action::FoundCity { unit: settler })
                .unwrap();
        }
        game.current = 0;
        game.turn = 60;
        let target_city = game.player_city_ids(1)[0];
        let objective = game.cities[&target_city].pos;

        // The declaration rule also requires a two-city operating base. Put
        // the second city close enough that this test isolates army staging.
        let second_site = game
            .map
            .tiles
            .iter()
            .filter(|(position, tile)| {
                game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && tile.owner_city.is_none()
                    && game.wdist(**position, objective) <= 18
                    && game
                        .cities
                        .values()
                        .all(|city| game.wdist(city.pos, **position) >= 4)
            })
            .map(|(position, _)| *position)
            .next()
            .expect("test map has a legal second-city site");
        let settler = game.spawn_test_unit("settler", 0, second_site);
        game.apply(0, &Action::FoundCity { unit: settler })
            .unwrap();

        for unit in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(unit);
        }
        for tile in game.map.tiles.values_mut() {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
        }
        let remote = game
            .map
            .tiles
            .keys()
            .copied()
            .find(|position| {
                game.wdist(*position, objective) >= 10
                    && game.city_at(*position).is_none()
                    && game.map.tiles[position]
                        .owner_city
                        .and_then(|city| game.cities.get(&city))
                        .is_none_or(|city| city.owner != 1)
            })
            .expect("test map has a remote muster position");
        let army: Vec<u32> = (0..4)
            .map(|_| game.spawn_test_unit("swordsman", 0, remote))
            .collect();
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: Some(target_city),
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);

        assert!(!ai.campaign_staged_for_war(&game, 0, 1, objective, true));
        let before = game.wdist(game.units[&army[0]].pos, objective);
        assert_eq!(
            ai.campaign_staging_step(&mut game, 0, army[0], &plan),
            Some(true)
        );
        assert!(game.wdist(game.units[&army[0]].pos, objective) < before);

        ai.advanced_diplomacy(&mut game, 0, &plan);
        assert!(
            !game.players[0]
                .denounced_until
                .get(&1)
                .is_some_and(|until| *until > game.turn),
            "remote global power must not begin the diplomatic war countdown"
        );

        let staging: Vec<Pos> = game
            .wdisk(objective, 7)
            .into_iter()
            .filter(|position| {
                (3..=5).contains(&game.wdist(*position, objective))
                    && game.city_at(*position).is_none()
            })
            .take(army.len())
            .collect();
        assert_eq!(staging.len(), army.len());
        for (unit, position) in army.iter().zip(staging) {
            let tile = game.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
            tile.owner_city = None;
            game.units.get_mut(unit).unwrap().pos = position;
        }
        assert!(ai.campaign_staged_for_war(&game, 0, 1, objective, true));

        ai.advanced_diplomacy(&mut game, 0, &plan);
        assert!(
            game.players[0]
                .denounced_until
                .get(&1)
                .is_some_and(|until| *until > game.turn),
            "the staged capture force should begin the formal-war countdown"
        );

        // A terminal threat must use the already-staged force even when the
        // rival's total military is too large for an elective war. Keep the
        // reinforcements away from the objective so this isolates the global
        // readiness gate from the local staging test above.
        game.players[0].denounced_until.remove(&1);
        game.players[1].science_projects.extend([
            "launch_earth_satellite".to_string(),
            "launch_moon_landing".to_string(),
            "launch_mars_colony".to_string(),
            "exoplanet_expedition".to_string(),
        ]);
        let rival_muster = game
            .map
            .tiles
            .keys()
            .copied()
            .find(|position| {
                game.wdist(*position, objective) >= 10 && game.city_at(*position).is_none()
            })
            .expect("test map has a remote rival muster position");
        for _ in 0..12 {
            game.spawn_test_unit("swordsman", 1, rival_muster);
        }
        assert!(
            game.military_power(0) <= game.military_power(1) * 1.32 + 12.0,
            "the usual elective-war margin must reject this outnumbered army"
        );

        ai.advanced_diplomacy(&mut game, 0, &plan);
        assert!(
            game.is_at_war(0, 1),
            "a staged counterforce must immediately deny a terminal science threat"
        );
    }

    #[test]
    fn war_opening_waits_for_formal_war_but_interrupts_for_imminent_victory() {
        let mut game = Game::new_full(2, 24, 16, 712, 300, 0, false);
        // A denunciation needs somebody to denounce.
        game.record_contact(0, 1);
        game.current = 0;
        game.turn = 60;
        let ai = AdvancedAi::new();

        assert_eq!(
            ai.preferred_war_opening(&game, 0, 1),
            Some(Action::Denounce { player: 1 })
        );
        game.apply(0, &Action::Denounce { player: 1 }).unwrap();
        game.turn = 64;
        assert_eq!(ai.preferred_war_opening(&game, 0, 1), None);
        game.turn = 65;
        assert_eq!(
            ai.preferred_war_opening(&game, 0, 1),
            Some(Action::DeclareWarWithCasusBelli {
                player: 1,
                casus_belli: "formal_war".to_string(),
            })
        );

        let mut emergency = Game::new_full(2, 24, 16, 713, 300, 0, false);
        emergency.record_contact(0, 1);
        emergency.current = 0;
        emergency.turn = 60;
        emergency.players[1]
            .science_projects
            .insert("exoplanet_expedition".to_string());
        assert_eq!(
            ai.preferred_war_opening(&emergency, 0, 1),
            Some(Action::DeclareWar { player: 1 })
        );

        let mut religious = Game::new_full(4, 24, 16, 714, 300, 0, false);
        for pid in 0..4 {
            for other in pid + 1..4 {
                religious.record_contact(pid, other);
            }
        }
        for pid in 0..4 {
            religious.current = pid;
            let settler = religious
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| religious.units[unit].kind == "settler")
                .unwrap();
            religious.apply(pid, &Action::FoundCity { unit: settler }).unwrap();
        }
        religious.current = 0;
        religious.turn = 60;
        religious.players[1].religion = Some("Runaway Faith".to_string());
        for owner in [1, 2, 3] {
            let city = religious.player_city_ids(owner)[0];
            religious
                .cities
                .get_mut(&city)
                .unwrap()
                .pressure
                .insert("Runaway Faith".to_string(), 1_000.0);
        }
        assert_eq!(
            ai.preferred_war_opening(&religious, 0, 1),
            Some(Action::DeclareWar { player: 1 }),
            "a religious match point cannot spend five turns preparing a formal war"
        );
    }

    #[test]
    fn strategic_plan_is_stable_inside_assessment_window() {
        let mut g = Game::new(2, 24, 16, 72, 30, 0);
        let mut ai = AdvancedAi::new();
        ai.take_turn(&mut g, 0);
        let first = ai.current_plan().unwrap().clone();
        assert!(!ai.plan_stale(&g, 0));
        assert_eq!(ai.current_plan(), Some(&first));
    }

    #[test]
    fn surprise_wars_and_imminent_victories_interrupt_the_planning_window() {
        let mut game = Game::new_full(3, 30, 18, 721, 300, 0, false);
        for pid in 0..3 {
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
        game.turn = 190;
        let mut ai = AdvancedAi::new();
        ai.plan = Some(StrategicPlan {
            strategy: GrandStrategy::Expansion,
            target_player: Some(2),
            target_city: None,
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: game.turn,
            rush: false,
        });
        assert!(!ai.plan_stale(&game, 0));

        game.at_war.insert((0, 1));
        assert!(ai.plan_stale(&game, 0), "a surprise war must replan now");

        game.at_war.clear();
        ai.plan.as_mut().unwrap().target_player = Some(1);
        game.players[2].science_projects.extend([
            "launch_earth_satellite".to_string(),
            "launch_moon_landing".to_string(),
            "launch_mars_colony".to_string(),
            "exoplanet_expedition".to_string(),
        ]);
        game.players[2].exoplanet_distance = 42.0;
        assert!(
            ai.plan_stale(&game, 0),
            "an imminent rival victory must replan now"
        );
    }

    #[test]
    fn recovery_requires_material_local_danger_and_ends_when_it_clears() {
        let mut game = Game::new_full(2, 30, 18, 7_218, 300, 0, false);
        for pid in 0..2 {
            game.current = pid;
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.apply(pid, &Action::FoundCity { unit: settler })
                .unwrap();
        }
        for unit in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(unit);
        }
        game.current = 0;
        game.turn = 90;
        game.at_war.insert((0, 1));
        let home = game.player_city_ids(0)[0];
        let home_pos = game.cities[&home].pos;
        let intruder_pos = game
            .wdisk(home_pos, 6)
            .into_iter()
            .find(|position| {
                game.wdist(*position, home_pos) == 3 && game.city_at(*position).is_none()
            })
            .unwrap();
        let far_pos = game
            .map
            .tiles
            .keys()
            .copied()
            .find(|position| {
                game.wdist(*position, home_pos) >= 9 && game.city_at(*position).is_none()
            })
            .unwrap();
        for position in [intruder_pos, far_pos] {
            let tile = game.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
        }
        for _ in 0..4 {
            game.spawn_test_unit("modern_armor", 0, far_pos);
        }
        let mut intruders = vec![game.spawn_test_unit("warrior", 1, intruder_pos)];
        let mut ai = AdvancedAi::new();

        assert_eq!(
            ai.threatened_city(&game, 0),
            None,
            "one losing contact in the outer radius must not recall a dominant army"
        );
        assert_eq!(ai.assess(&game, 0).strategy, GrandStrategy::Conquest);

        for _ in 0..4 {
            intruders.push(game.spawn_test_unit("modern_armor", 1, intruder_pos));
        }
        assert_eq!(ai.threatened_city(&game, 0), Some(home));
        let recovery = ai.assess(&game, 0);
        assert_eq!(recovery.strategy, GrandStrategy::Recovery);
        assert_eq!(recovery.threatened_city, Some(home));

        ai.plan = Some(recovery);
        for unit in intruders {
            game.remove_unit(unit);
        }
        assert!(
            ai.plan_stale(&game, 0),
            "clearing the emergency must resume the campaign immediately"
        );
        assert_eq!(ai.assess(&game, 0).strategy, GrandStrategy::Conquest);
    }

    #[test]
    fn religious_denial_triggers_with_one_unconverted_civilization() {
        let mut game = Game::new_full(4, 30, 18, 7_215, 300, 0, false);
        for pid in 0..4 {
            game.current = pid;
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.apply(pid, &Action::FoundCity { unit: settler })
                .unwrap();
        }
        game.current = 0;
        game.players[1].religion = Some("Rival Faith".to_string());
        for owner in [1, 2, 3] {
            let city = game.player_city_ids(owner)[0];
            game.cities
                .get_mut(&city)
                .unwrap()
                .pressure
                .insert("Rival Faith".to_string(), 1_000.0);
        }

        let ai = AdvancedAi::new();
        let pressure = ai.rival_victory_pressure(&game, 1);
        assert_eq!(pressure.strategy, GrandStrategy::Religion);
        assert_eq!(pressure.progress, 75);
        assert_eq!(
            ai.victory_denial(&game, 0),
            Some((1, GrandStrategy::Conquest))
        );

        // The `advanced_blind_to_leaders` ablation is silent on the same
        // position, and silent about its urgency, while reading the identical
        // pressure. An ablation that changed what the empire *sees* would
        // measure something other than the response.
        let mut blind = AdvancedAi::new();
        blind.deny_leaders = false;
        assert_eq!(blind.rival_victory_pressure(&game, 1).progress, 75);
        assert_eq!(blind.victory_denial(&game, 0), None);
        assert!(!blind.urgent_victory_threat(&game, 1));
        assert!(ai.urgent_victory_threat(&game, 1));
    }

    /// The site score is silent about barbarians and about being out of
    /// reach of your own cities. Off, it stays silent; on, both discount the
    /// site and neither can ever raise it.
    #[test]
    fn a_defensible_site_score_discounts_camps_and_isolation() {
        let mut game = Game::new_full(2, 30, 18, 7_741, 300, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("every empire starts with a settler");
        let home = game.units[&settler].pos;
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();

        // Somewhere well outside the founding city's support radius.
        let far = game
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| game.map.get(*pos).is_some_and(|t| !game.rules.is_water(t)))
            .find(|pos| game.wdist(home, *pos) > 8)
            .expect("the map is larger than the support radius");

        let plain = AdvancedAi::new();
        let mut weighed = AdvancedAi::new();
        weighed.defensible_sites = true;
        assert!(
            !plain.defensible_sites,
            "the measurement flag ships off"
        );

        // With no camp anywhere, only the isolation term can fire.
        assert!(game.barb_camps.is_empty());
        assert_eq!(weighed.defensibility(&game, 0, home), 0.0);
        assert!(
            weighed.defensibility(&game, 0, far) < 0.0,
            "a site out of reach of every friendly city is discounted"
        );
        assert_eq!(
            plain.settle_value(&game, 0, far),
            weighed.settle_value(&game, 0, far) - weighed.defensibility(&game, 0, far),
            "the flag adds exactly the defensibility term and nothing else"
        );

        // A camp beside the home tile discounts it; the untreated agent
        // scores that site exactly as it did before, which is the gap this
        // flag exists to measure.
        let before = plain.settle_value(&game, 0, home);
        let camp = *game
            .nbrs(home)
            .iter()
            .find(|pos| {
                game.map
                    .get(**pos)
                    .is_some_and(|t| !game.rules.is_water(t))
            })
            .expect("a founded city has a passable neighbour");
        game.barb_camps.insert(camp, 0);
        assert!(
            weighed.defensibility(&game, 0, home) < 0.0,
            "a camp one tile away discounts the site"
        );
        assert_eq!(
            plain.settle_value(&game, 0, home),
            before,
            "and the shipped score does not notice the camp at all"
        );
        assert!(
            weighed.settle_value(&game, 0, home) < before,
            "while the weighed score does"
        );
    }

    #[test]
    fn religious_denial_warns_early_but_never_ignores_match_point() {
        let mut game = Game::new_full(4, 30, 18, 7_216, 300, 0, false);
        for pid in 0..4 {
            game.current = pid;
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.apply(pid, &Action::FoundCity { unit: settler })
                .unwrap();
        }
        game.current = 0;
        game.players[1].religion = Some("Rival Faith".to_string());
        for owner in [1, 2] {
            let city = game.player_city_ids(owner)[0];
            game.cities
                .get_mut(&city)
                .unwrap()
                .pressure
                .insert("Rival Faith".to_string(), 1_000.0);
        }

        let ai = AdvancedAi::new();
        assert_eq!(ai.rival_victory_pressure(&game, 1).progress, 50);
        assert_eq!(
            ai.victory_denial(&game, 0),
            Some((1, GrandStrategy::Conquest)),
            "two remaining holdouts leave time to build and route a defense"
        );

        game.players[0].dvp = 13;
        assert_eq!(ai.victory_focus(&game, 0).progress, 65);
        assert_eq!(
            ai.victory_denial(&game, 0),
            None,
            "an early warning need not derail a meaningfully closer race"
        );

        let last_converted = game.player_city_ids(3)[0];
        game.cities
            .get_mut(&last_converted)
            .unwrap()
            .pressure
            .insert("Rival Faith".to_string(), 1_000.0);
        assert_eq!(ai.rival_victory_pressure(&game, 1).progress, 75);
        assert_eq!(
            ai.victory_denial(&game, 0),
            Some((1, GrandStrategy::Conquest)),
            "a one-conversion match point must interrupt even a close own race"
        );
    }

    /// The check this whole change rests on: does the filter ever fire in a
    /// real game?
    ///
    /// A treatment that does nothing reports a null for the wrong reason. The
    /// expansion-ceiling experiment cost a 240-game run to learn that, so this
    /// is asserted before any evaluation rather than after one: across ordinary
    /// four-player games Science must actually become unreachable, and the
    /// adaptive planner must actually stop choosing it.
    #[test]
    fn the_reachability_filter_fires_in_ordinary_games() {
        let mut refused = 0usize;
        let mut science_turns_with = 0usize;
        let mut science_turns_without = 0usize;
        let mut sampled = 0usize;

        for seed in 0..3u64 {
            let mut game = Game::new(4, 60, 38, 42_000 + seed, 500, 6);
            let mut ais = AdvancedAi::fleet(&game);
            let mut filtering = AdvancedAi::new();
            filtering.refuse_unreachable_lanes = true;
            let permissive = AdvancedAi::new();
            assert!(
                !permissive.refuse_unreachable_lanes,
                "the default is permissive: the filter measured no stronger"
            );

            while game.winner.is_none() && game.turn <= 260 {
                let pid = game.current;
                if pid == 0 {
                    sampled += 1;
                    if !filtering.science_reachable(&game, 0) {
                        refused += 1;
                    }
                    if filtering.victory_focus(&game, 0).strategy == GrandStrategy::Science {
                        science_turns_with += 1;
                    }
                    if permissive.victory_focus(&game, 0).strategy == GrandStrategy::Science {
                        science_turns_without += 1;
                    }
                }
                ais[pid].take_turn(&mut game, pid);
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &Action::EndTurn);
                }
            }
        }
        assert!(sampled > 100, "only {sampled} turns sampled");
        assert!(
            refused > 0,
            "Science was reachable on all {sampled} sampled turns, so the \
             filter never fires and any evaluation of it would measure the \
             stock agent under another name"
        );
        assert!(
            science_turns_with < science_turns_without,
            "the filter fired on {refused} turns but the adaptive planner still \
             chose Science as often as before ({science_turns_with} against \
             {science_turns_without}), so refusing the lane changed no decision"
        );
    }

    /// The same check for the routing reorder: does the opportunistic war arm
    /// actually preempt the finite Prophet race in ordinary games?
    ///
    /// The whole argument for `prophet_before_opportunism` is that the two
    /// windows overlap — the power-ratio arm opens at turn 55, and
    /// `religious_opening_viable` closes at 120. If they never actually
    /// collide, the reorder is a no-op and evaluating it would measure the
    /// stock agent under another name, which is exactly the failure the
    /// expansion-ceiling run paid 240 games to discover.
    #[test]
    fn the_prophet_reorder_fires_in_ordinary_games() {
        let mut preempted = 0usize;
        let mut differed = 0usize;
        let mut sampled = 0usize;

        for seed in 0..3u64 {
            let mut game = Game::new(4, 60, 38, 42_000 + seed, 500, 6);
            let mut ais = AdvancedAi::fleet(&game);
            let mut reordered = AdvancedAi::new();
            reordered.prophet_before_opportunism = true;
            let stock = AdvancedAi::new();
            assert!(
                !stock.prophet_before_opportunism,
                "the default keeps the shipped order, so this ships no behaviour change"
            );

            // Only the window where the two arms can collide is informative.
            while game.winner.is_none() && game.turn <= 130 {
                let pid = game.current;
                if pid == 0 && game.turn >= 40 {
                    sampled += 1;
                    let with = reordered.assess(&game, 0).strategy;
                    let without = stock.assess(&game, 0).strategy;
                    if with != without {
                        differed += 1;
                    }
                    if without == GrandStrategy::Conquest && with == GrandStrategy::Religion {
                        preempted += 1;
                    }
                }
                ais[pid].take_turn(&mut game, pid);
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &Action::EndTurn);
                }
            }
        }
        assert!(sampled > 100, "only {sampled} turns sampled");
        assert!(
            preempted > 0,
            "across {sampled} sampled turns in the 40..130 window the stock \
             cascade never chose Conquest where the reorder chooses Religion, \
             so the two arms do not actually collide and the treatment is a \
             no-op"
        );
        assert_eq!(
            differed, preempted,
            "the reorder is supposed to change exactly one thing — an \
             opportunistic war giving way to the Prophet race. It differed on \
             {differed} turns but only {preempted} of those were that swap, so \
             it is moving something else as well"
        );
    }

    #[test]
    fn victory_focus_tracks_religious_diplomatic_and_culture_races() {
        let ai = AdvancedAi::new();

        let mut religion = Game::new(2, 24, 16, 74, 80, 0);
        religion.players[0].religion = Some("Test Faith".to_string());
        assert_eq!(
            ai.victory_focus(&religion, 0).strategy,
            GrandStrategy::Religion
        );
        assert_eq!(
            AdvancedAi::legacy().victory_focus(&religion, 0).strategy,
            GrandStrategy::Science
        );

        let mut diplomacy = Game::new(2, 24, 16, 75, 80, 0);
        diplomacy.players[0].dvp = 14;
        assert_eq!(
            ai.victory_focus(&diplomacy, 0).strategy,
            GrandStrategy::Diplomacy
        );

        let mut culture = Game::new(2, 24, 16, 76, 80, 0);
        culture.players[0].tourism_lifetime = 100_000.0;
        culture.players[1].culture_lifetime = 100.0;
        assert_eq!(
            ai.victory_focus(&culture, 0).strategy,
            GrandStrategy::Culture
        );
    }

    #[test]
    fn bulk_rival_culture_pressure_matches_individual_victory_scans() {
        let mut game = Game::new(4, 30, 18, 7_603, 300, 0);
        for source in 0..4 {
            game.players[source].culture_lifetime = 500.0 + source as f64 * 350.0;
            for target in 0..4 {
                if source != target {
                    game.players[source]
                        .tourism_pressure
                        .insert(target, (source * 700 + target * 190) as f64);
                }
            }
        }

        let ai = AdvancedAi::new();
        let bulk = ai.rival_culture_pressures(&game);
        for pid in 0..4 {
            let batched =
                ai.rival_victory_pressure_with_culture(&game, pid, bulk.get(&pid).copied());
            let individual = ai.rival_victory_pressure(&game, pid);
            assert_eq!(batched.strategy, individual.strategy, "player {pid}");
            assert_eq!(batched.progress, individual.progress, "player {pid}");
            for rival in 0..4 {
                if rival == pid {
                    continue;
                }
                assert_eq!(
                    ai.rival_value_with_culture(&game, pid, rival, bulk.get(&rival).copied()),
                    ai.rival_value(&game, pid, rival),
                    "rival score {pid} -> {rival}"
                );
                assert_eq!(
                    ai.campaign_target_value_with_culture(
                        &game,
                        pid,
                        rival,
                        bulk.get(&rival).copied(),
                    ),
                    ai.campaign_target_value(&game, pid, rival),
                    "campaign score {pid} -> {rival}"
                );
            }
        }
        assert_eq!(
            ai.victory_denial_with_culture_pressures(&game, 0, &bulk),
            ai.victory_denial(&game, 0)
        );
    }

    #[test]
    fn disabled_victories_do_not_drive_strategy_or_rival_denial() {
        let mut game = Game::new(2, 24, 16, 7_602, 300, 0);
        game.victory_conditions.religious = false;
        game.victory_conditions.diplomatic = false;
        game.victory_conditions.score = false;
        game.players[0].dvp = 25;
        game.players[0].religion = Some("Test Faith".to_string());
        game.players[0].prophet_pending = true;
        game.players[1].dvp = 25;

        let ai = AdvancedAi::new();
        assert_eq!(
            ai.victory_focus(&game, 0).strategy,
            GrandStrategy::Science,
            "disabled diplomatic and religious progress must not outrank an enabled path"
        );
        assert!(!ai.religious_opening_viable(&game, 0));
        assert!(!ai.religious_offensive_posture(&game, 0, GrandStrategy::Science));

        let mut opening = Game::new(2, 24, 16, 7_604, 300, 0);
        opening.victory_conditions.religious = false;
        opening.players[0].prophet_pending = true;
        assert!(
            ai.religious_opening_viable(&opening, 0),
            "religion remains an economic subsystem when its victory is disabled"
        );
        let mut opening_ai = AdvancedAi::new();
        opening_ai.plan = Some(StrategicPlan {
            strategy: GrandStrategy::Religion,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: opening.turn,
            rush: false,
        });
        assert!(!opening_ai.plan_stale(&opening, 0));
        assert_ne!(
            ai.rival_victory_pressure(&game, 1).strategy,
            GrandStrategy::Diplomacy,
            "a disabled victory is not an imminent rival threat"
        );
        assert_eq!(ai.victory_denial(&game, 0), None);

        let mut targeted = AdvancedAi::targeting(VictoryTarget::Diplomacy);
        assert_eq!(
            targeted.victory_focus(&game, 0).strategy,
            GrandStrategy::Science,
            "an explicit target must yield when the game disables that victory"
        );
        targeted.plan = Some(StrategicPlan {
            strategy: GrandStrategy::Diplomacy,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        });
        assert!(targeted.plan_stale(&game, 0));

        let mut turn = Game::new(2, 24, 16, 7_603, 300, 0);
        turn.victory_conditions.religious = false;
        turn.victory_conditions.diplomatic = false;
        turn.victory_conditions.score = false;
        targeted.take_turn(&mut turn, 0);
        assert!(
            targeted.base.pursue_religion,
            "a disabled explicit target falls back to adaptive ancillary systems"
        );
        assert_ne!(
            targeted.current_plan().unwrap().strategy,
            GrandStrategy::Diplomacy,
            "the entire turn must fall back from a disabled explicit target"
        );
    }

    #[test]
    fn religious_focus_counts_foreign_conversions_not_the_founder() {
        let mut game = Game::new_full(4, 30, 18, 7_600, 300, 0, false);
        for pid in 0..4 {
            game.current = pid;
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.apply(pid, &Action::FoundCity { unit: settler })
                .unwrap();
        }
        game.current = 0;
        game.players[0].religion = Some("Test Faith".to_string());
        let convert = |game: &mut Game, owner: usize| {
            let city = game.player_city_ids(owner)[0];
            game.cities
                .get_mut(&city)
                .unwrap()
                .pressure
                .insert("Test Faith".to_string(), 1_000.0);
        };
        convert(&mut game, 0);

        let ai = AdvancedAi::new();
        let founded = ai.victory_focus(&game, 0);
        assert_eq!(founded.strategy, GrandStrategy::Religion);
        assert_eq!(
            founded.progress, 40,
            "the founder's own majority is not a foreign victory gain"
        );

        convert(&mut game, 1);
        assert_eq!(ai.victory_focus(&game, 0).progress, 60);
        convert(&mut game, 2);
        assert_eq!(ai.victory_focus(&game, 0).progress, 80);
        convert(&mut game, 3);
        assert_eq!(ai.victory_focus(&game, 0).progress, 100);
    }

    #[test]
    fn victory_focus_tracks_technology_before_the_first_space_project() {
        let ai = AdvancedAi::new();
        let mut game = Game::new(2, 24, 16, 761, 300, 0);
        game.turn = 111;
        let opening = ai.victory_focus(&game, 0);
        assert_eq!(opening.strategy, GrandStrategy::Science);
        assert_eq!(opening.progress, 25);

        let researched: Vec<Name> = game
            .rules
            .techs
            .keys()
            .take(game.rules.techs.len() * 2 / 3)
            .cloned()
            .collect();
        game.players[0].techs.extend(researched);
        let developed = ai.victory_focus(&game, 0);
        assert_eq!(developed.strategy, GrandStrategy::Science);
        assert!(developed.progress >= 44, "progress={}", developed.progress);

        game.players[0].civ = "China".to_string();
        game.players[0].techs.clear();
        assert_eq!(ai.victory_focus(&game, 0).progress, 45);
    }

    #[test]
    fn adaptive_science_readiness_commits_to_the_rocketry_path() {
        let mut game = Game::new(2, 24, 16, 76_001, 300, 0);
        let ai = AdvancedAi::new();
        let rocketry_path: Vec<Name> = game
            .rules
            .techs
            .keys()
            .filter(|tech| ai.tech_leads_to(&game, tech, "rocketry"))
            .cloned()
            .collect();
        for tech in rocketry_path
            .iter()
            .filter(|tech| tech.as_str() != "rocketry")
        {
            game.players[0].techs.insert(*tech);
        }
        game.players[0].dvp = 10;

        let focus = ai.victory_focus(&game, 0);
        assert_eq!(focus.strategy, GrandStrategy::Science);
        assert!(focus.progress > 50);

        let plan = StrategicPlan {
            strategy: GrandStrategy::Science,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };
        ai.advanced_research(&mut game, 0, &plan);
        assert_eq!(game.players[0].research.as_deref(), Some("rocketry"));
    }

    #[test]
    fn mature_diplomatic_plan_prepares_one_science_backup() {
        let mut game = Game::new(2, 24, 16, 76_002, 500, 0);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        let site = game.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != game.cities[&city].pos)
            .unwrap();
        {
            let tile = game.map.tiles.get_mut(&site).unwrap();
            tile.terrain = crate::name!("plains");
            tile.feature = None;
            tile.resource = None;
            tile.hills = false;
        }
        let ai = AdvancedAi::new();
        let rocketry_path: Vec<Name> = game
            .rules
            .techs
            .keys()
            .filter(|tech| ai.tech_leads_to(&game, tech, "rocketry"))
            .cloned()
            .collect();
        for tech in rocketry_path
            .iter()
            .filter(|tech| tech.as_str() != "rocketry")
        {
            game.players[0].techs.insert(*tech);
        }
        game.turn = game.standard_duration(220);
        let plan = StrategicPlan {
            strategy: GrandStrategy::Diplomacy,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };

        assert!(ai.diplomatic_science_backup(&game, 0, &plan));
        ai.advanced_research(&mut game, 0, &plan);
        assert_eq!(game.players[0].research.as_deref(), Some("rocketry"));

        game.players[0].techs.insert(crate::name!("rocketry"));
        game.players[0].research = None;
        ai.science_production(&mut game, 0);
        assert!(matches!(
            game.cities[&city].queue.first(),
            Some(Item::District { district, .. }) if district == "spaceport"
        ));

        game.victory_conditions.science = false;
        assert!(!ai.diplomatic_science_backup(&game, 0, &plan));
    }

    #[test]
    fn adaptive_research_routes_to_the_live_victory_plan() {
        let plan = |strategy| StrategicPlan {
            strategy,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: 1,
            rush: false,
        };
        let ai = AdvancedAi::new();

        let mut science = Game::new_full(1, 20, 14, 762, 300, 0, false);
        ai.advanced_research(&mut science, 0, &plan(GrandStrategy::Science));
        assert_eq!(
            science.players[0].research.as_deref(),
            Some("animal_husbandry"),
            "the cheapest available prerequisite toward Rocketry wins"
        );

        let mut culture = Game::new_full(1, 20, 14, 763, 300, 0, false);
        ai.advanced_research(&mut culture, 0, &plan(GrandStrategy::Culture));
        assert_eq!(
            culture.players[0].research.as_deref(),
            Some("mining"),
            "the available prerequisite toward Printing wins"
        );

        let mut diplomacy = Game::new_full(1, 20, 14, 764, 300, 0, false);
        ai.advanced_research(&mut diplomacy, 0, &plan(GrandStrategy::Diplomacy));
        let tech = diplomacy.players[0].research.as_deref().unwrap();
        assert!(
            ai.tech_leads_to(&diplomacy, tech, "seasteads"),
            "diplomatic research must advance toward Seasteads' victory point"
        );
        let civic = diplomacy.players[0].civic.as_deref().unwrap();
        assert!(
            ai.civic_leads_to(&diplomacy, civic, "global_warming_mitigation"),
            "diplomatic culture must advance toward Global Warming Mitigation's victory point"
        );
    }

    #[test]
    fn religious_openings_fill_available_prophet_slots_with_stable_contenders() {
        let mut game = Game::new_full(4, 34, 20, 76_101, 300, 0, false);
        let mut capitals = Vec::new();
        for pid in 0..4 {
            game.current = pid;
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.apply(pid, &Action::FoundCity { unit: settler })
                .unwrap();
            capitals.push(game.player_city_ids(pid)[0]);
        }
        for (pid, capital) in capitals.into_iter().enumerate() {
            let anchor = game.cities[&capital].pos;
            found_nearby_test_city(&mut game, pid, anchor);
        }
        game.current = 0;
        game.turn = 60;

        let ai = AdvancedAi::new();
        let contenders: Vec<_> = (0..4)
            .filter(|pid| ai.religious_opening_viable(&game, *pid))
            .collect();
        assert_eq!(contenders.len(), game.max_religions().min(4));

        let founder = contenders[0];
        game.players[founder].religion = Some("Rival Faith".to_string());
        let counters = (0..4)
            .filter(|pid| ai.religious_opening_viable(&game, *pid))
            .count();
        assert_eq!(counters, (game.max_religions() - 1).min(3));
    }

    #[test]
    fn ordinary_religious_plan_routes_research_to_astrology() {
        let mut game = Game::new_full(1, 20, 14, 76_102, 120, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let plan = StrategicPlan {
            strategy: GrandStrategy::Religion,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };

        AdvancedAi::new().advanced_research(&mut game, 0, &plan);
        assert_eq!(game.players[0].research.as_deref(), Some("astrology"));
    }

    #[test]
    fn religious_production_builds_prophet_infrastructure_then_runs_prayers() {
        let mut game = Game::new_full(1, 20, 14, 76_103, 120, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        game.players[0].techs.insert(crate::name!("astrology"));
        install_ai_test_district(&mut game, city, "holy_site");
        game.cities.get_mut(&city).unwrap().queue.clear();

        let ai = AdvancedAi::new();
        ai.religious_production(&mut game, 0);
        assert!(matches!(
            game.cities[&city].queue.first(),
            Some(Item::Building { building }) if building == "shrine"
        ));

        game.cities.get_mut(&city).unwrap().queue.clear();
        game.cities
            .get_mut(&city)
            .unwrap()
            .buildings
            .push(crate::name!("shrine"));
        ai.religious_production(&mut game, 0);
        assert!(matches!(
            game.cities[&city].queue.first(),
            Some(Item::Project { project }) if project == "holy_site_prayers"
        ));
    }

    #[test]
    fn competitive_religious_opening_produces_multiple_founders() {
        let mut game = Game::new_full(
            4, 24, 16, crate::rng::fixture_seed("PROPHET", 76_106), 110, 0, false,
        );
        let mut ais = AdvancedAi::fleet(&game);
        run_game(&mut game, &mut ais);
        assert!(
            game.religions_founded() >= 2,
            "a stock Prophet race should not end with one uncontested founder: turn {}, {:?}",
            game.turn,
            game.players
                .iter()
                .take(4)
                .map(|player| (
                    &player.civ,
                    &player.religion,
                    player.prophet_pending,
                    player.gpp.get("prophet"),
                    player.techs.contains(&crate::name!("astrology")),
                    game.player_city_ids(player.id)
                        .iter()
                        .filter(|city| game.cities[city].districts.contains_key(crate::name!("holy_site")))
                        .count(),
                ))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn rival_pressure_uses_living_civilizations_for_active_victory_races() {
        let mut game = Game::new_full(3, 24, 16, 760, 300, 0, false);
        game.players[1].tourism_lifetime = 300_000.0;
        game.players[0].culture_lifetime = 100.0;
        game.players[2].culture_lifetime = 1_000_000.0;
        game.players[2].alive = false;

        let pressure = AdvancedAi::new().rival_victory_pressure(&game, 1);
        assert_eq!(pressure.strategy, GrandStrategy::Culture);
        assert_eq!(pressure.progress, 100);
    }

    #[test]
    fn strategic_plan_denies_an_imminent_victory_instead_of_farming_a_weak_rival() {
        let establish_capitals = |game: &mut Game| {
            for pid in 0..3 {
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
            game.turn = 190;
        };

        let mut science = Game::new_full(3, 36, 22, 761, 300, 0, false);
        establish_capitals(&mut science);
        science.players[2].science_projects.extend([
            "launch_earth_satellite".to_string(),
            "launch_moon_landing".to_string(),
            "launch_mars_colony".to_string(),
            "exoplanet_expedition".to_string(),
        ]);
        science.players[2].exoplanet_distance = 42.0;
        let ai = AdvancedAi::new();
        let pressure = ai.rival_victory_pressure(&science, 2);
        assert_eq!(pressure.strategy, GrandStrategy::Science);
        assert!(pressure.progress >= 95);
        let plan = ai.assess(&science, 0);
        assert_eq!(plan.strategy, GrandStrategy::Conquest);
        assert_eq!(plan.target_player, Some(2));

        let mut culture = Game::new_full(3, 36, 22, 762, 300, 0, false);
        establish_capitals(&mut culture);
        culture.players[1].tourism_lifetime = 300_000.0;
        culture.players[0].culture_lifetime = 100.0;
        culture.players[2].culture_lifetime = 100.0;
        let pressure = ai.rival_victory_pressure(&culture, 1);
        assert_eq!(pressure.strategy, GrandStrategy::Culture);
        assert_eq!(pressure.progress, 100);
        let plan = ai.assess(&culture, 0);
        assert_eq!(plan.strategy, GrandStrategy::Culture);
        assert_eq!(plan.target_player, Some(1));
    }

    #[test]
    fn science_denial_starts_when_the_final_expedition_launches() {
        let mut game = Game::new_full(3, 36, 22, 76_120, 300, 0, false);
        for pid in 0..3 {
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
        game.turn = 190;
        game.players[2]
            .science_projects
            .insert("exoplanet_expedition".to_string());

        let ai = AdvancedAi::new();
        let pressure = ai.rival_victory_pressure(&game, 2);
        assert_eq!(pressure.strategy, GrandStrategy::Science);
        assert_eq!(pressure.progress, 78);
        assert_eq!(
            ai.victory_denial(&game, 0),
            Some((2, GrandStrategy::Conquest)),
            "the final launch must leave the defender time to begin its counter-campaign"
        );
        let plan = ai.assess(&game, 0);
        assert_eq!(plan.strategy, GrandStrategy::Conquest);
        assert_eq!(plan.target_player, Some(2));

        // The three response shapes on one position, so the arms cannot
        // silently converge: ship declares, `counter_in_lane` races the same
        // rival, `counter_stand_down` lets this threat pass. All three read
        // the identical pressure, so what separates them is the answer and
        // never the perception.
        let mut in_lane = AdvancedAi::new();
        in_lane.counter_in_lane = true;
        assert_eq!(
            in_lane.victory_denial(&game, 0),
            Some((2, GrandStrategy::Science))
        );

        let mut stand_down = AdvancedAi::new();
        stand_down.counter_stand_down = true;
        assert_eq!(stand_down.victory_denial(&game, 0), None);
        assert_eq!(stand_down.rival_victory_pressure(&game, 2).progress, 78);
    }

    #[test]
    fn religious_match_point_interrupts_before_the_winning_conversion() {
        let mut game = Game::new_full(4, 42, 24, 7_621, 300, 0, false);
        for pid in 0..4 {
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
        game.turn = 150;
        game.players[3].religion = Some("Runaway Faith".to_string());
        for owner in 1..4 {
            let city = game.player_city_ids(owner)[0];
            game.cities
                .get_mut(&city)
                .unwrap()
                .pressure
                .insert("Runaway Faith".to_string(), 1_000.0);
        }

        let ai = AdvancedAi::new();
        let pressure = ai.rival_victory_pressure(&game, 3);
        assert_eq!(pressure.strategy, GrandStrategy::Religion);
        assert_eq!(pressure.progress, 75);
        assert_eq!(
            ai.victory_denial(&game, 0),
            Some((3, GrandStrategy::Conquest)),
            "a non-founder must attack before the fourth conversion ends the game"
        );
        let plan = ai.assess(&game, 0);
        assert_eq!(plan.strategy, GrandStrategy::Conquest);
        assert_eq!(plan.target_player, Some(3));

        game.players[0].religion = Some("Home Faith".to_string());
        let home = game.player_city_ids(0)[0];
        game.cities
            .get_mut(&home)
            .unwrap()
            .pressure
            .insert("Home Faith".to_string(), 1_000.0);
        assert_eq!(
            ai.victory_denial(&game, 0),
            Some((3, GrandStrategy::Religion)),
            "a founder should defend its cities with its own religion"
        );
    }

    #[test]
    fn religious_match_point_spends_the_reserve_only_in_own_faith_cities() {
        let mut game = Game::new_full(4, 42, 24, 7_622, 300, 0, false);
        for pid in 0..4 {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.current = pid;
            game.apply(pid, &Action::FoundCity { unit: settler })
                .unwrap();
        }
        let converted_capital = game.player_city_ids(0)[0];
        let faithful_city = found_test_city(&mut game, 0);
        install_test_holy_site(&mut game, converted_capital);
        install_test_holy_site(&mut game, faithful_city);
        game.current = 0;
        game.turn = 150;
        game.players[0].religion = Some("Home Faith".to_string());
        game.players[0].holy_city = Some(converted_capital);
        game.players[0].techs.insert(crate::name!("astrology"));
        game.players[0].faith = 200.0;
        game.players[3].religion = Some("Runaway Faith".to_string());
        game.cities
            .get_mut(&converted_capital)
            .unwrap()
            .pressure
            .insert("Runaway Faith".to_string(), 1_000.0);
        game.cities
            .get_mut(&faithful_city)
            .unwrap()
            .pressure
            .insert("Home Faith".to_string(), 1_000.0);
        for owner in 1..4 {
            let city = game.player_city_ids(owner)[0];
            game.cities
                .get_mut(&city)
                .unwrap()
                .pressure
                .insert("Runaway Faith".to_string(), 1_000.0);
        }

        let ai = AdvancedAi::new();
        let emergency = ai
            .victory_denial(&game, 0)
            .is_some_and(|(_, counter)| counter == GrandStrategy::Religion);
        assert!(emergency);
        ai.religious_spending(&mut game, 0, emergency);

        let missionary = game
            .units
            .values()
            .find(|unit| unit.owner == 0 && unit.kind == "missionary")
            .expect("match-point defense should spend the ordinary Faith reserve");
        assert_eq!(missionary.religion.as_deref(), Some("Home Faith"));
        assert_eq!(missionary.pos, game.cities[&faithful_city].pos);
        assert!(game.players[0].faith < 1.0);
    }

    /// The military answer to a religious offensive: step onto an adjacent
    /// enemy Missionary inside our own territory and condemn it.
    #[test]
    fn military_units_step_onto_and_condemn_enemy_missionaries() {
        let mut game = Game::new_full(2, 30, 18, 7_631, 200, 0, false);
        for pid in 0..2 {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.current = pid;
            game.apply(pid, &Action::FoundCity { unit: settler }).unwrap();
        }
        game.current = 0;
        game.at_war.insert((0, 1));
        let home = game.cities[&game.player_city_ids(0)[0]].pos;
        // Both staged tiles must be free: the capital's own starting units
        // stand in this ring, and a friendly unit already there blocks the
        // step onto the intruder.
        let soldier_tile = game
            .nbrs(home)
            .into_iter()
            .find(|p| {
                game.units_at(*p).is_empty()
                    && game.map.get(*p).is_some_and(|t| {
                        game.rules.is_passable(t) && !game.rules.is_water(t)
                    })
            })
            .unwrap();
        // Condemning is a defense of our own territory, so the intruder has
        // to stand on a tile the capital actually owns - one that borders
        // both the soldier and the city centre.
        let intruder_tile = game
            .nbrs(soldier_tile)
            .into_iter()
            .find(|p| {
                *p != home
                    && game.nbrs(home).contains(p)
                    && game.units_at(*p).is_empty()
                    && game.map.get(*p).is_some_and(|t| {
                        game.rules.is_passable(t) && !game.rules.is_water(t)
                    })
            })
            .unwrap();
        for position in [soldier_tile, intruder_tile] {
            let tile = game.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("plains");
            tile.feature = None;
            tile.improvement = None;
            tile.hills = false;
        }
        game.map.set_river_edge(soldier_tile, intruder_tile, false);
        let soldier = game.spawn_test_unit("warrior", 0, soldier_tile);
        let missionary = game.spawn_test_unit("missionary", 1, intruder_tile);
        game.units.get_mut(&missionary).unwrap().religion = Some("Rival Faith".to_string());

        let mut ai = AdvancedAi::new();
        assert!(ai.condemn_step(&mut game, 0, soldier), "should engage");
        assert!(
            !game.units.contains_key(&missionary),
            "the adjacent missionary should be condemned, not ignored"
        );
    }

    #[test]
    fn non_founder_buys_adopted_faith_missionaries_to_defend_home() {
        let mut game = Game::new_full(3, 42, 24, 7_624, 300, 0, false);
        for pid in 0..3 {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.current = pid;
            game.apply(pid, &Action::FoundCity { unit: settler })
                .unwrap();
        }
        let converted_capital = game.player_city_ids(0)[0];
        let adopted_city = found_test_city(&mut game, 0);
        install_test_holy_site(&mut game, adopted_city);
        game.current = 0;
        game.turn = 150;
        // Player 0 founded no religion; a living rival's faith holds their
        // capital while a founderless neighbor faith holds the second city.
        game.players[0].techs.insert(crate::name!("astrology"));
        game.players[0].faith = 600.0;
        game.players[1].religion = Some("Runaway Faith".to_string());
        game.cities
            .get_mut(&converted_capital)
            .unwrap()
            .pressure
            .insert("Runaway Faith".to_string(), 1_000.0);
        game.cities
            .get_mut(&adopted_city)
            .unwrap()
            .pressure
            .insert("Neighbor Faith".to_string(), 1_000.0);

        let ai = AdvancedAi::new();
        let threat = ai
            .home_conversion_threat(&game, 0)
            .expect("a rival majority in the capital is a home threat");
        assert_eq!(threat, "Runaway Faith");
        ai.religious_defense(&mut game, 0, &threat);

        let missionary = game
            .units
            .values()
            .find(|unit| unit.owner == 0 && unit.kind == "missionary")
            .expect("defense should buy a missionary of the adopted faith");
        assert_eq!(missionary.religion.as_deref(), Some("Neighbor Faith"));
        assert_eq!(missionary.pos, game.cities[&adopted_city].pos);
    }

    #[test]
    fn apostle_launches_inquisition_before_evangelizing_when_core_is_lost() {
        let mut game = Game::new_full(2, 30, 18, 7_623, 200, 0, false);
        for pid in 0..2 {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.current = pid;
            game.apply(pid, &Action::FoundCity { unit: settler })
                .unwrap();
        }
        let holy_city = game.player_city_ids(0)[0];
        let converted_city = found_test_city(&mut game, 0);
        game.current = 0;
        game.players[0].religion = Some("Home Faith".to_string());
        game.players[0].holy_city = Some(holy_city);
        game.players[0].religion_beliefs = vec!["work_ethic".to_string(), "tithe".to_string()];
        game.cities
            .get_mut(&holy_city)
            .unwrap()
            .pressure
            .insert("Home Faith".to_string(), 1_000.0);
        game.cities
            .get_mut(&converted_city)
            .unwrap()
            .pressure
            .insert("Rival Faith".to_string(), 1_000.0);
        let apostle = game.spawn_test_unit("apostle", 0, game.cities[&holy_city].pos);
        game.units.get_mut(&apostle).unwrap().religion = Some("Home Faith".to_string());

        assert!(AdvancedAi::new().advanced_religious_step(&mut game, 0, apostle, false));
        assert!(!game.units.contains_key(&apostle));
        assert_eq!(
            game.players[0]
                .counters
                .get("inquisition")
                .copied()
                .unwrap_or(0),
            1
        );
        assert_eq!(game.players[0].religion_beliefs.len(), 2);
    }

    #[test]
    fn religious_strategy_reconverts_its_core_before_chasing_foreign_cities() {
        let mut game = Game::new_full(2, 30, 18, 763, 200, 0, false);
        for pid in 0..2 {
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
        game.players[0].religion = Some("Our Faith".to_string());
        let home = game.player_city_ids(0)[0];
        let foreign = game.player_city_ids(1)[0];
        game.cities
            .get_mut(&home)
            .unwrap()
            .pressure
            .insert("Rival Faith".to_string(), 1_000.0);
        game.cities
            .get_mut(&foreign)
            .unwrap()
            .pressure
            .insert("Rival Faith".to_string(), 1_000.0);
        let missionary = game.spawn_test_unit("missionary", 0, game.cities[&home].pos);
        game.units.get_mut(&missionary).unwrap().religion = Some("Our Faith".to_string());

        assert!(AdvancedAi::new().advanced_missionary_step(&mut game, 0, missionary, true));
        assert!(
            game.cities[&home]
                .pressure
                .get("Our Faith")
                .copied()
                .unwrap_or(0.0)
                > 0.0
        );
        assert_eq!(game.units[&missionary].pos, game.cities[&home].pos);
    }

    #[test]
    fn nonreligious_strategy_reinforces_its_founded_faith_before_conversion() {
        let mut game = Game::new_full(2, 30, 18, 7_634, 200, 0, false);
        for pid in 0..2 {
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
        game.players[0].religion = Some("Our Faith".to_string());
        let home = game.player_city_ids(0)[0];
        game.cities.get_mut(&home).unwrap().pressure.extend([
            ("Our Faith".to_string(), 1_000.0),
            ("Rival Faith".to_string(), 600.0),
        ]);
        assert_eq!(game.city_religion(&game.cities[&home]), Some("Our Faith"));
        let missionary = game.spawn_test_unit("missionary", 0, game.cities[&home].pos);
        game.units.get_mut(&missionary).unwrap().religion = Some("Our Faith".to_string());
        let before = game.cities[&home].pressure["Our Faith"];

        assert!(AdvancedAi::targeting(VictoryTarget::Science)
            .advanced_missionary_step(&mut game, 0, missionary, false));
        assert!(game.cities[&home].pressure["Our Faith"] > before);
        assert_eq!(game.units[&missionary].charges, 2);
        game.units.get_mut(&missionary).unwrap().moves_left = 4.0;
        assert!(
            !AdvancedAi::targeting(VictoryTarget::Science)
                .advanced_missionary_step(&mut game, 0, missionary, false),
            "a defensive unit must hold once its home is safe instead of starting a foreign crusade"
        );
    }

    #[test]
    fn missionary_routes_to_spread_range_around_a_mountain_detour() {
        let mut game = Game::new_full(2, 30, 18, 7_633, 200, 0, false);
        for pid in 0..2 {
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
        game.players[0].religion = Some("Our Faith".to_string());
        let target_city = game.player_city_ids(1)[0];
        let target = game.cities[&target_city].pos;
        game.cities
            .get_mut(&target_city)
            .unwrap()
            .pressure
            .insert("Rival Faith".to_string(), 1_000.0);
        // Saturate the missionary's own capital so the rival city is the only
        // thing left to convert. This case is about *routing*, and without it
        // the answer depends on where the generated map happened to put home:
        // a capital that lands near the missionary outscores a foreign city
        // three hexes away and the unit walks west, which is a correct choice
        // about a different question.
        let home_city = game.player_city_ids(0)[0];
        game.cities
            .get_mut(&home_city)
            .unwrap()
            .pressure
            .insert("Our Faith".to_string(), 100_000.0);

        let start = (target.0 - 3, target.1);
        let direct = (target.0 - 2, target.1);
        let detour = (target.0 - 2, target.1 - 1);
        let onward = (target.0 - 1, target.1 - 1);
        for position in [start, direct, detour, onward] {
            assert!(game.map.tiles.contains_key(&position));
        }
        // Flatten the approach so the detour is the only obstacle. Reading the
        // generated terrain instead left the case at the mercy of whatever the
        // seed happened to put beside the target - a river edge or a cliff in
        // the corridor blocks the first step for reasons the case is not about.
        for position in game.wdisk(target, 4) {
            if position == target {
                continue;
            }
            let tile = game.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
            tile.district = None;
            tile.wonder = None;
            tile.cliff_edges = [false; 6];
            tile.river_edges = [false; 6];
        }
        game.map.tiles.get_mut(&direct).unwrap().terrain = crate::name!("mountain");

        let missionary = game.spawn_test_unit("missionary", 0, start);
        game.units.get_mut(&missionary).unwrap().religion = Some("Our Faith".to_string());
        let ai = AdvancedAi::new();
        assert!(ai.advanced_missionary_step(&mut game, 0, missionary, true));
        assert_eq!(
            game.wdist(game.units[&missionary].pos, target),
            3,
            "the first legal route step must accept a sideways mountain detour"
        );

        for _ in 0..8 {
            if !game.units.contains_key(&missionary) || game.units[&missionary].charges < 3 {
                break;
            }
            game.units.get_mut(&missionary).unwrap().moves_left = 4.0;
            assert!(ai.advanced_missionary_step(&mut game, 0, missionary, true));
        }
        assert!(
            !game.units.contains_key(&missionary) || game.units[&missionary].charges < 3,
            "a reachable foreign city must receive a spread instead of trapping the unit"
        );
    }

    #[test]
    fn apostles_complete_one_worship_and_one_enhancer_belief_for_the_plan() {
        let mut game = Game::new(2, 24, 16, 7_632, 200, 0);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        game.players[0].religion = Some("Planned Faith".to_string());
        game.players[0].religion_beliefs = vec!["work_ethic".to_string(), "tithe".to_string()];
        let ai = AdvancedAi::targeting(VictoryTarget::Science);

        let first = game.spawn_test_unit("apostle", 0, game.cities[&city].pos);
        game.units.get_mut(&first).unwrap().religion = Some("Planned Faith".to_string());
        assert!(ai.advanced_religious_step(&mut game, 0, first, false));
        assert!(game.players[0]
            .religion_beliefs
            .contains(&"wat".to_string()));

        let second = game.spawn_test_unit("apostle", 0, game.cities[&city].pos);
        game.units.get_mut(&second).unwrap().religion = Some("Planned Faith".to_string());
        assert!(ai.advanced_religious_step(&mut game, 0, second, false));
        assert_eq!(game.players[0].religion_beliefs.len(), 4);
        assert_eq!(
            game.players[0]
                .religion_beliefs
                .iter()
                .filter(|belief| game.rules.beliefs.enhancer.contains_key(*belief))
                .count(),
            1
        );
        assert_eq!(
            game.players[0]
                .religion_beliefs
                .iter()
                .filter(|belief| game.rules.beliefs.worship.contains_key(*belief))
                .count(),
            1
        );
    }

    #[test]
    fn science_target_reserves_a_spaceport_then_queues_the_project_chain() {
        let mut g = Game::new(2, 24, 16, 71, 200, 0);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = g.player_city_ids(0)[0];
        let site = g.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != g.cities[&city].pos)
            .unwrap();
        {
            let tile = g.map.tiles.get_mut(&site).unwrap();
            tile.terrain = crate::name!("plains");
            tile.feature = None;
            tile.resource = None;
            tile.hills = false;
        }
        g.players[0].techs.insert(crate::name!("rocketry"));
        let ai = AdvancedAi::targeting(VictoryTarget::Science);
        ai.science_production(&mut g, 0);
        let spaceport = match g.cities[&city].queue.first() {
            Some(Item::District { district, pos }) if district == "spaceport" => *pos,
            queued => panic!("expected a queued spaceport, got {queued:?}"),
        };

        g.cities.get_mut(&city).unwrap().queue.clear();
        g.cities
            .get_mut(&city)
            .unwrap()
            .districts
            .insert(crate::name!("spaceport"), spaceport);
        ai.science_production(&mut g, 0);
        assert!(matches!(
            g.cities[&city].queue.first(),
            Some(Item::Project { project }) if project == "launch_earth_satellite"
        ));
    }

    #[test]
    fn science_target_parallelizes_lasers_across_cities_without_local_spaceport_spam() {
        let mut game = Game::new_full(1, 34, 20, 71_002, 320, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| game.units[uid].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let mut cities = game.player_city_ids(0);
        while cities.len() < 3 {
            let center = game
                .map
                .tiles
                .iter()
                .find(|(position, tile)| {
                    tile.owner_city.is_none()
                        && game.rules.is_passable(tile)
                        && !game.rules.is_water(tile)
                        && cities
                            .iter()
                            .all(|city| game.wdist(**position, game.cities[city].pos) >= 7)
                })
                .map(|(position, _)| *position)
                .unwrap();
            game.found_city_for(0, center, None);
            cities = game.player_city_ids(0);
        }
        game.players[0].techs = game.rules.techs.keys().cloned().collect();
        game.players[0].civics = game.rules.civics.keys().cloned().collect();
        game.players[0].science_projects.extend([
            "launch_earth_satellite".to_string(),
            "launch_moon_landing".to_string(),
            "launch_mars_colony".to_string(),
            "exoplanet_expedition".to_string(),
        ]);
        for city in &cities {
            game.cities.get_mut(city).unwrap().pop = 12;
            for position in game.cities[city].owned_tiles.clone() {
                if position == game.cities[city].pos {
                    continue;
                }
                let tile = game.map.tiles.get_mut(&position).unwrap();
                tile.terrain = crate::name!("plains");
                tile.feature = None;
                tile.hills = false;
                tile.resource = None;
                tile.improvement = None;
                tile.district = None;
                tile.district_foundation = None;
                tile.wonder = None;
            }
        }
        for city in cities.iter().take(2) {
            let position = game.cities[city]
                .owned_tiles
                .iter()
                .copied()
                .find(|position| *position != game.cities[city].pos)
                .unwrap();
            game.map.tiles.get_mut(&position).unwrap().district = Some(crate::name!("spaceport"));
            game.cities
                .get_mut(city)
                .unwrap()
                .districts
                .insert(crate::name!("spaceport"), position);
        }
        game.cities.get_mut(&cities[0]).unwrap().queue = vec![Item::Project {
            project: crate::name!("lagrange_laser_station"),
        }];

        let ai = AdvancedAi::targeting(VictoryTarget::Science);
        ai.science_production(&mut game, 0);
        assert_eq!(
            cities
                .iter()
                .filter(|city| matches!(
                    game.cities[city].queue.first(),
                    Some(Item::Project { project }) if project == "lagrange_laser_station"
                ))
                .count(),
            2
        );

        ai.science_production(&mut game, 0);
        assert!(matches!(
            game.cities[&cities[2]].queue.first(),
            Some(Item::District { district, .. }) if district == "spaceport"
        ));

        let duplicate = Item::District {
            district: crate::name!("spaceport"),
            pos: game.district_sites(cities[0], crate::name!("spaceport"))[0],
        };
        let plan = StrategicPlan {
            strategy: GrandStrategy::Science,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };
        assert!(
            ai.production_value(&game, 0, cities[0], &duplicate, &plan, &ai.counts(&game, 0))
                <= -10_000.0
        );
    }

    #[test]
    fn district_search_values_unique_families_and_real_housing_need() {
        let mut game = Game::new_full(1, 20, 14, 71_001, 200, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        let site = game.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != game.cities[&city].pos)
            .unwrap();
        {
            let tile = game.map.tiles.get_mut(&site).unwrap();
            tile.terrain = crate::name!("plains");
            tile.feature = None;
            tile.resource = None;
            tile.hills = true;
        }

        for (unique, family) in [
            ("seowon", "campus"),
            ("lavra", "holy_site"),
            ("hansa", "industrial_zone"),
            ("bath", "aqueduct"),
            ("mbanza", "neighborhood"),
        ] {
            assert_eq!(game.district_family(Name::new(unique)), Name::new(family));
        }

        let ai = AdvancedAi::targeting(VictoryTarget::Science);
        let counts = ai.counts(&game, 0);
        let mut plan = StrategicPlan {
            strategy: GrandStrategy::Science,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };
        let seowon = Item::District {
            district: crate::name!("seowon"),
            pos: site,
        };
        let science_value = ai.production_value(&game, 0, city, &seowon, &plan, &counts);
        plan.strategy = GrandStrategy::Expansion;
        let expansion_value = ai.production_value(&game, 0, city, &seowon, &plan, &counts);
        assert!(
            science_value > expansion_value,
            "a Seowon must inherit the Campus science strategy bonus"
        );

        // The rules engine, not a second AI-only district cap, decides
        // eligibility. A high-population city with an undeveloped specialty
        // core should still recognize the urgent housing value of a
        // Neighborhood.
        for district in ["campus", "holy_site", "commercial_hub"] {
            game.cities
                .get_mut(&city)
                .unwrap()
                .districts
                .insert(Name::new(district), site);
        }
        game.cities.get_mut(&city).unwrap().pop = 12;
        let crowded = Item::District {
            district: crate::name!("neighborhood"),
            pos: site,
        };
        let crowded_value = ai.production_value(&game, 0, city, &crowded, &plan, &counts);
        game.cities.get_mut(&city).unwrap().pop = 2;
        let roomy_value = ai.production_value(&game, 0, city, &crowded, &plan, &counts);
        assert!(crowded_value > -1_000.0);
        assert!(
            crowded_value > roomy_value,
            "appeal housing must be worth more when growth is constrained"
        );
    }

    #[test]
    fn production_search_uses_incremental_remaining_cost_for_paused_builds() {
        let mut game = Game::new_full(1, 20, 14, 71_002, 200, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.players[0].civ = "Egypt".to_string();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        let monument = Item::Building {
            building: crate::name!("monument"),
        };
        let builder = Item::Unit {
            unit: crate::name!("builder"),
        };
        let plan = StrategicPlan {
            strategy: GrandStrategy::Expansion,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };
        let ai = AdvancedAi::new();
        let counts = ai.counts(&game, 0);
        let fresh = ai.production_value(&game, 0, city, &monument, &plan, &counts);

        game.apply(
            0,
            &Action::Produce {
                city,
                item: monument.clone(),
            },
        )
        .unwrap();
        game.cities.get_mut(&city).unwrap().production = 20.0;
        game.apply(
            0,
            &Action::Produce {
                city,
                item: builder,
            },
        )
        .unwrap();
        let resumed = ai.production_value(&game, 0, city, &monument, &plan, &counts);

        assert_eq!(
            game.item_remaining_cost_for_city(0, city, &monument),
            game.item_cost_for_city(0, city, &monument) - 20.0
        );
        assert!(
            resumed > fresh,
            "incremental evaluation should prefer finishing invested infrastructure"
        );
    }

    #[test]
    fn military_production_keeps_land_sea_and_air_force_gaps_separate() {
        let mut game = Game::new_full(1, 20, 14, 71_003, 120, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        for unit in game.player_unit_ids(0) {
            if game.rules.units[game.units[&unit].kind].class == "military" {
                game.remove_unit(unit);
            }
        }
        let water = game
            .map
            .tiles
            .iter()
            .find(|(_, tile)| game.rules.is_water(tile))
            .map(|(position, _)| *position)
            .unwrap();
        game.spawn_test_unit("galley", 0, water);
        let city = game.player_city_ids(0)[0];
        let plan = StrategicPlan {
            strategy: GrandStrategy::Science,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 1,
            assessed_turn: game.turn,
            rush: false,
        };
        let ai = AdvancedAi::targeting(VictoryTarget::Science);
        let counts = ai.counts(&game, 0);
        assert_eq!(counts.naval, 1);
        assert_eq!(counts.military - counts.naval - counts.aircraft, 0);

        let defender = Item::Unit {
            unit: crate::name!("warrior"),
        };
        assert!(
            ai.production_value(&game, 0, city, &defender, &plan, &counts) > 0.0,
            "a Galley cannot satisfy the empire's missing land-defense quota"
        );
    }

    #[test]
    fn adaptive_conquest_turn_uses_the_live_plan_for_city_production() {
        let mut game = Game::new_full(1, 20, 14, 71_006, 120, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        for unit in game.player_unit_ids(0) {
            if game.rules.units[game.units[&unit].kind].class == "military" {
                game.remove_unit(unit);
            }
        }
        let city = game.player_city_ids(0)[0];
        game.cities.get_mut(&city).unwrap().queue.clear();
        install_ai_test_district(&mut game, city, "campus");
        game.players[0].techs.insert(crate::name!("writing"));
        game.apply(
            0,
            &Action::Produce {
                city,
                item: Item::Project {
                    project: crate::name!("campus_research_grants"),
                },
            },
        )
        .unwrap();
        let mut ai = AdvancedAi::new();
        ai.base.book_pos = 4;
        ai.plan = Some(StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 1,
            assessed_turn: game.turn,
            rush: false,
        });

        ai.take_turn(&mut game, 0);

        assert!(matches!(
            game.cities[&city].queue.first(),
            Some(Item::Unit { unit }) if game.rules.units[unit].class == "military"
        ));
    }

    #[test]
    fn adaptive_production_reserves_a_real_siege_support_capability() {
        let mut game = Game::new_full(2, 24, 16, 71_007, 160, 0, false);
        for pid in 0..2 {
            game.current = pid;
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.apply(pid, &Action::FoundCity { unit: settler })
                .unwrap();
        }
        game.current = 0;
        let home = game.player_city_ids(0)[0];
        let target = game.player_city_ids(1)[0];
        game.cities.get_mut(&home).unwrap().queue.clear();
        game.players[0].techs.insert(crate::name!("flight"));
        let position = game.cities[&home].pos;
        game.spawn_test_unit("catapult", 0, position);
        game.spawn_test_unit("warrior", 0, position);
        game.spawn_test_unit("warrior", 0, position);
        game.at_war.insert((0, 1));
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: Some(target),
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };
        let mut ai = AdvancedAi::new();
        ai.base.book_pos = 4;

        ai.advanced_support_production(&mut game, 0, &plan);

        assert!(matches!(
            game.cities[&home].queue.first(),
            Some(Item::Unit { unit }) if unit == "observation_balloon"
        ));
    }

    #[test]
    fn support_search_respects_ram_and_tower_wall_eras() {
        let mut game = Game::new_full(2, 24, 16, 71_008, 160, 0, false);
        for pid in 0..2 {
            game.current = pid;
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.apply(pid, &Action::FoundCity { unit: settler })
                .unwrap();
        }
        game.current = 0;
        let home = game.player_city_ids(0)[0];
        let target = game.player_city_ids(1)[0];
        let position = game.cities[&home].pos;
        game.spawn_test_unit("warrior", 0, position);
        game.spawn_test_unit("warrior", 0, position);
        game.spawn_test_unit("warrior", 0, position);
        game.cities.get_mut(&target).unwrap().buildings =
            vec![crate::name!("walls"), crate::name!("medieval_walls")];
        game.cities.get_mut(&target).unwrap().wall_hp = 200;
        game.at_war.insert((0, 1));
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: Some(target),
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };
        let ai = AdvancedAi::targeting(VictoryTarget::Domination);
        let counts = ai.counts(&game, 0);

        assert!(ai.support_unit_value(&game, 0, home, "battering_ram", &plan, &counts) < -9_000.0);
        assert!(ai.support_unit_value(&game, 0, home, "siege_tower", &plan, &counts) > 0.0);
    }

    #[test]
    fn support_search_builds_air_defense_only_for_a_real_hostile_air_threat() {
        let mut game = Game::new_full(2, 24, 16, 71_011, 160, 0, false);
        for pid in 0..2 {
            game.current = pid;
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.apply(pid, &Action::FoundCity { unit: settler })
                .unwrap();
        }
        game.current = 0;
        game.players[0]
            .techs
            .insert(crate::name!("advanced_ballistics"));
        game.at_war.insert((0, 1));
        let city = game.player_city_ids(0)[0];
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: game.player_city_ids(1).first().copied(),
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        ai.base.book_pos = 4;
        let item = Item::Unit {
            unit: crate::name!("anti_air_gun"),
        };
        let counts = ai.counts(&game, 0);
        assert_eq!(counts.air_defense, 0);
        assert!(ai.production_value(&game, 0, city, &item, &plan, &counts) < -9_000.0);

        let hostile_base = game.cities[&game.player_city_ids(1)[0]].pos;
        game.spawn_test_unit("bomber", 1, hostile_base);
        let counts = ai.counts(&game, 0);
        assert!(ai.production_value(&game, 0, city, &item, &plan, &counts) > 0.0);
        ai.advanced_support_production(&mut game, 0, &plan);
        assert!(matches!(
            game.cities[&city].queue.first(),
            Some(Item::Unit { unit }) if unit == "anti_air_gun"
        ));

        game.cities.get_mut(&city).unwrap().queue.clear();
        let city_pos = game.cities[&city].pos;
        game.spawn_test_unit("anti_air_gun", 0, city_pos);
        let counts = ai.counts(&game, 0);
        assert_eq!(counts.air_defense, 1);
        assert!(ai.production_value(&game, 0, city, &item, &plan, &counts) < -9_000.0);
    }

    #[test]
    fn force_readiness_excludes_aircraft_from_ground_armies() {
        let mut game = Game::new_full(2, 24, 16, 71_004, 120, 0, false);
        game.at_war.insert((0, 1));
        let staging = game
            .map
            .tiles
            .iter()
            .find(|(position, tile)| {
                game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && game.units_at(**position).is_empty()
            })
            .map(|(position, _)| *position)
            .unwrap();
        let warrior = game.spawn_test_unit("warrior", 0, staging);
        let bomber = game.spawn_test_unit("bomber", 0, staging);
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);

        ai.rebuild_force_groups(&game, 0, &plan);

        let army = ai
            .force_groups
            .iter()
            .find(|group| group.units.contains(&warrior))
            .expect("the ground unit forms an army order");
        assert!(!army.units.contains(&bomber));
        assert!(ai.force_groups.iter().all(|group| {
            group.units.iter().all(|unit| {
                game.rules.units[game.units[unit].kind]
                    .domain
                    .as_deref()
                    != Some("air")
            })
        }));
    }

    #[test]
    fn local_superiority_prices_the_objective_city_defense() {
        let mut game = Game::new_full(2, 24, 16, 71_006, 120, 0, false);
        game.current = 1;
        let settler = game
            .player_unit_ids(1)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(1, &Action::FoundCity { unit: settler }).unwrap();
        let target_city = game.player_city_ids(1)[0];
        for unit in game.player_unit_ids(1) {
            game.remove_unit(unit);
        }
        game.players[1]
            .counters
            .insert("strongest_unit_built".to_string(), 80);
        let city_pos = {
            let city = game.cities.get_mut(&target_city).unwrap();
            city.buildings.push(crate::name!("walls"));
            city.wall_hp = 100;
            city.pos
        };
        let staging =
            game.nbrs(city_pos)
                .into_iter()
                .find(|position| {
                    game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    }) && game.units_at(*position).is_empty()
                })
                .unwrap();
        let warrior = game.spawn_test_unit("warrior", 0, staging);
        game.at_war.insert((0, 1));
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);

        let ratio = ai.local_strength_ratio(&game, &[warrior], &[1], game.cities[&target_city].pos);

        assert!(
            ratio < 0.72,
            "one Warrior must not claim superiority over an intact defended city: {ratio}"
        );
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: Some(target_city),
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };
        ai.rebuild_force_groups(&game, 0, &plan);
        let order = ai
            .force_groups
            .iter()
            .find(|group| group.units.contains(&warrior))
            .unwrap();
        assert_eq!(order.posture, ForcePosture::Hold);
        assert_eq!(
            match order.posture {
                ForcePosture::Muster | ForcePosture::Hold | ForcePosture::Recover => order.anchor,
                ForcePosture::Engage => order.focus_target.unwrap_or(order.objective),
                ForcePosture::Advance => order.objective,
            },
            order.anchor,
            "an inferior force must hold its formation rather than target the city"
        );

        game.cities.get_mut(&target_city).unwrap().wall_hp = 0;
        game.cities.get_mut(&target_city).unwrap().hp = 1;
        ai.rebuild_force_groups(&game, 0, &plan);
        assert_eq!(
            ai.force_groups
                .iter()
                .find(|group| group.units.contains(&warrior))
                .unwrap()
                .posture,
            ForcePosture::Engage,
            "a forcing city capture must override the otherwise inferior local ratio"
        );
    }

    /// Build a game with player 0's capital founded, every other unit of its
    /// cleared away, and a declared war so force orders are built at all.
    fn empire_with_a_capital(seed: u64) -> (Game, u32, Pos) {
        let mut game = Game::new_full(2, 74, 46, seed, 200, 0, false);
        game.current = 0;
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        let home = game.cities[&city].pos;
        for unit in game.player_unit_ids(0) {
            game.remove_unit(unit);
        }
        game.at_war.insert((0, 1));
        (game, city, home)
    }

    /// A tile at exactly `distance` from `home`, chosen deterministically so
    /// the assertion is about the radius and not about map iteration order.
    fn anchor_at(game: &Game, home: Pos, distance: i32) -> Pos {
        let mut candidates: Vec<Pos> = game
            .map
            .tiles
            .keys()
            .copied()
            .filter(|position| game.wdist(*position, home) == distance)
            .collect();
        candidates.sort_unstable();
        *candidates.first().unwrap_or_else(|| {
            panic!("no tile sits exactly {distance} hexes from the capital")
        })
    }

    /// The relief radius must scale with the force's pace: cavalry can answer
    /// a call a siege train cannot, and a mixed column marches at the speed of
    /// its slowest member.
    #[test]
    fn relief_reach_scales_with_the_slowest_unit_in_the_group() {
        let (mut game, city, home) = empire_with_a_capital(71_100);
        let staging = anchor_at(&game, home, 2);
        let warrior = game.spawn_test_unit("warrior", 0, staging);
        let horseman = game.spawn_test_unit("horseman", 0, anchor_at(&game, home, 3));

        // Warrior: 2 moves -> 6 + 3*2 = 12 hexes of reach.
        assert!(AdvancedAi::can_relieve(
            &game,
            &[warrior],
            anchor_at(&game, home, 12),
            city
        ));
        assert!(!AdvancedAi::can_relieve(
            &game,
            &[warrior],
            anchor_at(&game, home, 13),
            city
        ));

        // Horseman: 4 moves -> 6 + 3*4 = 18.
        assert!(AdvancedAi::can_relieve(
            &game,
            &[horseman],
            anchor_at(&game, home, 18),
            city
        ));
        assert!(!AdvancedAi::can_relieve(
            &game,
            &[horseman],
            anchor_at(&game, home, 19),
            city
        ));

        // Together they march at the warrior's pace, not the horseman's.
        assert!(!AdvancedAi::can_relieve(
            &game,
            &[horseman, warrior],
            anchor_at(&game, home, 13),
            city
        ));

        // A city that no longer exists cannot be relieved.
        assert!(!AdvancedAi::can_relieve(&game, &[warrior], home, u32::MAX));
    }

    /// The behaviour this exists for: a locally superior force far from the
    /// emergency keeps prosecuting its campaign, while one close enough to
    /// matter halts. Before this, one threatened city anywhere held every
    /// force group in the empire.
    #[test]
    fn a_force_that_cannot_reach_the_threat_no_longer_halts_for_it() {
        let posture_at = |distance: i32| {
            let (mut game, city, home) = empire_with_a_capital(71_101);
            let warrior = game.spawn_test_unit("warrior", 0, anchor_at(&game, home, distance));
            let plan = StrategicPlan {
                strategy: GrandStrategy::Conquest,
                target_player: Some(1),
                target_city: None,
                threatened_city: Some(city),
                desired_cities: 3,
                assessed_turn: game.turn,
                rush: false,
            };
            let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
            ai.scoped_relief_hold = true;
            ai.rebuild_force_groups(&game, 0, &plan);
            let group = ai
                .force_groups
                .iter()
                .find(|group| group.units.contains(&warrior))
                .expect("the lone warrior must form a force group");
            assert!(
                group.local_strength_ratio >= LOCAL_SUPERIORITY_FLOOR,
                "the setup must leave the group strong enough to advance: {}",
                group.local_strength_ratio
            );
            group.posture
        };

        assert_eq!(
            posture_at(8),
            ForcePosture::Hold,
            "a force inside the relief radius must still answer the call"
        );
        assert_ne!(
            posture_at(20),
            ForcePosture::Hold,
            "a force twenty hexes away cannot defend the capital by standing still"
        );
    }

    /// The shipped agent is unchanged. The scoped hold measured no stronger
    /// than the global one, so it stays behind its flag, and a paired
    /// evaluation is only meaningful if the control really is the incumbent.
    #[test]
    fn the_default_agent_still_holds_for_a_threat_it_cannot_reach() {
        let (mut game, city, home) = empire_with_a_capital(71_102);
        let warrior = game.spawn_test_unit("warrior", 0, anchor_at(&game, home, 20));
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: None,
            threatened_city: Some(city),
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        assert!(!ai.scoped_relief_hold, "the shipped default must be unchanged");
        ai.rebuild_force_groups(&game, 0, &plan);
        assert_eq!(
            ai.force_groups
                .iter()
                .find(|group| group.units.contains(&warrior))
                .unwrap()
                .posture,
            ForcePosture::Hold,
        );
    }

    #[test]
    fn bomber_exact_result_search_prefers_a_kill_over_static_strength() {
        let mut game = Game::new_full(2, 24, 16, 71_005, 120, 0, false);
        game.at_war.insert((0, 1));
        for unit in game.player_unit_ids(0) {
            game.remove_unit(unit);
        }
        for unit in game.player_unit_ids(1) {
            game.remove_unit(unit);
        }
        let base = game
            .map
            .tiles
            .iter()
            .find(|(position, tile)| {
                game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && game.city_at(**position).is_none()
            })
            .map(|(position, _)| *position)
            .unwrap();
        let mut targets: Vec<Pos> = game
            .wdisk(base, game.rules.units["bomber"].range)
            .into_iter()
            .filter(|position| {
                *position != base
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
                    && game.city_at(*position).is_none()
            })
            .take(2)
            .collect();
        assert_eq!(targets.len(), 2);
        targets.sort_unstable();
        let bomber = game.spawn_test_unit("bomber", 0, base);
        game.spawn_test_unit("modern_armor", 1, targets[0]);
        let warrior = game.spawn_test_unit("warrior", 1, targets[1]);
        game.units.get_mut(&warrior).unwrap().hp = 1;
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };
        let ai = AdvancedAi::targeting(VictoryTarget::Domination);

        assert_eq!(
            ai.advanced_air_action(&game, 0, bomber, &plan),
            Some(Action::AirStrike {
                unit: bomber,
                target: targets[1],
            })
        );
    }

    #[test]
    fn bomber_planners_choose_high_value_air_pillage_over_low_value_strikes() {
        let mut game = Game::new_full(2, 24, 16, 71_006, 120, 0, false);
        game.at_war.insert((0, 1));
        for unit in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(unit);
        }
        let enemy_center = game
            .map
            .tiles
            .iter()
            .find(|(_, tile)| game.rules.is_passable(tile) && !game.rules.is_water(tile))
            .map(|(position, _)| *position)
            .unwrap();
        let enemy_city = game.found_city_for(1, enemy_center, None);
        let target = game
            .nbrs(enemy_center)
            .into_iter()
            .find(|position| {
                game.map
                    .get(*position)
                    .is_some_and(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile))
            })
            .unwrap();
        {
            let tile = game.map.tiles.get_mut(&target).unwrap();
            tile.owner_city = Some(enemy_city);
            tile.improvement = Some(crate::name!("airstrip"));
            tile.pillaged = false;
        }
        let base = game
            .wdisk(target, game.rules.units["bomber"].range)
            .into_iter()
            .find(|position| {
                *position != target
                    && *position != enemy_center
                    && game.wdist(*position, target) >= 3
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .unwrap();
        game.found_city_for(0, base, None);
        let bomber = game.spawn_test_unit("bomber", 0, base);
        let expected = Action::AirPillage {
            unit: bomber,
            target,
        };
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: Some(enemy_city),
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };

        assert_eq!(
            BasicAi::new().doctrine_action(&game, 0, bomber),
            Some(expected.clone())
        );
        assert_eq!(
            AdvancedAi::targeting(VictoryTarget::Domination)
                .advanced_air_action(&game, 0, bomber, &plan),
            Some(expected)
        );
    }

    #[test]
    fn jet_planners_priority_target_escorted_air_defenses() {
        let mut game = Game::new_full(2, 24, 16, 71_007, 120, 0, false);
        game.at_war.insert((0, 1));
        for unit in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(unit);
        }
        let base = game
            .map
            .tiles
            .iter()
            .find(|(_, tile)| game.rules.is_passable(tile) && !game.rules.is_water(tile))
            .map(|(position, _)| *position)
            .unwrap();
        let target = game
            .wdisk(base, game.rules.units["jet_bomber"].range)
            .into_iter()
            .find(|position| {
                *position != base
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .unwrap();
        game.found_city_for(0, base, None);
        game.spawn_test_unit("modern_armor", 1, target);
        game.spawn_test_unit("mobile_sam", 1, target);
        let jet = game.spawn_test_unit("jet_bomber", 0, base);
        let expected = Action::PriorityTarget { unit: jet, target };
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };

        assert_eq!(
            BasicAi::new().doctrine_action(&game, 0, jet),
            Some(expected.clone())
        );
        assert_eq!(
            AdvancedAi::targeting(VictoryTarget::Domination)
                .advanced_air_action(&game, 0, jet, &plan),
            Some(expected)
        );
    }

    #[test]
    fn exact_ground_search_prefers_the_high_value_kill_over_a_static_tie() {
        let mut game = Game::new_full(2, 24, 16, 71_009, 120, 0, false);
        game.at_war.insert((0, 1));
        let rival_origin = game
            .player_unit_ids(1)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .map(|unit| game.units[&unit].pos)
            .unwrap();
        game.found_city_for(1, rival_origin, None);
        for unit in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(unit);
        }
        let (base, mut targets) = game
            .map
            .tiles
            .iter()
            .filter(|(position, tile)| {
                game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && game.city_at(**position).is_none()
                    && game
                        .cities
                        .values()
                        .all(|city| game.wdist(**position, city.pos) > 5)
            })
            .find_map(|(base, _)| {
                let targets: Vec<Pos> = game
                    .nbrs(*base)
                    .into_iter()
                    .filter(|position| {
                        game.map.get(*position).is_some_and(|tile| {
                            game.rules.is_passable(tile) && !game.rules.is_water(tile)
                        }) && game.city_at(*position).is_none()
                    })
                    .collect();
                (targets.len() >= 2).then_some((*base, targets))
            })
            .expect("test map has an isolated two-target engagement");
        targets.sort_unstable();
        let robot = game.spawn_test_unit("giant_death_robot", 0, base);
        let warrior = game.spawn_test_unit("warrior", 1, targets[0]);
        let armor = game.spawn_test_unit("modern_armor", 1, targets[1]);
        game.units.get_mut(&warrior).unwrap().hp = 1;
        game.units.get_mut(&armor).unwrap().hp = 1;
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);

        assert_eq!(
            ai.base.exchange_score(&game, robot, targets[0], true),
            ai.base.exchange_score(&game, robot, targets[1], true),
            "the static evaluator intentionally sees two one-hit kills"
        );
        let warrior_value = ai.tactical_attack_value(
            &game,
            0,
            robot,
            &Action::Ranged {
                unit: robot,
                target: targets[0],
            },
            &plan,
        );
        let armor_value = ai.tactical_attack_value(
            &game,
            0,
            robot,
            &Action::Ranged {
                unit: robot,
                target: targets[1],
            },
            &plan,
        );
        assert!(armor_value > warrior_value + 100.0);
        assert!(ai.advanced_military_step(&mut game, 0, robot, &plan));
        assert!(!game.units.contains_key(&armor));
        assert!(game.units.contains_key(&warrior));
        assert!(matches!(
            game.log.last(),
            Some((0, Action::Attack { target, .. } | Action::Ranged { target, .. }))
                if *target == targets[1]
        ));
    }

    #[test]
    fn army_declines_a_captured_settler_when_no_city_site_remains() {
        let mut game = Game::new_full(2, 20, 14, 71_019, 120, 0, false);
        for pid in 0..2 {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.found_city_for(pid, game.units[&settler].pos, None);
            game.remove_unit(settler);
        }
        for unit in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(unit);
        }
        let home = game.cities[&game.player_city_ids(0)[0]].pos;
        let origin = game.nbrs(home)[0];
        let target = game
            .nbrs(origin)
            .into_iter()
            .find(|position| *position != home && game.wdist(home, *position) <= 2)
            .unwrap();
        let city_centers: BTreeSet<Pos> =
            game.cities.values().map(|city| city.pos).collect();
        for (position, tile) in &mut game.map.tiles {
            if ![home, origin, target].contains(&position)
                && !city_centers.contains(&position)
            {
                tile.terrain = crate::name!("ocean");
                tile.feature = None;
            }
        }
        for position in [origin, target] {
            let tile = game.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("plains");
            tile.feature = None;
            tile.hills = false;
        }
        game.at_war.insert((0, 1));
        let warrior = game.spawn_test_unit("warrior", 0, origin);
        let settler = game.spawn_test_unit("settler", 1, target);
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: game.player_city_ids(1).first().copied(),
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: game.turn,
            rush: false,
        };
        let mut ai = AdvancedAi::new();
        assert!(!ai.base.has_practical_settle_site(&game, 0));
        let mut direct_capture = game.clone();
        let capture_result = direct_capture.apply(0, &Action::Move { unit: warrior, to: target });
        assert!(capture_result.is_ok(), "staged capture was not legal: {capture_result:?}");
        assert_eq!(direct_capture.units[&settler].owner, 0);

        let _ = ai.advanced_military_step(&mut game, 0, warrior, &plan);

        assert_eq!(
            game.units.get(&settler).map(|unit| unit.owner),
            Some(1),
            "capturing the civilian would create a settler with no legal city site"
        );

        game.remove_unit(warrior);
        let scout = game.spawn_test_unit("scout", 0, origin);
        let _ = ai.advanced_military_step(&mut game, 0, scout, &plan);
        assert_eq!(
            game.units.get(&settler).map(|unit| unit.owner),
            Some(1),
            "a recon fallback must not bypass the unwanted-settler guard"
        );
    }

    #[test]
    fn army_takes_a_free_settler_after_reaching_its_planned_city_count() {
        let mut game = Game::new_full(2, 20, 14, 71_020, 120, 0, false);
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
        let settler = game.spawn_test_unit("settler", 1, target);
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: game.player_city_ids(1).first().copied(),
            threatened_city: None,
            desired_cities: 1,
            assessed_turn: game.turn,
            rush: false,
        };
        let mut ai = AdvancedAi::new();
        assert!(ai.base.has_practical_settle_site(&game, 0));

        assert!(ai.advanced_military_step(&mut game, 0, warrior, &plan));
        assert_eq!(game.units[&settler].owner, 0);
        assert!(matches!(
            game.log.last(),
            Some((0, Action::Move { unit, to })) if *unit == warrior && *to == target
        ));
    }

    #[test]
    fn exact_hybrid_search_uses_melee_to_finish_a_city() {
        let mut game = Game::new_full(2, 24, 16, 71_010, 120, 0, false);
        let rival_origin = game
            .player_unit_ids(1)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .map(|unit| game.units[&unit].pos)
            .unwrap();
        let city = game.found_city_for(1, rival_origin, None);
        for unit in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(unit);
        }
        let target = game.cities[&city].pos;
        let staging =
            game.nbrs(target)
                .into_iter()
                .find(|position| {
                    game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    }) && game.units_at(*position).is_empty()
                })
                .unwrap();
        game.cities.get_mut(&city).unwrap().hp = 0;
        game.cities.get_mut(&city).unwrap().wall_hp = 0;
        let robot = game.spawn_test_unit("giant_death_robot", 0, staging);
        game.at_war.insert((0, 1));
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: Some(city),
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);

        assert!(ai.advanced_military_step(&mut game, 0, robot, &plan));
        assert_eq!(game.cities[&city].owner, 0);
        assert!(matches!(
            game.log.last(),
            Some((0, Action::Attack { unit, target: action_target }))
                if *unit == robot && *action_target == target
        ));
    }

    #[test]
    fn conquest_waits_to_recapture_an_unholdable_original_capital() {
        let mut game = Game::new_full(3, 30, 18, 71_020, 120, 0, false);
        for pid in 0..3 {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            let position = game.units[&settler].pos;
            game.found_city_for(pid, position, None);
        }
        for unit in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(unit);
        }

        let target_city = game.player_city_ids(1)[0];
        let target = game.cities[&target_city].pos;
        let pressure_site = game
            .wdisk(target, 6)
            .into_iter()
            .find(|position| {
                (4..=6).contains(&game.wdist(*position, target))
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
                    && game.city_at(*position).is_none()
                    && game
                        .cities
                        .values()
                        .all(|city| game.wdist(city.pos, *position) >= 4)
            })
            .expect("test map has a nearby pressure city site");
        let pressure_city =
            game.found_city_for(1, pressure_site, Some("Pressure".to_string()));
        let remote_site = game
            .map
            .tiles
            .iter()
            .find_map(|(position, tile)| {
                (game.wdist(*position, target) > 9
                    && game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && game.city_at(*position).is_none()
                    && game
                        .cities
                        .values()
                        .all(|city| game.wdist(city.pos, *position) >= 4))
                .then_some(*position)
            })
            .expect("test map has a remote surviving city site");
        let remote_city = game.found_city_for(1, remote_site, Some("Reserve".to_string()));
        for city in game.cities.values_mut() {
            city.pop = 1;
        }
        game.cities.get_mut(&pressure_city).unwrap().pop = 40;
        game.cities.get_mut(&remote_city).unwrap().pop = 1;
        {
            let city = game.cities.get_mut(&target_city).unwrap();
            city.hp = 0;
            city.wall_hp = 0;
            city.loyalty = 100.0;
        }
        let staging = game
            .nbrs(target)
            .into_iter()
            .find(|position| {
                game.map.get(*position).is_some_and(|tile| {
                    game.rules.is_passable(tile) && !game.rules.is_water(tile)
                }) && game.units_at(*position).is_empty()
            })
            .expect("the capital has an open melee approach");
        let attacker = game.spawn_test_unit("giant_death_robot", 0, staging);
        game.at_war.insert((0, 1));
        game.current = 0;
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: Some(target_city),
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: game.turn,
            rush: false,
        };
        let capture = Action::Attack {
            unit: attacker,
            target,
        };
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);

        assert!(AdvancedAi::should_defer_city_capture(&game, 0, target_city));
        assert!(
            ai.campaign_city_value(&game, 0, &game.cities[&target_city], GrandStrategy::Conquest)
                > ai.campaign_city_value(
                    &game,
                    0,
                    &game.cities[&pressure_city],
                    GrandStrategy::Conquest,
                ),
            "the surrounding pressure city should become the campaign objective first"
        );
        assert_eq!(
            ai.tactical_attack_value(&game, 0, attacker, &capture, &plan),
            f64::NEG_INFINITY
        );
        let mut blocked = game.clone();
        let _ = ai.advanced_military_step(&mut blocked, 0, attacker, &plan);
        assert_eq!(
            blocked.cities[&target_city].owner, 1,
            "the army must not restart a forced capital recapture loop"
        );

        let mut supported = game;
        supported.cities.get_mut(&pressure_city).unwrap().owner = 0;
        assert_eq!(supported.player_city_ids(1).len(), 2);
        assert!(!AdvancedAi::should_defer_city_capture(
            &supported,
            0,
            target_city
        ));
        assert!(
            ai.tactical_attack_value(&supported, 0, attacker, &capture, &plan)
                .is_finite()
        );
        assert!(ai.advanced_military_step(&mut supported, 0, attacker, &plan));
        assert_eq!(supported.cities[&target_city].owner, 0);
    }

    #[test]
    fn culture_production_trains_one_archaeologist_for_available_artifact_slots() {
        let mut game = Game::new(2, 24, 16, 7_100, 1_500, 0);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        game.cities
            .get_mut(&city)
            .unwrap()
            .buildings
            .push(crate::name!("archaeological_museum"));
        game.players[0].civics.insert(crate::name!("natural_history"));
        let site = game.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != game.cities[&city].pos)
            .unwrap();
        let tile = game.map.tiles.get_mut(&site).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = Some(crate::name!("antiquity_site"));
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        let archaeologist_item = Item::Unit {
            unit: crate::name!("archaeologist"),
        };
        assert!(game.can_produce(0, city, &archaeologist_item));

        let plan = StrategicPlan {
            strategy: GrandStrategy::Culture,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };
        let ai = AdvancedAi::targeting(VictoryTarget::Culture);
        ai.advanced_production(&mut game, 0, &plan);
        assert!(
            matches!(
                game.cities[&city].queue.first(),
                Some(Item::Unit { unit }) if unit == "archaeologist"
            ),
            "queued {:?}",
            game.cities[&city].queue.first()
        );

        game.cities.get_mut(&city).unwrap().queue.clear();
        game.spawn_test_unit("archaeologist", 0, game.cities[&city].pos);
        ai.advanced_production(&mut game, 0, &plan);
        assert!(!matches!(
            game.cities[&city].queue.first(),
            Some(Item::Unit { unit }) if unit == "archaeologist"
        ));
    }

    #[test]
    fn project_search_maintains_aged_reactors_and_avoids_dirty_conversion_churn() {
        let mut game = Game::new(2, 24, 16, 7_101, 200, 0);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        install_ai_test_district(&mut game, city, "industrial_zone");
        game.cities.get_mut(&city).unwrap().buildings =
            vec![crate::name!("factory"), crate::name!("nuclear_power_plant")];
        let plan = StrategicPlan {
            strategy: GrandStrategy::Expansion,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };
        let counts = EmpireCounts::default();
        let ai = AdvancedAi::new();
        let recommission = Item::Project {
            project: crate::name!("recommission_reactor"),
        };

        assert!(ai.production_value(&game, 0, city, &recommission, &plan, &counts) < -9_000.0);
        game.cities.get_mut(&city).unwrap().reactor_age = 30;
        assert!(ai.production_value(&game, 0, city, &recommission, &plan, &counts) > 0.0);

        game.cities.get_mut(&city).unwrap().buildings =
            vec![crate::name!("factory"), crate::name!("oil_power_plant")];
        game.climate_phase = 6;
        game.players[0]
            .strategic_resources
            .insert(crate::name!("coal"), 10.0);
        game.players[0]
            .strategic_resources
            .insert(crate::name!("uranium"), 10.0);
        let coal = Item::Project {
            project: crate::name!("convert_reactor_to_coal"),
        };
        let nuclear = Item::Project {
            project: crate::name!("convert_reactor_to_uranium"),
        };
        assert!(
            ai.production_value(&game, 0, city, &nuclear, &plan, &counts)
                > ai.production_value(&game, 0, city, &coal, &plan, &counts)
        );
    }

    #[test]
    fn district_project_search_extends_only_concrete_great_person_races() {
        let mut game = Game::new(2, 24, 16, 7_103, 200, 0);
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
        game.players[0].techs.insert(crate::name!("writing"));

        let project = Item::Project {
            project: crate::name!("campus_research_grants"),
        };
        let library = Item::Building {
            building: crate::name!("library"),
        };
        assert!(game.can_produce(0, city, &project));
        assert!(game.can_produce(0, city, &library));
        let plan = StrategicPlan {
            strategy: GrandStrategy::Science,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };
        let ai = AdvancedAi::targeting(VictoryTarget::Science);
        let counts = ai.counts(&game, 0);
        let undeveloped = ai.production_value(&game, 0, city, &project, &plan, &counts);
        let first_building = ai.production_value(&game, 0, city, &library, &plan, &counts);
        assert!(
            first_building > undeveloped,
            "the first Campus building must precede a quiet project: {first_building} <= {undeveloped}"
        );

        game.cities
            .get_mut(&city)
            .unwrap()
            .buildings
            .push(crate::name!("library"));
        let far = ai.production_value(&game, 0, city, &project, &plan, &counts);
        let award =
            game.project_completion_gpp_awards(0, city, "campus_research_grants")["scientist"];
        let cost = game.gp_cost(0, "scientist");
        game.players[0]
            .gpp
            .insert("scientist".to_string(), cost - award);
        game.players[1]
            .gpp
            .insert("scientist".to_string(), cost - award * 0.5);
        let forcing = ai.production_value(&game, 0, city, &project, &plan, &counts);
        assert!(
            forcing > far + 100.0,
            "a project that claims and overtakes in the live race must receive an extension: {forcing} <= {far}"
        );

        game.active_congress_effects
            .push(crate::game::CongressEffect {
                resolution: "patronage".to_string(),
                outcome: "B".to_string(),
                target: "scientist".to_string(),
                expires: game.turn + 30,
            });
        let disabled = ai.production_value(&game, 0, city, &project, &plan, &counts);
        assert!(
            disabled < far,
            "a Congress-disabled class must not retain Great Person race value: {disabled} >= {far}"
        );
    }

    #[test]
    fn bread_and_circuses_value_tracks_real_loyalty_need() {
        let mut game = Game::new(1, 20, 14, 7_105, 160, 0);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        let district = game.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != game.cities[&city].pos)
            .unwrap();
        game.map.tiles.get_mut(&district).unwrap().district =
            Some(crate::name!("entertainment_complex"));
        game.cities
            .get_mut(&city)
            .unwrap()
            .districts
            .insert(crate::name!("entertainment_complex"), district);
        game.cities
            .get_mut(&city)
            .unwrap()
            .buildings
            .push(crate::name!("arena"));

        let plan = StrategicPlan {
            strategy: GrandStrategy::Expansion,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };
        let ai = AdvancedAi::new();
        let safe = ai.district_project_value(&game, 0, city, "bread_and_circuses", &plan);
        game.cities.get_mut(&city).unwrap().loyalty = 50.0;
        let pressured = ai.district_project_value(&game, 0, city, "bread_and_circuses", &plan);
        assert!(
            pressured > safe + 700.0,
            "loyalty recovery must transform Bread and Circuses from quiet to forcing"
        );
    }

    #[test]
    fn great_person_patronage_buys_close_strategy_races_without_spending_the_reserve() {
        let mut game = Game::new(2, 24, 16, 7_102, 200, 0);
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
        game.players[0]
            .gpp
            .insert("scientist".to_string(), cost - 5.0);
        let ai = AdvancedAi::targeting(VictoryTarget::Science);

        game.players[0].gold = 250.0;
        ai.advanced_great_people(&mut game, 0, GrandStrategy::Science);
        assert_eq!(
            game.players[0]
                .gp_claimed
                .get("scientist")
                .copied()
                .unwrap_or(0),
            0
        );

        game.players[0].gold = 500.0;
        ai.advanced_great_people(&mut game, 0, GrandStrategy::Science);
        assert_eq!(game.players[0].gp_claimed["scientist"], 1);
        assert_eq!(game.players[0].gold, 225.0);
    }

    #[test]
    fn culture_patronage_waits_for_compatible_great_work_slots() {
        let mut game = Game::new(1, 20, 14, 7_104, 200, 0);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        game.players[0]
            .counters
            .insert("great_work:writing".to_string(), 1);
        let cost = game.current_great_person("writer").unwrap().1.cost;
        game.players[0].gpp.insert("writer".to_string(), cost - 5.0);
        game.players[0].gold = 500.0;
        let ai = AdvancedAi::targeting(VictoryTarget::Culture);

        ai.advanced_great_people(&mut game, 0, GrandStrategy::Culture);
        assert_eq!(
            game.players[0]
                .gp_claimed
                .get("writer")
                .copied()
                .unwrap_or(0),
            0,
            "the occupied Palace slot cannot host another work"
        );

        game.cities
            .get_mut(&city)
            .unwrap()
            .buildings
            .push(crate::name!("amphitheater"));
        install_ai_test_district(&mut game, city, "theater_square");
        ai.advanced_great_people(&mut game, 0, GrandStrategy::Culture);
        assert_eq!(game.players[0].gp_claimed["writer"], 1);
    }

    #[test]
    fn faith_spending_buys_the_victory_aligned_worship_building() {
        let mut game = Game::new(2, 24, 16, 7_103, 200, 0);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        let holy_site = game.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != game.cities[&city].pos)
            .unwrap();
        game.map.tiles.get_mut(&holy_site).unwrap().district = Some(crate::name!("holy_site"));
        game.cities
            .get_mut(&city)
            .unwrap()
            .districts
            .insert(crate::name!("holy_site"), holy_site);
        game.cities.get_mut(&city).unwrap().buildings =
            vec![crate::name!("shrine"), crate::name!("temple")];
        game.players[0].religion = Some("Scholastic Faith".to_string());
        game.players[0].religion_beliefs = vec![
            "work_ethic".to_string(),
            "tithe".to_string(),
            "wat".to_string(),
        ];
        game.cities
            .get_mut(&city)
            .unwrap()
            .pressure
            .insert("Scholastic Faith".to_string(), 1_000.0);
        game.players[0].faith = 1_000.0;

        AdvancedAi::targeting(VictoryTarget::Science).faith_building_spending(
            &mut game,
            0,
            GrandStrategy::Science,
        );
        assert!(game.cities[&city].buildings.contains(&crate::name!("wat")));
        assert!(game.players[0].faith < 1_000.0);
    }

    #[test]
    fn religious_spending_uses_own_faith_inquisitors_without_funding_a_rival() {
        let mut game = Game::new(2, 24, 16, 7_104, 200, 0);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        install_ai_test_district(&mut game, city, "holy_site");
        game.cities.get_mut(&city).unwrap().buildings =
            vec![crate::name!("shrine"), crate::name!("temple")];
        game.players[0].civics.insert(crate::name!("theology"));
        game.players[0].religion = Some("Our Faith".to_string());
        game.players[0]
            .counters
            .insert("inquisition".to_string(), 1);
        game.players[0].faith = 1_000.0;
        game.cities.get_mut(&city).unwrap().pressure.extend([
            ("Our Faith".to_string(), 1_000.0),
            ("Rival Faith".to_string(), 600.0),
        ]);

        let ai = AdvancedAi::new();
        let mut converted = game.clone();
        converted
            .cities
            .get_mut(&city)
            .unwrap()
            .pressure
            .insert("Rival Faith".to_string(), 2_000.0);
        let converted_units = converted.player_unit_ids(0).len();
        ai.religious_spending(&mut converted, 0, true);
        assert_eq!(converted.player_unit_ids(0).len(), converted_units);
        assert_eq!(converted.players[0].faith, 1_000.0);

        let before_units = game.player_unit_ids(0).len();
        ai.religious_spending(&mut game, 0, true);
        assert_eq!(game.player_unit_ids(0).len(), before_units + 1);
        let inquisitor = game
            .units
            .values()
            .find(|unit| unit.owner == 0 && unit.kind == "inquisitor")
            .unwrap();
        assert_eq!(inquisitor.religion.as_deref(), Some("Our Faith"));
        assert!(game.players[0].faith < 1_000.0);
    }

    #[test]
    fn religious_spending_stops_at_a_target_scaled_unit_ceiling() {
        let mut game = Game::new_full(2, 30, 18, 7_105, 200, 0, false);
        for pid in 0..2 {
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
        let home = game.player_city_ids(0)[0];
        let target = game.player_city_ids(1)[0];
        install_ai_test_district(&mut game, home, "holy_site");
        game.cities.get_mut(&home).unwrap().buildings =
            vec![crate::name!("shrine"), crate::name!("temple")];
        game.players[0].techs.insert(crate::name!("astrology"));
        game.players[0].civics.insert(crate::name!("theology"));
        game.players[0].religion = Some("Our Faith".to_string());
        game.players[0].faith = 10_000.0;
        game.cities
            .get_mut(&home)
            .unwrap()
            .pressure
            .insert("Our Faith".to_string(), 1_000.0);
        game.cities
            .get_mut(&target)
            .unwrap()
            .pressure
            .insert("Rival Faith".to_string(), 1_000.0);

        for kind in [
            "apostle",
            "apostle",
            "guru",
            "missionary",
            "missionary",
            "missionary",
        ] {
            let unit = game.spawn_test_unit(kind, 0, game.cities[&home].pos);
            game.units.get_mut(&unit).unwrap().religion = Some("Our Faith".to_string());
        }
        let before_units = game.player_unit_ids(0).len();
        let before_faith = game.players[0].faith;
        AdvancedAi::new().religious_spending(&mut game, 0, true);
        assert_eq!(game.player_unit_ids(0).len(), before_units);
        assert_eq!(game.players[0].faith, before_faith);
    }

    #[test]
    fn surplus_faith_keeps_a_founded_secondary_campaign_in_motion() {
        let mut game = Game::new_full(2, 30, 18, 7_115, 200, 0, false);
        for pid in 0..2 {
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
        let home = game.player_city_ids(0)[0];
        let target = game.player_city_ids(1)[0];
        install_ai_test_district(&mut game, home, "holy_site");
        game.cities.get_mut(&home).unwrap().buildings =
            vec![crate::name!("shrine"), crate::name!("temple")];
        game.players[0].techs.insert(crate::name!("astrology"));
        game.players[0].civics.insert(crate::name!("theology"));
        game.players[0].religion = Some("Our Faith".to_string());
        game.cities
            .get_mut(&home)
            .unwrap()
            .pressure
            .insert("Our Faith".to_string(), 1_000.0);
        game.cities
            .get_mut(&target)
            .unwrap()
            .pressure
            .insert("Rival Faith".to_string(), 1_000.0);

        let ai = AdvancedAi::new();
        let reserve = game.game_speed.scale(1_200.0);
        game.players[0].faith = reserve;
        assert!(!ai.religious_offensive_posture(&game, 0, GrandStrategy::Science));

        game.players[0].faith = game.game_speed.scale(2_000.0);
        assert!(ai.religious_offensive_posture(&game, 0, GrandStrategy::Science));
        for _ in 0..3 {
            ai.religious_spending_with_reserve(&mut game, 0, true, reserve);
        }
        assert_eq!(
            game.units
                .values()
                .filter(|unit| unit.owner == 0 && unit.kind == "apostle")
                .count(),
            2
        );
        assert!(game.players[0].faith + f64::EPSILON >= reserve);

        // Saturate home, so the only thing left to convert is abroad.
        // A Missionary standing on its own capital can always spread there for
        // free, and whether that beats a march depends on how much pressure
        // the generated start happens to leave in the city.
        game.cities
            .get_mut(&home)
            .unwrap()
            .pressure
            .insert("Our Faith".to_string(), 100_000.0);

        let missionary = game.spawn_test_unit("missionary", 0, game.cities[&home].pos);
        game.units.get_mut(&missionary).unwrap().religion = Some("Our Faith".to_string());
        game.players[0].faith = 0.0;
        let home_pos = game.cities[&home].pos;
        let offensive = ai.religious_offensive_posture(&game, 0, GrandStrategy::Science);
        assert!(
            offensive,
            "a charged field unit should sustain the campaign"
        );
        assert!(ai.advanced_missionary_step(&mut game, 0, missionary, offensive));
        // Leaving home is the claim: the unit is off the city tile and still
        // carries all three charges, so it marched instead of converting the
        // capital it was standing in. Straight-line distance is the wrong
        // measure of that — a hex route around a lake or a border can open
        // with a lateral step and still be the route — and which of those the
        // unit faces is decided by wherever the map put the two capitals.
        assert_ne!(
            game.units[&missionary].pos, home_pos,
            "the secondary Missionary should leave home"
        );
        assert_eq!(
            game.units[&missionary].charges, 3,
            "it should march, not spend a charge on its own capital"
        );
    }

    #[test]
    fn nonreligious_strategy_buys_defense_only_when_its_home_is_pressured() {
        let mut game = Game::new_full(2, 30, 18, 7_106, 200, 0, false);
        for pid in 0..2 {
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
        let home = game.player_city_ids(0)[0];
        let foreign = game.player_city_ids(1)[0];
        install_ai_test_district(&mut game, home, "holy_site");
        game.cities.get_mut(&home).unwrap().buildings =
            vec![crate::name!("shrine"), crate::name!("temple")];
        game.players[0].techs.insert(crate::name!("astrology"));
        game.players[0].civics.insert(crate::name!("theology"));
        game.players[0].religion = Some("Our Faith".to_string());
        game.players[0].faith = 1_000.0;
        game.cities.get_mut(&home).unwrap().pressure.extend([
            ("Our Faith".to_string(), 1_000.0),
            ("Rival Faith".to_string(), 600.0),
        ]);
        game.cities
            .get_mut(&foreign)
            .unwrap()
            .pressure
            .insert("Rival Faith".to_string(), 1_000.0);

        let ai = AdvancedAi::targeting(VictoryTarget::Science);
        let mut safe = game.clone();
        safe.cities
            .get_mut(&home)
            .unwrap()
            .pressure
            .insert("Rival Faith".to_string(), 100.0);
        let safe_units = safe.player_unit_ids(0).len();
        ai.religious_spending(&mut safe, 0, false);
        assert_eq!(safe.player_unit_ids(0).len(), safe_units);

        let before_units = game.player_unit_ids(0).len();
        ai.religious_spending(&mut game, 0, false);
        assert_eq!(game.player_unit_ids(0).len(), before_units + 1);
        assert!(game
            .units
            .values()
            .any(|unit| unit.owner == 0 && unit.kind == "missionary"));
    }

    #[test]
    fn faith_spending_uses_valletta_wall_price_and_ignores_gold_actions() {
        let mut game = Game::new_full(1, 30, 18, 7_107, 160, 1, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        let valletta = game
            .players
            .iter()
            .find(|player| player.is_minor && !player.is_barbarian)
            .unwrap()
            .id;
        game.players[valletta].civ = "Valletta".to_string();
        game.players[0].envoys = vec![(valletta, 3)];
        game.players[0].techs.insert(crate::name!("masonry"));
        game.players[0].faith = 200.0;
        game.players[0].gold = 10_000.0;

        AdvancedAi::targeting(VictoryTarget::Domination).faith_building_spending(
            &mut game,
            0,
            GrandStrategy::Conquest,
        );

        assert!(game.cities[&city].buildings.contains(&crate::name!("walls")));
        assert_eq!(game.players[0].faith, 120.0);
        assert_eq!(game.players[0].gold, 10_000.0);
    }

    #[test]
    fn strategic_gold_purchase_buys_science_tempo_but_preserves_the_reserve() {
        let mut game = Game::new_full(1, 20, 14, 7_106, 160, 0, false);
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
        game.cities
            .get_mut(&city)
            .unwrap()
            .buildings
            .extend([crate::name!("monument"), crate::name!("granary")]);
        game.players[0].techs.insert(crate::name!("writing"));
        game.spawn_test_unit("builder", 0, game.cities[&city].pos);
        let plan = StrategicPlan {
            strategy: GrandStrategy::Science,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 1,
            assessed_turn: game.turn,
            rush: false,
        };
        let ai = AdvancedAi::targeting(VictoryTarget::Science);

        game.players[0].gold = 500.0;
        assert!(!ai.advanced_gold_spending(&mut game, 0, &plan));
        assert!(!game.cities[&city]
            .buildings
            .contains(&crate::name!("library")));

        game.players[0].gold = 1_000.0;
        assert!(ai.advanced_gold_spending(&mut game, 0, &plan));
        assert!(game.cities[&city]
            .buildings
            .contains(&crate::name!("library")));
        assert!(game.players[0].gold >= 300.0);
    }

    #[test]
    fn strategic_gold_purchase_annexes_a_luxury_and_preserves_the_reserve() {
        let mut game = Game::new_full(1, 20, 14, 7_106_002, 160, 0, false);
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
        game.players[0].gold = 550.0;
        let plan = StrategicPlan {
            strategy: GrandStrategy::Science,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 1,
            assessed_turn: game.turn,
            rush: false,
        };

        assert!(AdvancedAi::targeting(VictoryTarget::Science)
            .advanced_gold_spending(&mut game, 0, &plan));
        assert_eq!(game.map.tiles[&target].owner_city, Some(city));
        assert_eq!(game.players[0].gold, 500.0);
    }

    #[test]
    fn deep_treasury_buys_useful_infrastructure_in_multiple_cities() {
        let mut game = Game::new_full(1, 30, 18, 7_106_001, 160, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let first = game.player_city_ids(0)[0];
        let anchor = game.cities[&first].pos;
        let second = found_nearby_test_city(&mut game, 0, anchor);
        for city in [first, second] {
            install_ai_test_district(&mut game, city, "campus");
            game.cities
                .get_mut(&city)
                .unwrap()
                .buildings
                .extend([crate::name!("monument"), crate::name!("granary")]);
        }
        game.players[0].techs.insert(crate::name!("writing"));
        game.players[0].gold = 5_000.0;
        let plan = StrategicPlan {
            strategy: GrandStrategy::Science,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 2,
            assessed_turn: game.turn,
            rush: false,
        };
        let units_before = game.player_unit_ids(0).len();
        let buildings_before = game
            .cities
            .values()
            .filter(|city| city.owner == 0)
            .map(|city| city.buildings.len())
            .sum::<usize>();

        assert!(AdvancedAi::targeting(VictoryTarget::Science)
            .advanced_gold_spending(&mut game, 0, &plan));
        let units_after = game.player_unit_ids(0).len();
        let buildings_after = game
            .cities
            .values()
            .filter(|city| city.owner == 0)
            .map(|city| city.buildings.len())
            .sum::<usize>();
        assert!(
            units_after - units_before + buildings_after - buildings_before >= 2,
            "a deep treasury should fund more than one strategic acquisition"
        );
        assert!(units_after - units_before <= 2);
        assert!([first, second].into_iter().any(|city| game.cities[&city]
            .buildings
            .contains(&crate::name!("library"))));
        assert!(game.players[0].gold >= 350.0);
    }

    #[test]
    fn adaptive_turn_uses_its_live_plan_for_gold_purchases() {
        // The fixture needs a capital with somewhere to put a Campus, which is
        // a fact about the map rather than the thing under test. Take the first
        // seed that offers one instead of pinning a seed and trusting that map
        // generation never moves again.
        let (mut game, city) = (7_107..7_160u64)
            .find_map(|seed| {
                let mut game = Game::new_full(1, 20, 14, seed, 160, 0, false);
                let settler = game
                    .player_unit_ids(0)
                    .into_iter()
                    .find(|unit| game.units[unit].kind == "settler")?;
                game.apply(0, &Action::FoundCity { unit: settler }).ok()?;
                let city = *game.player_city_ids(0).first()?;
                game.turn = 10;
                game.players[0].techs.insert(crate::name!("writing"));
                game.players[0].gold = 10_000.0;
                game.players[0].governor_roster.insert(
                    "reyna".to_string(),
                    GovernorState {
                        city: Some(city),
                        assigned_turn: 0,
                        disabled_until: 0,
                        promotions: BTreeSet::from(["contractor".to_string()]),
                    },
                );
                game.legal_actions(0)
                    .iter()
                    .any(|action| {
                        matches!(
                            action,
                            Action::BuyDistrict { district, currency, .. }
                                if district == "campus" && currency == "gold"
                        )
                    })
                    .then_some((game, city))
            })
            .expect("no seed offered a lone capital with room to buy a Campus");

        let mut ai = AdvancedAi::new();
        ai.base.book_pos = 4;
        ai.plan = Some(StrategicPlan {
            strategy: GrandStrategy::Science,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 1,
            assessed_turn: game.turn,
            rush: false,
        });

        ai.take_turn(&mut game, 0);

        assert!(
            game.cities[&city].districts.contains_key(crate::name!("campus")),
            "an adaptive Science plan should convert surplus Gold into its Campus immediately"
        );
        assert!(game.players[0].gold >= 300.0);
    }

    #[test]
    fn explicit_targets_replace_early_cards_with_victory_policies() {
        let mut culture = Game::new(2, 24, 16, 78, 200, 0);
        culture.players[0].government = Some("chiefdom".to_string());
        culture.players[0]
            .civics
            .insert(crate::name!("cultural_heritage"));
        culture.players[0]
            .policies
            .extend([crate::name!("discipline"), crate::name!("urban_planning")]);
        AdvancedAi::targeting(VictoryTarget::Culture).strategic_policies(
            &mut culture,
            0,
            GrandStrategy::Expansion,
        );
        assert!(culture.players[0].policies.contains(&crate::name!("heritage_tourism")));
        assert!(culture.players[0].policies.contains(&crate::name!("discipline")));
        assert!(!culture.players[0].policies.contains(&crate::name!("urban_planning")));

        let mut science = Game::new(2, 24, 16, 79, 200, 0);
        science.players[0].government = Some("chiefdom".to_string());
        science.players[0].civics.insert(crate::name!("space_race"));
        science.players[0]
            .policies
            .extend([crate::name!("discipline"), crate::name!("urban_planning")]);
        AdvancedAi::targeting(VictoryTarget::Science).strategic_policies(
            &mut science,
            0,
            GrandStrategy::Expansion,
        );
        assert!(science.players[0]
            .policies
            
.contains(&crate::name!("integrated_space_cell")));
        assert!(science.players[0].policies.contains(&crate::name!("urban_planning")));
        assert!(!science.players[0].policies.contains(&crate::name!("discipline")));

        let mut reactive = culture.clone();
        reactive.players[0].policies.clear();
        reactive.players[0]
            .policies
            .extend([crate::name!("discipline"), crate::name!("urban_planning")]);
        AdvancedAi::new().strategic_policies(&mut reactive, 0, GrandStrategy::Culture);
        assert!(reactive.players[0].policies.contains(&crate::name!("heritage_tourism")));
    }

    #[test]
    fn future_policy_reassessment_replaces_ancient_fillers() {
        let mut game = Game::new(2, 24, 16, 79_100, 250, 0);
        let civics: Vec<Name> = game.rules.civics.keys().cloned().collect();
        game.players[0].civics.extend(civics);
        game.players[0].government = Some("synthetic_technocracy".to_string());
        let ancient = [
            "discipline",
            "agoge",
            "bastions",
            "retainers",
            "urban_planning",
            "god_king",
            "ilkum",
            "colonization",
            "insulae",
            "charismatic_leader",
        ];
        game.players[0]
            .policies
            .extend(ancient.iter().map(|card| Name::new(card)));

        AdvancedAi::targeting(VictoryTarget::Science).strategic_policies(
            &mut game,
            0,
            GrandStrategy::Science,
        );

        assert!(game.players[0]
            .policies
            
.contains(&crate::name!("integrated_space_cell")));
        assert!(game.players[0]
            .policies
            
.contains(&crate::name!("future_victory_science")));
        assert!(game.players[0].policies.contains(&crate::name!("five_year_plan")));
        assert!(game.players[0].policies.contains(&crate::name!("rationalism")));
        assert!(ancient
            .iter()
            .all(|card| !game.players[0].policies.contains(&Name::new(*card))));
    }

    #[test]
    fn policy_reassessment_only_executes_swaps_that_can_fit() {
        let install = |game: &mut Game, government: &str, policies: &[&str]| {
            let settler = game
                .player_unit_ids(0)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
            game.players[0].government = Some(government.to_string());
            game.players[0].policies = policies.iter().map(|card| Name::new(*card)).collect();
        };

        // Fascism plus Big Ben seats six Military cards by spending both
        // wildcard slots, two Economic cards, and one Diplomatic card. The
        // remaining Conquest card is Diplomatic, so removing Rationalism
        // cannot make Gunboat Diplomacy fit: the free Economic slot does not
        // relieve either Military overflow slot. Probing that impossible swap
        // used to emit an Unslot/Slot pair which restored the exact same state.
        let mut blocked = Game::new(2, 24, 16, 79_102, 250, 0);
        install(
            &mut blocked,
            "fascism",
            &[
                "lightning_warfare",
                "total_war",
                "propaganda",
                "levee_en_masse",
                "force_modernization",
                "logistics",
                "five_year_plan",
                "rationalism",
                "cryptography",
            ],
        );
        let city = blocked.player_city_ids(0)[0];
        let position = blocked.cities[&city].pos;
        blocked
            .cities
            .get_mut(&city)
            .unwrap()
            .wonders
            .insert(crate::name!("big_ben"), position);
        blocked.players[0]
            .civics
            .extend([crate::name!("ideology"), crate::name!("the_enlightenment")]);
        let policies_before = blocked.players[0].policies.clone();
        let log_before = blocked.log.len();

        AdvancedAi::new().strategic_policies(&mut blocked, 0, GrandStrategy::Conquest);

        assert_eq!(blocked.players[0].policies, policies_before);
        assert_eq!(
            blocked
                .log
                .since(log_before)
                .filter(|(_, action)| matches!(
                    action,
                    Action::SlotPolicy { .. } | Action::UnslotPolicy { .. }
                ))
                .count(),
            0,
            "an impossible replacement must not be executed and undone"
        );

        // A different typed card can still be the right replacement. Here an
        // extra Economic card occupies Autocracy's wildcard slot. Removing it
        // frees that wildcard for Logistics, so the exact set-fit check must
        // allow the cross-type Rationalism -> Logistics swap.
        let mut valid = Game::new(2, 24, 16, 79_103, 250, 0);
        install(
            &mut valid,
            "autocracy",
            &[
                "lightning_warfare",
                "five_year_plan",
                "rationalism",
                "cryptography",
            ],
        );
        valid.players[0]
            .civics
            .extend([
                crate::name!("ideology"),
                crate::name!("mercantilism"),
                crate::name!("the_enlightenment"),
            ]);
        let log_before = valid.log.len();

        AdvancedAi::new().strategic_policies(&mut valid, 0, GrandStrategy::Conquest);

        assert!(valid.players[0].policies.contains(&crate::name!("logistics")));
        assert!(!valid.players[0].policies.contains(&crate::name!("rationalism")));
        assert_eq!(
            valid
                .log
                .since(log_before)
                .filter(|(_, action)| matches!(
                    action,
                    Action::SlotPolicy { .. } | Action::UnslotPolicy { .. }
                ))
                .count(),
            2,
            "one valid replacement is one Unslot followed by one Slot"
        );
    }

    #[test]
    fn dark_age_policies_follow_strategy_and_never_close_live_expansion() {
        let mut game = Game::new(2, 24, 16, 79_101, 250, 0);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        install_ai_test_district(&mut game, city, "holy_site");
        game.world_era = 2;
        game.players[0].age = "dark".to_string();
        game.players[0].government = Some("classical_republic".to_string());

        AdvancedAi::targeting(VictoryTarget::Science).strategic_policies(
            &mut game,
            0,
            GrandStrategy::Science,
        );
        assert!(game.players[0].policies.contains(&crate::name!("monasticism")));

        game.players[0].policies.clear();
        game.players[0].policies.insert(crate::name!("isolationism"));
        game.spawn_test_unit("settler", 0, game.cities[&city].pos);
        AdvancedAi::new().strategic_policies(&mut game, 0, GrandStrategy::Expansion);
        assert!(!game.players[0].policies.contains(&crate::name!("isolationism")));

        game.players[0].policies.clear();
        game.at_war.insert((0, 1));
        AdvancedAi::new().strategic_policies(&mut game, 0, GrandStrategy::Conquest);
        assert!(game.players[0].policies.contains(&crate::name!("twilight_valor")));

        game.world_era = 8;
        AdvancedAi::new().strategic_policies(&mut game, 0, GrandStrategy::Conquest);
        assert!(!game.players[0].policies.contains(&crate::name!("twilight_valor")));
    }

    #[test]
    fn culture_trade_routes_connect_unpressured_rivals_before_duplicating_links() {
        let mut game = Game::new_full(3, 18, 10, 79_001, 200, 0, false);
        for pid in 0..3 {
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
        let origin = game.player_city_ids(0)[0];
        let connected = game.player_city_ids(1)[0];
        let unconnected = game.player_city_ids(2)[0];
        game.routes.push(crate::game::TradeRoute {
            origin,
            dest: connected,
            owner: 0,
            ends: 30,
        });

        let ai = AdvancedAi::targeting(VictoryTarget::Culture);
        let connected_value = ai.trade_route_destination_value(
            &game,
            0,
            &game.cities[&connected],
            GrandStrategy::Expansion,
        );
        let unconnected_value = ai.trade_route_destination_value(
            &game,
            0,
            &game.cities[&unconnected],
            GrandStrategy::Expansion,
        );
        assert!(unconnected_value > connected_value);

        let science_ai = AdvancedAi::targeting(VictoryTarget::Science);
        let science_connected = science_ai.trade_route_destination_value(
            &game,
            0,
            &game.cities[&connected],
            GrandStrategy::Expansion,
        );
        let science_unconnected = science_ai.trade_route_destination_value(
            &game,
            0,
            &game.cities[&unconnected],
            GrandStrategy::Expansion,
        );
        assert!(
            unconnected_value - science_unconnected > connected_value - science_connected,
            "only the Culture objective should add the missing-rival pressure bonus"
        );
    }

    #[test]
    fn advanced_trade_routes_value_named_great_person_destination_gold() {
        let mut game = Game::new_full(2, 20, 12, 79_004, 200, 0, false);
        for pid in 0..2 {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.current = pid;
            game.apply(pid, &Action::FoundCity { unit: settler })
                .unwrap();
        }
        let destination = game.player_city_ids(1)[0];
        let ai = AdvancedAi::targeting(VictoryTarget::Science);
        let value = |game: &Game| {
            ai.trade_route_destination_value(
                game,
                0,
                &game.cities[&destination],
                GrandStrategy::Expansion,
            )
        };
        let baseline = value(&game);

        game.cities
            .get_mut(&destination)
            .unwrap()
            .great_person_foreign_route_gold = 2.0;
        let city_bonus = value(&game);
        assert!(city_bonus > baseline);

        let resource_tile = game.cities[&destination]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != game.cities[&destination].pos)
            .unwrap();
        let tile = game.map.tiles.get_mut(&resource_tile).unwrap();
        tile.resource = Some(crate::name!("iron"));
        tile.improvement = Some(crate::name!("mine"));
        tile.pillaged = false;
        game.players[0].counters.insert(
            "great_person:strategic_destination_trade_gold".to_string(),
            2,
        );
        assert!(value(&game) > city_bonus);
    }

    #[test]
    fn advanced_trader_uses_an_unreserved_destination_empire_wide() {
        let mut game = Game::new_full(1, 30, 18, 79_003, 200, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let first = game.player_city_ids(0)[0];
        let first_pos = game.cities[&first].pos;
        let second = found_nearby_test_city(&mut game, 0, first_pos);
        let third = found_nearby_test_city(&mut game, 0, first_pos);
        game.players[0].civics.insert(crate::name!("foreign_trade"));
        game.players[0]
            .counters
            .insert("great_person_trade_capacity".to_string(), 1);
        game.routes.push(crate::game::TradeRoute {
            origin: second,
            dest: first,
            owner: 0,
            ends: game.turn + 30,
        });
        let trader = game.spawn_test_unit("trader", 0, game.cities[&third].pos);

        assert!(AdvancedAi::new().advanced_trader_step(
            &mut game,
            0,
            trader,
            GrandStrategy::Expansion,
        ));
        assert!(!game.units.contains_key(&trader));
        assert!(game
            .routes
            .iter()
            .any(|route| route.origin == third && route.dest == second));
    }

    #[test]
    fn advanced_trader_relocates_to_a_city_with_a_legal_route() {
        let mut game = Game::new_full(1, 30, 18, 79_003, 200, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let destination = game.player_city_ids(0)[0];
        let destination_pos = game.cities[&destination].pos;
        let current_city = found_nearby_test_city(&mut game, 0, destination_pos);
        game.players[0].civics.insert(crate::name!("foreign_trade"));
        game.players[0]
            .counters
            .insert("great_person_trade_capacity".to_string(), 1);
        game.routes.push(crate::game::TradeRoute {
            origin: current_city,
            dest: destination,
            owner: 0,
            ends: game.turn + 30,
        });
        let start = game.cities[&current_city].pos;
        let target = game.cities[&destination].pos;
        let trader = game.spawn_test_unit("trader", 0, start);
        let before = game.wdist(start, target);

        assert!(AdvancedAi::new().advanced_trader_step(
            &mut game,
            0,
            trader,
            GrandStrategy::Expansion,
        ));
        assert!(game.units.contains_key(&trader));
        assert_ne!(game.units[&trader].pos, start);
        for _ in 0..20 {
            if game.units[&trader].pos == target {
                break;
            }
            game.units.get_mut(&trader).unwrap().moves_left = 4.0;
            assert!(AdvancedAi::new().advanced_trader_step(
                &mut game,
                0,
                trader,
                GrandStrategy::Expansion,
            ));
        }
        assert_eq!(game.units[&trader].pos, target);
        assert!(game.wdist(game.units[&trader].pos, target) < before);
    }

    #[test]
    fn trader_production_requires_an_open_route_and_respects_idle_supply() {
        let mut game = Game::new_full(1, 30, 18, 79_004, 200, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        game.players[0].civics.insert(crate::name!("foreign_trade"));
        let city = game.player_city_ids(0)[0];
        let item = Item::Unit {
            unit: crate::name!("trader"),
        };
        let plan = StrategicPlan {
            strategy: GrandStrategy::Expansion,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 2,
            assessed_turn: game.turn,
            rush: false,
        };
        let ai = AdvancedAi::new();

        let counts = ai.counts(&game, 0);
        assert!(ai.production_value(&game, 0, city, &item, &plan, &counts) < -9_000.0);

        let city_pos = game.cities[&city].pos;
        found_nearby_test_city(&mut game, 0, city_pos);
        let counts = ai.counts(&game, 0);
        assert!(ai.production_value(&game, 0, city, &item, &plan, &counts) > 0.0);

        game.spawn_test_unit("trader", 0, game.cities[&city].pos);
        let counts = ai.counts(&game, 0);
        assert!(ai.production_value(&game, 0, city, &item, &plan, &counts) < -9_000.0);
    }

    #[test]
    fn strategic_governments_use_late_tiers_and_match_the_culture_holdout() {
        let mut culture = Game::new_full(3, 18, 10, 79_002, 200, 0, false);
        culture.players[0]
            .civics
            .extend([crate::name!("class_struggle"), crate::name!("suffrage")]);
        culture.players[1].government = Some("communism".to_string());
        culture.players[1].culture_lifetime = 20_000.0;
        culture.players[2].government = Some("democracy".to_string());
        culture.players[2].culture_lifetime = 10_000.0;
        AdvancedAi::targeting(VictoryTarget::Culture).strategic_government(
            &mut culture,
            0,
            GrandStrategy::Culture,
        );
        assert_eq!(culture.players[0].government.as_deref(), Some("communism"));

        let mut science = Game::new_full(2, 18, 10, 79_003, 200, 0, false);
        science.players[0]
            .civics
            .insert(crate::name!("synthetic_technocracy"));
        AdvancedAi::targeting(VictoryTarget::Science).strategic_government(
            &mut science,
            0,
            GrandStrategy::Science,
        );
        assert_eq!(
            science.players[0].government.as_deref(),
            Some("synthetic_technocracy")
        );

        // An adaptive plan must not fall from its one ideal Tier-2 civic all
        // the way back to Tier 1. The live archive had a Science Rome with
        // Divine Right stuck in Classical Republic and a Religion Egypt with
        // Exploration doing the same, despite their unlocked six-slot choices.
        let mut science_fallback = Game::new_full(1, 18, 10, 79_013, 200, 0, false);
        science_fallback.players[0].government = Some("classical_republic".to_string());
        science_fallback.players[0]
            .civics
            .insert(crate::name!("divine_right"));
        AdvancedAi::new().strategic_government(
            &mut science_fallback,
            0,
            GrandStrategy::Science,
        );
        assert_eq!(
            science_fallback.players[0].government.as_deref(),
            Some("monarchy")
        );

        let mut religion_fallback = Game::new_full(1, 18, 10, 79_014, 200, 0, false);
        religion_fallback.players[0].government = Some("classical_republic".to_string());
        religion_fallback.players[0]
            .civics
            .insert(crate::name!("exploration"));
        AdvancedAi::new().strategic_government(
            &mut religion_fallback,
            0,
            GrandStrategy::Religion,
        );
        assert_eq!(
            religion_fallback.players[0].government.as_deref(),
            Some("merchant_republic")
        );
    }

    #[test]
    fn adaptive_government_does_not_repeat_lateral_anarchy() {
        let mut game = Game::new_full(2, 18, 10, 79_015, 200, 0, false);
        game.players[0]
            .civics
            .extend([crate::name!("suffrage"), crate::name!("totalitarianism")]);
        game.players[0].government = Some("fascism".to_string());
        game.players[0]
            .past_governments
            .extend(["fascism".to_string(), "democracy".to_string()]);
        game.players[1].government = Some("democracy".to_string());
        game.players[1].culture_lifetime = 20_000.0;

        AdvancedAi::new().strategic_government(&mut game, 0, GrandStrategy::Culture);

        assert_eq!(game.players[0].government.as_deref(), Some("fascism"));
        assert_eq!(game.players[0].anarchy_turns, 0);
        assert!(game.players[0].pending_government.is_none());

        // Returning to a genuinely larger government remains worthwhile:
        // the two dead turns buy a persistent jump from six to eight slots.
        game.players[0].government = Some("merchant_republic".to_string());
        game.players[0]
            .past_governments
            .insert("merchant_republic".to_string());
        AdvancedAi::new().strategic_government(&mut game, 0, GrandStrategy::Conquest);
        assert!(game.players[0].government.is_none());
        assert_eq!(game.players[0].pending_government.as_deref(), Some("fascism"));
        assert!(game.players[0].anarchy_turns > 0);
    }

    #[test]
    fn advanced_turn_does_not_run_the_baseline_government_selector_first() {
        let mut game = Game::new_full(2, 18, 10, 79_016, 200, 0, false);
        game.players[0].civics.extend([
            crate::name!("code_of_laws"),
            crate::name!("political_philosophy"),
        ]);
        game.players[0].government = Some("chiefdom".to_string());
        game.players[0]
            .past_governments
            .insert("chiefdom".to_string());

        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        ai.take_turn(&mut game, 0);

        assert_eq!(game.players[0].government.as_deref(), Some("oligarchy"));
        assert_eq!(game.players[0].anarchy_turns, 0);
        assert!(game.players[0].pending_government.is_none());
        assert!(game.players[0].past_governments.contains("oligarchy"));
        assert!(
            !game.players[0]
                .past_governments
                .contains("classical_republic"),
            "the baseline selector must not install a throwaway government before the strategic one"
        );
    }

    #[test]
    fn adaptive_government_does_not_create_anarchy_by_downgrading_first() {
        let mut game = Game::new_full(2, 18, 10, 79_017, 200, 0, false);
        game.players[0].civics.extend([
            crate::name!("class_struggle"),
            crate::name!("reformed_church"),
        ]);
        game.players[0].government = Some("communism".to_string());
        game.players[0]
            .past_governments
            .insert("communism".to_string());
        game.players[0].faith = 1_000.0;

        AdvancedAi::new().strategic_government(&mut game, 0, GrandStrategy::Conquest);

        assert_eq!(game.players[0].government.as_deref(), Some("communism"));
        assert_eq!(game.players[0].anarchy_turns, 0);
        assert!(game.players[0].pending_government.is_none());
        assert!(
            !game.players[0].past_governments.contains("theocracy"),
            "a free first adoption is still a downgrade when it discards two policy slots"
        );
    }

    #[test]
    fn faith_stockpile_mobilizes_for_an_imminent_threat() {
        let mut game = Game::new_full(2, 24, 16, 79_012, 200, 0, false);
        for pid in 0..2 {
            game.current = pid;
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.apply(pid, &Action::FoundCity { unit: settler })
                .unwrap();
        }
        game.current = 0;
        game.players[0].civics.insert(crate::name!("reformed_church"));
        game.players[0].faith = 1_500.0;
        let target = game.player_city_ids(1)[0];
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: Some(target),
            threatened_city: None,
            desired_cities: 1,
            assessed_turn: game.turn,
            rush: false,
        };
        let before_units = game.player_unit_ids(0).len();
        let ai = AdvancedAi::new();

        ai.strategic_government(&mut game, 0, plan.strategy);
        assert_eq!(game.players[0].government.as_deref(), Some("theocracy"));
        assert!(ai.military_faith_spending(&mut game, 0, &plan));
        assert_eq!(game.player_unit_ids(0).len(), before_units + 1);
        assert!(game.players[0].faith < 1_500.0);
    }

    #[test]
    fn culture_quick_deals_buy_the_direction_that_increases_our_tourism() {
        let mut game = Game::new_full(2, 18, 10, 79_004, 200, 0, false);
        // A quick deal is offered to a counterparty, so there has to be one.
        game.record_contact(0, 1);
        game.turn = 6;
        game.players[0].gold = 1_000.0;
        game.players[1].gold = 1_000.0;
        game.players[0].civics.insert(crate::name!("early_empire"));
        game.players[1].civics.insert(crate::name!("early_empire"));

        AdvancedAi::targeting(VictoryTarget::Culture).strategic_bilateral_trade(
            &mut game,
            0,
            None,
            GrandStrategy::Expansion,
        );

        assert!(game.has_open_borders(0, 1));
        assert!(!game.has_open_borders(1, 0));
        assert_eq!(game.international_tourism_multiplier(0, 1, false), 1.25);
    }

    #[test]
    fn culture_quick_deals_buy_housed_great_works_and_preserve_our_own() {
        let mut game = Game::new_full(2, 20, 12, 79_005, 200, 0, false);
        // A quick deal is offered to a counterparty, so there has to be one.
        game.record_contact(0, 1);
        for pid in 0..2 {
            game.current = pid;
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.apply(pid, &Action::FoundCity { unit: settler })
                .unwrap();
            let city = game.player_city_ids(pid)[0];
            install_ai_test_district(&mut game, city, "theater_square");
            game.cities
                .get_mut(&city)
                .unwrap()
                .buildings
                .push(crate::name!("amphitheater"));
            game.players[pid].gold = 1_000.0;
        }
        game.current = 0;
        game.players[1]
            .counters
            .insert("great_work:writing".to_string(), 2);
        game.turn = 6;

        AdvancedAi::targeting(VictoryTarget::Culture).strategic_bilateral_trade(
            &mut game,
            0,
            None,
            GrandStrategy::Expansion,
        );
        assert_eq!(game.players[0].counters["great_work:writing"], 1);
        assert_eq!(game.players[1].counters["great_work:writing"], 1);

        let mut preserve = Game::new_full(2, 20, 12, 79_006, 200, 0, false);
        for pid in 0..2 {
            preserve.current = pid;
            let settler = preserve
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| preserve.units[unit].kind == "settler")
                .unwrap();
            preserve
                .apply(pid, &Action::FoundCity { unit: settler })
                .unwrap();
            let city = preserve.player_city_ids(pid)[0];
            install_ai_test_district(&mut preserve, city, "theater_square");
            preserve
                .cities
                .get_mut(&city)
                .unwrap()
                .buildings
                .push(crate::name!("amphitheater"));
            preserve.players[pid].gold = 1_000.0;
        }
        preserve.current = 0;
        preserve.players[0]
            .counters
            .insert("great_work:writing".to_string(), 2);
        preserve.turn = 6;
        AdvancedAi::targeting(VictoryTarget::Culture).strategic_bilateral_trade(
            &mut preserve,
            0,
            None,
            GrandStrategy::Expansion,
        );
        assert_eq!(preserve.players[0].counters["great_work:writing"], 2);
        assert_eq!(
            preserve.players[1]
                .counters
                .get("great_work:writing")
                .copied()
                .unwrap_or(0),
            0
        );
    }

    #[test]
    fn explicit_targets_choose_synergistic_secret_societies() {
        for (index, (target, expected)) in [
            (VictoryTarget::Science, "hermetic_order"),
            (VictoryTarget::Culture, "voidsingers"),
            (VictoryTarget::Religion, "voidsingers"),
            (VictoryTarget::Diplomacy, "owls_of_minerva"),
            (VictoryTarget::Domination, "owls_of_minerva"),
        ]
        .into_iter()
        .enumerate()
        {
            let mut game = Game::new(2, 24, 16, 110 + index as u64, 80, 0);
            // Secret Societies is a New Frontier mode a lobby has to switch on.
            game.game_modes.insert("secret_societies".to_string());
            game.players[0].civics.insert(crate::name!("code_of_laws"));
            let ai = AdvancedAi::targeting(target);
            ai.advanced_secret_society(&mut game, 0, target.strategy());
            assert_eq!(game.players[0].secret_society.as_deref(), Some(expected));
        }
    }

    /// Over a whole six-player game the commonest reason an upgrade was
    /// unavailable was not distance up the tree but a node its owner had
    /// simply never taken: Archery, Iron Working and Machinery accounted for
    /// most of it, with empires holding ten to thirty technologies still
    /// fielding the Slingers and Warriors those nodes retire.
    #[test]
    fn research_prices_the_node_that_retires_the_army_already_in_the_field() {
        let mut g = Game::new(2, 24, 16, 21, 80, 0);
        let ai = AdvancedAi::new();
        let home = g.units[&g.player_unit_ids(0)[0]].pos;
        let slingers = |g: &Game| {
            g.units
                .values()
                .filter(|unit| unit.owner == 0 && unit.kind == "slinger")
                .count()
        };
        let raise = |g: &mut Game, wanted: usize| {
            let before = slingers(g);
            for pos in g.wdisk(home, 4) {
                if slingers(g) >= before + wanted {
                    break;
                }
                g.spawn_test_unit("slinger", 0, pos);
            }
            slingers(g) - before
        };

        // Measure the second batch against the first: the other terms that
        // read the empire's units settle after the first Slinger, so only the
        // waiting upgrades separate the two readings.
        let grown = raise(&mut g, 3);
        assert!(grown >= 3, "test needs a garrison, spawned {grown}");
        let three = ai.tech_value(&g, 0, "archery", GrandStrategy::Science);
        let grown = raise(&mut g, 3);
        assert!(grown >= 3, "test needs a bigger garrison, spawned {grown}");
        let six = ai.tech_value(&g, 0, "archery", GrandStrategy::Science);

        // `tech_value` divides by the square root of the technology's cost,
        // so three more Slingers waiting on Archery (15 to 25 apiece, at the
        // peacetime rate) read as a few points here, not forty.
        assert!(six > three + 4.0, "three={three} six={six}");
        // A node that retires nothing this empire owns gains nothing.
        assert!(
            ai.tech_value(&g, 0, "iron_working", GrandStrategy::Science) < six,
            "no unit here upgrades through Iron Working"
        );
    }

    #[test]
    fn culture_strategy_treats_tourism_as_a_builder_yield() {
        let mut g = Game::new(2, 24, 16, 73, 80, 0);
        let pos = *g.map.tiles.keys().next().unwrap();
        for neighbor in g.nbrs(pos) {
            let tile = g.map.tiles.get_mut(&neighbor).unwrap();
            tile.terrain = crate::name!("coast");
            tile.feature = None;
            tile.district = None;
            tile.wonder = None;
            tile.improvement = None;
            tile.pillaged = false;
        }
        assert!(g.tile_appeal(pos) >= 4);
        let ai = AdvancedAi::targeting(VictoryTarget::Culture);

        let resort = ai.improvement_value(&g, pos, "seaside_resort", GrandStrategy::Culture);
        let farm = ai.improvement_value(&g, pos, "farm", GrandStrategy::Culture);

        assert!(resort > farm + 100.0, "resort={resort}, farm={farm}");
    }

    #[test]
    fn culture_builders_upgrade_farms_to_resorts_without_reverting_them() {
        let mut g = Game::new(2, 24, 16, 74, 80, 0);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        g.players[0].techs.insert(crate::name!("radio"));
        let city = g.player_city_ids(0)[0];
        let pos = g.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|pos| *pos != g.cities[&city].pos)
            .unwrap();
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
        tile.improvement = Some(crate::name!("farm"));
        for neighbor in g.nbrs(pos) {
            let tile = g.map.tiles.get_mut(&neighbor).unwrap();
            tile.terrain = crate::name!("coast");
            tile.feature = None;
            tile.resource = None;
            tile.improvement = None;
            tile.district = None;
            tile.wonder = None;
            tile.pillaged = false;
        }
        assert!(g.tile_appeal(pos) >= 4);

        let ai = AdvancedAi::targeting(VictoryTarget::Culture);
        let upgrades = ai.worthwhile_improvements(&g, 0, pos, GrandStrategy::Culture);
        assert_eq!(upgrades.first().map(|name| name.as_str()), Some("seaside_resort"));

        g.map.tiles.get_mut(&pos).unwrap().improvement = Some(crate::name!("seaside_resort"));
        assert!(ai
            .worthwhile_improvements(&g, 0, pos, GrandStrategy::Culture)
            .is_empty());
    }

    #[test]
    fn diplomatic_strategy_concentrates_envoys_into_a_suzerainty() {
        let mut g = Game::new(2, 24, 16, 77, 80, 2);
        g.players[0].envoys_free = 3;
        AdvancedAi::new().advanced_envoys(&mut g, 0, GrandStrategy::Diplomacy, None);
        assert_eq!(g.players[0].envoys_free, 0);
        assert!(g.players[0].envoys.iter().any(|(_, count)| *count >= 3));
    }

    #[test]
    fn envoy_strategy_prices_the_next_active_building_threshold_per_envoy() {
        let mut game = Game::new_full(1, 28, 18, 7_710, 120, 2, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        let city = game.found_city_for(0, game.units[&settler].pos, None);
        game.remove_unit(settler);
        install_ai_test_district(&mut game, city, "commercial_hub");
        install_ai_test_district(&mut game, city, "harbor");
        install_ai_test_district(&mut game, city, "diplomatic_quarter");
        game.cities.get_mut(&city).unwrap().buildings.extend(
            ["stock_exchange", "seaport", "chancery"]
                .into_iter()
                .map(Name::new),
        );

        let states: Vec<usize> = game
            .players
            .iter()
            .filter(|player| player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .collect();
        let hattusa = states[0];
        let zanzibar = states[1];
        game.players[hattusa].civ = "Hattusa".to_string();
        game.players[zanzibar].civ = "Zanzibar".to_string();
        game.players[0].envoys = vec![(hattusa, 4), (zanzibar, 5)];
        game.players[0].envoys_free = 1;

        let (science_steps, science_gain) = game.next_envoy_type_bonus(0, hattusa).unwrap();
        let (gold_steps, gold_gain) = game.next_envoy_type_bonus(0, zanzibar).unwrap();
        assert_eq!((science_steps, science_gain.science), (2, 3.0));
        assert_eq!((gold_steps, gold_gain.gold), (1, 18.0));

        AdvancedAi::new().advanced_envoys(&mut game, 0, GrandStrategy::Science, None);
        assert_eq!(game.envoys_at(0, hattusa), 4);
        assert_eq!(game.envoys_at(0, zanzibar), 6);
    }

    #[test]
    fn religious_envoys_prefer_yerevan_but_skip_a_bonus_shared_by_economic_alliance() {
        let mut game = Game::new_full(2, 32, 20, 7_711, 120, 2, false);
        let minors: Vec<usize> = game
            .players
            .iter()
            .filter(|player| player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .collect();
        assert_eq!(minors.len(), 2);
        game.players[minors[0]].civ = "Kandy".to_string();
        game.players[minors[1]].civ = "Yerevan".to_string();
        game.players[0].envoys_free = 1;

        AdvancedAi::new().advanced_envoys(&mut game, 0, GrandStrategy::Religion, None);
        assert_eq!(game.envoys_at(0, minors[0]), 0);
        assert_eq!(game.envoys_at(0, minors[1]), 1);

        game.players[0].envoys.clear();
        game.players[0].envoys_free = 1;
        game.players[1].envoys = vec![(minors[1], 3)];
        let alliance = crate::game::AllianceState {
            kind: "economic".to_string(),
            points: 240.0,
            level: 3,
            ends: game.turn + 30,
        };
        game.players[0].alliances.insert(1, alliance.clone());
        game.players[1].alliances.insert(0, alliance);

        AdvancedAi::new().advanced_envoys(&mut game, 0, GrandStrategy::Religion, None);
        assert_eq!(game.envoys_at(0, minors[0]), 1);
        assert_eq!(game.envoys_at(0, minors[1]), 0);
    }

    #[test]
    fn command_phase_spends_promotions_and_links_support() {
        let mut g = Game::new_full(2, 24, 16, 79, 80, 0, false);
        let veteran = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| !g.available_promotions(*uid).is_empty())
            .or_else(|| {
                let uid = g.player_unit_ids(0).into_iter().find(|uid| {
                    !g.rules.units[g.units[uid].kind]
                        .promotion_class
                        .is_empty()
                })?;
                g.units.get_mut(&uid).unwrap().xp = 15;
                Some(uid)
            })
            .expect("major starts with a promotable military class");
        g.units.get_mut(&veteran).unwrap().xp = 15;
        g.units.get_mut(&veteran).unwrap().hp = 45;
        AdvancedAi::new().advanced_promotions(&mut g, 0, GrandStrategy::Conquest);
        assert_eq!(g.units[&veteran].promotions.len(), 1);
        assert_eq!(g.units[&veteran].hp, 95);
        assert_eq!(g.units[&veteran].moves_left, 0.0);

        let pos = g
            .map
            .tiles
            .iter()
            .find(|(pos, tile)| {
                g.rules.is_passable(tile) && !g.rules.is_water(tile) && g.units_at(**pos).is_empty()
            })
            .map(|(pos, _)| *pos)
            .unwrap();
        let escort = g.spawn_test_unit("warrior", 0, pos);
        let support = g.spawn_test_unit("battering_ram", 0, pos);
        AdvancedAi::new().advanced_formations(&mut g, 0);
        assert_eq!(g.units[&escort].linked_to, Some(support));
        assert_eq!(g.units[&support].linked_to, Some(escort));
    }

    #[test]
    fn command_phase_forms_corps_without_hollowing_out_the_army() {
        let mut g = Game::new_full(2, 24, 16, 80, 80, 0, false);
        g.players[0].civics.insert(crate::name!("nationalism"));
        let pos = g
            .map
            .tiles
            .iter()
            .find(|(_, tile)| g.rules.is_passable(tile) && !g.rules.is_water(tile))
            .map(|(pos, _)| *pos)
            .unwrap();
        for _ in 0..6 {
            g.spawn_test_unit("warrior", 0, pos);
        }
        let before = g
            .player_unit_ids(0)
            .into_iter()
            .filter(|uid| g.rules.units[g.units[uid].kind].class == "military")
            .count();
        AdvancedAi::new().advanced_formations(&mut g, 0);
        let military: Vec<u32> = g
            .player_unit_ids(0)
            .into_iter()
            .filter(|uid| g.rules.units[g.units[uid].kind].class == "military")
            .collect();
        assert!(military.len() < before);
        assert!(military.len() >= 5);
        assert!(military.iter().any(|uid| g.units[uid].formation == 1));
    }


    /// A staged battle only tests the code under test if the organic map is
    /// not also fighting one nearby: units and cities the generator placed
    /// give the planner closer targets and quietly rewrite the expected order.
    /// Arenas are therefore ranked by how far they sit from everything the
    /// generator put down, so the quietest corner of any map wins.
    fn organic_clearance(g: &Game, pos: Pos) -> i32 {
        g.units
            .values()
            .map(|unit| g.wdist(unit.pos, pos))
            .chain(g.cities.values().map(|city| g.wdist(city.pos, pos)))
            .min()
            .unwrap_or(i32::MAX)
    }

    fn quietest_first(g: &Game, mut candidates: Vec<Pos>) -> Vec<Pos> {
        candidates.sort_by_key(|pos| (std::cmp::Reverse(organic_clearance(g, *pos)), *pos));
        candidates
    }

    #[test]
    fn armies_and_fleets_receive_domain_specific_shared_orders() {
        let mut g = Game::new_full(2, 24, 16, 78, 80, 0, false);
        g.at_war.insert((0, 1));

        let land_candidates = quietest_first(
            &g,
            g.map
                .tiles
                .iter()
                .filter(|(pos, tile)| {
                    g.rules.is_passable(tile)
                        && !g.rules.is_water(tile)
                        && g.units_at(**pos).is_empty()
                })
                .map(|(pos, _)| *pos)
                .collect(),
        );
        let land_target = land_candidates
            .into_iter()
            .find_map(|pos| {
                let ring: Vec<Pos> = g
                    .nbrs(pos)
                    .into_iter()
                    .filter(|neighbor| {
                        g.map.get(*neighbor).is_some_and(|tile| {
                            g.rules.is_passable(tile)
                                && !g.rules.is_water(tile)
                                && g.units_at(*neighbor).is_empty()
                        })
                    })
                    .collect();
                (ring.len() >= 3).then_some((pos, ring))
            })
            .expect("test map has an open land engagement");
        for position in [land_target.0, land_target.1[0], land_target.1[1], land_target.1[2]] {
            let tile = g.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("plains");
            tile.feature = None;
            tile.improvement = None;
            tile.hills = false;
        }
        for ring_tile in land_target.1.iter().copied() {
            g.map.set_river_edge(land_target.0, ring_tile, false);
        }
        let army = [
            g.spawn_test_unit("warrior", 0, land_target.1[0]),
            g.spawn_test_unit("archer", 0, land_target.1[1]),
            g.spawn_test_unit("catapult", 0, land_target.1[2]),
        ];
        // A mirror matchup on level ground is an even trade, which the
        // planner is right to decline; this test is about which unit
        // receives the order, so leave the defender plainly worth striking.
        let defender = g.spawn_test_unit("warrior", 1, land_target.0);
        g.units.get_mut(&defender).unwrap().hp = 20;

        let sea_candidates = quietest_first(
            &g,
            g.map
                .tiles
                .iter()
                .filter(|(pos, tile)| g.rules.is_water(tile) && g.units_at(**pos).is_empty())
                .map(|(pos, _)| *pos)
                .collect(),
        );
        let sea_target = sea_candidates
            .into_iter()
            .find_map(|pos| {
                let ring: Vec<Pos> = g
                    .nbrs(pos)
                    .into_iter()
                    .filter(|neighbor| {
                        g.map.get(*neighbor).is_some_and(|tile| {
                            g.rules.is_water(tile) && g.units_at(*neighbor).is_empty()
                        })
                    })
                    .collect();
                (ring.len() >= 2).then_some((pos, ring))
            })
            .expect("test map has an open naval engagement");
        let fleet = [
            g.spawn_test_unit("galley", 0, sea_target.1[0]),
            g.spawn_test_unit("galley", 0, sea_target.1[1]),
        ];
        g.spawn_test_unit("galley", 1, sea_target.0);

        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: None,
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: g.turn,
            rush: false,
        };
        let mut ai = AdvancedAi::new();
        ai.rebuild_force_groups(&g, 0, &plan);

        let army_orders = ai
            .force_groups()
            .iter()
            .find(|group| army.iter().all(|uid| group.units.contains(uid)))
            .expect("combined-arms units should share one army order");
        assert_eq!(army_orders.domain, ForceDomain::Land);
        assert_eq!(army_orders.focus_target, Some(land_target.0));
        assert_eq!(army_orders.posture, ForcePosture::Engage);

        let fleet_orders = ai
            .force_groups()
            .iter()
            .find(|group| fleet.iter().all(|uid| group.units.contains(uid)))
            .expect("nearby ships should share one fleet order");
        assert_eq!(fleet_orders.domain, ForceDomain::Sea);
        assert_eq!(fleet_orders.objective, sea_target.0);
        assert_eq!(fleet_orders.focus_target, Some(sea_target.0));
        assert_eq!(fleet_orders.posture, ForcePosture::Engage);

        let acted = ai.advanced_military_step(&mut g, 0, army[0], &plan);
        let last = g.log.last().cloned();
        assert!(
            matches!(
                last,
                Some((0, Action::Attack { unit, target }))
                    if unit == army[0] && target == land_target.0
            ),
            "the army's lead unit should strike the focus target: acted={acted}, log={last:?}"
        );
    }

    #[test]
    fn city_state_wars_receive_a_campaign_target_and_combined_arms_orders() {
        let mut g = Game::new_full(2, 24, 16, 96, 80, 1, false);
        let minor = g
            .players
            .iter()
            .find(|player| player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .expect("test map has a city-state");
        let target_city = g.player_city_ids(minor)[0];
        let target = g.cities[&target_city].pos;
        let staging = g
            .nbrs(target)
            .into_iter()
            .find(|position| {
                g.map.get(*position).is_some_and(|tile| {
                    g.rules.is_passable(tile)
                        && !g.rules.is_water(tile)
                        && g.units_at(*position).is_empty()
                })
            })
            .expect("city-state needs an open attack front");
        // Three units, not two. The posture this case asserts turns on the
        // attackers being locally superior, and two of them cleared the 0.72
        // threshold by 0.009 against a city and its two defenders - close
        // enough that any change to the generated map decided the outcome.
        let attackers = [
            g.spawn_test_unit("warrior", 0, staging),
            g.spawn_test_unit("warrior", 0, staging),
            g.spawn_test_unit("archer", 0, staging),
        ];
        g.at_war.insert((0, minor));

        let mut ai = AdvancedAi::new();
        let plan = ai.assess(&g, 0);
        assert_eq!(plan.target_player, Some(minor));
        assert_eq!(plan.target_city, Some(target_city));

        ai.rebuild_force_groups(&g, 0, &plan);
        let orders = ai
            .force_groups()
            .iter()
            .find(|group| attackers.iter().all(|unit| group.units.contains(unit)))
            .expect("the city-state front should form a shared army order");
        assert_eq!(orders.domain, ForceDomain::Land);
        assert_eq!(orders.objective, target);
        let focus = orders
            .focus_target
            .expect("the army should focus a city-state defender or its city");
        assert!(
            g.city_at(focus)
                .is_some_and(|city| g.cities[&city].owner == minor)
                || g.units_at(focus)
                    .iter()
                    .any(|unit| g.units[unit].owner == minor)
        );
        assert_eq!(orders.posture, ForcePosture::Engage);
    }

    #[test]
    fn coordinated_force_moves_most_routed_units_on_advance() {
        let mut g = Game::new_full(2, 24, 16, 80, 80, 0, false);
        g.at_war.insert((0, 1));
        let (target, staging) = g
            .map
            .tiles
            .iter()
            .filter(|(pos, tile)| {
                g.rules.is_passable(tile) && !g.rules.is_water(tile) && g.units_at(**pos).is_empty()
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
                    })
                    .take(6)
                    .collect();
                (staging.len() == 6).then_some((*target, staging))
            })
            .expect("test map needs an open land campaign");
        g.spawn_test_unit("warrior", 1, target);
        let army: Vec<u32> = staging
            .into_iter()
            .map(|pos| g.spawn_test_unit("warrior", 0, pos))
            .collect();
        let orders = ForceGroup {
            id: army[0],
            domain: ForceDomain::Land,
            units: army.clone(),
            anchor: g.units[&army[0]].pos,
            objective: target,
            focus_target: None,
            posture: ForcePosture::Advance,
            readiness: 1.0,
            local_strength_ratio: 2.0,
        };
        let ai = AdvancedAi::new();
        for uid in &army {
            ai.coordinated_tactical_step(&mut g, 0, *uid, &orders, &[1], false);
        }
        let moved = army.iter().filter(|uid| g.units[uid].moved).count();
        assert!(
            moved * 2 > army.len(),
            "expected most coordinated troops to advance; moved {moved}/{}",
            army.len()
        );
    }

    #[test]
    fn recon_explores_independently_while_combat_roles_form_the_army() {
        let mut g = Game::new_full(2, 24, 16, 81, 80, 0, false);
        g.at_war.insert((0, 1));
        let positions: Vec<Pos> = g
            .map
            .tiles
            .iter()
            .filter(|(pos, tile)| {
                g.rules.is_passable(tile) && !g.rules.is_water(tile) && g.units_at(**pos).is_empty()
            })
            .map(|(pos, _)| *pos)
            .take(6)
            .collect();
        let scout = g.spawn_test_unit("scout", 0, positions[0]);
        let vanguard = g.spawn_test_unit("swordsman", 0, positions[1]);
        let mobile = g.spawn_test_unit("horseman", 0, positions[2]);
        let ranged = g.spawn_test_unit("archer", 0, positions[3]);
        let siege = g.spawn_test_unit("catapult", 0, positions[4]);
        let support = g.spawn_test_unit("battering_ram", 0, positions[5]);
        assert_eq!(AdvancedAi::force_role(&g, scout), ForceRole::Recon);
        assert_eq!(AdvancedAi::force_role(&g, vanguard), ForceRole::Vanguard);
        assert_eq!(AdvancedAi::force_role(&g, mobile), ForceRole::Mobile);
        assert_eq!(AdvancedAi::force_role(&g, ranged), ForceRole::Ranged);
        assert_eq!(AdvancedAi::force_role(&g, siege), ForceRole::Siege);
        assert_eq!(AdvancedAi::force_role(&g, support), ForceRole::Support);

        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: None,
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: g.turn,
            rush: false,
        };
        let mut ai = AdvancedAi::new();
        ai.rebuild_force_groups(&g, 0, &plan);
        assert!(
            ai.force_groups()
                .iter()
                .all(|group| !group.units.contains(&scout)),
            "recon with unexplored terrain should not make the army wait to muster"
        );
        assert!(ai
            .force_groups()
            .iter()
            .any(|group| group.units.contains(&vanguard)));
    }

    #[test]
    fn forcing_reply_search_avoids_a_poisoned_capture() {
        let mut g = Game::new_full(2, 24, 16, 8_117, 80, 0, false);
        g.at_war.insert((0, 1));
        g.current = 0;
        let (anchor, risky, safe, reply_squares) = g
            .map
            .tiles
            .iter()
            .filter(|(position, tile)| {
                g.rules.is_passable(tile)
                    && !g.rules.is_water(tile)
                    && g.units_at(**position).is_empty()
                    && g.city_at(**position).is_none()
                    && g.cities
                        .values()
                        .all(|city| g.wdist(**position, city.pos) > 5)
                    && g.units
                        .values()
                        .filter(|unit| unit.owner != 0)
                        .all(|unit| g.wdist(**position, unit.pos) > 5)
            })
            .find_map(|(anchor, _)| {
                let targets: Vec<Pos> = g
                    .nbrs(*anchor)
                    .into_iter()
                    .filter(|position| {
                        g.map.get(*position).is_some_and(|tile| {
                            g.rules.is_passable(tile) && !g.rules.is_water(tile)
                        }) && g.units_at(*position).is_empty()
                            && g.city_at(*position).is_none()
                    })
                    .collect();
                for risky in &targets {
                    for safe in &targets {
                        if risky == safe || g.wdist(*risky, *safe) < 2 {
                            continue;
                        }
                        let replies: Vec<Pos> = g
                            .wdisk(*risky, 2)
                            .into_iter()
                            .filter(|reply| {
                                g.wdist(*risky, *reply) == 2
                                    // A ranged unit may move one tile before
                                    // firing. Keep the safe capture outside
                                    // both its current and move-then-fire reach
                                    // so this fixture is independent of the
                                    // generated terrain's movement costs.
                                    && g.wdist(*safe, *reply) > 3
                                    && *reply != *anchor
                                    && g.map.get(*reply).is_some_and(|tile| {
                                        g.rules.is_passable(tile) && !g.rules.is_water(tile)
                                    })
                                    && g.units_at(*reply).is_empty()
                                    && g.city_at(*reply).is_none()
                            })
                            .collect();
                        if replies.len() >= 2 {
                            return Some((*anchor, *risky, *safe, replies));
                        }
                    }
                }
                None
            })
            .expect("test map has an isolated poisoned-capture geometry");

        for position in g.wdisk(risky, 2) {
            let tile = g.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("plains");
            tile.feature = None;
            tile.hills = false;
        }
        // The reply search extends one approach step, so an archer three or
        // four tiles from the safe square could still move and shoot it.
        // Moat the approach ring — water stops land approaches without
        // blocking the archers' sight lines — so the safe capture stays
        // unpunishable by construction, not by map luck.
        for position in g.wdisk(safe, 2) {
            if position == safe
                || position == anchor
                || position == risky
                || !g.units_at(position).is_empty()
                || g.city_at(position).is_some()
            {
                continue;
            }
            let tile = g.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("coast");
            tile.feature = None;
            tile.resource = None;
            tile.improvement = None;
            tile.hills = false;
        }
        let attacker = g.spawn_test_unit("swordsman", 0, anchor);
        let risky_defender = g.spawn_test_unit("warrior", 1, risky);
        let safe_defender = g.spawn_test_unit("warrior", 1, safe);
        g.units.get_mut(&risky_defender).unwrap().hp = 1;
        g.units.get_mut(&safe_defender).unwrap().hp = 1;
        g.spawn_test_unit("archer", 1, reply_squares[0]);

        let risky_action = Action::Attack {
            unit: attacker,
            target: risky,
        };
        let safe_action = Action::Attack {
            unit: attacker,
            target: safe,
        };
        let mut ai = AdvancedAi::legacy();
        let single_reply = ai.forcing_reply_penalty(&g, 0, attacker, &risky_action);
        g.spawn_test_unit("archer", 1, reply_squares[1]);
        let risky_reply = ai.forcing_reply_penalty(&g, 0, attacker, &risky_action);
        let safe_reply = ai.forcing_reply_penalty(&g, 0, attacker, &safe_action);
        assert!(
            risky_reply > single_reply + 5.0,
            "the reply extension must price coordinated focus fire: single={single_reply}, risky={risky_reply}, safe={safe_reply}"
        );
        assert!(
            risky_reply > safe_reply + 5.0,
            "the ranged recapture must make the exposed kill materially worse: single={single_reply}, risky={risky_reply}, safe={safe_reply}"
        );

        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: None,
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: g.turn,
            rush: false,
        };
        assert!(ai.advanced_military_step(&mut g, 0, attacker, &plan));
        assert!(!g.units.contains_key(&safe_defender));
        assert!(g.units.contains_key(&risky_defender));
        assert_eq!(g.units[&attacker].pos, safe);
    }

    #[test]
    fn forcing_reply_search_prices_a_move_then_attack_counter() {
        let mut game = Game::new_full(2, 24, 16, 8_118, 80, 0, false);
        game.at_war.insert((0, 1));
        game.current = 0;
        let (anchor, prize, counter) = game
            .map
            .tiles
            .iter()
            .filter(|(position, tile)| {
                game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && game.units_at(**position).is_empty()
                    && game.city_at(**position).is_none()
                    && game
                        .cities
                        .values()
                        .all(|city| game.wdist(**position, city.pos) > 5)
                    && game
                        .units
                        .values()
                        .all(|unit| game.wdist(**position, unit.pos) > 5)
            })
            .find_map(|(anchor, _)| {
                game.nbrs(*anchor).into_iter().find_map(|prize| {
                    let prize_tile = game.map.get(prize)?;
                    if !game.rules.is_passable(prize_tile)
                        || game.rules.is_water(prize_tile)
                        || !game.units_at(prize).is_empty()
                        || game.city_at(prize).is_some()
                    {
                        return None;
                    }
                    game.wdisk(prize, 3).into_iter().find_map(|counter| {
                        let tile = game.map.get(counter)?;
                        (game.wdist(prize, counter) == 3
                            && game.rules.is_passable(tile)
                            && !game.rules.is_water(tile)
                            && game.units_at(counter).is_empty()
                            && game.city_at(counter).is_none()
                            && game.nbrs(counter).into_iter().any(|step| {
                                game.wdist(step, prize) == 2
                                    && game.map.get(step).is_some_and(|tile| {
                                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                                    })
                                    && game.units_at(step).is_empty()
                                    && game.city_at(step).is_none()
                            }))
                        .then_some((*anchor, prize, counter))
                    })
                })
            })
            .expect("test map has a one-step ranged-counter geometry");

        for position in game.wdisk(prize, 3) {
            let tile = game.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("plains");
            tile.feature = None;
            tile.hills = false;
        }
        let attacker = game.spawn_test_unit("swordsman", 0, anchor);
        let defender = game.spawn_test_unit("warrior", 1, prize);
        game.units.get_mut(&defender).unwrap().hp = 1;
        let capture = Action::Attack {
            unit: attacker,
            target: prize,
        };
        let ai = AdvancedAi::new();
        let quiet = ai.forcing_reply_penalty(&game, 0, attacker, &capture);
        game.spawn_test_unit("archer", 1, counter);
        let mobile_counter = ai.forcing_reply_penalty(&game, 0, attacker, &capture);
        assert!(
            mobile_counter > quiet + 5.0,
            "a ranged unit one step outside range must still count as a forcing reply: {mobile_counter} <= {quiet}"
        );
    }

    #[test]
    fn explicit_victory_command_phase_fires_city_center_strikes() {
        let mut game = Game::new_full(2, 20, 14, 8_119, 80, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        let center = game.cities[&city].pos;
        for position in game.wdisk(center, 2) {
            let tile = game.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("plains");
            tile.feature = None;
            tile.hills = false;
        }
        let target = game
            .nbrs(center)
            .into_iter()
            .find(|position| {
                game.units_at(*position).is_empty()
                    && game.city_at(*position).is_none()
                    && game.encampment_at(*position).is_none()
            })
            .unwrap();
        game.at_war.insert((0, 1));
        game.cities.get_mut(&city).unwrap().wall_hp = 100;
        let enemy = game.spawn_test_unit("warrior", 1, target);
        let before = game.units[&enemy].hp;

        AdvancedAi::targeting(VictoryTarget::Domination).advanced_city_strikes(&mut game, 0);

        assert!(game.cities[&city].struck);
        assert!(
            game.units
                .get(&enemy)
                .is_none_or(|defender| defender.hp < before),
            "the explicit victory command phase must spend an available wall strike"
        );
    }

    #[test]
    fn encampment_strikes_choose_the_exact_kill_over_static_unit_strength() {
        let mut game = Game::new_full(2, 20, 14, 8_120, 80, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        let center = game.cities[&city].pos;
        let encampment = game.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != center)
            .unwrap();
        for position in game.wdisk(encampment, 2) {
            let tile = game.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("plains");
            tile.feature = None;
            tile.hills = false;
        }
        game.map.tiles.get_mut(&encampment).unwrap().district = Some(crate::name!("encampment"));
        game.cities
            .get_mut(&city)
            .unwrap()
            .districts
            .insert(crate::name!("encampment"), encampment);
        {
            let city = game.cities.get_mut(&city).unwrap();
            city.encampment_hp = 100;
            city.encampment_wall_hp = 100;
        }
        game.at_war.insert((0, 1));
        let targets: Vec<Pos> = game
            .nbrs(encampment)
            .into_iter()
            .filter(|position| {
                *position != encampment
                    && game.city_at(*position).is_none()
                    && game.encampment_at(*position).is_none()
                    && game.units_at(*position).is_empty()
            })
            .take(2)
            .collect();
        assert_eq!(targets.len(), 2);
        let armor = game.spawn_test_unit("modern_armor", 1, targets[0]);
        let weak = game.spawn_test_unit("warrior", 1, targets[1]);
        game.units.get_mut(&weak).unwrap().hp = 1;
        let armor_static = game.unit_strength(&game.units[&armor], true);
        let weak_static = game.unit_strength(&game.units[&weak], true) + 99.0 * 0.6;
        assert!(
            armor_static > weak_static,
            "the legacy heuristic chose the armor"
        );

        AdvancedAi::targeting(VictoryTarget::Domination).advanced_encampment_strikes(&mut game, 0);

        assert!(game.cities[&city].encampment_struck);
        assert!(game.units.contains_key(&armor));
        assert!(!game.units.contains_key(&weak));
    }

    #[test]
    fn force_replans_focus_after_each_battlefield_action() {
        let mut g = Game::new_full(2, 24, 16, 79, 80, 0, false);
        g.at_war.insert((0, 1));
        let front_candidates = quietest_first(
            &g,
            g.map
                .tiles
                .iter()
                .filter(|(pos, tile)| {
                    g.rules.is_passable(tile)
                        && !g.rules.is_water(tile)
                        && g.units_at(**pos).is_empty()
                })
                .map(|(pos, _)| *pos)
                .collect(),
        );
        let (first_target, second_target, firing_line) = front_candidates
            .into_iter()
            .find_map(|first| {
                g.nbrs(first).into_iter().find_map(|second| {
                    let second_tile = g.map.get(second)?;
                    if !g.rules.is_passable(second_tile)
                        || g.rules.is_water(second_tile)
                        || !g.units_at(second).is_empty()
                    {
                        return None;
                    }
                    let second_neighbors = g.nbrs(second);
                    let common: Vec<Pos> = g
                        .nbrs(first)
                        .into_iter()
                        .filter(|pos| second_neighbors.contains(pos))
                        .filter(|pos| {
                            g.map.get(*pos).is_some_and(|tile| {
                                g.rules.is_passable(tile)
                                    && !g.rules.is_water(tile)
                                    && g.units_at(*pos).is_empty()
                            })
                        })
                        .collect();
                    (common.len() >= 2).then_some((first, second, common))
                })
            })
            .expect("test map has a two-target engagement with a shared front");

        // Level the arena: the test exercises replanning after a kill, and
        // must not hinge on whichever defense modifiers the organic map put
        // under the four staged tiles.
        for position in [first_target, second_target, firing_line[0], firing_line[1]] {
            let tile = g.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("plains");
            tile.feature = None;
            tile.hills = false;
        }

        let attackers = [
            g.spawn_test_unit("warrior", 0, firing_line[0]),
            g.spawn_test_unit("warrior", 0, firing_line[1]),
        ];
        let first_enemy = g.spawn_test_unit("warrior", 1, first_target);
        g.units.get_mut(&first_enemy).unwrap().hp = 1;
        let second_enemy = g.spawn_test_unit("warrior", 1, second_target);
        g.units.get_mut(&second_enemy).unwrap().hp = 1;
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: None,
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: g.turn,
            rush: false,
        };
        let mut ai = AdvancedAi::new();
        ai.rebuild_force_groups(&g, 0, &plan);
        let initial = ai
            .force_groups()
            .iter()
            .find(|group| attackers.iter().all(|uid| group.units.contains(uid)))
            .unwrap();
        assert_eq!(initial.focus_target, Some(first_target));

        assert!(ai.advanced_military_step(&mut g, 0, attackers[0], &plan));
        assert!(!g.units.contains_key(&first_enemy));
        assert!(ai.advanced_military_step(&mut g, 0, attackers[1], &plan));
        let replanned = ai
            .force_groups()
            .iter()
            .find(|group| group.units.contains(&attackers[1]))
            .unwrap();
        assert_eq!(replanned.focus_target, Some(second_target));
        assert!(
            matches!(
                g.log.last(),
                Some((0, Action::Attack { unit, target }))
                    if *unit == attackers[1] && *target == second_target
            ),
            "unexpected replanned action log: {:?}",
            g.log
        );
    }

    #[test]
    fn advanced_ai_votes_in_special_sessions_and_liberates_emergency_objectives() {
        let mut vote_game = Game::new_full(3, 26, 16, 73_001, 120, 0, false);
        for player in 0..3 {
            let settler = vote_game
                .player_unit_ids(player)
                .into_iter()
                .find(|unit| vote_game.units[unit].kind == "settler")
                .unwrap();
            vote_game.found_city_for(player, vote_game.units[&settler].pos, None);
        }
        let objective = vote_game.player_city_ids(0)[0];
        vote_game.pending_emergencies = vec![crate::game::EmergencyProposal {
            id: 77,
            kind: "city_state".to_string(),
            target: 0,
            city: objective,
            original_owner: 1,
            eligible: [2].into_iter().collect(),
            requested: vote_game.turn,
        }];
        vote_game.congress = Some(crate::game::CongressSession {
            convened: vote_game.turn,
            closes: vote_game.turn + 5,
            resolutions: vec![CongressResolution {
                id: "emergency:77".to_string(),
                title: "City-State Emergency".to_string(),
                choices: vec!["A:support".to_string(), "B:oppose".to_string()],
                ballots: BTreeMap::new(),
            }],
        });
        vote_game.current = 2;
        let plan = StrategicPlan {
            strategy: GrandStrategy::Science,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: vote_game.turn,
            rush: false,
        };
        let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
        ai.advanced_diplomacy(&mut vote_game, 2, &plan);
        assert_eq!(
            vote_game.congress.as_ref().unwrap().resolutions[0].ballots[&2].0,
            "A:support"
        );
        assert_eq!(
            ai.congress_choice(
                &vote_game,
                0,
                &vote_game.congress.as_ref().unwrap().resolutions[0],
                GrandStrategy::Conquest,
            ),
            Some("B:oppose".to_string())
        );

        let mut conquest = Game::new_full(3, 26, 16, 73_002, 120, 0, false);
        for player in 0..3 {
            let settler = conquest
                .player_unit_ids(player)
                .into_iter()
                .find(|unit| conquest.units[unit].kind == "settler")
                .unwrap();
            conquest.found_city_for(player, conquest.units[&settler].pos, None);
        }
        let objective = conquest.player_city_ids(1)[0];
        {
            let city = conquest.cities.get_mut(&objective).unwrap();
            city.owner = 0;
            city.captured_from = None;
            city.occupied_from = Some(1);
        }
        conquest.active_emergencies = vec![crate::game::Emergency {
            id: 78,
            kind: "military".to_string(),
            target: 0,
            city: objective,
            original_owner: 1,
            members: [2].into_iter().collect(),
            contributions: BTreeMap::new(),
            started: conquest.turn,
            ends: conquest.turn + 30,
        }];
        conquest.current = 2;
        let emergency_plan = ai.assess(&conquest, 2);
        assert_eq!(emergency_plan.target_player, Some(0));
        assert_eq!(emergency_plan.target_city, Some(objective));
        {
            let city = conquest.cities.get_mut(&objective).unwrap();
            city.owner = 2;
            city.captured_from = Some(0);
            city.occupied_from = Some(0);
        }
        ai.resolve_city_dispositions(&mut conquest, 2, GrandStrategy::Science);
        assert_eq!(conquest.cities[&objective].owner, 1);
        assert!(conquest.active_emergencies.is_empty());
    }

    #[test]
    fn advanced_selfplay_completes() {
        let mut g = Game::new(2, 20, 14, 73, 65, 1);
        let mut ais = AdvancedAi::fleet(&g);
        run_game(&mut g, &mut ais);
        assert!(g.winner.is_some());
        assert!(g
            .players
            .iter()
            .filter(|p| !p.is_minor && p.alive)
            .all(|p| p.techs.len() > 1));
        // Settlers lost to Barbarians or captured legitimately break any
        // lifetime produced-vs-founded accounting. Guard the behavior this
        // test actually cares about instead: the production gate only queues a
        // Settler while the player holds none, so a player must never end the
        // game sitting on a backlog of idle Settlers.
        for player in g.players.iter().filter(|p| !p.is_minor && p.alive) {
            let idle = g
                .units
                .values()
                .filter(|u| u.owner == player.id && u.kind == "settler")
                .count();
            assert!(
                idle <= 1,
                "advanced AI accumulated idle Settlers: player {} holds {idle}",
                player.id
            );
        }
    }

    #[test]
    fn disposition_search_liberates_for_diplomacy_but_keeps_a_developed_conquest_prize() {
        let captured_city_state = |seed| {
            let mut game = Game::new_full(3, 26, 16, seed, 80, 1, false);
            let minor = game
                .players
                .iter()
                .find(|player| player.is_minor && !player.is_barbarian)
                .unwrap()
                .id;
            let city = game.player_city_ids(minor)[0];
            let captured = game.cities.get_mut(&city).unwrap();
            captured.owner = 0;
            captured.captured_from = Some(1);
            captured.loyalty = 50.0;
            (game, minor, city)
        };

        let (mut diplomatic, minor, city) = captured_city_state(106);
        let mut ai = AdvancedAi::targeting(VictoryTarget::Diplomacy);
        ai.resolve_city_dispositions(&mut diplomatic, 0, GrandStrategy::Diplomacy);
        assert_eq!(diplomatic.cities[&city].owner, minor);
        assert_eq!(diplomatic.players[0].diplomatic_favor, 100.0);

        let (mut conquest, _, city) = captured_city_state(107);
        conquest.cities.get_mut(&city).unwrap().pop = 10;
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        ai.resolve_city_dispositions(&mut conquest, 0, GrandStrategy::Conquest);
        assert_eq!(conquest.cities[&city].owner, 0);
        assert_eq!(conquest.cities[&city].captured_from, None);
    }

    #[test]
    fn conquest_razes_a_hopeless_isolated_city_instead_of_recapturing_it() {
        let mut game = Game::new_full(2, 30, 18, 107_002, 120, 0, false);
        for pid in 0..2 {
            game.current = pid;
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.apply(pid, &Action::FoundCity { unit: settler })
                .unwrap();
        }
        let home = game.player_city_ids(0)[0];
        let rival_capital = game.player_city_ids(1)[0];
        let rival_pos = game.cities[&rival_capital].pos;
        let outpost_pos = game
            .wdisk(rival_pos, 4)
            .into_iter()
            .find(|position| {
                game.wdist(*position, rival_pos) == 4
                    && game.wdist(*position, game.cities[&home].pos) > 9
                    && game.city_at(*position).is_none()
                    && game.map.tiles[position].owner_city.is_none()
            })
            .expect("test map has an isolated rival outpost site");
        {
            let tile = game.map.tiles.get_mut(&outpost_pos).unwrap();
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
        }
        game.cities.get_mut(&rival_capital).unwrap().pop = 15;
        game.cities.get_mut(&home).unwrap().pop = 3;
        let outpost = game.found_city_for(1, outpost_pos, Some("Revolt Loop".to_string()));
        {
            let captured = game.cities.get_mut(&outpost).unwrap();
            captured.owner = 0;
            captured.pop = 3;
            captured.loyalty = 50.0;
            captured.captured_from = Some(1);
            captured.occupied_from = Some(1);
        }
        game.current = 0;
        assert!(game
            .legal_city_disposition_actions(0)
            .iter()
            .any(|action| matches!(action, Action::RazeCity { city } if *city == outpost)));

        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        assert!(AdvancedAi::population_loyalty_delta(&game, 0, outpost) <= -8.0);

        // A nearby core is not enough to save a tiny conquest that is already
        // forecast to revolt in three or four turns. The live Alexandria loop
        // was one population, six tiles from China, and changed hands five
        // times in seventeen turns because the older rule treated distance as
        // an absolute exemption.
        let mut nearby = game.clone();
        let near_home_pos = nearby
            .wdisk(outpost_pos, 6)
            .into_iter()
            .find(|position| {
                nearby.wdist(*position, outpost_pos) == 6
                    && nearby.city_at(*position).is_none()
                    && nearby.map.tiles[position].owner_city.is_none()
                    && nearby.rules.is_passable(&nearby.map.tiles[position])
                    && !nearby.rules.is_water(&nearby.map.tiles[position])
            })
            .expect("test map has a core site six tiles from the captured outpost");
        nearby.cities.get_mut(&home).unwrap().pos = near_home_pos;
        assert_eq!(
            nearby
                .cities
                .values()
                .filter(|city| city.owner == 0 && city.id != outpost)
                .map(|city| nearby.wdist(city.pos, outpost_pos))
                .min(),
            Some(6)
        );
        assert!(AdvancedAi::population_loyalty_delta(&nearby, 0, outpost) <= -8.0);
        let mut nearby_ai = AdvancedAi::targeting(VictoryTarget::Domination);
        nearby_ai.resolve_city_dispositions(&mut nearby, 0, GrandStrategy::Conquest);
        assert!(
            !nearby.cities.contains_key(&outpost),
            "an imminent low-value revolt should be razed even near a core city"
        );

        ai.resolve_city_dispositions(&mut game, 0, GrandStrategy::Conquest);

        assert!(
            !game.cities.contains_key(&outpost),
            "a city forecast to revolt before support can arrive should be razed once"
        );
    }

    #[test]
    fn adaptive_turn_uses_live_victory_focus_for_mandatory_city_disposition() {
        let mut game = Game::new_full(2, 24, 16, 107_001, 80, 1, false);
        let minor = game
            .players
            .iter()
            .find(|player| player.is_minor && !player.is_barbarian)
            .unwrap()
            .id;
        let city = game.player_city_ids(minor)[0];
        {
            let captured = game.cities.get_mut(&city).unwrap();
            captured.owner = 0;
            captured.captured_from = Some(1);
            captured.occupied_from = Some(1);
            captured.loyalty = 50.0;
        }
        game.players[0].dvp = 19;
        let mut ai = AdvancedAi::new();
        assert_eq!(
            ai.victory_focus(&game, 0).strategy,
            GrandStrategy::Diplomacy
        );

        ai.take_turn(&mut game, 0);

        assert_eq!(game.cities[&city].owner, minor);
        assert_eq!(game.players[0].diplomatic_favor, 100.0);
    }

    #[test]
    fn occupation_reserves_a_reachable_garrison_during_war() {
        let mut game = Game::new_full(3, 26, 16, 108, 80, 1, false);
        let city = game
            .players
            .iter()
            .find(|player| player.is_minor && !player.is_barbarian)
            .and_then(|minor| game.player_city_ids(minor.id).first().copied())
            .unwrap();
        {
            let occupied = game.cities.get_mut(&city).unwrap();
            occupied.owner = 0;
            occupied.captured_from = None;
            occupied.occupied_from = Some(1);
            occupied.loyalty = 35.0;
        }
        for unit in game.player_unit_ids(0) {
            game.remove_unit(unit);
        }
        let city_pos = game.cities[&city].pos;
        for unit in game.units_at(city_pos) {
            game.remove_unit(unit);
        }
        let start =
            game.nbrs(city_pos)
                .into_iter()
                .find(|position| {
                    game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    }) && game.units_at(*position).is_empty()
                })
                .unwrap();
        let warrior = game.spawn_test_unit("warrior", 0, start);
        game.at_war.insert((0, 1));
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: game.player_city_ids(1).first().copied(),
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: game.turn,
            rush: false,
        };
        let mut ai = AdvancedAi::new();

        assert_eq!(
            ai.occupation_garrison_target(&game, 0, warrior),
            Some(city_pos)
        );
        assert!(ai.advanced_military_step(&mut game, 0, warrior, &plan));
        assert_eq!(game.units[&warrior].pos, city_pos);
    }

    #[test]
    fn a_spy_posted_to_a_razed_city_does_not_bring_the_server_down() {
        let mut game = Game::new_full(2, 24, 16, 109, 120, 0, false);
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
        let spy = game.next_id;
        game.next_id += 1;
        game.spies.insert(
            spy,
            crate::game::Spy {
                id: spy,
                owner: 0,
                level: 1,
                promotions: Default::default(),
                city: Some(cities[0]),
                ready_turn: game.turn,
                mission: None,
                sources_city: None,
                sources_until: 0,
                captured_by: None,
            },
        );
        // The city the agent is posted to is razed out from under it, which is
        // an ordinary wartime event. Indexing the city map for it panicked the
        // AI thread, and that poisoned the game mutex so every later HTTP
        // request died too: one razed city took the whole exhibition offline.
        game.cities.remove(&cities[0]);

        let plan = StrategicPlan {
            strategy: GrandStrategy::Science,
            target_player: Some(1),
            target_city: Some(cities[1]),
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };
        AdvancedAi::targeting(VictoryTarget::Science).advanced_spies(&mut game, 0, &plan);
        assert!(game.spies.contains_key(&spy), "the agent survives the raze");
    }

    #[test]
    fn science_strategy_uses_an_established_spy_to_steal_a_rival_technology() {
        let mut game = Game::new_full(2, 24, 16, 109, 120, 0, false);
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
        let campus = game.cities[&target]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != game.cities[&target].pos)
            .unwrap();
        game.map.tiles.get_mut(&campus).unwrap().district = Some(crate::name!("campus"));
        game.cities
            .get_mut(&target)
            .unwrap()
            .districts
            .insert(crate::name!("campus"), campus);
        game.players[1].techs.insert(crate::name!("writing"));
        let spy = game.next_id;
        game.next_id += 1;
        game.spies.insert(
            spy,
            crate::game::Spy {
                id: spy,
                owner: 0,
                level: 2,
                promotions: ["technologist".to_string(), "disguise".to_string()]
                    .into_iter()
                    .collect(),
                city: Some(target),
                ready_turn: game.turn,
                mission: None,
                sources_city: Some(target),
                sources_until: game.turn + 24,
                captured_by: None,
            },
        );
        let plan = StrategicPlan {
            strategy: GrandStrategy::Science,
            target_player: Some(1),
            target_city: Some(target),
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };

        AdvancedAi::targeting(VictoryTarget::Science).advanced_spies(&mut game, 0, &plan);
        assert_eq!(
            game.spies[&spy]
                .mission
                .as_ref()
                .map(|mission| mission.kind.as_str()),
            Some("steal_tech_boost")
        );
    }

    #[test]
    fn basil_stages_hippodromes_then_resumes_them_at_divine_right() {
        let mut game = Game::new_full(
            2, 24, 16, crate::rng::fixture_seed("BASIL", 111), 200, 0, false,
        );
        game.players[0].civ = "Byzantium".to_string();
        game.players[0].religion = Some("Eastern Orthodoxy".to_string());
        game.players[0].civics.insert(crate::name!("games_recreation"));
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        let city = game.found_city_for(0, game.units[&settler].pos, None);
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: None,
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: game.turn,
            rush: false,
        };
        let ai = AdvancedAi::new();
        ai.byzantium_tagma_production(&mut game, 0, &plan);
        assert!(matches!(
            game.cities[&city].queue.first(),
            Some(Item::District { district, .. }) if district == "hippodrome"
        ));

        let cost = game.rules.districts["hippodrome"].cost;
        game.cities.get_mut(&city).unwrap().production = cost - 0.5;
        ai.byzantium_tagma_production(&mut game, 0, &plan);
        assert!(
            !matches!(
                game.cities[&city].queue.first(),
                Some(Item::District { district, .. }) if district == "hippodrome"
            ),
            "the one-turn Hippodrome must be banked until the Tagma civic"
        );

        game.players[0].civics.insert(crate::name!("divine_right"));
        ai.byzantium_tagma_production(&mut game, 0, &plan);
        assert!(matches!(
            game.cities[&city].queue.first(),
            Some(Item::District { district, .. }) if district == "hippodrome"
        ));
    }

    #[test]
    fn a_religious_basil_commits_to_conquest_and_divine_right() {
        let mut game = Game::new_full(2, 24, 16, 111, 200, 0, false);
        game.players[0].civ = "Byzantium".to_string();
        game.players[0].religion = Some("Eastern Orthodoxy".to_string());
        let ai = AdvancedAi::new();
        let plan = ai.assess(&game, 0);
        assert_eq!(plan.strategy, GrandStrategy::Conquest);
        assert_eq!(plan.target_player, Some(1));

        ai.advanced_research(&mut game, 0, &plan);
        let civic = game.players[0].civic.as_deref().unwrap();
        assert!(
            ai.civic_leads_to(&game, civic, "divine_right"),
            "{civic} should be on the direct Divine Right beeline"
        );
    }

    /// Fires-check for `city_strategy`. The control must stamp nothing at all,
    /// and the treatment must stamp exactly one directive per owned city
    /// carrying the lane in force. A treatment that changes no state cannot
    /// change a decision, so this is the screen that has to pass before any
    /// eval is worth its compute.
    #[test]
    fn city_strategy_stamps_one_directive_per_city_and_the_control_stamps_none() {
        let mut game = Game::new_full(2, 24, 16, 411_001, 120, 1, false);
        found_test_city(&mut game, 0);
        found_test_city(&mut game, 0);
        let cities = game.player_city_ids(0);
        assert!(cities.len() >= 2, "need a multi-city seat to type roles");

        let mut control = AdvancedAi::new();
        control.take_turn(&mut game.clone(), 0);
        assert!(
            game.players[0].city_directives.is_empty(),
            "the shipped agent must not express a directive"
        );

        let mut treated = AdvancedAi::new();
        treated.city_strategy = true;
        treated.take_turn(&mut game, 0);

        let stamped = &game.players[0].city_directives;
        assert_eq!(
            stamped.len(),
            game.player_city_ids(0).len(),
            "every owned city gets exactly one directive"
        );
        for cid in game.player_city_ids(0) {
            assert!(stamped.contains_key(&cid), "city {cid} was left unstamped");
        }
    }

    /// The whole point of the channel: the same city works different tiles
    /// under different empire objectives. If a Science lane and a Culture lane
    /// produce identical governor weights the directive is decorative.
    #[test]
    fn the_lane_reaches_the_tile_the_citizen_works() {
        let science = AdvancedAi::lane_emphasis(GrandStrategy::Science);
        let culture = AdvancedAi::lane_emphasis(GrandStrategy::Culture);
        let conquest = AdvancedAi::lane_emphasis(GrandStrategy::Conquest);
        assert!(science.science > culture.science);
        assert!(culture.culture > science.culture);
        assert!(conquest.production > science.production);

        let mut game = Game::new_full(2, 24, 16, 411_002, 120, 1, false);
        let city = found_test_city(&mut game, 0);
        let shipped = game.citizen_strategy(city);

        game.players[0].city_directives.insert(
            city,
            CityDirective {
                emphasis: science,
                role: CityRole::Balanced,
                pressure: 0.0,
                halt_growth: false,
            },
        );
        let under_science = game.citizen_strategy(city);
        assert!(
            under_science.weights.science > shipped.weights.science,
            "a science empire must want science tiles more than the default does"
        );
        // Additive, not a takeover: everything the local evidence said is
        // still there underneath.
        assert_eq!(under_science.weights.food, shipped.weights.food);
        assert_eq!(under_science.weights.culture, shipped.weights.culture);

        game.players[0].city_directives.insert(
            city,
            CityDirective {
                emphasis: culture,
                role: CityRole::Balanced,
                pressure: 0.0,
                halt_growth: false,
            },
        );
        let under_culture = game.citizen_strategy(city);
        assert!(under_culture.weights.culture > under_science.weights.culture);
        assert!(under_culture.weights.science < under_science.weights.science);
    }

    /// Military awareness is per city, not per empire. A Bastion wants hammers
    /// and refuses to grow into a siege; local pressure raises the hammer
    /// appetite on its own axis, so a city being approached reacts before the
    /// empire-wide alarm names it.
    #[test]
    fn a_pressed_city_wants_hammers_and_a_bastion_stops_growing() {
        let mut game = Game::new_full(2, 24, 16, 411_003, 120, 1, false);
        let city = found_test_city(&mut game, 0);
        let calm = game.citizen_strategy(city);

        game.players[0].city_directives.insert(
            city,
            CityDirective {
                emphasis: Yields::default(),
                role: CityRole::Balanced,
                pressure: 0.6,
                halt_growth: false,
            },
        );
        let approached = game.citizen_strategy(city);
        assert!(
            approached.weights.production > calm.weights.production,
            "a city with a hostile force in reach must want production"
        );
        assert_eq!(
            approached.food_target, calm.food_target,
            "pressure alone raises hammers; it does not halt growth"
        );

        game.players[0].city_directives.insert(
            city,
            CityDirective {
                emphasis: Yields::default(),
                role: CityRole::Bastion,
                pressure: 0.6,
                halt_growth: false,
            },
        );
        let besieged = game.citizen_strategy(city);
        assert!(besieged.weights.production > approached.weights.production);
        // ⚠ A Bastion wants hammers and KEEPS GROWING. The opposite -- cutting
        // its food weight and refusing a growth surplus -- reads as obviously
        // correct and is the single mechanism the per-rung bisect convicted:
        // isolated over 120 paired maps it scores 43.8%, Elo -44 at p=0.0107,
        // and costs a fifth of the empire's cities. A besieged city that stops
        // growing does not become safer, only permanently smaller.
        assert_eq!(
            besieged.food_target, calm.food_target,
            "a bastion must not trade away its own growth"
        );
        assert_eq!(besieged.weights.food, calm.weights.food);

        game.players[0].city_directives.insert(
            city,
            CityDirective {
                emphasis: Yields::default(),
                role: CityRole::Bastion,
                pressure: 0.6,
                halt_growth: true,
            },
        );
        let halted = game.citizen_strategy(city);
        assert!(halted.weights.food < calm.weights.food);
        assert_eq!(
            halted.food_target,
            2.0 * game.cities[&city].pop as f64,
            "the frozen control still expresses the measured-worse stance"
        );
        // Nutrition is never traded away, even under the losing stance.
        assert!(halted.food_target >= 2.0 * game.cities[&city].pop as f64);
    }

    /// `city_pressure` is the number `threatened_city` already computed and
    /// discarded. It must read zero in peace, so a directive stamped every
    /// turn of a quiet game costs the governor nothing.
    #[test]
    fn city_pressure_is_zero_with_no_hostile_force_in_reach() {
        let mut game = Game::new_full(2, 24, 16, 411_004, 120, 1, false);
        found_test_city(&mut game, 0);
        for cid in game.player_city_ids(0) {
            assert_eq!(AdvancedAi::city_pressure(&game, 0, cid), 0.0);
        }
    }

    /// Census, not an assertion: how often each rung of the role ladder fires
    /// over real games, and what share of Bastion stamps are driven by
    /// barbarians alone.
    ///
    /// The roles half of `city_strategy` lost 120 maps at Elo -55 while the
    /// emphasis half was a clean null, and the empire it produced was smaller
    /// in every column (cities 2.14 against 2.77). Two candidate mechanisms
    /// survive that: Bastion halting growth on barbarian contact, or
    /// Forge/Specialist pulling citizens off food in empires too small for
    /// `ROLE_MARGIN` to type meaningfully. Guessing between them once already
    /// cost a wrong hypothesis, so this counts instead.
    ///
    /// Run with `cargo test --release role_ladder_census -- --nocapture`.
    #[test]
    #[ignore = "census, not an assertion; run explicitly with --nocapture"]
    fn role_ladder_census() {
        let mut totals = BTreeMap::<&str, u64>::new();
        let mut lanes = BTreeMap::<&str, u64>::new();
        let mut barbarian_only_bastions = 0u64;
        let mut pressured = 0u64;
        let mut pressure_sum = 0.0f64;
        let mut extra_hammers = 0.0f64;
        // Per-turn means hide minority behaviour, and the settler window is a
        // minority of the game. Band it.
        let mut band_turns = [0u64; 4];
        let mut band_bastions = [0u64; 4];
        let mut city_turns = 0u64;
        let mut halted_pop = 0u64;

        for map in 0..8u64 {
            let mut game = Game::new_full(4, 24, 16, 420_000 + map, 200, 1, false);
            let mut ais: Vec<AdvancedAi> = (0..game.players.len())
                .map(|_| {
                    let mut ai = AdvancedAi::new();
                    ai.city_strategy = true;
                    ai.city_strategy_emphasis = false;
                    ai
                })
                .collect();
            game.set_fog_memory(false);
            while game.winner.is_none() && game.turn <= game.max_turns {
                let pid = game.current;
                ais[pid].take_turn(&mut game, pid);
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &Action::EndTurn);
                }
                if pid != 0 {
                    continue;
                }
                if let Some(plan) = ais[0].plan.as_ref() {
                    *lanes.entry(plan.strategy.as_str()).or_default() += 1;
                }
                let directives = game.players[0].city_directives.clone();
                for (cid, directive) in &directives {
                    let Some(city) = game.cities.get(cid) else {
                        continue;
                    };
                    city_turns += 1;
                    *totals.entry(directive.role.as_str()).or_default() += 1;
                    let band = match game.turn {
                        0..=39 => 0,
                        40..=79 => 1,
                        80..=139 => 2,
                        _ => 3,
                    };
                    band_turns[band] += 1;
                    if directive.role == CityRole::Bastion {
                        band_bastions[band] += 1;
                    }
                    if directive.pressure > 0.0 {
                        pressured += 1;
                        pressure_sum += directive.pressure;
                        extra_hammers += directive.pressure.min(2.0) * 0.80;
                    }
                    if directive.role != CityRole::Bastion {
                        continue;
                    }
                    halted_pop += city.pop.max(0) as u64;
                    // Recompute the same ratio with barbarians removed. If it
                    // falls under the threshold, the only thing that stamped
                    // this city a Bastion was a barbarian.
                    let major: f64 = game
                        .units
                        .values()
                        .filter(|unit| {
                            unit.owner != 0
                                && !game.players[unit.owner].is_barbarian
                                && game.is_at_war(0, unit.owner)
                        })
                        .filter(|unit| game.wdist(city.pos, unit.pos) <= 6)
                        .filter(|unit| game.rules.units[unit.kind].class == "military")
                        .map(|unit| {
                            crate::game::effective_strength(game.unit_strength(unit, false), unit.hp)
                        })
                        .sum();
                    let friendly = game.city_strength(*cid)
                        + game
                            .units
                            .values()
                            .filter(|unit| unit.owner == 0)
                            .filter(|unit| game.wdist(city.pos, unit.pos) <= 6)
                            .filter(|unit| game.rules.units[unit.kind].class == "military")
                            .map(|unit| {
                                crate::game::effective_strength(
                                    game.unit_strength(unit, true),
                                    unit.hp,
                                )
                            })
                            .sum::<f64>();
                    if major / friendly.max(1.0) < BASTION_PRESSURE
                        && ais[0].plan.as_ref().and_then(|p| p.threatened_city) != Some(*cid)
                    {
                        barbarian_only_bastions += 1;
                    }
                }
            }
        }

        let plan_turns: u64 = lanes.values().sum();
        println!("\n=== grand strategy in force, {plan_turns} seat-turns ===");
        for (lane, count) in &lanes {
            println!(
                "  {lane:<12} {count:>6}  ({:>5.1}%)",
                *count as f64 / plan_turns.max(1) as f64 * 100.0
            );
        }
        println!("\n=== role ladder census: {city_turns} city-turns over 8 maps ===");
        for (role, count) in &totals {
            println!(
                "  {role:<12} {count:>6}  ({:>5.1}%)",
                *count as f64 / city_turns.max(1) as f64 * 100.0
            );
        }
        println!(
            "  pressure > 0 on {pressured} of {city_turns} city-turns ({:.1}%), mean {:.2} when it fires",
            pressured as f64 / city_turns.max(1) as f64 * 100.0,
            pressure_sum / pressured.max(1) as f64
        );
        println!(
            "  mean production weight added by pressure across ALL city-turns: +{:.3} (shipped weight is 1.55)",
            extra_hammers / city_turns.max(1) as f64
        );
        for (band, label) in ["t1-39", "t40-79", "t80-139", "t140+"].iter().enumerate() {
            println!(
                "  {label:<8} bastion on {:>4} of {:>5} city-turns ({:>5.1}%)",
                band_bastions[band],
                band_turns[band],
                band_bastions[band] as f64 / band_turns[band].max(1) as f64 * 100.0
            );
        }
        let bastions = totals.get("bastion").copied().unwrap_or(0);
        println!(
            "  of {bastions} bastion stamps, {barbarian_only_bastions} were BARBARIAN-ONLY ({:.1}%)",
            barbarian_only_bastions as f64 / bastions.max(1) as f64 * 100.0
        );
        println!(
            "  population under a growth halt: {halted_pop} citizen-turns ({:.1}% of all city-turns were halted)\n",
            bastions as f64 / city_turns.max(1) as f64 * 100.0
        );
    }

    /// Census, not an assertion: how often a city faces a locally competitive
    /// hostile force that `production_value`'s `threatened` flag cannot see.
    ///
    /// `threatened` is true only when the city is *the* empire's single worst
    /// (`plan.threatened_city` returns one `Option<u32>`) or when it was hit in
    /// the last four turns. `threatened_city` additionally gates on
    /// `danger >= 0.90`, or `>= 0.45` with a breach or a recent hit against a
    /// damaged city. So a second city under siege, or any city with an army
    /// standing next to it that has not struck yet, prioritises no defenses at
    /// all -- which is exactly what `early_rush`'s doc comment describes as the
    /// timing rule that lane plays against.
    ///
    /// Run with `cargo test --release blind_defense_census -- --ignored --nocapture`.
    #[test]
    #[ignore = "census, not an assertion; run explicitly with --nocapture"]
    fn blind_defense_census() {
        let mut city_turns = 0u64;
        let mut pressed = 0u64;
        let mut pressed_and_blind = 0u64;
        let mut blind_by_band = [0u64; 4];
        let mut band_pressed = [0u64; 4];

        for map in 0..8u64 {
            let mut game = Game::new_full(4, 24, 16, 430_000 + map, 200, 1, false);
            let mut ais: Vec<AdvancedAi> =
                (0..game.players.len()).map(|_| AdvancedAi::new()).collect();
            game.set_fog_memory(false);
            while game.winner.is_none() && game.turn <= game.max_turns {
                let pid = game.current;
                ais[pid].take_turn(&mut game, pid);
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &Action::EndTurn);
                }
                if pid != 0 {
                    continue;
                }
                let Some(plan) = ais[0].plan.clone() else {
                    continue;
                };
                let band = match game.turn {
                    0..=39 => 0,
                    40..=79 => 1,
                    80..=139 => 2,
                    _ => 3,
                };
                for cid in game.player_city_ids(0) {
                    city_turns += 1;
                    let pressure = AdvancedAi::city_pressure(&game, 0, cid);
                    if pressure < BASTION_PRESSURE {
                        continue;
                    }
                    pressed += 1;
                    band_pressed[band] += 1;
                    let city = &game.cities[&cid];
                    let threatened = plan.threatened_city == Some(cid)
                        || (city.last_attacked > 0
                            && game.turn.saturating_sub(city.last_attacked) <= 4);
                    if !threatened {
                        pressed_and_blind += 1;
                        blind_by_band[band] += 1;
                    }
                }
            }
        }

        println!("\n=== blind-defense census: {city_turns} city-turns over 8 maps ===");
        println!(
            "  locally competitive hostile force present on {pressed} city-turns ({:.1}%)",
            pressed as f64 / city_turns.max(1) as f64 * 100.0
        );
        println!(
            "  of those, {pressed_and_blind} were INVISIBLE to `threatened` ({:.1}%)",
            pressed_and_blind as f64 / pressed.max(1) as f64 * 100.0
        );
        for (band, label) in ["t1-39", "t40-79", "t80-139", "t140+"].iter().enumerate() {
            println!(
                "  {label:<8} {:>4} pressed, {:>4} blind ({:>5.1}%)",
                band_pressed[band],
                blind_by_band[band],
                blind_by_band[band] as f64 / band_pressed[band].max(1) as f64 * 100.0
            );
        }
        println!();
    }

    /// Census, not an assertion: what a city actually decides, turn by turn.
    ///
    /// Five arms of scripted "give the city a strategy" treatment have now
    /// measured null across three seeds, so this stops proposing tilts to the
    /// city's decisions and asks a different question: are any of them plainly
    /// *broken*? That is the question that paid last time -- the growth halt
    /// was a real -38 Elo defect and no amount of reasoning about weights
    /// found it, a census did.
    ///
    /// Run with `cargo test --release city_decision_census -- --ignored --nocapture`.
    #[test]
    #[ignore = "census, not an assertion; run explicitly with --nocapture"]
    fn city_decision_census() {
        let mut city_turns = 0u64;
        let mut idle = 0u64;
        let mut head_changes = 0u64;
        let mut stranded = 0.0f64;
        let mut kinds = BTreeMap::<&str, u64>::new();
        let mut walls_built = 0u64;
        let mut cities_seen = BTreeSet::<(u64, u32)>::new();
        let mut walled = BTreeSet::<(u64, u32)>::new();
        let mut first_wall_turn = Vec::<u32>::new();
        let mut food_now = 0.0f64;
        let mut food_ceiling = 0.0f64;
        let mut prod_now = 0.0f64;
        let mut prod_ceiling = 0.0f64;
        let mut prev_head = BTreeMap::<u32, String>::new();

        for map in 0..8u64 {
            let mut game = Game::new_full(4, 24, 16, 440_000 + map, 200, 1, false);
            let mut ais: Vec<AdvancedAi> =
                (0..game.players.len()).map(|_| AdvancedAi::new()).collect();
            game.set_fog_memory(false);
            prev_head.clear();
            while game.winner.is_none() && game.turn <= game.max_turns {
                let pid = game.current;
                ais[pid].take_turn(&mut game, pid);
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &Action::EndTurn);
                }
                if pid != 0 {
                    continue;
                }
                for cid in game.player_city_ids(0) {
                    city_turns += 1;
                    cities_seen.insert((map, cid));
                    let city = &game.cities[&cid];
                    match city.queue.first() {
                        None => idle += 1,
                        Some(item) => {
                            let key = match item {
                                Item::Unit { .. } | Item::Formation { .. } => "unit",
                                Item::Building { .. } => "building",
                                Item::District { .. } => "district",
                                Item::Wonder { .. } => "wonder",
                                Item::Project { .. } => "project",
                                Item::Product { .. } => "product",
                                Item::Repair { .. } => "repair",
                            };
                            *kinds.entry(key).or_default() += 1;
                            let head = format!("{item:?}");
                            if prev_head.get(&cid).is_some_and(|old| *old != head) {
                                head_changes += 1;
                            }
                            prev_head.insert(cid, head);
                        }
                    }
                    // Production banked against builds the city walked away
                    // from. `item_remaining_cost_for_city` is what the AI
                    // prices a resumed build with, so progress that is not on
                    // the queue head is capital sitting idle.
                    stranded += city.production_progress.values().sum::<f64>();
                    if city.buildings.iter().any(|b| b.contains("walls")) {
                        if walled.insert((map, cid)) {
                            walls_built += 1;
                            first_wall_turn.push(game.turn);
                        }
                    }
                    // How much of the city's own ceiling do the shipped
                    // weights actually claim? `city_yields_weighted` is the
                    // public substitution instrument `docs/OPENINGS.md` uses to
                    // bound the capital's food ceiling; asking it per city-turn
                    // turns that one-off into a standing measurement.
                    let now = game.city_yields(cid);
                    let all_food = Yields {
                        food: 10.0,
                        ..Yields::default()
                    };
                    let all_prod = Yields {
                        production: 10.0,
                        ..Yields::default()
                    };
                    food_now += now.food;
                    food_ceiling += game.city_yields_weighted(cid, all_food).food;
                    prod_now += now.production;
                    prod_ceiling += game.city_yields_weighted(cid, all_prod).production;
                }
            }
        }

        let pct = |n: u64| n as f64 / city_turns.max(1) as f64 * 100.0;
        println!("\n=== city decision census: {city_turns} city-turns over 8 maps ===");
        println!("  idle queue          {idle:>6}  ({:>5.1}%)", pct(idle));
        println!(
            "  queue head changed  {head_changes:>6}  ({:>5.1}% of city-turns)",
            pct(head_changes)
        );
        println!(
            "  mean production stranded off the queue head: {:.1} per city-turn",
            stranded / city_turns.max(1) as f64
        );
        println!(
            "  food claimed {:.1}% of the city's own food ceiling ({:.1} of {:.1} per city-turn)",
            food_now / food_ceiling.max(1e-9) * 100.0,
            food_now / city_turns.max(1) as f64,
            food_ceiling / city_turns.max(1) as f64
        );
        println!(
            "  production claimed {:.1}% of its ceiling ({:.1} of {:.1} per city-turn)",
            prod_now / prod_ceiling.max(1e-9) * 100.0,
            prod_now / city_turns.max(1) as f64,
            prod_ceiling / city_turns.max(1) as f64
        );
        println!("  what the queue head is:");
        for (kind, count) in &kinds {
            println!("    {kind:<10} {count:>6}  ({:>5.1}%)", pct(*count));
        }
        println!(
            "  cities that ever built walls: {walls_built} of {} ({:.1}%)",
            cities_seen.len(),
            walls_built as f64 / cities_seen.len().max(1) as f64 * 100.0
        );
        first_wall_turn.sort_unstable();
        if !first_wall_turn.is_empty() {
            println!(
                "  median turn walls appeared: {}",
                first_wall_turn[first_wall_turn.len() / 2]
            );
        }
        println!();
    }

    /// Census, not an assertion: how good are the sites a settler actually
    /// picks, against the best site it could have picked nearby?
    ///
    /// This is the last unmeasured city decision. #532 bounded what a city
    /// works (89.3% of its food ceiling, 99.5% of its production ceiling),
    /// #534 bounded what it owns (perfect border growth, p=0.7283) and #542
    /// bounded where its districts go (p=0.6989). All three are downstream of
    /// *where the city stands*, and nothing had measured that.
    ///
    /// It is measured rather than granted deliberately. An oracle that
    /// teleports settlers cannot work here: `AdvancedAi` founds only when a
    /// settler's cached `settler_targets` entry equals its current position,
    /// so a relocated settler walks back to its old target and never founds.
    /// The grant would fire forever, suppress the seat's expansion entirely,
    /// and report a catastrophic loss that measured the harness rather than
    /// the subsystem. The ratio below asks the same question and cannot lie
    /// in that direction.
    ///
    /// The comparison set is every legal site within eight tiles of the one
    /// chosen — roughly a settler's remaining walk — judged by the agent's own
    /// `settle_value`, with the just-founded city excluded from the minimum
    /// separation rule so the chosen tile competes on equal terms.
    ///
    /// ## ⚠ RETRACTION: the gap this census originally reported was its own bug
    ///
    /// The first two versions of this census (#548, #550) scored the chosen
    /// site **after** the city was founded, and reported that the agent sites
    /// cities at 62-70% of the best available -- billed as the one unsaturated
    /// city decision. That was an artifact.
    ///
    /// `settle_value` skips any tile another city owns
    /// (`if t.owner_city.is_some() && p != pos { continue; }`), and a city
    /// claims its centre and all six ring-one tiles the instant it is founded.
    /// Ring one carries full weight in that sum; ring two is discounted to
    /// 0.45. So founding a city destroys **54.8%** of its own site's measured
    /// value -- `settle_value_before_and_after_founding` measures exactly that
    /// -- while rival candidates four or more tiles away keep theirs intact.
    /// The census was comparing a site stripped of its best tiles against
    /// rivals that still had theirs.
    ///
    /// Scored the turn **before** founding, which is the last moment the site
    /// is still what the settler was choosing between, the answer inverts:
    ///
    /// ```text
    /// against RAW settle_value            3 tiles 99.7%   8 tiles 99.7%
    /// against the AGENT'S OWN objective   99.9%
    /// against its OWN settle_sites here   98.8%
    /// the chosen site WAS the best available on 15 of 17 foundings (88.2%)
    /// worst case 95.2%; capitals 99.0%
    /// ```
    ///
    /// **Settle siting is saturated like every other city decision.** The agent
    /// takes essentially all the value on offer and picks the literal best site
    /// seven times in eight.
    ///
    /// Two things corroborate it rather than resting on this one repair.
    /// `settle_search_reach_census` shows the site search discards no reachable
    /// ground: over 158 settler-turns the generator offered only five
    /// candidates above what the search returned and **none of the five was
    /// reachable**. And a per-turn re-score of stale targets was built and
    /// measured **inert** in #550 -- which is what a saturated decision looks
    /// like from the inside.
    ///
    /// The distance-discount baseline stays because it is still the right one:
    /// `settle_sites` ranks by `settle_value - 0.9 * distance`, so raw
    /// `settle_value` is not the objective the agent pursues.
    ///
    /// Run with `cargo test --release settle_siting_census -- --ignored --nocapture`.
    #[test]
    #[ignore = "census, not an assertion; run explicitly with --nocapture"]
    fn settle_siting_census() {
        let ai = AdvancedAi::new();
        let mut ratios: Vec<f64> = Vec::new();
        let mut near_ratios: Vec<f64> = Vec::new();
        let mut step_away: Vec<i32> = Vec::new();
        let mut discounted_ratios: Vec<f64> = Vec::new();
        let mut offered_ratios: Vec<f64> = Vec::new();
        let mut chosen_was_best = 0u64;
        let mut founded = 0u64;
        let mut capital_ratio: Vec<f64> = Vec::new();

        for map in 0..8u64 {
            let mut game = Game::new_full(4, 24, 16, 470_000 + map, 200, 1, false);
            let mut ais: Vec<AdvancedAi> =
                (0..game.players.len()).map(|_| AdvancedAi::new()).collect();
            game.set_fog_memory(false);
            let mut known: BTreeSet<u32> = game.player_city_ids(0).into_iter().collect();
            while game.winner.is_none() && game.turn <= game.max_turns {
                let pid = game.current;
                // ⚠ Score every tile a settler stands on BEFORE the turn runs.
                // `settle_value` skips tiles another city owns, and a city
                // claims its centre and all six ring-one tiles the instant it
                // is founded -- ring one being the full-weight ring, against
                // 0.45 for ring two. Measured after the fact, founding a city
                // destroys 54.8% of its own site's score
                // (`settle_value_before_and_after_founding`), while rival
                // candidates four or more tiles away keep theirs intact. The
                // first version of this census did exactly that and reported a
                // siting gap that was mostly its own artifact.
                let mut pre: BTreeMap<Pos, f64> = BTreeMap::new();
                if pid == 0 {
                    for uid in game.player_unit_ids(0) {
                        if game.units[&uid].kind == "settler" {
                            let pos = game.units[&uid].pos;
                            pre.insert(pos, ai.settle_value(&game, 0, pos));
                        }
                    }
                }
                ais[pid].take_turn(&mut game, pid);
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &crate::game::Action::EndTurn);
                }
                if pid != 0 {
                    continue;
                }
                for cid in game.player_city_ids(0) {
                    if !known.insert(cid) {
                        continue;
                    }
                    // A city this seat gained by conquest was never sited by
                    // its settler, so it says nothing about this decision.
                    let city = &game.cities[&cid];
                    if city.captured_from.is_some() {
                        continue;
                    }
                    let chosen_pos = city.pos;
                    let Some(chosen) = pre.get(&chosen_pos).copied() else {
                        continue;
                    };
                    // Only ground this seat had actually EXPLORED counts. A
                    // settler cannot choose a site nobody has seen, and
                    // scoring it against one would measure the fog rather than
                    // the decision.
                    let mut best_at = |radius: i32| {
                        game.wdisk(chosen_pos, radius)
                            .into_iter()
                            .filter(|pos| {
                                let Some(tile) = game.map.get(*pos) else {
                                    return false;
                                };
                                if !game.players[0].explored.contains(pos) {
                                    return false;
                                }
                                if game.rules.is_water(tile)
                                    || !game.rules.is_passable(tile)
                                    || game.tile_is_natural_wonder(tile)
                                {
                                    return false;
                                }
                                // Every other city still enforces its
                                // separation; the one just founded must not
                                // veto its own site.
                                game.cities
                                    .values()
                                    .filter(|other| other.id != cid)
                                    .all(|other| game.wdist(other.pos, *pos) >= 4)
                            })
                            .map(|pos| ai.settle_value(&game, 0, pos))
                            .fold(chosen, f64::max)
                    };
                    // How far away is the better site? `settle_value` does
                    // not price the walk, so a gap three tiles off is worth
                    // much less than the same gap one tile off, and the
                    // distance is what makes the number readable.
                    let mut best_near_pos = chosen_pos;
                    let mut best_near_val = chosen;
                    for pos in game.wdisk(chosen_pos, 3) {
                        let Some(tile) = game.map.get(pos) else {
                            continue;
                        };
                        if !game.players[0].explored.contains(&pos)
                            || game.rules.is_water(tile)
                            || !game.rules.is_passable(tile)
                            || game.tile_is_natural_wonder(tile)
                            || !game
                                .cities
                                .values()
                                .filter(|other| other.id != cid)
                                .all(|other| game.wdist(other.pos, pos) >= 4)
                        {
                            continue;
                        }
                        let value = ai.settle_value(&game, 0, pos);
                        if value > best_near_val {
                            best_near_val = value;
                            best_near_pos = pos;
                        }
                    }
                    if best_near_pos != chosen_pos {
                        step_away.push(game.wdist(chosen_pos, best_near_pos));
                    }
                    let near = best_at(3);
                    let best = best_at(8);
                    // ⚠ The comparison above is against RAW `settle_value`,
                    // which is not the objective this agent actually pursues.
                    // `settle_sites` ranks candidates by
                    // `settle_value - 0.9 * distance`, so the agent trades
                    // site quality for proximity on purpose. Scoring it
                    // against a function that does not charge for the walk
                    // measures a preference it never held.
                    let discounted = game
                        .wdisk(chosen_pos, 8)
                        .into_iter()
                        .filter(|pos| {
                            let Some(tile) = game.map.get(*pos) else {
                                return false;
                            };
                            game.players[0].explored.contains(pos)
                                && !game.rules.is_water(tile)
                                && game.rules.is_passable(tile)
                                && !game.tile_is_natural_wonder(tile)
                                && game
                                    .cities
                                    .values()
                                    .filter(|other| other.id != cid)
                                    .all(|other| game.wdist(other.pos, *pos) >= 4)
                        })
                        .map(|pos| {
                            ai.settle_value(&game, 0, pos)
                                - game.wdist(chosen_pos, pos) as f64 * SETTLE_DISTANCE_PENALTY
                        })
                        .fold(chosen, f64::max);
                    discounted_ratios.push(chosen / discounted.max(1e-9));
                    // And now the decisive one: what does the agent's OWN
                    // candidate generator offer from this very tile? Anything
                    // the two comparisons above can see that this cannot is
                    // excluded by `settle_sites`' filters -- the value >= 12.0
                    // floor, foreign-owned ground, the four-tile separation --
                    // rather than missed by the choice.
                    let offered = ai
                        .settle_sites(&game, 0, chosen_pos, 8)
                        .into_iter()
                        .map(|(_, value)| value)
                        .fold(chosen, f64::max);
                    offered_ratios.push(chosen / offered.max(1e-9));
                    founded += 1;
                    if best <= chosen + 1e-9 {
                        chosen_was_best += 1;
                    }
                    near_ratios.push(chosen / near.max(1e-9));
                    let ratio = chosen / best.max(1e-9);
                    ratios.push(ratio);
                    if known.len() == 1 {
                        capital_ratio.push(ratio);
                    }
                }
            }
        }

        ratios.sort_by(f64::total_cmp);
        let mean = ratios.iter().sum::<f64>() / ratios.len().max(1) as f64;
        println!("\n=== settle siting census: {founded} cities founded over 8 maps ===");
        let near_mean = near_ratios.iter().sum::<f64>() / near_ratios.len().max(1) as f64;
        let disc_mean =
            discounted_ratios.iter().sum::<f64>() / discounted_ratios.len().max(1) as f64;
        println!(
            "  against RAW settle_value  -- within 3 tiles: {:.1}%   within 8 tiles: {:.1}%",
            near_mean * 100.0,
            mean * 100.0
        );
        let off_mean = offered_ratios.iter().sum::<f64>() / offered_ratios.len().max(1) as f64;
        println!(
            "  against the AGENT'S OWN objective (settle_value - {SETTLE_DISTANCE_PENALTY}/tile): {:.1}%",
            disc_mean * 100.0
        );
        println!(
            "  against what its OWN `settle_sites` generator offers here: {:.1}%",
            off_mean * 100.0
        );
        if !ratios.is_empty() {
            println!(
                "  percentiles  p10 {:.1}%   median {:.1}%   p90 {:.1}%   worst {:.1}%",
                ratios[ratios.len() / 10] * 100.0,
                ratios[ratios.len() / 2] * 100.0,
                ratios[ratios.len() * 9 / 10] * 100.0,
                ratios[0] * 100.0
            );
        }
        println!(
            "  the chosen site WAS the best available on {chosen_was_best} of {founded} foundings ({:.1}%)",
            chosen_was_best as f64 / founded.max(1) as f64 * 100.0
        );
        step_away.sort_unstable();
        if !step_away.is_empty() {
            println!(
                "  when a better site existed within 3 tiles it was a median {} tile(s) away ({} cases)",
                step_away[step_away.len() / 2],
                step_away.len()
            );
        }
        if !capital_ratio.is_empty() {
            let cap = capital_ratio.iter().sum::<f64>() / capital_ratio.len() as f64;
            println!("  capitals alone: {:.1}% ({} sampled)", cap * 100.0, capital_ratio.len());
        }
        println!();
    }

    /// Census, not an assertion: does the settler's site search discard ground
    /// it could actually reach?
    ///
    /// `settle_siting_census` establishes that the agent founds at ~65% of what
    /// its own `settle_sites` generator offers from the very tile it chose, and
    /// that re-scoring a stale target changes nothing. The only machinery
    /// between the generator and the choice is
    /// `BasicAi::first_reachable_settle_site`, which walks the value-sorted
    /// candidate list **in chunks of 40** and skips an entire chunk when
    /// `route_step_to_any` reaches none of it. Because the list is sorted by
    /// value descending, one failed probe on the first chunk discards the forty
    /// best sites at once.
    ///
    /// Two readings, and they call for opposite conclusions:
    ///
    /// - the skipped sites are genuinely unreachable, so the siting census
    ///   overstates its gap and the axis closes like the other three; or
    /// - the chunked scan is throwing away reachable ground, which is a defect.
    ///
    /// This asks each better-than-chosen candidate the route question
    /// **individually**, which is the same question the chunk asks collectively,
    /// and reports how the two answers differ.
    ///
    /// Run with `cargo test --release settle_search_reach_census -- --ignored --nocapture`.
    #[test]
    #[ignore = "census, not an assertion; run explicitly with --nocapture"]
    fn settle_search_reach_census() {
        let mut settler_turns = 0u64;
        let mut search_returned_nothing = 0u64;
        let mut better_offered = 0u64;
        let mut better_individually_reachable = 0u64;
        let mut turns_with_a_reachable_better_site = 0u64;
        let mut best_gap: Vec<f64> = Vec::new();

        for map in 0..8u64 {
            let mut game = Game::new_full(4, 24, 16, 470_000 + map, 200, 1, false);
            let mut ais: Vec<AdvancedAi> =
                (0..game.players.len()).map(|_| AdvancedAi::new()).collect();
            game.set_fog_memory(false);
            while game.winner.is_none() && game.turn <= game.max_turns {
                let pid = game.current;
                if pid == 0 {
                    let probe = ais[0].clone();
                    for uid in game.player_unit_ids(0) {
                        if game.units[&uid].kind != "settler" {
                            continue;
                        }
                        let from = game.units[&uid].pos;
                        let candidates = probe.settle_sites(&game, 0, from, 8);
                        if candidates.is_empty() {
                            continue;
                        }
                        settler_turns += 1;
                        let returned =
                            BasicAi::first_reachable_settle_site(&game, uid, &candidates);
                        let floor = match returned {
                            Some((_, value)) => value,
                            None => {
                                search_returned_nothing += 1;
                                f64::NEG_INFINITY
                            }
                        };
                        let mut found_one = false;
                        for (pos, value) in &candidates {
                            if *value <= floor {
                                continue;
                            }
                            better_offered += 1;
                            // The same question the chunk asked collectively.
                            if *pos == from || game.route_step(uid, *pos, 0).is_some() {
                                better_individually_reachable += 1;
                                if !found_one {
                                    found_one = true;
                                    best_gap.push(value - floor.max(0.0));
                                }
                            }
                        }
                        if found_one {
                            turns_with_a_reachable_better_site += 1;
                        }
                    }
                }
                ais[pid].take_turn(&mut game, pid);
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &crate::game::Action::EndTurn);
                }
            }
        }

        best_gap.sort_by(f64::total_cmp);
        println!("\n=== settle search reach census: {settler_turns} settler-turns over 8 maps ===");
        println!("  the search returned NOTHING on {search_returned_nothing} of them");
        println!(
            "  candidates the generator offered above what the search returned: {better_offered}"
        );
        println!(
            "  of those, INDIVIDUALLY reachable by the same route query: {better_individually_reachable} ({:.1}%)",
            better_individually_reachable as f64 / better_offered.max(1) as f64 * 100.0
        );
        println!(
            "  settler-turns where a strictly better REACHABLE site was skipped: {turns_with_a_reachable_better_site} ({:.1}%)",
            turns_with_a_reachable_better_site as f64 / settler_turns.max(1) as f64 * 100.0
        );
        if !best_gap.is_empty() {
            println!(
                "  when skipped, the value left behind was median {:.1} (p90 {:.1}, max {:.1})",
                best_gap[best_gap.len() / 2],
                best_gap[best_gap.len() * 9 / 10],
                best_gap[best_gap.len() - 1]
            );
        }
        println!();
    }

    /// Census, not an assertion: is `settle_siting_census` measuring the site
    /// or measuring the city that was just built on it?
    ///
    /// `settle_value` skips any tile another city owns:
    /// `if t.owner_city.is_some() && p != pos { continue; }`. A city claims its
    /// centre and all six ring-one tiles the moment it is founded, and ring one
    /// carries the full weight in that sum while ring two is discounted to
    /// 0.45. So scoring the chosen site *after* founding strips it of its
    /// highest-weighted contributions, while rival candidates four or more
    /// tiles away keep theirs intact.
    ///
    /// If that is what `settle_siting_census` measured, its gap is an artifact
    /// of the measurement rather than a fact about the decision, and the whole
    /// "settle siting is the one unsaturated city axis" claim falls.
    ///
    /// Run with `cargo test --release settle_value_before_and_after_founding -- --ignored --nocapture`.
    #[test]
    #[ignore = "census, not an assertion; run explicitly with --nocapture"]
    fn settle_value_before_and_after_founding() {
        let scorer = AdvancedAi::new();
        let mut before_after: Vec<(f64, f64)> = Vec::new();

        for map in 0..8u64 {
            let mut game = Game::new_full(4, 24, 16, 470_000 + map, 200, 1, false);
            let mut ais: Vec<AdvancedAi> =
                (0..game.players.len()).map(|_| AdvancedAi::new()).collect();
            game.set_fog_memory(false);
            let mut known: BTreeSet<u32> = game.player_city_ids(0).into_iter().collect();
            while game.winner.is_none() && game.turn <= game.max_turns {
                let pid = game.current;
                // Score every tile a settler is standing on BEFORE the turn
                // runs, which is the last moment the site is still unowned.
                let mut pre: BTreeMap<Pos, f64> = BTreeMap::new();
                if pid == 0 {
                    for uid in game.player_unit_ids(0) {
                        if game.units[&uid].kind == "settler" {
                            let pos = game.units[&uid].pos;
                            pre.insert(pos, scorer.settle_value(&game, 0, pos));
                        }
                    }
                }
                ais[pid].take_turn(&mut game, pid);
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &crate::game::Action::EndTurn);
                }
                if pid != 0 {
                    continue;
                }
                for cid in game.player_city_ids(0) {
                    if !known.insert(cid) {
                        continue;
                    }
                    let city = &game.cities[&cid];
                    if city.captured_from.is_some() {
                        continue;
                    }
                    if let Some(before) = pre.get(&city.pos) {
                        let after = scorer.settle_value(&game, 0, city.pos);
                        before_after.push((*before, after));
                    }
                }
            }
        }

        let n = before_after.len().max(1) as f64;
        let before: f64 = before_after.iter().map(|(b, _)| b).sum::<f64>() / n;
        let after: f64 = before_after.iter().map(|(_, a)| a).sum::<f64>() / n;
        println!("\n=== settle_value before and after founding: {} sites ===", before_after.len());
        println!("  mean settle_value the turn BEFORE the city existed: {before:.1}");
        println!("  mean settle_value once the city owns its ring one:  {after:.1}");
        println!(
            "  founding the city destroys {:.1}% of its own site's measured value",
            (1.0 - after / before.max(1e-9)) * 100.0
        );
        println!();
    }

    /// Census, not an assertion: which conjunct actually blocks the settler?
    ///
    /// #554 measured that handing a seat a free Settler while it is short of
    /// its own city target more than doubles its win rate — 23.0% to 52.3% over
    /// 300 games at p=0.0000, the only subsystem grant this harness has ever
    /// returned HEADROOM for. Seats target six cities and finish with 2.1–2.8.
    /// **The agent cannot afford the empire it has already decided it wants.**
    ///
    /// That says the cost binds. It does not say *which* cost. The settler
    /// branch of `production_value` is a five-way conjunction, and any one of
    /// them turns the item's value into −10,000:
    ///
    /// ```text
    /// city_count + counts.settlers < plan.desired_cities   // target not met
    ///   && counts.settlers < in_flight_allowed             // serialization
    ///   && city.pop >= 2                                   // size gate
    ///   && expansion_open                                  // time window
    ///   && site.is_some()                                  // somewhere to go
    /// ```
    ///
    /// This counts, over every city-turn where the empire is short of its
    /// target, which conjuncts were false. A conjunct that never fails cannot
    /// be the constraint; one that fails almost always is where the settler
    /// economy actually stalls. Shares sum past 100% because several can fail
    /// at once, so the last block reports the turns where exactly one did —
    /// those are the ones a single fix would unblock.
    ///
    /// Run with `cargo test --release expansion_funnel_blocker_census -- --ignored --nocapture`.
    #[test]
    #[ignore = "census, not an assertion; run explicitly with --nocapture"]
    fn expansion_funnel_blocker_census() {
        // ⚠ `ai_eval` defaults to 24x16 at 4 players, which is 96 tiles per
        // player; the exhibition this engine actually serves runs 74x46 at 6,
        // which is 567. "No settle site in reach" is exactly the kind of
        // reading that could be pure map scarcity at the small size, so both
        // are measured and reported side by side.
        for (label, players, width, height) in
            [("eval 4p 24x16", 4usize, 24i32, 16i32), ("deployment 6p 74x46", 6, 74, 46)]
        {
            expansion_funnel_at(label, players, width, height);
        }
    }

    fn expansion_funnel_at(label: &str, players: usize, width: i32, height: i32) {
        let mut short_city_turns = 0u64;
        let mut fail_serial = 0u64;
        let mut fail_pop = 0u64;
        let mut fail_window = 0u64;
        let mut fail_site = 0u64;
        let mut all_clear = 0u64;
        let mut sole_serial = 0u64;
        let mut sole_pop = 0u64;
        let mut sole_window = 0u64;
        let mut sole_site = 0u64;
        let mut settler_turns_when_clear: Vec<f64> = Vec::new();

        for map in 0..8u64 {
            let mut game =
                Game::new_full(players, width, height, 480_000 + map, 200, 1, false);
            let mut ais: Vec<AdvancedAi> =
                (0..game.players.len()).map(|_| AdvancedAi::new()).collect();
            game.set_fog_memory(false);
            while game.winner.is_none() && game.turn <= game.max_turns {
                let pid = game.current;
                if pid == 0 {
                    if let Some(plan) = ais[0].plan.clone() {
                        let cities = game.player_city_ids(0);
                        let settlers = game
                            .player_unit_ids(0)
                            .into_iter()
                            .filter(|uid| game.units[uid].kind == "settler")
                            .count();
                        if cities.len() + settlers < plan.desired_cities {
                            let window = AdvancedAi::expansion_window_open(&game);
                            for cid in cities {
                                short_city_turns += 1;
                                let city = &game.cities[&cid];
                                let pos = city.pos;
                                let pop = city.pop;
                                let site = ais[0].best_settle_site(&game, 0, pos, 11).is_some();
                                // `in_flight_allowed` is 1 for the shipped
                                // agent, so serialization fails exactly when a
                                // settler already exists.
                                let serial_ok = settlers < 1;
                                let bad = [!serial_ok, pop < 2, !window, !site];
                                if bad[0] {
                                    fail_serial += 1;
                                }
                                if bad[1] {
                                    fail_pop += 1;
                                }
                                if bad[2] {
                                    fail_window += 1;
                                }
                                if bad[3] {
                                    fail_site += 1;
                                }
                                match bad.iter().filter(|b| **b).count() {
                                    0 => {
                                        all_clear += 1;
                                        // Everything permits a settler. How
                                        // long would one take to pay for?
                                        let production =
                                            game.city_yields(cid).production.max(1.0);
                                        let cost = game.item_remaining_cost_for_city(
                                            0,
                                            cid,
                                            &Item::Unit {
                                                unit: "settler".into(),
                                            },
                                        );
                                        settler_turns_when_clear.push(cost / production);
                                    }
                                    1 => {
                                        if bad[0] {
                                            sole_serial += 1;
                                        } else if bad[1] {
                                            sole_pop += 1;
                                        } else if bad[2] {
                                            sole_window += 1;
                                        } else {
                                            sole_site += 1;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                ais[pid].take_turn(&mut game, pid);
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &crate::game::Action::EndTurn);
                }
            }
        }

        let pct = |n: u64| n as f64 / short_city_turns.max(1) as f64 * 100.0;
        println!("\n=== expansion funnel [{label}]: {short_city_turns} city-turns while SHORT of target ===");
        println!("  a settler already walking (serialization)  {fail_serial:>6}  ({:>5.1}%)", pct(fail_serial));
        println!("  city below pop 2                           {fail_pop:>6}  ({:>5.1}%)", pct(fail_pop));
        println!("  expansion window shut                      {fail_window:>6}  ({:>5.1}%)", pct(fail_window));
        println!("  no settle site in reach                    {fail_site:>6}  ({:>5.1}%)", pct(fail_site));
        println!("  NOTHING blocking — the settler was allowed {all_clear:>6}  ({:>5.1}%)", pct(all_clear));
        println!("  turns where EXACTLY ONE conjunct failed (a single fix would unblock these):");
        println!("    serialization {sole_serial}   pop<2 {sole_pop}   window {sole_window}   no site {sole_site}");
        if !settler_turns_when_clear.is_empty() {
            let mut t = settler_turns_when_clear.clone();
            t.sort_by(f64::total_cmp);
            println!(
                "  when allowed, a settler costs a median {:.1} turns of this city's production (p90 {:.1})",
                t[t.len() / 2],
                t[t.len() * 9 / 10]
            );
        }
        println!();
    }

    /// Fires-check for `expansion_pays_back`, at both map scales.
    ///
    /// The treatment exists to unblock the 31.2% of short city-turns where
    /// `expansion_funnel_blocker_census` measures the shut window as the sole
    /// blocker, so the thing to verify before spending an eval is that it
    /// actually opens on those turns — and that it is not simply "always open",
    /// which would make it a deletion of the gate rather than a payback test.
    ///
    /// Run with `cargo test --release expansion_payback_fires -- --ignored --nocapture`.
    #[test]
    #[ignore = "census, not an assertion; run explicitly with --nocapture"]
    fn expansion_payback_fires() {
        for (label, players, width, height) in
            [("eval 4p 24x16", 4usize, 24i32, 16i32), ("deployment 6p 74x46", 6, 74, 46)]
        {
            let mut agree = 0u64;
            let mut opened = 0u64;
            let mut closed = 0u64;
            for map in 0..6u64 {
                let mut game =
                    Game::new_full(players, width, height, 480_000 + map, 200, 1, false);
                let mut ais: Vec<AdvancedAi> =
                    (0..game.players.len()).map(|_| AdvancedAi::new()).collect();
                let treated = {
                    let mut ai = AdvancedAi::new();
                    ai.expansion_pays_back = true;
                    ai
                };
                game.set_fog_memory(false);
                while game.winner.is_none() && game.turn <= game.max_turns {
                    let pid = game.current;
                    if pid == 0 {
                        let flat = AdvancedAi::expansion_window_open(&game);
                        for cid in game.player_city_ids(0) {
                            let pays = treated.expansion_pays_back_for(&game, 0, cid);
                            match (flat, pays) {
                                (a, b) if a == b => agree += 1,
                                (false, true) => opened += 1,
                                (true, false) => closed += 1,
                                _ => {}
                            }
                        }
                    }
                    ais[pid].take_turn(&mut game, pid);
                    if game.winner.is_none() && game.current == pid {
                        let _ = game.apply(pid, &crate::game::Action::EndTurn);
                    }
                }
            }
            let total = agree + opened + closed;
            println!(
                "\n=== expansion payback vs flat reserve [{label}]: {total} city-turns ===",
            );
            println!("  agree                     {agree:>6}  ({:>5.1}%)", agree as f64 / total.max(1) as f64 * 100.0);
            println!("  payback OPENS what the flat reserve shut   {opened:>6}  ({:>5.1}%)", opened as f64 / total.max(1) as f64 * 100.0);
            println!("  payback SHUTS what the flat reserve opened {closed:>6}  ({:>5.1}%)", closed as f64 / total.max(1) as f64 * 100.0);
        }
        println!();
    }

    /// Census, not an assertion: where does an empire's production actually go,
    /// and how much of it would the missing cities have cost?
    ///
    /// This is the last open question on the expansion axis, and the axis is
    /// worth closing properly because #554 measured real headroom there — a
    /// free Settler while short of the city target more than doubles the win
    /// rate (23.0% → 52.3%, p=0.0000), the only subsystem grant this harness
    /// has ever returned HEADROOM for.
    ///
    /// Every *decision* mechanism proposed for it has since measured null:
    ///
    /// - the expansion window (#562): +0.12 cities, wins unmeasurable
    /// - production preemption (`docs/EVAL.md` 2026-07-28): cities at end
    ///   **2.21 vs 2.21**, settlers started 2.46 vs 2.42
    /// - the settler's own valuation: when a city is free it already picks the
    ///   settler, and the genuine free-city loss is only **2.6%**
    /// - capital growth (`docs/OPENINGS.md` §12): every city after the first
    ///   arrives **later**, monotonically in the dose
    ///
    /// That leaves one explanation standing, and it is not a decision at all:
    /// **the empire cannot afford the settlers.** This measures it directly.
    /// If the settler share of production is already large and the shortfall
    /// would cost more than the empire ever produces, the axis is closed to
    /// decision changes and the oracle's headroom is only reachable by having a
    /// bigger economy — which #532 measured at 99.5% of its own ceiling.
    ///
    /// Run with `cargo test --release production_allocation_census -- --ignored --nocapture`.
    #[test]
    #[ignore = "census, not an assertion; run explicitly with --nocapture"]
    fn production_allocation_census() {
        for (label, players, width, height) in
            [("eval 4p 24x16", 4usize, 24i32, 16i32), ("deployment 6p 74x46", 6, 74, 46)]
        {
            let mut spent = BTreeMap::<&str, f64>::new();
            let mut total = 0.0f64;
            let mut end_cities = 0.0f64;
            let mut target = 0.0f64;
            let mut shortfall_cost = 0.0f64;
            let maps = 6u64;

            for map in 0..maps {
                let mut game =
                    Game::new_full(players, width, height, 480_000 + map, 200, 1, false);
                let mut ais: Vec<AdvancedAi> =
                    (0..game.players.len()).map(|_| AdvancedAi::new()).collect();
                game.set_fog_memory(false);
                let mut prev: BTreeMap<u32, f64> = BTreeMap::new();
                while game.winner.is_none() && game.turn <= game.max_turns {
                    let pid = game.current;
                    ais[pid].take_turn(&mut game, pid);
                    if game.winner.is_none() && game.current == pid {
                        let _ = game.apply(pid, &crate::game::Action::EndTurn);
                    }
                    if pid != 0 {
                        continue;
                    }
                    // Production is banked per item key, so the turn-over-turn
                    // rise in a city's yield is what it actually spent, and the
                    // queue head says on what.
                    for cid in game.player_city_ids(0) {
                        let city = &game.cities[&cid];
                        let yield_now = game.city_yields(cid).production.max(0.0);
                        let key = match city.queue.first() {
                            Some(Item::Unit { unit }) if unit.as_str() == "settler" => "settler",
                            Some(Item::Unit { unit }) => {
                                if game.rules.units[unit.as_str()].class == "military" {
                                    "military unit"
                                } else {
                                    "civilian unit"
                                }
                            }
                            Some(Item::Formation { .. }) => "military unit",
                            Some(Item::Building { .. }) => "building",
                            Some(Item::District { .. }) => "district",
                            Some(Item::Wonder { .. }) => "wonder",
                            Some(Item::Project { .. }) => "project",
                            Some(Item::Product { .. }) => "product",
                            Some(Item::Repair { .. }) => "repair",
                            None => "idle",
                        };
                        *spent.entry(key).or_default() += yield_now;
                        total += yield_now;
                        prev.insert(cid, yield_now);
                    }
                }
                let held = game.player_city_ids(0).len() as f64;
                end_cities += held;
                let want = ais[0]
                    .plan
                    .as_ref()
                    .map(|p| p.desired_cities as f64)
                    .unwrap_or(0.0);
                target += want;
                // Civ VI escalates settler cost 80/110/140/...; price the
                // cities the empire never built at that schedule.
                let missing = (want - held).max(0.0) as usize;
                let built = held.max(1.0) as usize;
                for n in 0..missing {
                    shortfall_cost += 80.0 + 30.0 * (built + n) as f64;
                }
            }

            println!("\n=== production allocation [{label}]: {total:.0} production over {maps} maps ===");
            let mut rows: Vec<(&&str, &f64)> = spent.iter().collect();
            rows.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
            for (kind, amount) in rows {
                println!("  {kind:<14} {amount:>10.0}  ({:>5.1}%)", amount / total.max(1.0) * 100.0);
            }
            println!(
                "  cities held {:.2} against a target of {:.2}",
                end_cities / maps as f64,
                target / maps as f64
            );
            println!(
                "  the missing cities would have cost {:.0} production — {:.1}% of everything the empire made",
                shortfall_cost / maps as f64,
                shortfall_cost / total.max(1.0) * 100.0
            );
        }
        println!();
    }

    /// Fires-check for `city_target_floor`, at both map scales.
    ///
    /// ⚠ The criterion is the **outcome** — cities at end — not a mechanism
    /// bucket. `docs/EVAL.md` records the last expansion fires-check choosing
    /// "the every-city-mid-build bucket must collapse", which the treatment
    /// could never have moved, so it was unfalsifiable in the helpful
    /// direction. If cities at end do not rise, nothing else here matters.
    ///
    /// Run with `cargo test --release city_target_floor_fires -- --ignored --nocapture`.
    #[test]
    #[ignore = "census, not an assertion; run explicitly with --nocapture"]
    fn city_target_floor_fires() {
        for (label, players, width, height) in
            [("eval 4p 24x16", 4usize, 24i32, 16i32), ("deployment 6p 74x46", 6, 74, 46)]
        {
            for floor in [3usize, 6] {
                let mut cities = 0.0f64;
                let mut target = 0.0f64;
                let mut score = 0.0f64;
                let maps = 6u64;
                for map in 0..maps {
                    let mut game =
                        Game::new_full(players, width, height, 480_000 + map, 200, 1, false);
                    let mut ais: Vec<AdvancedAi> = (0..game.players.len())
                        .map(|_| {
                            let mut ai = AdvancedAi::new();
                            ai.city_target_floor = floor;
                            ai
                        })
                        .collect();
                    game.set_fog_memory(false);
                    while game.winner.is_none() && game.turn <= game.max_turns {
                        let pid = game.current;
                        ais[pid].take_turn(&mut game, pid);
                        if game.winner.is_none() && game.current == pid {
                            let _ = game.apply(pid, &crate::game::Action::EndTurn);
                        }
                    }
                    cities += game.player_city_ids(0).len() as f64;
                    score += game.score(0) as f64;
                    target += ais[0]
                        .plan
                        .as_ref()
                        .map(|p| p.desired_cities as f64)
                        .unwrap_or(0.0);
                }
                println!(
                    "  [{label}] floor={floor}  cities {:.2} / target {:.2}   score {:.0}",
                    cities / maps as f64,
                    target / maps as f64,
                    score / maps as f64
                );
            }
        }
        println!();
    }

    /// Census, not an assertion: how much of its treasury does an empire never
    /// spend?
    ///
    /// The `ablate` harness's calibration grant hands a seat 200 Gold and 100
    /// Faith a turn and wins **89 of 100** — by a wide margin the largest
    /// effect any grant has produced, against `ground`, `siting`, `taker`,
    /// `modernity` and `attrition` all null and `expansion` at 52.3%. That
    /// grant is not a subsystem and proves nothing on its own; it is the
    /// instrument's proof that it can detect an advantage.
    ///
    /// But it does raise a question nothing in `docs/` has asked: the treasury
    /// is evidently worth an enormous amount, so **does this agent use the one
    /// it already has?** A balance sitting idle is capital earning nothing, and
    /// unlike expansion it costs no production to deploy.
    ///
    /// Reported as *turns of income held*, because an absolute balance is not
    /// interpretable on its own — 300 Gold is prudent at 5 gold per turn and
    /// dead weight at 40.
    ///
    /// Run with `cargo test --release idle_treasury_census -- --ignored --nocapture`.
    #[test]
    #[ignore = "census, not an assertion; run explicitly with --nocapture"]
    fn idle_treasury_census() {
        for (label, players, width, height) in
            [("eval 4p 24x16", 4usize, 24i32, 16i32), ("deployment 6p 74x46", 6, 74, 46)]
        {
            let mut gold_samples: Vec<f64> = Vec::new();
            let mut faith_samples: Vec<f64> = Vec::new();
            let mut gpt_samples: Vec<f64> = Vec::new();
            let mut gold_turns_held: Vec<f64> = Vec::new();
            let mut peak_gold = 0.0f64;
            let maps = 6u64;

            for map in 0..maps {
                let mut game =
                    Game::new_full(players, width, height, 480_000 + map, 200, 1, false);
                let mut ais: Vec<AdvancedAi> =
                    (0..game.players.len()).map(|_| AdvancedAi::new()).collect();
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
                    let gold = game.players[0].gold;
                    let faith = game.players[0].faith;
                    // Income, not balance: what the empire earns each turn is
                    // the yardstick a balance has to be read against.
                    let gpt = game
                        .player_city_ids(0)
                        .into_iter()
                        .map(|cid| game.city_yields(cid).gold)
                        .sum::<f64>()
                        .max(0.1);
                    gold_samples.push(gold);
                    faith_samples.push(faith);
                    gpt_samples.push(gpt);
                    gold_turns_held.push(gold / gpt);
                    peak_gold = peak_gold.max(gold);
                }
            }

            let mean = |v: &Vec<f64>| v.iter().sum::<f64>() / v.len().max(1) as f64;
            let median = |v: &Vec<f64>| {
                let mut c = v.clone();
                c.sort_by(f64::total_cmp);
                c[c.len() / 2]
            };
            println!("\n=== idle treasury [{label}]: {} seat-turns ===", gold_samples.len());
            println!(
                "  Gold held      mean {:.0}   median {:.0}   peak {:.0}",
                mean(&gold_samples),
                median(&gold_samples),
                peak_gold
            );
            println!("  Gold income    mean {:.1}/turn", mean(&gpt_samples));
            println!(
                "  ★ Gold held as TURNS OF INCOME   mean {:.1}   median {:.1}",
                mean(&gold_turns_held),
                median(&gold_turns_held)
            );
            println!(
                "  Faith held     mean {:.0}   median {:.0}",
                mean(&faith_samples),
                median(&faith_samples)
            );
            // The mean sits far above the median, so the distribution has a
            // tail and the tail is where any real waste lives. A seat holding
            // thirty turns of income is not keeping a prudent reserve.
            for cut in [10.0f64, 30.0, 60.0] {
                let share = gold_turns_held.iter().filter(|t| **t > cut).count() as f64
                    / gold_turns_held.len().max(1) as f64
                    * 100.0;
                println!("    seat-turns holding more than {cut:>4.0} turns of income: {share:>5.1}%");
            }
        }
        println!();
    }

    /// Census, not an assertion: is the envoy gap a resource shortfall or an
    /// allocation failure?
    ///
    /// #602 measured `Grant::Suzerain` — suzerain of every met city-state — at
    /// **56.7% against a 22.7% control**, p=0.0000 over 400 maps and 168
    /// discordant cells, 150 directions for to 18 against. That is the largest
    /// subsystem headroom this harness has found, larger than `expansion`.
    ///
    /// A large ceiling is not a reachable one. Expansion has an equally real
    /// ceiling and **seven consecutive treatments failed to reach it**, because
    /// the oracle there removed the settler's *cost* and no decision can remove
    /// a cost. So the question that decides whether this axis is worth a
    /// treatment at all is which kind of gap it is:
    ///
    /// - **`envoys_free` accumulates** → the seat earns envoys and does not
    ///   place them. An allocation failure, and `advanced_envoys` can fix it.
    /// - **`envoys_free` sits near zero** → the seat spends everything it earns
    ///   and is simply outbid. A resource gap, and this goes the way expansion
    ///   went.
    ///
    /// The deficit distribution decides how much it would take: being outbid by
    /// one envoy at many city-states is a very different problem from being
    /// outbid by ten at a few.
    ///
    /// Run with `cargo test --release envoy_allocation_census -- --ignored --nocapture`.
    #[test]
    #[ignore = "census, not an assertion; run explicitly with --nocapture"]
    fn envoy_allocation_census() {
        for (label, players, width, height, city_states) in [
            ("eval 4p 24x16", 4usize, 24i32, 16i32, 4usize),
            ("deployment 6p 74x46", 6, 74, 46, 12),
        ] {
            let mut free_samples: Vec<f64> = Vec::new();
            let mut placed_samples: Vec<f64> = Vec::new();
            let mut met_samples: Vec<f64> = Vec::new();
            let mut held_samples: Vec<f64> = Vec::new();
            let mut deficits: Vec<i64> = Vec::new();
            let maps = 6u64;

            for map in 0..maps {
                let mut game = Game::new_full(
                    players,
                    width,
                    height,
                    480_000 + map,
                    200,
                    city_states,
                    false,
                );
                let mut ais: Vec<AdvancedAi> =
                    (0..game.players.len()).map(|_| AdvancedAi::new()).collect();
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
                    let minors: Vec<usize> = game
                        .players
                        .iter()
                        .filter(|m| m.is_minor && m.alive && !m.is_barbarian)
                        .map(|m| m.id)
                        .filter(|m| game.has_met(0, *m))
                        .collect();
                    if minors.is_empty() {
                        continue;
                    }
                    free_samples.push(game.players[0].envoys_free as f64);
                    placed_samples
                        .push(game.players[0].envoys.iter().map(|(_, n)| *n).sum::<i64>() as f64);
                    met_samples.push(minors.len() as f64);
                    let held = minors
                        .iter()
                        .filter(|m| game.suzerain_of(**m) == Some(0))
                        .count();
                    held_samples.push(held as f64);
                    // For each city-state it does NOT hold, how many more
                    // envoys would it have taken? That is the size of the
                    // reallocation the oracle performed for free.
                    for minor in &minors {
                        if game.suzerain_of(*minor) == Some(0) {
                            continue;
                        }
                        let best_rival = game
                            .players
                            .iter()
                            .filter(|o| !o.is_minor && o.alive && o.id != 0)
                            .map(|o| game.envoys_at(o.id, *minor))
                            .max()
                            .unwrap_or(0);
                        let want = (best_rival + 1).max(3);
                        deficits.push((want - game.envoys_at(0, *minor)).max(0));
                    }
                }
            }

            let mean = |v: &Vec<f64>| v.iter().sum::<f64>() / v.len().max(1) as f64;
            deficits.sort_unstable();
            let total_deficit: i64 = deficits.iter().sum();
            println!("\n=== envoy allocation [{label}]: {} seat-turns ===", free_samples.len());
            println!(
                "  ★ envoys UNSPENT in the pool   mean {:.2}   (a pool near zero means resource-bound)",
                mean(&free_samples)
            );
            println!("  envoys placed on the board     mean {:.1}", mean(&placed_samples));
            println!(
                "  city-states met {:.1}, suzerain of {:.1}  ({:.0}% held)",
                mean(&met_samples),
                mean(&held_samples),
                mean(&held_samples) / mean(&met_samples).max(1e-9) * 100.0
            );
            if !deficits.is_empty() {
                println!(
                    "  when NOT suzerain, envoys short: median {}  p90 {}  max {}",
                    deficits[deficits.len() / 2],
                    deficits[deficits.len() * 9 / 10],
                    deficits[deficits.len() - 1]
                );
                println!(
                    "  ★ total shortfall {:.1} envoys per seat-turn against a pool of {:.2}",
                    total_deficit as f64 / free_samples.len().max(1) as f64,
                    mean(&free_samples)
                );
            }
        }
        println!();
    }
}
