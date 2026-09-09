//! Science threat denial: delay the rival's finish, not only speed ours.
//!
//! ★★★★ THE SCIENCE LANE HAS AN OFFENSE AND NO DEFENSE. Every science gene in
//! this controller pushes our own launch forward — `science-victory-drive`
//! picks a launch city, `science-endgame` reserves the production, the boost
//! genes buy the techs. Nothing in the controller is aimed at the rival's
//! Spaceport. The Emperor ladder shows the cost: the rivals finish the space
//! race at standard turns 213–228 while we stand at a Spaceport with one to
//! three of four launches done. On the Immortal and Deity rungs the rival
//! carries a +24 or +32 percent yield handicap on top of that, so speeding
//! our own launch cannot close the gap by itself. The finish has to be taken
//! away from them.
//!
//! The one existing piece of denial is `deny_leaders`' espionage bonus
//! (`SCIENCE_ROCKETRY_DENIAL_PRIORITY`), and it arms at
//! `SCIENCE_ROCKETRY_DENIAL_PROGRESS` — 65 in `rival_victory_pressure`'s
//! currency, which is *the Mars colony already landed*, the third of four
//! stages. By then the seat has one launch left to stop and a spy that has
//! not yet travelled.
//!
//! ## What the gene does
//!
//! A rival is a [science threat](AdvancedAi::science_threats) when we have met
//! it and it is a living major that is not our teammate, the science victory
//! is enabled, and either
//!
//! - we have seen a Spaceport of theirs, or they have completed any space
//!   project — the race is physically under way — or
//! - they lead our technology count by [`SCIENCE_THREAT_TECH_LEAD`] after
//!   standard turn [`SCIENCE_THREAT_TECH_LEAD_TURN`]; the pace that produces
//!   a t213 finish is visible long before the first pad.
//!
//! Against the threats, in escalating order, each rung its own constant and
//! its own journal line:
//!
//! 1. **Diplomacy.** No research agreement and no alliance of any kind with a
//!    threat, no sale of passage and no sale of a Great Work to one, and the
//!    most pressing threat is denounced once per turn (the same primitive
//!    `culture-threat-early` uses, [`AdvancedAi::denounce_most_pressing`]).
//!    A denouncement costs them the friendship and alliance routes to our
//!    market and starts the Formal War clock rung 4 reads.
//! 2. **Espionage.** The model's launch-denial mission is `disrupt_rocketry`:
//!    it is gated on the `spaceport` district family, it pillages the pad and
//!    it zeroes the city's spaceport-project progress (`Game`'s
//!    `apply_spy_mission_effect`). That is the sabotage-of-production this
//!    lane wants, so it is what the gene sends — our best idle spy assigned
//!    to the threat's launch city ([`DENIAL_SPY_ASSIGN_PRIORITY`]) and the
//!    disruption run from it ahead of every other operation
//!    ([`DENIAL_SPY_MISSION_PRIORITY`]), at our own threat threshold rather
//!    than at Mars.
//! 3. **Raid.** While we are at war with a threat, the
//!    [`DENIAL_RAID_SIZE`] nearest mobile soldiers within
//!    [`DENIAL_RAID_REACH`] of the pad walk onto it and pillage it. A pillaged
//!    Spaceport cannot run a project until it is repaired. The party never
//!    includes the lone garrison of one of our cities — the existing floor
//!    `opportunistic_war` uses for the same question.
//! 4. **War.** When a threat is inside [`DENIAL_WAR_LAUNCH_HORIZON`] standard
//!    turns of finishing and we are not inside a shorter horizon of our own
//!    finish, the cheapest legal war is declared for the sake of rung 3 —
//!    subject to the war-affordability gates that already exist
//!    (`war-needs-a-treasury` through `war_is_affordable`,
//!    `one-war-at-a-time` through `one_war_holds_declaration`) and to the
//!    raid being able to *reach* the pad at all
//!    ([`AdvancedAi::science_denial_raid_can_reach`]). Peace is offered as
//!    soon as the pad is pillaged, or after [`DENIAL_WAR_MAX_TURNS`],
//!    whichever comes first.
//!
//! Off by default: opt-in registry row `science-threat-denial`. With the flag
//! off every entry point below returns before it reads the board, so the
//! controller is byte-identical to the one without the gene.

use std::collections::BTreeSet;

use super::opportunistic_war::RAID_PEACE_EARLIEST;
use super::{AdvancedAi, StrategicPlan};
use crate::game::{Action, ActionFamilies, Game, Item, QuickDeal};
use crate::name::Name;
use crate::think;
use crate::Pos;

/// Technologies a rival must lead us by to be a threat on pace alone. The
/// Emperor games we lose run 12–33 techs behind by the end
/// (`docs/civvis-the-empire-builds-forts-not-science.md`); six is where the
/// gap stops being noise and starts compounding, and it is half the smallest
/// deficit any lost game finished with.
pub(crate) const SCIENCE_THREAT_TECH_LEAD: usize = 6;

/// No pace threat before this standard turn. Early tech counts swing on a
/// single hut or a first-to-meet bonus, and a six-tech gap at turn 40 is a
/// scouting result, not a race.
pub(crate) const SCIENCE_THREAT_TECH_LEAD_TURN: u32 = 120;

/// Added to a spy assignment whose destination is a threat's launch city.
/// The stock Science table already pays 150 for any Spaceport and 180 for the
/// war plan's target; this outranks both together so the pad that is actually
/// going to launch is the posting, not merely a good one.
pub(crate) const DENIAL_SPY_ASSIGN_PRIORITY: i32 = 340;

/// Added to a `disrupt_rocketry` mission run against a threat. The stock
/// Science table values Tech Boost at 320 and Rocketry disruption at 290,
/// and the disruption's base success chance is 0.20 against the boost's 0.35,
/// so nothing under a doubling of the gap ever selects it. This clears it.
pub(crate) const DENIAL_SPY_MISSION_PRIORITY: f64 = 700.0;

/// Soldiers sent at the pad. Two is a pillage party, not a campaign: the tile
/// is taken by movement and one action, and a third body is better spent on
/// the front the declaration opens at home.
pub(crate) const DENIAL_RAID_SIZE: usize = 2;

/// A soldier this far from the pad joins the raid party.
pub(crate) const DENIAL_RAID_REACH: i32 = 8;

/// A threat inside this many standard turns of finishing is worth a war. It
/// is deliberately longer than the war itself: the declaration, the march and
/// the pillage all have to land before the last project completes.
pub(crate) const DENIAL_WAR_LAUNCH_HORIZON: u32 = 40;

/// Peace is offered once a denial war is this old however it has gone. A war
/// that has not reached the pad in this many standard turns is not going to.
pub(crate) const DENIAL_WAR_MAX_TURNS: u32 = 25;

/// Of the [`DENIAL_WAR_MAX_TURNS`] a denial war lasts, this share is the
/// march: the rest is the pillage itself and the peace the engine's minimum
/// war length holds us to. A declaration whose objective cannot be walked to
/// inside it buys the grievances and none of the denial — the first probe of
/// this gene opened 13 wars and pillaged 2 pads without this gate.
pub(crate) const DENIAL_MARCH_SHARE: f64 = 0.6;

/// The projects the science victory is made of, in the order they are built.
/// `Game::victory_races` counts exactly these four.
const SPACE_PROJECTS: [&str; 4] = [
    "launch_earth_satellite",
    "launch_moon_landing",
    "launch_mars_colony",
    "exoplanet_expedition",
];

/// A rival's science endgame as this seat can read it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ScienceThreat {
    /// The threatening major.
    pub(crate) rival: usize,
    /// Space projects they have completed, 0 to 4.
    pub(crate) stages: usize,
    /// Technologies they lead us by; 0 when they do not lead.
    pub(crate) tech_lead: usize,
    /// Their launch city and its Spaceport tile, when we have seen one.
    pub(crate) pad: Option<(u32, Pos)>,
}

impl ScienceThreat {
    /// Most pressing first: stages landed, then the technology lead, then the
    /// lower seat so the order is total and stable across turns.
    fn rank(&self) -> (std::cmp::Reverse<usize>, std::cmp::Reverse<usize>, usize) {
        (
            std::cmp::Reverse(self.stages),
            std::cmp::Reverse(self.tech_lead),
            self.rival,
        )
    }
}

/// The denial war this controller opened and has not yet closed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DenialWar {
    /// The rival it was declared on.
    pub(crate) target: usize,
    /// The turn of the declaration.
    pub(crate) declared: u32,
    /// The pad it was declared for.
    pub(crate) pad: Pos,
}

impl AdvancedAi {
    /// Whether the denial layer runs at all: the gene, the seat's victory
    /// planning, and a science victory that can actually be won.
    fn science_denial_active(&self, g: &Game) -> bool {
        self.science_threat_denial && self.victory_planning && g.victory_conditions.science
    }

    /// The Spaceport city of `rival` we would aim at: the one whose pad we
    /// have explored and whose city produces the most, ties to the older
    /// city. A pad we have never seen is not a target — the raid has to walk
    /// to a tile and the spy has to be posted to a city.
    pub(crate) fn science_denial_pad(g: &Game, pid: usize, rival: usize) -> Option<(u32, Pos)> {
        let explored = &g.players[pid].explored;
        let _memo = g.query_memo();
        g.player_city_ids(rival)
            .into_iter()
            .filter_map(|cid| {
                let city = &g.cities[&cid];
                let pad = city.districts.iter().find_map(|(district, position)| {
                    (g.district_family(*district) == "spaceport").then_some(*position)
                })?;
                explored.contains(&pad).then_some((cid, pad))
            })
            .max_by(|(left, _), (right, _)| {
                g.city_yields(*left)
                    .production
                    .total_cmp(&g.city_yields(*right).production)
                    .then_with(|| right.cmp(left))
            })
    }

    /// Space projects `pid` has completed.
    fn science_denial_stages(g: &Game, pid: usize) -> usize {
        let completed = &g.players[pid].science_projects;
        SPACE_PROJECTS
            .iter()
            .filter(|project| completed.contains(**project))
            .count()
    }

    /// Every rival that threatens to win the science victory, most pressing
    /// first. Empty when the gene is off, so every rung below it is inert.
    ///
    /// The guards are `culture_trade_threats`': met, alive, a major, not a
    /// teammate. The two admissions are the physical race — a pad we have
    /// seen or a project they have landed — and the pace, a technology lead
    /// of [`SCIENCE_THREAT_TECH_LEAD`] after
    /// [`SCIENCE_THREAT_TECH_LEAD_TURN`] standard turns.
    pub(crate) fn science_threats(&self, g: &Game, pid: usize) -> Vec<ScienceThreat> {
        if !self.science_denial_active(g) {
            return Vec::new();
        }
        let ours = g.players[pid].techs.len();
        let pace_open = g.turn >= g.standard_duration(SCIENCE_THREAT_TECH_LEAD_TURN);
        let mut threats: Vec<ScienceThreat> = g
            .players
            .iter()
            .filter(|rival| {
                rival.id != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && !g.same_team(pid, rival.id)
                    && g.has_met(pid, rival.id)
            })
            .filter_map(|rival| {
                let stages = Self::science_denial_stages(g, rival.id);
                let pad = Self::science_denial_pad(g, pid, rival.id);
                let tech_lead = rival.techs.len().saturating_sub(ours);
                let racing = stages > 0 || pad.is_some();
                let pacing = pace_open && tech_lead >= SCIENCE_THREAT_TECH_LEAD;
                (racing || pacing).then_some(ScienceThreat {
                    rival: rival.id,
                    stages,
                    tech_lead,
                    pad,
                })
            })
            .collect();
        threats.sort_by_key(ScienceThreat::rank);
        threats
    }

    /// The threatening seats alone, for the filters that only need the set.
    pub(crate) fn science_threat_seats(&self, g: &Game, pid: usize) -> BTreeSet<usize> {
        self.science_threats(g, pid)
            .into_iter()
            .map(|threat| threat.rival)
            .collect()
    }

    // ---- Rung 1: diplomacy ------------------------------------------------

    /// Rung 1: passage and Great Works are the two sales that pay a rival's
    /// space race directly — passage in Gold and route yields for the whole
    /// treaty, a Great Work in the Culture and the Gold of the sale. Neither
    /// is sold to a threat. Purchases from a threat and every other sale stay
    /// open: starving our own treasury does not slow their launch.
    ///
    /// Off, and with no threat on the board, this is `true` for every deal.
    pub(crate) fn science_denial_deal_allowed(
        &self,
        deal: &QuickDeal,
        threats: &BTreeSet<usize>,
    ) -> bool {
        !(self.science_threat_denial
            && deal.direction == "sell"
            && threats.contains(&deal.partner)
            && (deal.item == "open_borders" || deal.category == "great_work"))
    }

    /// Rung 1: no research agreement and no alliance of any other kind with a
    /// threat. A research agreement hands a science threat exactly the yield
    /// it is winning with, and any alliance makes the war of rung 4 illegal
    /// for its whole term.
    ///
    /// Off, and with no threat on the board, this is `false` for everyone.
    pub(crate) fn science_denial_refuses_alliance(
        &self,
        threats: &BTreeSet<usize>,
        other: usize,
    ) -> bool {
        self.science_threat_denial && threats.contains(&other)
    }

    /// Rung 1: denounce the most pressing threat, once per turn. Returns the
    /// denounced rival for the caller's journal line.
    pub(crate) fn science_threat_denunciation(&self, g: &mut Game, pid: usize) -> Option<usize> {
        let ranked: Vec<usize> = self
            .science_threats(g, pid)
            .into_iter()
            .map(|threat| threat.rival)
            .collect();
        Self::denounce_most_pressing(g, pid, &ranked)
    }

    // ---- Rung 2: espionage ------------------------------------------------

    /// The launch cities of every threat: the spy pass reads this once a turn
    /// rather than per candidate city, because the threat model walks every
    /// rival's cities. Empty when the gene is off.
    pub(crate) fn science_denial_pad_cities(&self, g: &Game, pid: usize) -> BTreeSet<u32> {
        self.science_threats(g, pid)
            .into_iter()
            .filter_map(|threat| threat.pad)
            .map(|(cid, _)| cid)
            .collect()
    }

    /// Rung 2: what a spy posting to `cid` is worth beyond the stock table.
    /// Only a threat's launch city earns it, so a Spaceport belonging to a
    /// rival that is not racing is still valued exactly as before.
    pub(crate) fn science_denial_spy_assignment_bonus(pads: &BTreeSet<u32>, cid: u32) -> i32 {
        if pads.contains(&cid) {
            DENIAL_SPY_ASSIGN_PRIORITY
        } else {
            0
        }
    }

    /// Rung 2: what a `disrupt_rocketry` run against a threat is worth beyond
    /// the stock table. `disrupt_rocketry` is the model's launch sabotage —
    /// it pillages the pad and zeroes the city's spaceport-project progress —
    /// so it is the mission this rung sends. Every other mission, and every
    /// city that is not a threat's, scores exactly as before.
    pub(crate) fn science_denial_spy_mission_bonus(
        threats: &BTreeSet<usize>,
        city_owner: usize,
        mission: &str,
    ) -> f64 {
        if mission == "disrupt_rocketry" && threats.contains(&city_owner) {
            DENIAL_SPY_MISSION_PRIORITY
        } else {
            0.0
        }
    }

    // ---- Rung 3: the raid -------------------------------------------------

    /// The pad this seat is currently raiding: a threat we are at war with
    /// whose Spaceport tile we have seen and which is not pillaged already.
    /// The most pressing such threat, so two racing rivals do not split the
    /// party.
    fn science_denial_raid_pad(&self, g: &Game, pid: usize) -> Option<(usize, Pos)> {
        self.science_threats(g, pid)
            .into_iter()
            .filter(|threat| g.is_at_war(pid, threat.rival))
            .find_map(|threat| {
                let (_, pad) = threat.pad?;
                let standing = g.map.get(pad).is_some_and(|tile| !tile.pillaged);
                standing.then_some((threat.rival, pad))
            })
    }

    /// Our mobile soldiers within [`DENIAL_RAID_REACH`] of `pad`, nearest
    /// first, excluding the lone garrison of one of our cities: an empty city
    /// is the answer to a raid, and the existing floor
    /// ([`AdvancedAi::lone_garrison`]) is the one `opportunistic_war` already
    /// keeps for the same question.
    fn science_denial_raid_party(&self, g: &Game, pid: usize, pad: Pos) -> Vec<u32> {
        let mut party: Vec<(i32, u32)> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter_map(|uid| {
                let unit = &g.units[&uid];
                let spec = &g.rules.units[unit.kind];
                if spec.class != "military"
                    || spec.domain.as_deref() == Some("air")
                    || spec.domain.as_deref() == Some("sea")
                    || g.is_embarked(unit)
                    || Self::lone_garrison(g, pid, uid)
                {
                    return None;
                }
                let distance = g.wdist(unit.pos, pad);
                (distance <= DENIAL_RAID_REACH).then_some((distance, uid))
            })
            .collect();
        party.sort_unstable();
        party.truncate(DENIAL_RAID_SIZE);
        party.into_iter().map(|(_, uid)| uid).collect()
    }

    /// Whether the raid this war is declared for could actually walk to
    /// `pad` before the war is over.
    ///
    /// The route is read on a speculative board with the declaration already
    /// applied, for the reason `opportunistic-war` version two reads it there:
    /// before the war the target's closed borders make every route to its
    /// interior look impassable, and a distance on the map is not a route
    /// across an ocean at all. A soldier qualifies when its route to the pad
    /// is inside what it can march in [`DENIAL_MARCH_SHARE`] of
    /// [`DENIAL_WAR_MAX_TURNS`].
    fn science_denial_raid_can_reach(
        &self,
        g: &Game,
        pid: usize,
        pad: Pos,
        opening: &Action,
    ) -> bool {
        let mut board = g.speculative_clone();
        if board.apply(pid, opening).is_err() {
            return false;
        }
        let march =
            (g.standard_duration(DENIAL_WAR_MAX_TURNS) as f64 * DENIAL_MARCH_SHARE) as usize;
        g.player_unit_ids(pid).into_iter().any(|uid| {
            let unit = &g.units[&uid];
            let spec = &g.rules.units[unit.kind];
            if spec.class != "military"
                || spec.domain.as_deref() == Some("air")
                || spec.domain.as_deref() == Some("sea")
                || Self::lone_garrison(g, pid, uid)
            {
                return false;
            }
            let reach = (g.unit_max_moves(uid).floor().max(1.0) as usize) * march;
            board
                .route_distance(uid, pad, 0)
                .is_some_and(|steps| steps <= reach)
        })
    }

    /// Rung 3: this unit's step in the pad raid — pillage the pad it stands
    /// on, else one step toward it. `None` when the unit has no part in the
    /// raid this turn, which leaves the rest of the ladder untouched.
    pub(crate) fn science_denial_raid_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
    ) -> Option<bool> {
        if !self.science_threat_denial {
            return None;
        }
        let unit = g.units.get(&uid)?;
        if unit.moves_left <= 0.0 {
            return None;
        }
        // A soldier holding a city under pressure holds it; a pad on the far
        // side of the world is not a reason to open our own gate. This is the
        // guard `raid_prize_step` keeps, with its radius.
        if plan.threatened_city.is_some_and(|cid| {
            g.cities
                .get(&cid)
                .is_some_and(|city| g.wdist(unit.pos, city.pos) <= 3)
        }) {
            return None;
        }
        let (target, pad) = self.science_denial_raid_pad(g, pid)?;
        if !self.science_denial_raid_party(g, pid, pad).contains(&uid) {
            return None;
        }
        let here = g.units[&uid].pos;
        if here == pad {
            if !g.pillageable_at(pid, pad) {
                return None;
            }
            think!(self.journal(), Military, Decision,
                   "Pillaging the Spaceport of {}", g.players[target].civ;
                   "a pillaged pad runs no space project until it is repaired";
                   pad);
            let pillaged = g.apply(pid, &Action::Pillage { unit: uid }).is_ok();
            if pillaged {
                // The seat's own record of the rung, beside `denial_wars`, so
                // a screen row can say whether the raid ever reached a pad
                // rather than only whether the gene was on.
                *g.players[pid]
                    .counters
                    .entry("denial_pillages".to_string())
                    .or_insert(0) += 1;
            }
            return Some(pillaged);
        }
        let next = g
            .route_step(uid, pad, 0)
            .filter(|next| g.can_move(uid, *next))?;
        think!(self.journal(), Military, Decision,
               "{} marches on the Spaceport of {}", crate::reasoning::plain(&g.units[&uid].kind), g.players[target].civ;
               "{} tiles from the pad", g.wdist(here, pad);
               pad);
        Some(
            g.apply(
                pid,
                &Action::Move {
                    unit: uid,
                    to: next,
                },
            )
            .is_ok(),
        )
    }

    // ---- Rung 4: the war --------------------------------------------------

    /// Standard turns we project `pid` needs to finish the science victory,
    /// `None` when the race has not begun or cannot be projected.
    ///
    /// The projects still to build are priced at the launch city's own rate
    /// with [`AdvancedAi::science_project_build_turns`], the helper the
    /// science drive already uses to rank its own pads; the expedition's
    /// flight is priced at the distance left over the speed the engine
    /// reports ([`Game::exoplanet_speed`]). With no launch city on the board
    /// nothing can be projected, and this reads the same public
    /// victory-screen shape `rival_victory_pressure` does.
    pub(crate) fn science_denial_turns_to_finish(g: &Game, pid: usize) -> Option<f64> {
        let launch = Self::science_drive_pick_launch_city(g, pid)?;
        let completed = &g.players[pid].science_projects;
        let mut turns = 0.0;
        for project in SPACE_PROJECTS {
            if completed.contains(project) {
                continue;
            }
            turns += Self::science_project_build_turns(
                g,
                pid,
                launch,
                &Item::Project {
                    project: Name::new(project),
                },
            );
        }
        if completed.contains("exoplanet_expedition") {
            // The expedition is flying: what is left is distance, not
            // production. A launched expedition always reports a speed of at
            // least one light-year a turn (`modeled_exoplanet_speed`), but a
            // mirrored host may report none, and that is unprojectable rather
            // than instant.
            let speed = g.exoplanet_speed(pid);
            if speed <= 0.0 {
                return None;
            }
            let left =
                (g.science_victory_points_needed(pid) - g.science_victory_points(pid)).max(0.0);
            turns += left / speed;
        }
        turns.is_finite().then_some(turns)
    }

    /// Whether a war on `threat` is admissible this turn: it is not already
    /// ours to fight, it is inside [`DENIAL_WAR_LAUNCH_HORIZON`] of finishing,
    /// we are not inside a shorter horizon of our own finish, and the two
    /// existing war gates admit it.
    fn science_denial_war_admissible(&self, g: &Game, pid: usize, threat: &ScienceThreat) -> bool {
        if g.is_at_war(pid, threat.rival) || threat.pad.is_none() {
            return false;
        }
        let horizon = g.standard_duration(DENIAL_WAR_LAUNCH_HORIZON) as f64;
        let Some(theirs) = Self::science_denial_turns_to_finish(g, threat.rival) else {
            return false;
        };
        if theirs > horizon {
            return false;
        }
        // Our own finish first: a war we would win the game before fighting
        // costs production the last project needs.
        if Self::science_denial_turns_to_finish(g, pid).is_some_and(|ours| ours < theirs) {
            return false;
        }
        self.war_is_affordable(g, pid) && !self.one_war_holds_declaration(g, pid, threat.rival)
    }

    /// Rung 4: open a denial war, or close the one that is running. Returns
    /// `true` when a declaration was made this turn, so the caller's other
    /// roads to war stand down — the turn has one declaration.
    pub(crate) fn science_denial_war_diplomacy(&mut self, g: &mut Game, pid: usize) -> bool {
        if !self.science_threat_denial {
            self.denial_war = None;
            return false;
        }
        if let Some(war) = self.denial_war {
            if !g.is_at_war(pid, war.target) || !g.players[war.target].alive {
                self.denial_war = None;
                return false;
            }
            self.close_denial_war_when_paid(g, pid, war);
            return false;
        }
        let Some(threat) = self
            .science_threats(g, pid)
            .into_iter()
            .find(|threat| self.science_denial_war_admissible(g, pid, threat))
        else {
            return false;
        };
        let Some(action) = self.preferred_war_opening(g, pid, threat.rival) else {
            return false;
        };
        // The denouncement half of `preferred_war_opening` is rung 1's; it is
        // not a declaration and does not consume the turn's one war.
        if !matches!(
            action,
            Action::DeclareWar { .. } | Action::DeclareWarWithCasusBelli { .. }
        ) {
            let _ = g.apply(pid, &action);
            return false;
        }
        let Some((_, pad)) = threat.pad else {
            return false;
        };
        // The war exists for rung 3. If nothing of ours can walk to the pad
        // inside it, the declaration buys grievances and no denial.
        if !self.science_denial_raid_can_reach(g, pid, pad, &action) {
            return false;
        }
        think!(self.journal(), Military, Strategy,
               "Declaring war on {} to deny the space race", g.players[threat.rival].civ;
               "they have landed {} of {} space projects and lead us by {} tech{}; \
                the pad is the objective, not the city",
               threat.stages, SPACE_PROJECTS.len(), threat.tech_lead,
               if threat.tech_lead == 1 { "" } else { "s" });
        self.base.war_eve_liquidation(g, pid, &action);
        if g.apply(pid, &action).is_err() {
            return false;
        }
        *g.players[pid]
            .counters
            .entry("denial_wars".to_string())
            .or_insert(0) += 1;
        self.denial_war = Some(DenialWar {
            target: threat.rival,
            declared: g.turn,
            pad,
        });
        self.major_war_since = Some(g.turn);
        self.war_census.declarations += 1;
        true
    }

    /// Offer peace once the pad is pillaged or gone, or once the war has run
    /// [`DENIAL_WAR_MAX_TURNS`]. The engine's own minimum war length
    /// ([`RAID_PEACE_EARLIEST`]) is respected: peace cannot be concluded
    /// before it, so it is not proposed before it either.
    fn close_denial_war_when_paid(&mut self, g: &mut Game, pid: usize, war: DenialWar) {
        let age = g.turn.saturating_sub(war.declared);
        if age < g.standard_duration(RAID_PEACE_EARLIEST) {
            return;
        }
        let expired = age >= g.standard_duration(DENIAL_WAR_MAX_TURNS);
        // The pad is paid for when it is pillaged, and equally when the tile
        // has stopped being a Spaceport at all (razed with its city).
        let pad_pillaged = g.map.get(war.pad).is_none_or(|tile| tile.pillaged);
        if !expired && !pad_pillaged {
            return;
        }
        let peace_pending = g.pending_deals.iter().any(|deal| {
            deal.peace
                && ((deal.from == pid && deal.to == war.target)
                    || (deal.from == war.target && deal.to == pid))
                && deal.expires >= g.turn
        });
        if peace_pending || self.peace_offers.contains(&war.target) {
            return;
        }
        think!(self.journal(), Diplomacy, Decision,
               "Offering peace to {}", g.players[war.target].civ;
               "the denial war opened on turn {} has {}",
               war.declared,
               if pad_pillaged { "pillaged the pad" } else { "run its course" });
        self.peace_offers.insert(war.target);
        let _ = g.apply(
            pid,
            &Action::ProposeDeal {
                player: war.target,
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

/// The shared denunciation primitive, used by `culture-threat-early` and
/// `science-threat-denial` alike.
impl AdvancedAi {
    /// Denounce the first rival on `ranked` — most pressing first — that the
    /// engine lists as a legal denouncement, and return it.
    ///
    /// The legality (met, at peace, not friends or allied, not already
    /// denounced) is read from the diplomacy family rather than assumed, so
    /// an active denouncement is never repeated and a friend is never
    /// denounced. One per turn: the loop stops at the first that applies.
    pub(super) fn denounce_most_pressing(
        g: &mut Game,
        pid: usize,
        ranked: &[usize],
    ) -> Option<usize> {
        if ranked.is_empty() {
            return None;
        }
        let legal = g.legal_actions_within(pid, ActionFamilies::DIPLOMACY);
        let rival = *ranked
            .iter()
            .find(|rival| legal.contains(&Action::Denounce { player: **rival }))?;
        g.apply(pid, &Action::Denounce { player: rival })
            .is_ok()
            .then_some(rival)
    }
}

#[cfg(test)]
mod tests;
