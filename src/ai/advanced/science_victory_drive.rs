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
use crate::game::{Game, Item, EXOPLANET_DESTINATION};
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

    /// The empire's science a turn: every city's, the player-level extras,
    /// and on the live bridge the mirror's correction to the observed figure
    /// (`observed_yield_adjustments`), which is how the mirror itself derives
    /// the number it was shown.
    pub fn empire_science(g: &Game, pid: usize) -> f64 {
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
        if !self.science_drive_enabled() || !g.victory_conditions.science {
            self.science_drive = None;
            return;
        }
        let assigned = match self.victory_target {
            Some(VictoryTarget::Science) => true,
            Some(_) => {
                self.science_drive = None;
                return;
            }
            None => false,
        };
        let review = g.standard_duration(SCIENCE_DRIVE_REVIEW).max(1);
        let start = Self::science_drive_start(g);
        let due = match self.science_drive {
            Some(drive) => g.turn.saturating_sub(drive.reviewed) >= review,
            None => assigned || (g.turn >= start && (g.turn - start).is_multiple_of(review)),
        };
        if !due {
            return;
        }
        let standing = Self::science_standing(g, pid);
        let driving = assigned
            || match self.science_drive {
                Some(_) if self.science_victory_drive_2 => standing.holds_v2(),
                Some(_) => standing.holds(),
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
                   if assigned { "the assigned lane" } else { "leading the field" },
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
        });
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
                ((EXOPLANET_DESTINATION - rival.exoplanet_distance).max(0.0)
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
mod tests {
    use super::*;
    use crate::ai::StrategicPlan;
    use crate::game::{Action, Game};
    use crate::Pos;

    /// A two-player board with one founded city each on a 200-turn clock.
    fn board() -> (Game, u32, u32) {
        let mut g = Game::new(2, 24, 16, 71, 200, 0);
        let found = |g: &mut Game, pid: usize| {
            let settler = g
                .player_unit_ids(pid)
                .into_iter()
                .find(|uid| g.units[uid].kind == "settler")
                .unwrap();
            g.current = pid;
            g.apply(pid, &Action::FoundCity { unit: settler }).unwrap();
            g.player_city_ids(pid)[0]
        };
        let ours = found(&mut g, 0);
        let theirs = found(&mut g, 1);
        g.current = 0;
        (g, ours, theirs)
    }

    /// A flat, empty tile of the city for a district.
    fn flat_site(g: &mut Game, city: u32) -> Pos {
        let center = g.cities[&city].pos;
        let site = g.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != center && g.map.tiles[position].district.is_none())
            .unwrap();
        let tile = g.map.tiles.get_mut(&site).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
        site
    }

    fn install_pad(g: &mut Game, city: u32) -> Pos {
        let site = flat_site(g, city);
        g.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("spaceport"));
        g.cities
            .get_mut(&city)
            .unwrap()
            .districts
            .insert(crate::name!("spaceport"), site);
        site
    }

    fn give_techs(g: &mut Game, pid: usize, count: usize) {
        let techs: Vec<Name> = g
            .rules
            .techs
            .keys()
            .take(count)
            .map(|t| Name::new(t))
            .collect();
        g.players[pid].techs.extend(techs);
    }

    #[test]
    fn targeted_launch_uses_banked_progress_instead_of_raw_production_rank() {
        let (mut g, ours, second) = board();
        g.cities.get_mut(&second).unwrap().owner = 0;
        for tech in g.rules.tech_ancestors["rocketry"].clone() {
            g.players[0].techs.insert(Name::new(&tech));
        }
        g.players[0].techs.insert(crate::name!("rocketry"));
        install_pad(&mut g, ours);
        install_pad(&mut g, second);
        std::sync::Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
            ours,
            crate::rules::Yields {
                production: 100.0,
                ..Default::default()
            },
        );
        let project = Item::Project {
            project: crate::name!("launch_earth_satellite"),
        };
        let cost = g.item_cost_for_city(0, second, &project);
        g.cities
            .get_mut(&second)
            .unwrap()
            .production_progress
            .insert("project:launch_earth_satellite".into(), cost - 1.0);
        assert!(
            AdvancedAi::science_project_build_turns(&g, 0, second, &project)
                < AdvancedAi::science_project_build_turns(&g, 0, ours, &project)
        );
        AdvancedAi::targeting(VictoryTarget::Science).science_production(&mut g, 0);
        assert_eq!(g.cities[&second].queue.first(), Some(&project));
    }

    #[test]
    fn a_known_rival_flight_cuts_support_detours_without_private_information() {
        let (mut g, ours, _) = board();
        g.players[1]
            .science_projects
            .insert("exoplanet_expedition".into());
        g.players[1].exoplanet_distance = 49.0;
        // The fixture can spawn within sight; explicitly forget contact first.
        g.players[0].met.clear();
        assert_eq!(
            AdvancedAi::science_support_horizon(&g, 0),
            200.0 - g.turn as f64
        );
        g.players[0].met.insert(1);
        assert_eq!(AdvancedAi::science_support_horizon(&g, 0), 1.0);
        give_techs(&mut g, 0, 30);
        let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
        ai.enable_science_victory_drive_2();
        ai.maintain_science_drive(&g, 0);
        assert_eq!(
            ai.science_drive_production_bonus(
                &g,
                0,
                ours,
                &Item::Building {
                    building: crate::name!("research_lab")
                }
            ),
            0.0
        );
    }

    #[test]
    fn off_by_default_and_toggles() {
        let ai = AdvancedAi::new();
        assert!(!ai.science_victory_drive, "an opt-in ships off");
        assert!(!ai.science_victory_drive_2, "version 2 also ships off");
        assert!(ai.science_drive().is_none());
        assert!(!AdvancedAi::legacy().science_victory_drive);
        assert!(!AdvancedAi::legacy().science_victory_drive_2);
        let v2 = crate::ai::advanced::genes::gene("science-victory-drive-2")
            .expect("version 2 is registered");
        assert!(v2.opt_in() && v2.screenable() && !v2.live());
        let mut ai = AdvancedAi::new();
        ai.enable_science_victory_drive();
        assert!(ai.science_victory_drive);
        assert!(!ai.science_victory_drive_2);
        ai.enable_science_victory_drive_2();
        assert!(
            !ai.science_victory_drive && ai.science_victory_drive_2,
            "a family seat plays the new implementation, not both versions"
        );
        ai.disable_science_victory_drive_2();
        assert!(!ai.science_victory_drive_2);
        ai.enable_science_victory_drive();
        ai.disable_science_victory_drive();
        assert!(!ai.science_victory_drive);
    }

    #[test]
    fn the_standing_leads_and_holds() {
        let ahead = ScienceStanding {
            own_science: 100.0,
            best_rival_science: 80.0,
            own_techs: 40,
            best_rival_techs: 44,
        };
        assert!(ahead.leads(), "leads on science though behind on techs");
        let by_techs = ScienceStanding {
            own_science: 50.0,
            best_rival_science: 80.0,
            own_techs: 44,
            best_rival_techs: 44,
        };
        assert!(by_techs.leads(), "leads on techs though behind on science");
        assert!(
            !by_techs.leads_v2(),
            "version 2 does not mistake parity for a lead"
        );
        let one_tech_ahead = ScienceStanding {
            own_science: 50.0,
            best_rival_science: 80.0,
            own_techs: 45,
            best_rival_techs: 44,
        };
        assert!(
            one_tech_ahead.leads_v2(),
            "one completed tech is a meaningful version-2 lead"
        );
        assert!(
            !ScienceStanding::default().holds_v2(),
            "an unobserved standing cannot keep version 2 driving"
        );
        let slipping = ScienceStanding {
            own_science: 62.0,
            best_rival_science: 80.0,
            own_techs: 41,
            best_rival_techs: 44,
        };
        assert!(!slipping.leads());
        assert!(
            slipping.holds(),
            "within the hold on science and within the tech slack"
        );
        let gone = ScienceStanding {
            own_science: 50.0,
            best_rival_science: 80.0,
            own_techs: 40,
            best_rival_techs: 44,
        };
        assert!(!gone.holds(), "below the hold and past the slack");
    }

    #[test]
    fn a_seat_leading_the_field_drives_after_the_start_and_no_sooner() {
        let (mut g, ours, _) = board();
        give_techs(&mut g, 0, 30);
        give_techs(&mut g, 1, 20);
        let mut ai = AdvancedAi::new();
        ai.maintain_science_drive(&g, 0);
        assert!(ai.science_drive().is_none(), "off: no drive");
        ai.enable_science_victory_drive();
        g.turn = AdvancedAi::science_drive_start(&g) - 1;
        ai.maintain_science_drive(&g, 0);
        assert!(
            ai.science_drive().is_none(),
            "before the start the field is not read"
        );
        g.turn = AdvancedAi::science_drive_start(&g);
        ai.maintain_science_drive(&g, 0);
        let drive = ai
            .science_drive()
            .expect("leading the field in techs drives");
        assert_eq!(drive.launch_city, Some(ours));
        assert!(!drive.assigned);
        assert_eq!(ai.raced_target(), Some(VictoryTarget::Science));
        assert!(ai.science_drive_opens(GrandStrategy::Conquest));
        assert!(!ai.science_drive_opens(GrandStrategy::Recovery));
        ai.disable_science_victory_drive();
        ai.maintain_science_drive(&g, 0);
        assert!(
            ai.science_drive().is_none(),
            "switching the gene off clears the state"
        );
    }

    #[test]
    fn a_seat_behind_the_field_does_not_drive_but_an_assigned_one_does() {
        let (mut g, _, _) = board();
        give_techs(&mut g, 0, 20);
        give_techs(&mut g, 1, 30);
        g.turn = AdvancedAi::science_drive_start(&g);
        let mut ai = AdvancedAi::new();
        ai.enable_science_victory_drive();
        ai.maintain_science_drive(&g, 0);
        assert!(
            ai.science_drive().is_none(),
            "behind on techs and level on science"
        );
        assert_eq!(ai.raced_target(), None);

        let mut assigned = AdvancedAi::targeting(VictoryTarget::Science);
        assigned.enable_science_victory_drive();
        g.turn = 1;
        assigned.maintain_science_drive(&g, 0);
        assert!(
            assigned.science_drive().is_some_and(|d| d.assigned),
            "the assigned lane drives from turn one"
        );

        let mut other = AdvancedAi::targeting(VictoryTarget::Culture);
        other.enable_science_victory_drive();
        give_techs(&mut g, 0, 40);
        g.turn = AdvancedAi::science_drive_start(&g);
        other.maintain_science_drive(&g, 0);
        assert!(
            other.science_drive().is_none(),
            "a seat assigned another lane never drives"
        );
    }

    #[test]
    fn the_launch_city_prices_its_production_chain_and_its_pad() {
        let (mut g, ours, theirs) = board();
        give_techs(&mut g, 0, 30);
        g.turn = AdvancedAi::science_drive_start(&g);
        g.world_era = SCIENCE_DRIVE_PRODUCTION_ERA;
        let mut ai = AdvancedAi::new();
        ai.enable_science_victory_drive();
        ai.maintain_science_drive(&g, 0);
        let site = flat_site(&mut g, ours);
        let zone = Item::District {
            district: crate::name!("industrial_zone"),
            pos: site,
        };
        let factory = Item::Building {
            building: crate::name!("factory"),
        };
        let pad = Item::District {
            district: crate::name!("spaceport"),
            pos: site,
        };
        assert_eq!(
            ai.science_drive_production_bonus(&g, 0, ours, &zone),
            SCIENCE_DRIVE_ZONE_BONUS
        );
        assert_eq!(
            ai.science_drive_production_bonus(&g, 0, ours, &factory),
            700.0
        );
        assert_eq!(
            ai.science_drive_production_bonus(&g, 0, ours, &pad),
            0.0,
            "no pad before Rocketry"
        );
        assert_eq!(
            ai.science_drive_production_bonus(&g, 0, theirs, &zone),
            0.0,
            "not the launch city"
        );
        g.players[0].techs.insert(crate::name!("rocketry"));
        assert_eq!(
            ai.science_drive_production_bonus(&g, 0, ours, &pad),
            SCIENCE_DRIVE_PAD_BONUS
        );
        assert!(ai.science_drive_spends(&g, 0));
        g.cities
            .get_mut(&ours)
            .unwrap()
            .buildings
            .push(crate::name!("coal_power_plant"));
        let oil = Item::Building {
            building: crate::name!("oil_power_plant"),
        };
        assert_eq!(
            ai.science_drive_production_bonus(&g, 0, ours, &oil),
            0.0,
            "one power plant is enough"
        );
        let off = AdvancedAi::new();
        assert_eq!(off.science_drive_production_bonus(&g, 0, ours, &zone), 0.0);
    }

    #[test]
    fn science_drive_v2_only_selects_a_legal_launch_city() {
        let (mut g, ours, theirs) = board();
        // Use two cities in one empire, then make the higher-production one
        // a host-refused Spaceport site. The version-2 picker must not retain
        // it merely because it has the better production figure.
        g.cities.get_mut(&theirs).unwrap().owner = 0;
        std::sync::Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
            ours,
            crate::rules::Yields {
                production: 100.0,
                ..crate::rules::Yields::default()
            },
        );
        let blocked: BTreeSet<Name> = [crate::name!("spaceport")].into_iter().collect();
        std::sync::Arc::make_mut(&mut g.blocked_districts).insert(ours, blocked.clone());
        assert!(g.district_sites(ours, crate::name!("spaceport")).is_empty());
        assert!(!g
            .district_sites(theirs, crate::name!("spaceport"))
            .is_empty());
        assert_eq!(
            AdvancedAi::science_drive_pick_launch_city_v2(&g, 0),
            Some(theirs),
            "a legal site outranks an unbuildable high-production city"
        );
        std::sync::Arc::make_mut(&mut g.blocked_districts).insert(theirs, blocked);
        assert_eq!(
            AdvancedAi::science_drive_pick_launch_city_v2(&g, 0),
            None,
            "with no legal or already-committed pad, there is no launch city"
        );
    }

    #[test]
    fn science_drive_v2_keeps_a_live_pad_moving_on_its_next_rung() {
        let (mut g, ours, _) = board();
        give_techs(&mut g, 0, 30);
        for tech in g.rules.tech_ancestors["rocketry"].clone() {
            g.players[0].techs.insert(Name::new(&tech));
        }
        g.players[0].techs.insert(crate::name!("rocketry"));
        install_pad(&mut g, ours);
        std::sync::Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
            ours,
            crate::rules::Yields {
                production: 60.0,
                ..crate::rules::Yields::default()
            },
        );
        // The remaining full chain cannot fit, but the pad can still run its
        // next project. Version 2 must keep that launch path alive rather
        // than treating a currently incomplete whole race as a hard stop.
        g.turn = g.max_turns - 10;
        let mut v1 = AdvancedAi::new();
        v1.enable_science_victory_drive();
        v1.maintain_science_drive(&g, 0);
        let mut v2 = AdvancedAi::new();
        v2.enable_science_victory_drive_2();
        v2.maintain_science_drive(&g, 0);
        assert!(v1.science_drive().is_some());
        assert!(v2.science_drive().is_some());
    }

    #[test]
    fn science_race_repairs_a_pillaged_spaceport_before_stalling() {
        let (mut g, ours, _) = board();
        let pad = install_pad(&mut g, ours);
        g.map.tiles.get_mut(&pad).unwrap().pillaged = true;
        let project = Item::Project {
            project: crate::name!("launch_earth_satellite"),
        };
        g.cities.get_mut(&ours).unwrap().queue.push(project);
        g.cities.get_mut(&ours).unwrap().production = 123.0;

        let ai = AdvancedAi::targeting(VictoryTarget::Science);
        ai.repair_stalled_science_project_queues(&mut g, 0);

        assert!(matches!(
            g.cities[&ours].queue.first(),
            Some(Item::Repair { repair, pos })
                if repair == "district" && *pos == pad
        ));
        assert_eq!(
            g.cities[&ours]
                .production_progress
                .get("project:launch_earth_satellite"),
            Some(&123.0),
            "switching to repair banks the stalled project's progress"
        );
    }

    #[test]
    fn science_drive_v2_values_the_research_funnel_only_in_the_launch_city() {
        let (mut g, ours, theirs) = board();
        // Make both cities ours and give the first one a standing pad. The
        // second city is therefore a real non-launch queue, not another
        // player's city accidentally included in a valuation test.
        g.cities.get_mut(&theirs).unwrap().owner = 0;
        install_pad(&mut g, ours);
        give_techs(&mut g, 0, 30);
        g.turn = AdvancedAi::science_drive_start(&g);
        let mut v1 = AdvancedAi::new();
        v1.enable_science_victory_drive();
        v1.maintain_science_drive(&g, 0);
        let mut v2 = AdvancedAi::new();
        v2.enable_science_victory_drive_2();
        v2.maintain_science_drive(&g, 0);
        assert!(v1.science_drive().is_some());
        assert!(v2.science_drive().is_some());
        assert_eq!(
            v2.science_drive().and_then(|drive| drive.launch_city),
            Some(ours)
        );

        let campus_site = flat_site(&mut g, theirs);
        let campus = Item::District {
            district: crate::name!("campus"),
            pos: campus_site,
        };
        assert_eq!(
            v1.science_drive_production_bonus(&g, 0, theirs, &campus),
            0.0,
            "version 1 keeps its original launch-city-only pricing"
        );
        assert_eq!(
            v2.science_drive_production_bonus(&g, 0, theirs, &campus),
            0.0,
            "version 2 does not pull a non-launch queue into the research funnel"
        );

        let launch_campus_site = flat_site(&mut g, ours);
        assert_eq!(
            v2.science_drive_production_bonus(&g, 0, ours, &campus),
            SCIENCE_DRIVE_CAMPUS_BONUS,
            "version 2 keeps the research funnel in its launch city"
        );
        g.map.tiles.get_mut(&launch_campus_site).unwrap().district = Some(crate::name!("campus"));
        g.cities
            .get_mut(&ours)
            .unwrap()
            .districts
            .insert(crate::name!("campus"), launch_campus_site);
        g.map.tiles.get_mut(&campus_site).unwrap().district = Some(crate::name!("campus"));
        g.cities
            .get_mut(&theirs)
            .unwrap()
            .districts
            .insert(crate::name!("campus"), campus_site);
        let library = Item::Building {
            building: crate::name!("library"),
        };
        assert_eq!(
            v1.science_drive_production_bonus(&g, 0, theirs, &library),
            0.0,
            "the original gene remains unchanged"
        );
        assert_eq!(
            v2.science_drive_production_bonus(&g, 0, theirs, &library),
            0.0,
            "a non-launch city does not get the launch funnel's building credit"
        );
        assert_eq!(
            v2.science_drive_production_bonus(&g, 0, ours, &library),
            SCIENCE_DRIVE_RESEARCH_BUILDING_BONUS[0].1
        );
        // Make the first research rung genuinely available so the project
        // guard is tested against a legal alternative, not merely a missing
        // technology in this small board.
        g.players[0].techs.insert(crate::name!("writing"));
        let grants = Item::Project {
            project: crate::name!("campus_research_grants"),
        };
        assert_eq!(
            v2.science_drive_production_bonus(&g, 0, theirs, &grants),
            0.0,
            "the non-launch city cannot claim the launch funnel's project credit"
        );
        g.cities
            .get_mut(&theirs)
            .unwrap()
            .buildings
            .push(crate::name!("library"));
        assert_eq!(
            v2.science_drive_production_bonus(&g, 0, theirs, &library),
            0.0,
            "a completed research rung is not repeatedly rewarded"
        );
        // Remove every local building rung the project is allowed to wait
        // behind. The test does not need to model their completion actions;
        // the valuation only consumes the resulting city state.
        g.cities.get_mut(&theirs).unwrap().buildings.extend(
            [
                "university",
                "research_lab",
                "workshop",
                "factory",
                "coal_power_plant",
            ]
            .into_iter()
            .map(Name::new),
        );
        g.cities.get_mut(&ours).unwrap().buildings.extend(
            [
                "library",
                "university",
                "research_lab",
                "workshop",
                "factory",
                "coal_power_plant",
            ]
            .into_iter()
            .map(Name::new),
        );
        assert_eq!(
            v2.science_drive_production_bonus(&g, 0, theirs, &grants),
            0.0,
            "a finished non-launch research city still gets no launch credit"
        );
        assert_eq!(
            v2.science_drive_production_bonus(&g, 0, ours, &grants),
            SCIENCE_DRIVE_CAMPUS_PROJECT_BONUS,
            "a finished launch city gets a science-producing filler"
        );

        // Exercise the actual project valuation, not only its bonus helper:
        // the integration call must put the credit on the raw side of the
        // existing caps where the chooser can see it.
        let (mut developed, developed_city, _) = board();
        give_techs(&mut developed, 0, 30);
        let campus_site = flat_site(&mut developed, developed_city);
        developed.map.tiles.get_mut(&campus_site).unwrap().district = Some(crate::name!("campus"));
        developed
            .cities
            .get_mut(&developed_city)
            .unwrap()
            .districts
            .insert(crate::name!("campus"), campus_site);
        developed
            .cities
            .get_mut(&developed_city)
            .unwrap()
            .buildings
            .extend(
                [
                    "library",
                    "university",
                    "research_lab",
                    "workshop",
                    "factory",
                    "coal_power_plant",
                ]
                .into_iter()
                .map(Name::new),
            );
        developed.turn = AdvancedAi::science_drive_start(&developed);
        let mut driven = AdvancedAi::targeting(VictoryTarget::Science);
        driven.enable_science_victory_drive_2();
        driven.maintain_science_drive(&developed, 0);
        let plan = StrategicPlan {
            strategy: GrandStrategy::Science,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: developed.turn,
            rush: false,
        };
        let stock_value = AdvancedAi::new().district_project_value(
            &developed,
            0,
            developed_city,
            "campus_research_grants",
            &plan,
        );
        let driven_value = driven.district_project_value(
            &developed,
            0,
            developed_city,
            "campus_research_grants",
            &plan,
        );
        assert!((driven_value - stock_value - SCIENCE_DRIVE_CAMPUS_PROJECT_BONUS).abs() < 1e-9);
        v2.disable_science_victory_drive_2();
        v2.maintain_science_drive(&g, 0);
        assert_eq!(
            v2.science_drive_production_bonus(&g, 0, theirs, &library),
            0.0,
            "the new behavior is an exact no-op when its gene is off"
        );
    }

    #[test]
    fn the_milestone_is_the_next_unknown_tech_and_the_beeline_holds_through_a_build() {
        let (mut g, ours, _) = board();
        give_techs(&mut g, 0, 30);
        for tech in g.rules.tech_ancestors["rocketry"].clone() {
            g.players[0].techs.insert(Name::new(&tech));
        }
        g.players[0].techs.insert(crate::name!("rocketry"));
        install_pad(&mut g, ours);
        g.turn = AdvancedAi::science_drive_start(&g);
        let mut ai = AdvancedAi::new();
        ai.enable_science_victory_drive();
        ai.maintain_science_drive(&g, 0);
        assert_eq!(
            AdvancedAi::science_drive_milestone(&g, 0),
            Some("satellites")
        );
        // While the Earth Satellite is unbuilt stock keys the beeline on
        // Rocketry, which is known: nothing leads to it any more.
        let stock = AdvancedAi::new();
        let value =
            |ai: &AdvancedAi, tech: &str| ai.tech_value(&g, 0, tech, GrandStrategy::Science);
        let stock_gap = value(&stock, "advanced_flight") - value(&stock, "computers");
        let drive_gap = value(&ai, "advanced_flight") - value(&ai, "computers");
        // `tech_value` scales by the research horizon, so read the sign.
        assert!(
            drive_gap > stock_gap + 1.0,
            "advanced_flight leads to satellites: drive {drive_gap} v stock {stock_gap}"
        );
        assert_eq!(
            ai.science_drive_tech_bonus(&g, 0, "electricity"),
            0.0,
            "Rocketry known: no production-tech bonus"
        );
    }

    #[test]
    fn before_rocketry_the_production_techs_carry_their_own_value() {
        let (mut g, _, _) = board();
        give_techs(&mut g, 0, 30);
        g.players[0]
            .techs
            .remove(&crate::name!("industrialization"));
        g.players[0].techs.remove(&crate::name!("electricity"));
        g.world_era = SCIENCE_DRIVE_PRODUCTION_ERA;
        g.turn = AdvancedAi::science_drive_start(&g);
        let mut ai = AdvancedAi::new();
        ai.enable_science_victory_drive();
        ai.maintain_science_drive(&g, 0);
        assert!(ai.science_drive().is_some());
        // Industrialization is on the way to Electricity too, so it carries both.
        assert!(
            ai.science_drive_tech_bonus(&g, 0, "industrialization")
                >= SCIENCE_DRIVE_PRODUCTION_TECH
        );
        assert_eq!(
            ai.science_drive_tech_bonus(&g, 0, "electricity"),
            SCIENCE_DRIVE_PRODUCTION_TECH
        );
        assert!(
            ai.science_drive_tech_bonus(&g, 0, "steam_power") >= SCIENCE_DRIVE_PRODUCTION_TECH,
            "on the way to electricity"
        );
        let unrelated = *g
            .rules
            .techs
            .keys()
            .find(|tech| {
                !["industrialization", "electricity"].iter().any(|goal| {
                    *tech == goal || g.rules.tech_ancestors[*goal].contains(tech.as_str())
                })
            })
            .expect("a tech off both paths");
        assert_eq!(
            ai.science_drive_tech_bonus(&g, 0, &unrelated),
            0.0,
            "{unrelated}"
        );
        assert_eq!(
            AdvancedAi::new().science_drive_tech_bonus(&g, 0, "industrialization"),
            0.0
        );
    }

    #[test]
    fn two_pads_by_the_earth_satellite_three_by_mars() {
        let mut none = BTreeSet::new();
        assert_eq!(AdvancedAi::science_drive_desired_pads(&none), 1);
        none.insert("launch_earth_satellite".to_string());
        assert_eq!(AdvancedAi::science_drive_desired_pads(&none), 2);
        none.insert("launch_moon_landing".to_string());
        assert_eq!(AdvancedAi::science_drive_desired_pads(&none), 2);
        none.insert("launch_mars_colony".to_string());
        assert_eq!(AdvancedAi::science_drive_desired_pads(&none), 3);
    }

    #[test]
    fn pingala_prefers_the_launch_city_once_a_pad_stands() {
        let (mut g, ours, theirs) = board();
        give_techs(&mut g, 0, 30);
        g.turn = AdvancedAi::science_drive_start(&g);
        let mut ai = AdvancedAi::new();
        ai.enable_science_victory_drive();
        ai.maintain_science_drive(&g, 0);
        assert_eq!(
            ai.science_drive_governor_bonus(&g, "pingala", ours),
            0.0,
            "no pad yet"
        );
        install_pad(&mut g, ours);
        assert_eq!(
            ai.science_drive_governor_bonus(&g, "pingala", ours),
            SCIENCE_DRIVE_PINGALA_BONUS
        );
        assert_eq!(ai.science_drive_governor_bonus(&g, "magnus", ours), 0.0);
        assert_eq!(ai.science_drive_governor_bonus(&g, "pingala", theirs), 0.0);
    }
}
