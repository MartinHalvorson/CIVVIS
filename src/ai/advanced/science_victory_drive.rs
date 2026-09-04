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
//! journal says why: from turn ~150 `space_race_can_finish` refused the race
//! every turn — *"101 turns left; the launch pad, the remaining projects,
//! their techs and fifty light-years do not fit"*. That estimate prices the
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
//! 5. **The horizon prices the race the engine actually runs.**
//!    [`AdvancedAi::science_drive_race_fits`] replaces `space_race_can_finish`
//!    while driving: the launch city's production with the zone chain it is
//!    about to build, the engine's project multiplier (the +100%, Pingala,
//!    the policy card), research overlapping production, and a flight
//!    simulated with every pad building laser stations — accepted inside
//!    [`SCIENCE_DRIVE_STRETCH`] of the turns left, [`SCIENCE_DRIVE_STRETCH_COMMITTED`]
//!    once a pad stands or a project is done, always once the expedition
//!    is away.
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

/// The four launch projects, in order; each needs the one before it.
pub const SCIENCE_DRIVE_PROJECTS: [&str; 4] = [
    "launch_earth_satellite",
    "launch_moon_landing",
    "launch_mars_colony",
    "exoplanet_expedition",
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
/// The race is attempted while its estimate fits in this multiple of the
/// turns left: the estimate has no Great Engineer, no chop, no policy the
/// seat has not slotted yet, and every turn the seat waits is a turn of
/// production it cannot get back.
pub const SCIENCE_DRIVE_STRETCH: f64 = 1.3;
/// The same once a pad stands or a project is done: what is built is built.
pub const SCIENCE_DRIVE_STRETCH_COMMITTED: f64 = 1.6;
/// Version 2 advances the next launch rung while a standing or queued pad's
/// immediate project fits, even when the whole remaining chain does not yet.
pub const SCIENCE_DRIVE_STEP_STRETCH: f64 = 1.6;
/// Version 2 can seed its first Spaceport when the pad and first launch fit,
/// rather than requiring the whole unstarted chain to fit at once.
pub const SCIENCE_DRIVE_BOOTSTRAP_STRETCH: f64 = 1.5;
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
/// How much of the launch city's production the zone chain still to be
/// built is projected to add: a Factory and a Power Plant, one share each.
pub const SCIENCE_DRIVE_CHAIN_UPLIFT: f64 = 0.15;
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
        // The launch chain has one serial bottleneck: every queue spent on a
        // research building in a non-launch city is a queue not available to
        // the next Spaceport or a parallel production site.  V2 originally
        // paid this credit in every city, which made the deployed Science
        // lane fill the empire with Campus buildings while its first pad and
        // projects slipped past the 250-turn clock.  Keep the funnel credit
        // where it compounds the race, and let the ordinary science-building
        // debt value the other cities.
        let launch_city = self.science_drive_launch_city(g, pid);
        let research_bonus = (city.owner == pid && launch_city == Some(cid))
            .then(|| Self::science_drive_research_bonus(g, pid, city, item))
            .unwrap_or(0.0);
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

    /// Whether the race fits the turns left, priced as the engine runs it.
    /// Always true without a turn limit, and once the expedition is away.
    pub(super) fn science_drive_race_fits(&self, g: &Game, pid: usize) -> bool {
        if self.science_victory_drive_2 {
            return self.science_drive_race_fits_v2(g, pid);
        }
        if g.max_turns == 0 {
            return true;
        }
        let player = &g.players[pid];
        if player.science_projects.contains("exoplanet_expedition") {
            return true;
        }
        let Some(launch) = self.science_drive_launch_city(g, pid) else {
            return false;
        };
        let remaining = g.max_turns.saturating_sub(g.turn) as f64;
        let pad = crate::name!("spaceport");
        let _memo = g.query_memo();
        let city_ids = g.player_city_ids(pid);
        let pads_standing = city_ids
            .iter()
            .filter(|cid| g.cities[cid].districts.contains_key(pad))
            .count();
        let pad_queued = city_ids.iter().any(|cid| {
            g.cities[cid].queue.iter().any(|item| {
                matches!(item, Item::District { district, .. } if g.district_family(*district) == pad)
            })
        });

        // The launch city's production, with the zone chain it is about to
        // build projected in.
        let launch_city = &g.cities[&launch];
        let held = |name: &str| {
            launch_city
                .buildings
                .iter()
                .any(|b| Self::base_building(g, b) == name)
        };
        let uplift = if g.world_era >= SCIENCE_DRIVE_PRODUCTION_ERA {
            SCIENCE_DRIVE_CHAIN_UPLIFT
                * (usize::from(!held("factory"))
                    + usize::from(!POWER_PLANTS.iter().any(|plant| held(plant))))
                    as f64
        } else {
            0.0
        };
        let base = g.city_yields(launch).production.max(1.0) * (1.0 + uplift);
        let project_item = Item::Project {
            project: Name::new(SCIENCE_DRIVE_PROJECTS[0]),
        };
        let project_rate = base * g.item_prod_mult(pid, launch, Some(&project_item)).max(1.0);

        // Production: the pad if none stands or is on its way, then every
        // project not yet completed.
        let mut production_turns = 0.0;
        if pads_standing == 0 && !pad_queued {
            let pad_item = Item::District {
                district: pad,
                pos: launch_city.pos,
            };
            let pad_rate = base * g.item_prod_mult(pid, launch, Some(&pad_item)).max(1.0);
            production_turns += g.item_cost(&pad_item) / pad_rate;
        }
        let mut techs_needed: BTreeSet<Name> = BTreeSet::new();
        let mut need_tech = |tech: &Name| {
            if !player.techs.contains(tech) {
                techs_needed.insert(*tech);
                if let Some(ancestors) = g.rules.tech_ancestors.get(tech.as_str()) {
                    for ancestor in ancestors {
                        let ancestor = Name::new(ancestor);
                        if !player.techs.contains(&ancestor) {
                            techs_needed.insert(ancestor);
                        }
                    }
                }
            }
        };
        for project in SCIENCE_DRIVE_PROJECTS {
            if player.science_projects.contains(project) {
                continue;
            }
            let Some(spec) = g.rules.projects.get(project) else {
                continue;
            };
            production_turns += g.item_cost(&Item::Project {
                project: Name::new(project),
            }) / project_rate;
            if let Some(tech) = spec.tech.as_ref() {
                need_tech(tech);
            }
        }
        // The stations that carry the flight need the gateway tech too.
        need_tech(&crate::name!("offworld_mission"));
        // The empire's own pace: techs known per turn so far, floored at the
        // ruleset's ordinary cadence (stock's own rule).
        let turns_per_tech = if player.techs.is_empty() {
            8.0
        } else {
            (g.turn as f64 / player.techs.len() as f64).max(2.0)
        };
        let research_turns = techs_needed.len() as f64 * turns_per_tech;

        // The flight: every pad the drive will have builds laser stations
        // from the launch, each +1 light-year a turn.
        let pads_planned = 3.min(city_ids.len()).max(pads_standing);
        let mut pad_rates: Vec<f64> = city_ids
            .iter()
            .filter(|cid| **cid != launch)
            .map(|cid| {
                g.city_yields(*cid).production.max(1.0)
                    * g.item_prod_mult(pid, *cid, Some(&project_item)).max(1.0)
            })
            .collect();
        pad_rates.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        pad_rates.truncate(pads_planned.saturating_sub(1));
        pad_rates.insert(0, project_rate);
        let station_cost = g.item_cost(&Item::Project {
            project: crate::name!("lagrange_laser_station"),
        });
        // No worse than stock's flight, which assumes the two stations a
        // racing empire builds triple the base speed.
        let flight_turns = Self::science_drive_flight_turns(station_cost, &pad_rates)
            .min(EXOPLANET_DESTINATION / 3.0);

        let committed = pads_standing > 0 || !player.science_projects.is_empty();
        let stretch = if committed {
            SCIENCE_DRIVE_STRETCH_COMMITTED
        } else {
            SCIENCE_DRIVE_STRETCH
        };
        let total = production_turns.max(research_turns) + flight_turns;
        let fits = total <= remaining * stretch;
        if !fits {
            think!(self.journal(), Cities, Detail,
                   "The science drive cannot land the race";
                   "{remaining:.0} turns left; {production_turns:.0} of production at {project_rate:.0} a turn \
                    in {} ({pads_standing} pads), {research_turns:.0} of research, {flight_turns:.0} of flight",
                   launch_city.name);
        }
        fits
    }

    /// Version 2 prices the route the continuing drive can really use: legal
    /// pad sites, queue progress, parallel pad construction, actual research
    /// cost and progress, and the next launch rung. Version one above keeps
    /// its original estimate for an honest family comparison.
    fn science_drive_race_fits_v2(&self, g: &Game, pid: usize) -> bool {
        if g.max_turns == 0 {
            return true;
        }
        let player = &g.players[pid];
        if player.science_projects.contains("exoplanet_expedition") {
            return true;
        }
        let Some(launch) = self.science_drive_launch_city(g, pid) else {
            return false;
        };
        let remaining = g.max_turns.saturating_sub(g.turn) as f64;
        let pad = crate::name!("spaceport");
        let _memo = g.query_memo();
        let city_ids = g.player_city_ids(pid);
        let has_pad = |cid: u32| g.cities[&cid].districts.contains_key(pad);
        let pad_in_queue = |cid: u32| {
            g.cities[&cid].queue.iter().any(|item| {
                matches!(item, Item::District { district, .. }
                    if g.district_family(*district) == pad)
            })
        };

        // A pad city is a real parallel site. The v1 estimate uses the
        // remembered launch city's raw production for every project and
        // assumes future pads for the flight, which can be pessimistic about
        // projects and optimistic about stations at the same time.
        let project_item = Item::Project {
            project: Name::new(SCIENCE_DRIVE_PROJECTS[0]),
        };
        let site_rate = |cid: u32| {
            let mut production = g.city_yields(cid).production.max(1.0);
            if cid == launch && g.world_era >= SCIENCE_DRIVE_PRODUCTION_ERA {
                let city = &g.cities[&cid];
                let held = |name: &str| {
                    city.buildings
                        .iter()
                        .any(|building| Self::base_building(g, building) == name)
                };
                let uplift = SCIENCE_DRIVE_CHAIN_UPLIFT
                    * (usize::from(!held("factory"))
                        + usize::from(!POWER_PLANTS.iter().any(|plant| held(plant))))
                        as f64;
                production *= 1.0 + uplift;
            }
            production * g.item_prod_mult(pid, cid, Some(&project_item)).max(1.0)
        };
        let viable_site =
            |cid: u32| has_pad(cid) || pad_in_queue(cid) || !g.district_sites(cid, pad).is_empty();
        let mut sites: Vec<(u32, bool, bool, f64)> = city_ids
            .iter()
            .copied()
            .filter(|cid| viable_site(*cid))
            .map(|cid| (cid, has_pad(cid), pad_in_queue(cid), site_rate(cid)))
            .collect();
        // A host can omit the plot list during a partial live export. Keep
        // the remembered city as an estimate in that case; the production pass
        // still asks the host for a legal plot before applying the order.
        if sites.is_empty() {
            sites.push((
                launch,
                has_pad(launch),
                pad_in_queue(launch),
                site_rate(launch),
            ));
        }
        sites.sort_by(|left, right| {
            right
                .1
                .cmp(&left.1)
                .then_with(|| right.2.cmp(&left.2))
                .then_with(|| right.3.total_cmp(&left.3))
                .then_with(|| left.0.cmp(&right.0))
        });
        let pads_standing = sites.iter().filter(|(_, standing, _, _)| *standing).count();
        let pads_committed = sites.iter().filter(|(_, _, queued, _)| *queued).count();
        let desired_pads = Self::science_drive_desired_pads(&player.science_projects)
            .max(pads_standing + pads_committed)
            .min(city_ids.len().max(1));

        let mut pad_sites: Vec<(u32, f64)> = sites
            .iter()
            .filter(|(_, standing, queued, _)| *standing || *queued)
            .map(|(cid, _, _, rate)| (*cid, *rate))
            .collect();
        for (cid, standing, queued, rate) in &sites {
            if pad_sites.len() >= desired_pads || *standing || *queued {
                continue;
            }
            pad_sites.push((*cid, *rate));
        }
        if pad_sites.is_empty() {
            pad_sites.push((launch, site_rate(launch)));
        }
        pad_sites.sort_by(|left, right| {
            right
                .1
                .total_cmp(&left.1)
                .then_with(|| left.0.cmp(&right.0))
        });
        pad_sites.truncate(desired_pads.clamp(1, 3));

        // A first pad blocks all launch projects. Later pads can be built in
        // parallel by other cities, so they join the critical path only until
        // their own completion rather than being charged serially to launch.
        let pad_turns = |cid: u32| {
            let position = g
                .district_sites(cid, pad)
                .into_iter()
                .next()
                .unwrap_or(g.cities[&cid].pos);
            let item = Item::District {
                district: pad,
                pos: position,
            };
            g.item_remaining_cost_for_city(pid, cid, &item)
                / (g.city_yields(cid).production.max(1.0)
                    * g.item_prod_mult(pid, cid, Some(&item)).max(1.0))
        };
        let queued_pad_turns = |cid: u32| {
            let mut turns = 0.0;
            for item in &g.cities[&cid].queue {
                turns += g.item_remaining_cost_for_city(pid, cid, item)
                    / (g.city_yields(cid).production.max(1.0)
                        * g.item_prod_mult(pid, cid, Some(item)).max(1.0));
                if matches!(item, Item::District { district, .. }
                    if g.district_family(*district) == pad)
                {
                    return Some(turns);
                }
            }
            None
        };
        let queued_item_turns = |cid: u32, target: &Item| {
            let mut turns = 0.0;
            for item in &g.cities[&cid].queue {
                turns += g.item_remaining_cost_for_city(pid, cid, item)
                    / (g.city_yields(cid).production.max(1.0)
                        * g.item_prod_mult(pid, cid, Some(item)).max(1.0));
                if item == target {
                    return Some(turns);
                }
            }
            None
        };
        let first_pad_turns = if pads_standing > 0 {
            0.0
        } else if let Some(turns) = sites
            .iter()
            .filter(|(_, _, queued, _)| *queued)
            .filter_map(|(cid, _, _, _)| queued_pad_turns(*cid))
            .min_by(|a, b| a.total_cmp(b))
        {
            turns
        } else {
            pad_sites
                .first()
                .map(|(cid, _)| pad_turns(*cid))
                .unwrap_or(f64::INFINITY)
        };
        let additional_pad_turns = pad_sites
            .iter()
            .filter(|(cid, _)| !has_pad(*cid) && !pad_in_queue(*cid))
            .map(|(cid, _)| pad_turns(*cid))
            .fold(0.0, f64::max);

        // The empire's best pad city can run every sequential launch project.
        // If one is already in a queue, preserve its actual remaining progress
        // rather than charging the full project again.
        let project_rate = pad_sites.iter().map(|(_, rate)| *rate).fold(1.0, f64::max);
        let mut project_turns = 0.0;
        let mut first_project_turns = None;
        let mut techs_needed: BTreeSet<Name> = BTreeSet::new();
        let mut need_tech = |tech: &Name| {
            if !player.techs.contains(tech) {
                techs_needed.insert(*tech);
                if let Some(ancestors) = g.rules.tech_ancestors.get(tech.as_str()) {
                    for ancestor in ancestors {
                        let ancestor = Name::new(ancestor);
                        if !player.techs.contains(&ancestor) {
                            techs_needed.insert(ancestor);
                        }
                    }
                }
            }
        };
        for project in SCIENCE_DRIVE_PROJECTS {
            if player.science_projects.contains(project) {
                continue;
            }
            let Some(spec) = g.rules.projects.get(project) else {
                continue;
            };
            let item = Item::Project {
                project: Name::new(project),
            };
            let queued = city_ids
                .iter()
                .find_map(|cid| queued_item_turns(*cid, &item));
            let turns = queued.unwrap_or_else(|| g.item_cost(&item) / project_rate);
            first_project_turns.get_or_insert(turns);
            project_turns += turns;
            if let Some(tech) = spec.tech.as_ref() {
                need_tech(tech);
            }
        }
        // The stations that carry the flight need the gateway tech too.
        need_tech(&crate::name!("offworld_mission"));
        // Retain v1's historical cadence as a bounded prior, but use a real
        // science rate and current research progress when the live state has
        // them. A low/partial mirror cannot block the race indefinitely.
        let turns_per_tech = if player.techs.is_empty() {
            8.0
        } else {
            (g.turn as f64 / player.techs.len() as f64).max(2.0)
        };
        let historical_research_turns = techs_needed.len() as f64 * turns_per_tech;
        let mut research_cost = techs_needed
            .iter()
            .map(|tech| g.tech_cost(tech.as_str()))
            .sum::<f64>();
        if let Some(current) = player.research.as_deref() {
            if techs_needed.contains(&Name::new(current)) {
                research_cost -= player.research_progress.min(g.tech_cost(current));
            }
        } else {
            research_cost -= player.research_overflow;
        }
        let science_rate = Self::empire_science(g, pid);
        let live_research_turns = if science_rate > 1.0 {
            (research_cost.max(0.0) / science_rate).max(0.0)
        } else {
            historical_research_turns
        };
        let research_turns = live_research_turns.min(historical_research_turns * 1.5);

        // Only pads the drive can actually stand or build contribute stations.
        let pad_rates: Vec<f64> = pad_sites.iter().map(|(_, rate)| *rate).collect();
        let station_cost = g.item_cost(&Item::Project {
            project: crate::name!("lagrange_laser_station"),
        });
        let flight_turns = Self::science_drive_flight_turns(station_cost, &pad_rates)
            .min(EXOPLANET_DESTINATION / 3.0);

        let committed =
            pads_standing > 0 || pads_committed > 0 || !player.science_projects.is_empty();
        let stretch = if committed {
            SCIENCE_DRIVE_STRETCH_COMMITTED
        } else {
            SCIENCE_DRIVE_STRETCH
        };
        let production_turns = first_pad_turns + project_turns;
        let production_critical = production_turns.max(additional_pad_turns);
        let total = production_critical.max(research_turns) + flight_turns;
        let full_race_fits = total <= remaining * stretch;
        let next_step_fits = (pads_standing > 0 || pads_committed > 0)
            && first_project_turns
                .is_some_and(|project| project <= remaining * SCIENCE_DRIVE_STEP_STRETCH);
        let bootstrap_fits = pads_standing == 0
            && pads_committed == 0
            && player.techs.contains(&crate::name!("rocketry"))
            && first_pad_turns.is_finite()
            && first_project_turns.is_some_and(|project| {
                first_pad_turns + project <= remaining * SCIENCE_DRIVE_BOOTSTRAP_STRETCH
            });
        let fits = full_race_fits || bootstrap_fits || next_step_fits;
        if !fits {
            think!(self.journal(), Cities, Detail,
                   "The science drive cannot land the race";
                   "{remaining:.0} turns left; {production_critical:.0} of production at {project_rate:.0} a turn \
                    in {} ({pads_standing} pads, {pads_committed} queued, {desired_pads} planned), \
                   {research_turns:.0} of research, {flight_turns:.0} of flight",
                   g.cities[&launch].name);
        } else if bootstrap_fits && !full_race_fits {
            think!(self.journal(), Cities, Detail,
                   "The science drive seeds its first Spaceport";
                   "the full chain is not priced inside the horizon yet, but the pad and \
                    first launch fit in {remaining:.0} turns; build the bottleneck now in {}",
                   g.cities[&launch].name);
        }
        fits
    }

    /// Turns from the launch to the destination when each pad puts its
    /// production into laser stations: the ship leaves at one light-year a
    /// turn and every finished station adds one.
    pub(super) fn science_drive_flight_turns(station_cost: f64, pad_rates: &[f64]) -> f64 {
        let station_cost = station_cost.max(1.0);
        let mut progress = vec![0.0; pad_rates.len()];
        let mut speed = 1.0;
        let mut distance = 0.0;
        let mut turns = 0.0;
        while distance < EXOPLANET_DESTINATION && turns < 200.0 {
            for (done, rate) in progress.iter_mut().zip(pad_rates) {
                *done += rate;
                while *done >= station_cost {
                    *done -= station_cost;
                    speed += 1.0;
                }
            }
            distance += speed;
            turns += 1.0;
        }
        turns
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
        assert!(
            !v1.science_drive_race_fits(&g, 0),
            "version 1 requires the entire remaining chain to fit"
        );
        assert!(
            v2.science_drive_race_fits(&g, 0),
            "version 2 advances the next project behind a standing pad"
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
    fn the_flight_is_faster_with_more_pads() {
        let one = AdvancedAi::science_drive_flight_turns(300.0, &[60.0]);
        let two = AdvancedAi::science_drive_flight_turns(300.0, &[60.0, 40.0]);
        let none = AdvancedAi::science_drive_flight_turns(300.0, &[]);
        assert_eq!(none, 50.0, "no stations: one light-year a turn");
        assert!(one < none && two < one, "{none} {one} {two}");
    }

    /// The live defect: a seat with a pad and Rocketry refused by the stock
    /// horizon, which prices the projects at the city's raw production and
    /// ignores the engine's +100% on every Spaceport project.
    #[test]
    fn the_drive_prices_the_race_the_engine_runs_and_races_where_stock_refuses() {
        let (mut g, ours, _) = board();
        give_techs(&mut g, 0, 30);
        for tech in g.rules.tech_ancestors["rocketry"].clone() {
            g.players[0].techs.insert(Name::new(&tech));
        }
        g.players[0].techs.insert(crate::name!("rocketry"));
        install_pad(&mut g, ours);
        // A launch city with a real late-game production line, the way the
        // mirror corrects a live city's yields.
        std::sync::Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
            ours,
            crate::rules::Yields {
                production: 60.0,
                ..crate::rules::Yields::default()
            },
        );
        assert!(g.city_yields(ours).production >= 60.0);
        let mut ai = AdvancedAi::new();
        ai.enable_science_victory_drive();
        let stock = AdvancedAi::new();
        let mut drive_only = Vec::new();
        let mut stock_only = Vec::new();
        for turn in (20..200).step_by(5) {
            g.turn = turn;
            ai.maintain_science_drive(&g, 0);
            let s = stock.space_race_can_finish(&g, 0);
            let d = ai.space_race_can_finish(&g, 0);
            if d && !s {
                drive_only.push(turn);
            }
            if s && !d {
                stock_only.push(turn);
            }
        }
        assert!(
            !drive_only.is_empty(),
            "some turn where the drive races and stock refuses"
        );
        assert!(
            stock_only.is_empty(),
            "stock never races where the drive refuses: {stock_only:?}"
        );
        g.turn = 199;
        ai.maintain_science_drive(&g, 0);
        assert!(
            !ai.space_race_can_finish(&g, 0),
            "one turn left: nothing fits"
        );
        g.players[0]
            .science_projects
            .insert("exoplanet_expedition".to_string());
        assert!(
            ai.space_race_can_finish(&g, 0),
            "the expedition is away: always finish the flight"
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
