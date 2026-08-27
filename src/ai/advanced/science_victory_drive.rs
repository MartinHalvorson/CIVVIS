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
/// A driving seat keeps driving while it is within this many techs of the
/// field leader.
pub const SCIENCE_DRIVE_TECH_SLACK: usize = 3;
/// The race is attempted while its estimate fits in this multiple of the
/// turns left: the estimate has no Great Engineer, no chop, no policy the
/// seat has not slotted yet, and every turn the seat waits is a turn of
/// production it cannot get back.
pub const SCIENCE_DRIVE_STRETCH: f64 = 1.3;
/// The same once a pad stands or a project is done: what is built is built.
pub const SCIENCE_DRIVE_STRETCH_COMMITTED: f64 = 1.6;
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

    /// Close enough to the leader to keep driving.
    pub fn holds(&self) -> bool {
        self.own_science >= SCIENCE_DRIVE_HOLD * self.best_rival_science
            || self.own_techs + SCIENCE_DRIVE_TECH_SLACK >= self.best_rival_techs
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
        if !self.science_victory_drive || !g.victory_conditions.science {
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
                Some(_) => standing.holds(),
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
        let launch_city = Self::science_drive_pick_launch_city(g, pid);
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

    /// The launch city this turn: the one read at the last review while it
    /// is still ours, else re-picked.
    pub(super) fn science_drive_launch_city(&self, g: &Game, pid: usize) -> Option<u32> {
        let drive = self.science_drive?;
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
        assert!(ai.science_drive().is_none());
        assert!(!AdvancedAi::legacy().science_victory_drive);
        let mut ai = AdvancedAi::new();
        ai.enable_science_victory_drive();
        assert!(ai.science_victory_drive);
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
