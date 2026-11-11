//! `science-victory-drive`: an empire that dominates science drives the
//! space race — beelines the chain, builds the launch city's production,
//! attempts the race the stock horizon refuses, and lands the projects
//! before the field.
//!
//! Operator, 2026-08-24: *"we have regularly led science and not even
//! attempted a science victory. a top heuristic would be noting that we are
//! dominating science towards the end game and scaling science harder,
//! beelining science victory techs, accelerating the bottleneck, and
//! completing the projects before others! maybe 2 spaceports."*
//!
//! ## What the live seat did (`--victory science`, Settler, 250 turns Online)
//!
//! `~/civvis-civ6-runs/control`, the last complete games before the halt
//! (2026-08-19), read from their `events.jsonl`:
//!
//! | run | science/turn at t250 | techs (best rival) | pads | own projects | rivals' projects |
//! |---|---:|---:|---:|---:|---|
//! | `081800Z` | **334** | 71 (72) | 1, at ~t210 | 0 | 2, 2, 2 |
//! | `102855Z` | 234 | 67 (66) | 0 | 0 | 2, 1 |
//! | `090732Z` | 203 | 64 (76) | 1 | 1, at t242 | 3, 2 |
//!
//! The seat led the field in science and never ran a launch project. Its
//! journal says why: from turn ~150 the stock turn-limit horizon
//! (`score-horizon`'s `space_race_can_finish`, both removed 2026-09-09)
//! refused the race every turn — *"101 turns left; the launch pad, the
//! remaining projects, their techs and fifty light-years do not fit"*. That
//! estimate priced the
//! whole chain at the best city's **current** production, 31–46 a turn in
//! those games, and ignores the engine's own +100% on every Spaceport
//! project (`Game::item_prod_mult`), so it reads 96 turns of production
//! where the engine would take 59. The refusal is self-fulfilling: the pass
//! it skips is the one that sites the pad, and nothing else builds
//! production for the race. The late game went to Builders (20–30 a game
//! after t180), anti-tank crews and Campus Research Grants instead.
//!
//! ## What the gene does
//!
//! 1. **Reads the field.** Every [`SCIENCE_DRIVE_REVIEW`] standard turns
//!    from [`SCIENCE_DRIVE_START`] of the clock, the seat's science a turn
//!    and tech count are read against every living major's — public
//!    victory-screen information, [`AdvancedAi::empire_science`]. An
//!    adaptive seat that leads the field in either is **driving**, and stays
//!    driving while it holds [`SCIENCE_DRIVE_HOLD`] of the leader's science
//!    or is within [`SCIENCE_DRIVE_TECH_SLACK`] techs of the leader's count.
//!    An adaptive seat whose own public victory focus is Science at or above
//!    [`SCIENCE_DRIVE_PLAN_FLOOR`] also drives, so it can build the chain that
//!    produces a lead; a queued or completed launch starts the drive
//!    immediately, even before the ordinary turn-87 field review.
//!    A seat assigned Science (`--victory science`, which the live seat
//!    always is) or committed to it by `lane-commit` drives from turn one; a
//!    seat assigned any other lane never drives.
//! 2. **The science keys.** While driving, [`AdvancedAi::raced_target`]
//!    answers Science: the rocketry-path tech value is 900, the pad count
//!    grows past one, a launch project may claim any pad city. The
//!    space-race pass runs under every plan short of Recovery, and the
//!    research beeline (`advanced_research`'s forced goal) follows the chain.
//! 3. **The milestone is the next unknown tech, not the next unbuilt
//!    project.** Stock keys the beeline on the first unfinished project, so
//!    while the Earth Satellite is being built (Rocketry already known) no
//!    tech leads to the milestone and research wanders. Here it is
//!    [`SCIENCE_DRIVE_CHAIN`], whichever is not yet known; before Rocketry the
//!    launch city's production techs (`industrialization`, `electricity`)
//!    carry [`SCIENCE_DRIVE_PRODUCTION_TECH`] of their own.
//! 4. **A launch city, and its production.** The Spaceport city (the best
//!    producer of them), else the city with a pad in its queue, else the
//!    best-production city. From the Industrial era its Industrial Zone,
//!    Workshop, Factory and Power Plant are priced as the race's bottleneck
//!    ([`AdvancedAi::science_drive_production_bonus`]); the Spaceport itself
//!    on top of the 3,000-point first-pad rung once Rocketry is known; a
//!    Military Academy once the `space_race` civic makes Integrated Space
//!    Cell slottable (+15% on the projects, and the engine enforces the
//!    Academy); the Royal Society where a Government Plaza can hold it (a
//!    Builder charge is 2% of a project). Pingala prefers the launch city
//!    once a pad stands there (Space Initiative, +30%), and the Gold reserve
//!    falls to [`SCIENCE_DRIVE_GOLD_RESERVE`] once Rocketry is known so the
//!    pad and its buildings can be bought.
//! 5. **The horizon left with `score-horizon`.** A driving horizon
//!    (`science_drive_race_fits`, priced as the engine runs the race) once
//!    replaced the stock `space_race_can_finish` while driving, but both were
//!    reachable only behind `score-horizon`, which the ledger held off on
//!    every deployed seat and which left the code on 2026-09-09 under the
//!    batch rule (-11/-50/-25). The drive races whenever its lane is active.
//! 6. **Two pads by the Earth Satellite, three by Mars.** Stock waits for the
//!    Moon Landing before a second pad. Here the second city builds its pad
//!    while the first runs the chain, so both build laser stations the turn
//!    the expedition launches.
//!
//! Nothing here touches `assess`, the expansion arm, the policy deck, the
//! Congress or any objective routing — the four `lane-commit` probes priced
//! every form of that reach at −1 to −2 pp of share (`docs/VICTORY_GENES.md`
//! §10). Exact no-op while off: every hook reads `science_drive`, which
//! `maintain_science_drive` clears while the gene is off.

use super::{AdvancedAi, GrandStrategy, VictoryTarget};
use crate::game::{Game, Item};
use crate::name::Name;
use crate::think;
use std::collections::BTreeSet;

/// The science-victory tech chain, in the order the projects need it.
pub const SCIENCE_DRIVE_CHAIN: [&str; 5] = [
    "rocketry",
    "satellites",
    "nanotechnology",
    "smart_materials",
    "offworld_mission",
];

/// The fraction of the turn cap after which an adaptive seat reads the
/// field: turn 87 on the 250-turn Online standard.
pub const SCIENCE_DRIVE_START: f64 = 0.35;
/// Standard turns before the first read when there is no turn cap.
pub const SCIENCE_DRIVE_START_STANDARD: u32 = 100;
/// Standard turns between reads of the field.
pub const SCIENCE_DRIVE_REVIEW: u32 = 5;
/// A driving seat keeps driving while its science is this share of the
/// field leader's.
pub const SCIENCE_DRIVE_HOLD: f64 = 0.75;

/// `science-drive-answers-the-launch`: a Diplomatic seat this many Diplomatic
/// Victory Points or more keeps its own lane rather than answering a rival's
/// launch. Live Emperor civvis-20261030T111154Z opened the answer at t194
/// standing on 18 of 20, one session from its own victory.
pub const SCIENCE_ANSWER_DIPLO_YIELD: i64 = 16;
/// `science-drive-leads-beside-the-lane`: the Standard turn (Online turn
/// 140) from which a seat assigned another lane races on its own outright
/// science lead, without waiting for a rival's launch.
pub const SCIENCE_LEAD_OPEN_STANDARD: u32 = 210;
/// Version 2 requires this relative science lead before an adaptive race
/// starts. A one-beaker fluctuation is not a victory commitment.
pub const SCIENCE_DRIVE_LEAD_MARGIN: f64 = 0.10;
/// Version 2 does not promote a young empire from a small absolute science
/// lead, nor treat a blank/partial rival observation as decisive.
pub const SCIENCE_DRIVE_MIN_SCIENCE: f64 = 20.0;
/// A driving seat keeps driving while it is within this many techs of the
/// field leader.
pub const SCIENCE_DRIVE_TECH_SLACK: usize = 3;
/// Version 2 requires one completed technology beyond the rival, rather than
/// treating parity as an adaptive lead.
pub const SCIENCE_DRIVE_TECH_LEAD: usize = 1;
/// An adaptive seat may commit from its own public Science lane once that
/// lane has cleared the opening floor.
pub const SCIENCE_DRIVE_PLAN_FLOOR: i32 = 40;
/// The world era from which the launch city's production chain is priced
/// (Industrial: the Factory's era).
pub const SCIENCE_DRIVE_PRODUCTION_ERA: usize = 4;
/// The Gold reserve of a driving seat with Rocketry: `(flat, per city)`.
pub const SCIENCE_DRIVE_GOLD_RESERVE: (f64, f64) = (100.0, 25.0);
/// Value of a tech on the way to the launch city's production techs while
/// Rocketry is still unknown.
pub const SCIENCE_DRIVE_PRODUCTION_TECH: f64 = 400.0;
/// The launch city's Spaceport, on top of the first-pad rung.
pub const SCIENCE_DRIVE_PAD_BONUS: f64 = 2_000.0;
/// The launch city's first Industrial Zone.
pub const SCIENCE_DRIVE_ZONE_BONUS: f64 = 1_200.0;
/// The launch city's Workshop, Factory, first Power Plant, Military Academy
/// (with the `space_race` civic) and Royal Society.
pub const SCIENCE_DRIVE_BUILDING_BONUS: [(&str, f64); 7] = [
    ("workshop", 500.0),
    ("factory", 700.0),
    ("coal_power_plant", 700.0),
    ("oil_power_plant", 700.0),
    ("nuclear_power_plant", 700.0),
    ("military_academy", 350.0),
    ("royal_society", 400.0),
];
/// Pingala's preference for the launch city once a pad stands there.
pub const SCIENCE_DRIVE_PINGALA_BONUS: f64 = 1_500.0;
/// Version 2's credit for a Campus while the science drive is active.
pub const SCIENCE_DRIVE_CAMPUS_BONUS: f64 = 700.0;
/// Version 2's credits for missing research buildings behind a Campus.
pub const SCIENCE_DRIVE_RESEARCH_BUILDING_BONUS: [(&str, f64); 3] = [
    ("library", 500.0),
    ("university", 900.0),
    ("research_lab", 1_400.0),
];
/// Version 2's credit for the repeatable Campus Research Grants project once
/// the city's research and launch-production buildings are caught up.
pub const SCIENCE_DRIVE_CAMPUS_PROJECT_BONUS: f64 = 900.0;

/// A queued or completed project is a durable Science commitment for an
/// adaptive seat, even while the public focus score is still below its
/// opening floor.
const SCIENCE_LAUNCH_PROJECTS: [&str; 4] = [
    "launch_earth_satellite",
    "launch_moon_landing",
    "launch_mars_colony",
    "exoplanet_expedition",
];

/// The local buildings that a science city should finish before converting
/// its Campus into a repeatable project. The ordinary project cap also uses
/// this debt; keeping the gate here prevents the science-drive bonus from
/// reopening that old overhang through a second valuation path.
const SCIENCE_DRIVE_PROJECT_BUILDING_DEBT: [&str; 8] = [
    "library",
    "university",
    "research_lab",
    "workshop",
    "factory",
    "coal_power_plant",
    "oil_power_plant",
    "nuclear_power_plant",
];

const POWER_PLANTS: [&str; 3] = ["coal_power_plant", "oil_power_plant", "nuclear_power_plant"];

/// The drive's state while it is on: when it started, when the field was
/// last read and what it read, and the launch city.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScienceDrive {
    /// The turn the seat started driving.
    pub since: u32,
    /// The turn the field was last read.
    pub reviewed: u32,
    /// The reading that turn.
    pub standing: ScienceStanding,
    /// The city the race runs from, if the seat has one.
    pub launch_city: Option<u32>,
    /// Driving because Science is assigned or committed, not because of the
    /// reading.
    pub assigned: bool,
    /// `science-drive-answers-the-launch`: a seat assigned another lane,
    /// driving because a rival's space race is under way and it leads.
    pub answering: bool,
}

/// The seat's science against the field's best: science a turn and techs
/// known, own and the best living major rival's.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ScienceStanding {
    pub own_science: f64,
    pub best_rival_science: f64,
    pub own_techs: usize,
    pub best_rival_techs: usize,
}

impl ScienceStanding {
    /// Leads the field: more science a turn than any rival, or at least as
    /// many techs as any.
    /// `science-drive-answers-the-launch`: more science a turn than any rival
    /// AND at least as many techs. The answer opens on this, not on
    /// [`Self::leads`]: live Emperor civvis-20261030T111154Z answered at t194
    /// on 288 science against 286 while 62 techs to 67 behind. The paired
    /// probe's one win (seed 51000: 576 against 392, 77 techs each) leads on
    /// both.
    pub fn leads_outright(&self) -> bool {
        self.own_science > self.best_rival_science && self.own_techs >= self.best_rival_techs
    }

    pub fn leads(&self) -> bool {
        self.own_science > self.best_rival_science || self.own_techs >= self.best_rival_techs
    }

    /// Version 2's meaningful adaptive lead. The version-one predicate stays
    /// above untouched so each family member remains independently testable.
    pub fn leads_v2(&self) -> bool {
        let science_lead = self.own_science >= SCIENCE_DRIVE_MIN_SCIENCE
            && self.own_science
                >= self.best_rival_science.max(1.0) * (1.0 + SCIENCE_DRIVE_LEAD_MARGIN);
        let tech_lead = self.own_techs
            >= self
                .best_rival_techs
                .saturating_add(SCIENCE_DRIVE_TECH_LEAD);
        science_lead || tech_lead
    }

    /// Close enough to the leader to keep driving.
    pub fn holds(&self) -> bool {
        self.own_science >= SCIENCE_DRIVE_HOLD * self.best_rival_science
            || self.own_techs + SCIENCE_DRIVE_TECH_SLACK >= self.best_rival_techs
    }

    /// Version 2 refuses to keep a stale drive alive from an all-zero,
    /// no-observation standing.
    pub fn holds_v2(&self) -> bool {
        let has_signal = self.own_science > 0.0 || self.own_techs > 0;
        let science_hold = self.own_science >= SCIENCE_DRIVE_HOLD * self.best_rival_science;
        let tech_hold =
            self.own_techs.saturating_add(SCIENCE_DRIVE_TECH_SLACK) >= self.best_rival_techs;
        has_signal && (science_hold || tech_hold)
    }
}

impl AdvancedAi {
    /// The lane this seat is playing for: the operator's assignment, or —
    /// while it drives the space race — Science. Only the science keys read
    /// this (the rocketry-path tech value and the space-race projects and
    /// Spaceports); every other read of an assigned lane, the objective
    /// resolutions included, keeps reading `victory_target`.
    pub(super) fn raced_target(&self) -> Option<VictoryTarget> {
        if self.victory_portfolio && self.portfolio_specializing() == Some(true) {
            return self.portfolio_target();
        }
        // `science-drive-answers-the-launch`: while the answer drives, the
        // space race's keys read Science beside the assigned lane.
        if self.science_drive.is_some_and(|drive| drive.answering) {
            return Some(VictoryTarget::Science);
        }
        self.victory_target.or_else(|| {
            self.science_drive_active()
                .then_some(VictoryTarget::Science)
        })
    }

    /// The drive's state, for instruments and tests.
    pub fn science_drive(&self) -> Option<ScienceDrive> {
        self.science_drive
    }

    /// Whether the seat is driving this turn.
    pub(super) fn science_drive_active(&self) -> bool {
        self.science_drive.is_some()
    }

    /// Either independently-screened implementation can maintain the common
    /// drive state. The version-two flag replaces, rather than patches, v1.
    fn science_drive_enabled(&self) -> bool {
        self.science_victory_drive || self.science_victory_drive_2
    }

    /// Whether an adaptive seat has a queued or completed launch of its own.
    /// That project is already consuming the finite race chain, so it can
    /// start the drive before the public focus score clears its opening floor.
    fn adaptive_science_launch_committed(&self, g: &Game, pid: usize) -> bool {
        self.victory_planning && self.victory_target.is_none() && g.victory_conditions.science && {
            SCIENCE_LAUNCH_PROJECTS.iter().any(|project| {
                g.players[pid].science_projects.contains(*project)
                    || Self::science_project_is_queued(g, pid, project)
            })
        }
    }

    /// Whether an adaptive seat has a credible Science commitment of its own.
    /// A public Science focus can start the chain before a lead exists, while
    /// a weak opening tie remains below the drive's normal floor.
    fn adaptive_science_plan(&self, g: &Game, pid: usize) -> bool {
        self.adaptive_science_launch_committed(g, pid)
            || (self.victory_planning
                && self.victory_target.is_none()
                && g.victory_conditions.science
                && {
                    let focus = self.victory_focus(g, pid);
                    focus.strategy == GrandStrategy::Science
                        && focus.progress >= SCIENCE_DRIVE_PLAN_FLOOR
                })
    }

    /// The empire's science a turn: every city's, the player-level extras,
    /// and on the live bridge the mirror's correction to the observed figure
    /// (`observed_yield_adjustments`), which is how the mirror itself derives
    /// the number it was shown.
    pub fn empire_science(g: &Game, pid: usize) -> f64 {
        // One memo for the whole sweep; see `Game::query_memo`.
        let _memo = g.query_memo();
        let cities: f64 = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.city_yields(cid).science)
            .sum();
        cities
            + g.player_yield_extras(pid).science
            + g.observed_yield_adjustments
                .get(&pid)
                .map_or(0.0, |adjustment| adjustment.science)
    }

    /// The seat's science against every living major's.
    pub fn science_standing(g: &Game, pid: usize) -> ScienceStanding {
        let _memo = g.query_memo();
        let mut standing = ScienceStanding {
            own_science: Self::empire_science(g, pid),
            own_techs: g.players[pid].techs.len(),
            ..ScienceStanding::default()
        };
        for rival in g
            .players
            .iter()
            .filter(|p| p.id != pid && p.alive && !p.is_minor && !p.is_barbarian)
        {
            standing.best_rival_science = standing
                .best_rival_science
                .max(Self::empire_science(g, rival.id));
            standing.best_rival_techs = standing.best_rival_techs.max(rival.techs.len());
        }
        standing
    }

    /// The turn an adaptive seat first reads the field.
    pub(super) fn science_drive_start(g: &Game) -> u32 {
        g.turn_limit()
            .map(|limit| (limit as f64 * SCIENCE_DRIVE_START) as u32)
            .unwrap_or_else(|| g.standard_duration(SCIENCE_DRIVE_START_STANDARD))
    }

    /// Read the field and decide whether the seat drives this turn. Exact
    /// no-op while the gene is off (the state is cleared). Called once a
    /// turn from `take_turn_inner`, before the plan is assessed.
    pub(super) fn maintain_science_drive(&mut self, g: &Game, pid: usize) {
        if self.portfolio_specializing().is_some()
            && self.portfolio_target() != Some(VictoryTarget::Science)
        {
            self.science_drive = None;
            return;
        }
        if !self.science_drive_enabled() || !g.victory_conditions.science {
            self.science_drive = None;
            return;
        }
        // `science-drive-answers-the-launch`: another lane's seat may drive
        // once a rival has landed a space project.
        let answering = self.science_drive_answers_the_launch
            && self
                .victory_target
                .is_some_and(|target| target != VictoryTarget::Science)
            && Self::rival_landed_a_space_project(g, pid)
            // A Diplomatic seat near its own victory keeps the lane; an open
            // answer then closes through the assigned-lane return below.
            && !(self.victory_target == Some(VictoryTarget::Diplomacy)
                && g.players[pid].dvp >= SCIENCE_ANSWER_DIPLO_YIELD);
        // `science-drive-leads-beside-the-lane`: from Online turn 140 a seat
        // assigned another lane also races on its own outright lead.
        let leading_open = self.science_lead_opens(g, pid);
        let assigned = match self.victory_target {
            Some(VictoryTarget::Science) => true,
            Some(_) if answering || leading_open => false,
            Some(_) => {
                self.science_drive = None;
                return;
            }
            None => false,
        };
        let review = g.standard_duration(SCIENCE_DRIVE_REVIEW).max(1);
        let start = Self::science_drive_start(g);
        let launch_committed = self.adaptive_science_launch_committed(g, pid);
        let portfolio_science = self.portfolio_specializing() == Some(true)
            && self.portfolio_target() == Some(VictoryTarget::Science);
        let due = match self.science_drive {
            Some(drive) => g.turn.saturating_sub(drive.reviewed) >= review,
            None => {
                assigned
                    || answering
                    || leading_open
                    || portfolio_science
                    || launch_committed
                    || (g.turn >= start && (g.turn - start).is_multiple_of(review))
            }
        };
        if !due {
            return;
        }
        let standing = Self::science_standing(g, pid);
        let adaptive_science_plan = self.adaptive_science_plan(g, pid);
        // `science-drive-answers-from-the-pad`: the answer also opens, and
        // holds, while our own landed projects are level with the leading
        // racer's (within one under `-2`), whatever the science reading.
        let from_the_pad = answering && self.answer_holds_the_pad(g, pid);
        let driving = assigned
            || portfolio_science
            || adaptive_science_plan
            || from_the_pad
            || match self.science_drive {
                Some(_) if self.science_victory_drive_2 => standing.holds_v2(),
                Some(_) => standing.holds(),
                None if answering => {
                    standing.leads_outright() || self.domination_answers_at_parity(&standing)
                }
                None if leading_open => Self::science_lead_is_meaningful(&standing),
                None if self.science_victory_drive_2 => g.turn >= start && standing.leads_v2(),
                None => g.turn >= start && standing.leads(),
            };
        if !driving {
            if self.science_drive.take().is_some() {
                think!(self.journal(), Strategy, Decision,
                       "The science drive stands down";
                       "{:.0} science a turn against the field's {:.0}, {} techs against {}",
                       standing.own_science, standing.best_rival_science,
                       standing.own_techs, standing.best_rival_techs);
            }
            return;
        }
        let launch_city = if self.science_victory_drive_2 {
            Self::science_drive_pick_launch_city_v2(g, pid)
        } else {
            Self::science_drive_pick_launch_city(g, pid)
        };
        let since = self.science_drive.map_or(g.turn, |drive| drive.since);
        if self.science_drive.is_none() {
            think!(self.journal(), Strategy, Decision,
                   "Driving for a science victory";
                   "{}: {:.0} science a turn against the field's {:.0}, {} techs against {}; \
                    launch city {}",
                   if assigned {
                       "the assigned lane"
                   } else if answering {
                       if self.domination_answers_at_parity(&standing)
                           && !standing.leads_outright()
                           && !from_the_pad
                       {
                           "domination-joins-the-race: a Domination seat within reach of the field's techs answers a rival's space race beside its war"
                       } else if from_the_pad && !standing.leads_outright() {
                           if self.science_drive_answers_from_the_pad_2 {
                               "science-drive-answers-from-the-pad-2: our own pads within one of the racer's, answered beside the assigned lane"
                           } else {
                               "science-drive-answers-from-the-pad: our own pads level with the racer's, answered beside the assigned lane"
                           }
                       } else {
                           "a rival's space race, answered beside the assigned lane"
                       }
                   } else if leading_open {
                       "science-drive-leads-beside-the-lane: an outright science lead, raced beside the assigned lane"
                   } else if adaptive_science_plan {
                       "the public Science focus"
                   } else {
                       "leading the field"
                   },
                   standing.own_science, standing.best_rival_science,
                   standing.own_techs, standing.best_rival_techs,
                   launch_city.map_or("none".to_string(), |cid| g.cities[&cid].name.clone()));
        }
        self.science_drive = Some(ScienceDrive {
            since,
            reviewed: g.turn,
            standing,
            launch_city,
            assigned,
            // A lead-opened drive races beside the lane exactly as an answer
            // does: `raced_target` reads Science, and a Domination seat's
            // space-race exclusions lift under `domination-joins-the-race`.
            answering: answering || leading_open,
        });
    }

    /// `science-drive-leads-beside-the-lane`: whether a seat assigned another
    /// lane may open the drive on its own lead this turn: the gene and the
    /// answer on, Online turn 140 reached, and not a Diplomatic seat near its
    /// own victory. `false` with the gene off.
    ///
    /// Live King, 2026-11-01: 7 of 25 seats assigned Culture, Religion or
    /// Domination led the field outright (Science 1.1x the best rival's and
    /// a tech more) before any rival landed a space project, 9 to 86 turns
    /// before it. The answer waited for that launch in every one. Two won
    /// anyway (071051Z, 110755Z). 075446Z (Religion) led from 184, no rival
    /// launched until 215, and it lost the Exoplanet race at 245 by four
    /// turns. 141505Z and 151815Z (Domination) led from 143 and 110, at 1.5x
    /// and 1.8x the field's Science by 160, opened the drive only at the
    /// rival launches of 190 and 184, landed their first projects at 211 and
    /// 208, and lost to rival Culture victories at 216 and 214. 091422Z and
    /// 131400Z led 13 and 9 turns before the first launch.
    pub(super) fn science_lead_opens(&self, g: &Game, pid: usize) -> bool {
        self.science_drive_leads_beside_the_lane
            && self.science_drive_answers_the_launch
            && self
                .victory_target
                .is_some_and(|target| target != VictoryTarget::Science)
            && g.turn >= g.standard_duration(SCIENCE_LEAD_OPEN_STANDARD)
            && !(self.victory_target == Some(VictoryTarget::Diplomacy)
                && g.players[pid].dvp >= SCIENCE_ANSWER_DIPLO_YIELD)
    }

    /// An outright lead worth racing on: ahead on techs and at least
    /// [`SCIENCE_DRIVE_LEAD_MARGIN`] over the best rival's Science.
    pub(super) fn science_lead_is_meaningful(standing: &ScienceStanding) -> bool {
        standing.leads_outright()
            && standing.own_science
                >= standing.best_rival_science * (1.0 + SCIENCE_DRIVE_LEAD_MARGIN)
    }

    /// `science-drive-answers-from-the-pad`: whether our own space race is
    /// level with the field's: at least one project landed, and as many as
    /// any met, living rival major. The answer opens on an outright science
    /// lead, which a seat already level on the pads can lack. Live King
    /// civvis-20261101T075446Z (Religion) landed its Satellite at 208 and its
    /// Moon at 215, ahead of the Ottomans (215, 224), with 320 science against
    /// 394: no drive opened, research stayed on the war plan (Predictive
    /// Systems over Smart Materials at 212), Smart Materials came at 227, our
    /// Exoplanet launched at 240 against theirs at 236, and the Ottomans won
    /// at 245. 071051Z, whose answer opened on an outright lead at 181, won.
    ///
    /// `science-drive-answers-from-the-pad-2`: within one project of every
    /// rival, not only level. Live King civvis-20261101T095815Z (v1 armed
    /// from t203) landed its Moon at 202; Gran Colombia landed Mars at 203,
    /// one ahead, so the pad reading never held and the answer waited for an
    /// outright science lead at 218. Research ran the war plan in between
    /// (Nuclear Fusion over Nanotechnology at 202, then Computers and
    /// Telecommunications), Smart Materials came at 221 and Offworld Mission
    /// at 231. Our Exoplanet launched at 223 against theirs at 221, no laser
    /// station could start before 231, and Gran Colombia won at 233.
    /// `false` with both genes off.
    pub(super) fn answer_holds_the_pad(&self, g: &Game, pid: usize) -> bool {
        if !self.science_drive_answers_from_the_pad && !self.science_drive_answers_from_the_pad_2 {
            return false;
        }
        let slack = usize::from(self.science_drive_answers_from_the_pad_2);
        let ours = Self::science_denial_stages(g, pid);
        ours >= 1
            && g.players
                .iter()
                .filter(|rival| {
                    rival.id != pid
                        && rival.alive
                        && !rival.is_minor
                        && !rival.is_barbarian
                        && g.has_met(pid, rival.id)
                })
                .all(|rival| Self::science_denial_stages(g, rival.id) <= ours + slack)
    }

    /// `science-drive-answers-the-launch`: whether a met, living rival major
    /// has landed a space project (the same four `science-threat-denial`
    /// counts as the physical race).
    pub(super) fn rival_landed_a_space_project(g: &Game, pid: usize) -> bool {
        g.players.iter().any(|rival| {
            rival.id != pid
                && rival.alive
                && !rival.is_minor
                && !rival.is_barbarian
                && g.has_met(pid, rival.id)
                && [
                    "launch_earth_satellite",
                    "launch_moon_landing",
                    "launch_mars_colony",
                    "exoplanet_expedition",
                ]
                .iter()
                .any(|project| rival.science_projects.contains(*project))
        })
    }

    /// The city the race runs from: the Spaceport city (the best producer
    /// of them), else the city with a pad in its queue, else the
    /// best-production city; ties to the older city.
    pub(super) fn science_drive_pick_launch_city(g: &Game, pid: usize) -> Option<u32> {
        let pad = crate::name!("spaceport");
        let _memo = g.query_memo();
        let rank = |cid: u32| {
            let city = &g.cities[&cid];
            let tier = if city.districts.contains_key(pad) {
                2
            } else if city.queue.iter().any(|item| {
                matches!(item, Item::District { district, .. } if g.district_family(*district) == pad)
            }) {
                1
            } else {
                0
            };
            (tier, g.city_yields(cid).production, std::cmp::Reverse(cid))
        };
        g.player_city_ids(pid).into_iter().max_by(|a, b| {
            rank(*a)
                .partial_cmp(&rank(*b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Version 2 does not remember a high-production city as the launch city
    /// when the current board says it has no legal Spaceport site. Existing or
    /// queued pads remain viable if a partial host export omits plot detail.
    pub(super) fn science_drive_pick_launch_city_v2(g: &Game, pid: usize) -> Option<u32> {
        let pad = crate::name!("spaceport");
        let _memo = g.query_memo();
        let viable = |cid: u32| {
            let city = &g.cities[&cid];
            city.districts.contains_key(pad)
                || city.queue.iter().any(|item| {
                    matches!(item, Item::District { district, .. }
                        if g.district_family(*district) == pad)
                })
                || !g.district_sites(cid, pad).is_empty()
        };
        let rank = |cid: u32| {
            let city = &g.cities[&cid];
            let tier = if city.districts.contains_key(pad) {
                2
            } else if city.queue.iter().any(|item| {
                matches!(item, Item::District { district, .. } if g.district_family(*district) == pad)
            }) {
                1
            } else {
                0
            };
            (tier, g.city_yields(cid).production, std::cmp::Reverse(cid))
        };
        g.player_city_ids(pid)
            .into_iter()
            .filter(|cid| viable(*cid))
            .max_by(|a, b| {
                rank(*a)
                    .partial_cmp(&rank(*b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// The launch city this turn: the one read at the last review while it
    /// is still ours, else re-picked.
    pub(super) fn science_drive_launch_city(&self, g: &Game, pid: usize) -> Option<u32> {
        let drive = self.science_drive?;
        if self.science_victory_drive_2 {
            return drive
                .launch_city
                .filter(|cid| {
                    g.cities.get(cid).is_some_and(|city| {
                        city.owner == pid
                            && (city.districts.contains_key(crate::name!("spaceport"))
                                || city.queue.iter().any(|item| {
                                    matches!(item, Item::District { district, .. }
                                        if g.district_family(*district) == "spaceport")
                                })
                                || !g.district_sites(*cid, crate::name!("spaceport")).is_empty())
                    })
                })
                .or_else(|| Self::science_drive_pick_launch_city_v2(g, pid));
        }
        drive
            .launch_city
            .filter(|cid| g.cities.get(cid).is_some_and(|city| city.owner == pid))
            .or_else(|| Self::science_drive_pick_launch_city(g, pid))
    }

    /// The dispatcher's half: the space-race pass runs for a driving seat
    /// under every plan short of Recovery.
    pub(super) fn science_drive_opens(&self, strategy: GrandStrategy) -> bool {
        self.science_drive_active() && strategy != GrandStrategy::Recovery
    }

    /// Whether a driving seat spends its Gold on the race: Rocketry known.
    pub(super) fn science_drive_spends(&self, g: &Game, pid: usize) -> bool {
        self.science_drive_active() && g.players[pid].techs.contains(&crate::name!("rocketry"))
    }

    /// The first tech of the chain the seat does not know, if any.
    pub(super) fn science_drive_milestone(g: &Game, pid: usize) -> Option<&'static str> {
        let techs = &g.players[pid].techs;
        SCIENCE_DRIVE_CHAIN
            .into_iter()
            .find(|tech| !techs.contains(&Name::new(tech)))
    }

    /// Value of `tech` to a driving seat beyond the chain milestone (which
    /// `tech_value` prices at 900 through `raced_target`): before Rocketry,
    /// the techs on the way to the launch city's Factory and Power Plant.
    pub(super) fn science_drive_tech_bonus(&self, g: &Game, pid: usize, tech: &str) -> f64 {
        if !self.science_drive_active() {
            return 0.0;
        }
        let techs = &g.players[pid].techs;
        if techs.contains(&crate::name!("rocketry")) || g.world_era < SCIENCE_DRIVE_PRODUCTION_ERA {
            return 0.0;
        }
        ["industrialization", "electricity"]
            .into_iter()
            .filter(|goal| !techs.contains(&Name::new(goal)) && self.tech_leads_to(g, tech, goal))
            .map(|_| SCIENCE_DRIVE_PRODUCTION_TECH)
            .sum()
    }

    /// The race's bottleneck, priced in the launch city: the zone chain
    /// from the Industrial era, the pad once Rocketry is known, the Academy
    /// once the policy card exists, the Royal Society. Zero elsewhere.
    pub(super) fn science_drive_production_bonus(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
    ) -> f64 {
        if self.science_victory_drive_2 {
            return self.science_drive_production_bonus_v2(g, pid, cid, item);
        }
        let Some(drive) = self.science_drive else {
            return 0.0;
        };
        if drive.launch_city != Some(cid) {
            return 0.0;
        }
        let city = &g.cities[&cid];
        let player = &g.players[pid];
        let rocketry = player.techs.contains(&crate::name!("rocketry"));
        let industrial = rocketry || g.world_era >= SCIENCE_DRIVE_PRODUCTION_ERA;
        match item {
            Item::District { district, .. } => {
                let family = g.district_family(*district);
                if family == "spaceport" {
                    if rocketry && !city.districts.contains_key(crate::name!("spaceport")) {
                        SCIENCE_DRIVE_PAD_BONUS
                    } else {
                        0.0
                    }
                } else if family == "industrial_zone"
                    && industrial
                    && !city
                        .districts
                        .keys()
                        .any(|held| g.district_family(*held) == "industrial_zone")
                {
                    SCIENCE_DRIVE_ZONE_BONUS
                } else {
                    0.0
                }
            }
            Item::Building { building } if industrial => {
                let base = Self::base_building(g, building);
                let held_power_plant = city
                    .buildings
                    .iter()
                    .any(|held| POWER_PLANTS.contains(&Self::base_building(g, held)));
                if POWER_PLANTS.contains(&base) && held_power_plant {
                    return 0.0;
                }
                if base == "military_academy"
                    && !player.civics.contains(&crate::name!("space_race"))
                {
                    return 0.0;
                }
                SCIENCE_DRIVE_BUILDING_BONUS
                    .iter()
                    .find(|(name, _)| *name == base)
                    .map_or(0.0, |(_, bonus)| *bonus)
            }
            _ => 0.0,
        }
    }

    /// Version 2's production path. Unlike v1 it can price the research
    /// funnel in the launch city, re-picks an invalid launch city, and never
    /// pays a second Spaceport credit while that city already queues one.
    fn science_drive_production_bonus_v2(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
    ) -> f64 {
        let Some(_) = self.science_drive else {
            return 0.0;
        };
        let city = &g.cities[&cid];
        let player = &g.players[pid];
        // Support that cannot finish before a known rival flight arrives
        // cannot accelerate our launch. Keep racing projects themselves alive:
        // a deadline is a reason to cut detours, not abandon the race.
        if matches!(item, Item::Building { .. } | Item::District { .. })
            && !matches!(item, Item::District { district, .. }
                if g.district_family(*district) == "spaceport")
            && Self::science_project_build_turns(g, pid, cid, item)
                >= Self::science_support_horizon(g, pid)
        {
            return 0.0;
        }
        // The launch chain has one serial bottleneck: every queue spent on a
        // research building in a non-launch city is a queue not available to
        // the next Spaceport or a parallel production site.  V2 originally
        // paid this credit in every city, which made the deployed Science
        // lane fill the empire with Campus buildings while its first pad and
        // projects slipped past the 250-turn clock.  Keep the funnel credit
        // where it compounds the race, and let the ordinary science-building
        // debt value the other cities.
        let launch_city = self.science_drive_launch_city(g, pid);
        let research_bonus = if city.owner == pid && launch_city == Some(cid) {
            Self::science_drive_research_bonus(g, pid, city, item)
        } else {
            0.0
        };
        if launch_city != Some(cid) {
            return research_bonus;
        }
        let rocketry = player.techs.contains(&crate::name!("rocketry"));
        let industrial = rocketry || g.world_era >= SCIENCE_DRIVE_PRODUCTION_ERA;
        research_bonus
            + match item {
                Item::District { district, .. } => {
                    let family = g.district_family(*district);
                    if family == "spaceport" {
                        let pad_queued = city.queue.iter().any(|queued| {
                            matches!(queued, Item::District { district, .. }
                            if g.district_family(*district) == "spaceport")
                        });
                        if rocketry
                            && !city.districts.contains_key(crate::name!("spaceport"))
                            && !pad_queued
                        {
                            SCIENCE_DRIVE_PAD_BONUS
                        } else {
                            0.0
                        }
                    } else if family == "industrial_zone"
                        && industrial
                        && !city
                            .districts
                            .keys()
                            .any(|held| g.district_family(*held) == "industrial_zone")
                    {
                        SCIENCE_DRIVE_ZONE_BONUS
                    } else {
                        0.0
                    }
                }
                Item::Building { building } if industrial => {
                    let base = Self::base_building(g, building);
                    let held_power_plant = city
                        .buildings
                        .iter()
                        .any(|held| POWER_PLANTS.contains(&Self::base_building(g, held)));
                    if POWER_PLANTS.contains(&base) && held_power_plant {
                        return 0.0;
                    }
                    if base == "military_academy"
                        && !player.civics.contains(&crate::name!("space_race"))
                    {
                        return 0.0;
                    }
                    SCIENCE_DRIVE_BUILDING_BONUS
                        .iter()
                        .find(|(name, _)| *name == base)
                        .map_or(0.0, |(_, bonus)| *bonus)
                }
                _ => 0.0,
            }
    }

    /// Earliest observable flight deadline; unmet rivals never supply private
    /// progress. Missing launch evidence leaves the configured turn limit.
    fn science_support_horizon(g: &Game, pid: usize) -> f64 {
        let limit = if g.max_turns == 0 {
            f64::INFINITY
        } else {
            g.max_turns.saturating_sub(g.turn) as f64
        };
        g.players
            .iter()
            .filter(|rival| {
                rival.id != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && g.has_met(pid, rival.id)
                    && rival.science_projects.contains("exoplanet_expedition")
            })
            .map(|rival| {
                ((g.science_victory_points_needed(rival.id) - g.science_victory_points(rival.id))
                    .max(0.0)
                    / g.exoplanet_speed(rival.id).max(1.0))
                .ceil()
            })
            .fold(limit, f64::min)
    }

    /// Compare actual remaining work at this city's item-specific rate.
    /// Production banks and Space Race modifiers can reverse a raw-yield rank.
    pub(super) fn science_project_build_turns(g: &Game, pid: usize, cid: u32, item: &Item) -> f64 {
        g.item_remaining_cost_for_city(pid, cid, item)
            / (g.city_yields(cid).production.max(0.1)
                * g.item_prod_mult(pid, cid, Some(item)).max(0.1))
    }

    /// Version 2 keeps the launch city's research funnel alive even when the
    /// general strategic plan has not selected Science. The credit gets the
    /// launch city's Campus and each missing research rung rather than only
    /// pricing its industrial chain. Once that funnel and the local
    /// production chain are complete, it also keeps a Campus Research Grants
    /// queue competitive with the rest of the late-game filler.
    fn science_drive_research_bonus(
        g: &Game,
        pid: usize,
        city: &crate::game::City,
        item: &Item,
    ) -> f64 {
        match item {
            Item::District { district, .. }
                if g.district_family(*district) == crate::name!("campus")
                    && !city
                        .districts
                        .keys()
                        .any(|held| g.district_family(*held) == crate::name!("campus"))
                    && !city.queue.iter().any(|queued| {
                        matches!(queued, Item::District { district, .. }
                            if g.district_family(*district) == crate::name!("campus"))
                    }) =>
            {
                SCIENCE_DRIVE_CAMPUS_BONUS
            }
            Item::Building { building }
                if city
                    .districts
                    .keys()
                    .any(|held| g.district_family(*held) == crate::name!("campus"))
                    && !city.buildings.iter().any(|held| {
                        Self::base_building(g, held) == Self::base_building(g, building)
                    }) =>
            {
                SCIENCE_DRIVE_RESEARCH_BUILDING_BONUS
                    .iter()
                    .find(|(name, _)| *name == Self::base_building(g, building))
                    .map_or(0.0, |(_, bonus)| *bonus)
            }
            Item::Project { project }
                if project == "campus_research_grants"
                    && city
                        .districts
                        .keys()
                        .any(|held| g.district_family(*held) == crate::name!("campus"))
                    && !Self::science_drive_project_building_debt(g, pid, city.id) =>
            {
                SCIENCE_DRIVE_CAMPUS_PROJECT_BONUS
            }
            _ => 0.0,
        }
    }

    /// Whether a city can still start one of the concrete buildings that a
    /// repeatable Campus project must wait behind. A unique replacement counts
    /// as its base building, and an already queued copy is debt already being
    /// paid. Power plants are one chain rung: once any plant stands or is
    /// queued, another fuel is not a reason to suppress research grants.
    fn science_drive_project_building_debt(g: &Game, pid: usize, cid: u32) -> bool {
        let city = &g.cities[&cid];
        let has_power_plant = city
            .buildings
            .iter()
            .any(|building| POWER_PLANTS.contains(&Self::base_building(g, building)))
            || city.queue.iter().any(|item| {
                matches!(item, Item::Building { building }
                if POWER_PLANTS.contains(&Self::base_building(g, building)))
            });

        SCIENCE_DRIVE_PROJECT_BUILDING_DEBT
            .iter()
            .filter(|base| !has_power_plant || !POWER_PLANTS.contains(base))
            .any(|base| {
                let held_or_queued = city
                    .buildings
                    .iter()
                    .any(|building| Self::base_building(g, building) == *base)
                    || city.queue.iter().any(|item| {
                        matches!(item, Item::Building { building }
                        if Self::base_building(g, building) == *base)
                    });
                if held_or_queued {
                    return false;
                }
                g.rules.buildings.keys().any(|building| {
                    Self::base_building(g, building) == *base
                        && g.can_produce(
                            pid,
                            cid,
                            &Item::Building {
                                building: *building,
                            },
                        )
                })
            })
    }

    /// A unique building's base, else the building itself.
    fn base_building<'a>(g: &'a Game, building: &'a Name) -> &'a str {
        g.rules
            .buildings
            .get(building)
            .and_then(|spec| spec.replaces.as_ref())
            .map_or(building.as_str(), |base| base.as_str())
    }

    /// Pingala's preference for the launch city once a pad stands there.
    pub(super) fn science_drive_governor_bonus(&self, g: &Game, governor: &str, cid: u32) -> f64 {
        if self.science_victory_drive_2 {
            let Some(_) = self.science_drive else {
                return 0.0;
            };
            let Some(pid) = g.cities.get(&cid).map(|city| city.owner) else {
                return 0.0;
            };
            if governor != "pingala" || self.science_drive_launch_city(g, pid) != Some(cid) {
                return 0.0;
            }
            return if g.cities[&cid]
                .districts
                .contains_key(crate::name!("spaceport"))
            {
                SCIENCE_DRIVE_PINGALA_BONUS
            } else {
                0.0
            };
        }
        let Some(drive) = self.science_drive else {
            return 0.0;
        };
        if governor != "pingala" || drive.launch_city != Some(cid) {
            return 0.0;
        }
        if g.cities[&cid]
            .districts
            .contains_key(crate::name!("spaceport"))
        {
            SCIENCE_DRIVE_PINGALA_BONUS
        } else {
            0.0
        }
    }

    /// Pads a driving seat wants: two once the Earth Satellite is up, three
    /// once the Mars colony is; one before.
    pub(super) fn science_drive_desired_pads(completed: &BTreeSet<String>) -> usize {
        if completed.contains("launch_mars_colony") {
            3
        } else if completed.contains("launch_earth_satellite") {
            2
        } else {
            1
        }
    }
}

#[cfg(test)]
mod lead_tests;
#[cfg(test)]
mod pad_tests;

#[cfg(test)]
mod tests;
