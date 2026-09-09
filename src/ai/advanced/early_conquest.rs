//! `early-conquest-opening`: the opening that takes a neighbour's city.
//!
//! ## Why
//!
//! The live seat has never won at Emperor. The rung is not a difficulty
//! slider on the same game: an Emperor rival takes +16% on every yield, an
//! Immortal +24%, a Deity +32%, and each of them is handed a free Settler
//! every era. Parity play compounds that bonus for a hundred and fifty turns
//! and then loses to it — the ladder's own record is 0 wins in 111 Emperor
//! games, 19 of them lost to a rival's science or culture finish. A human
//! who beats those rungs does not out-develop the handicap; they take a
//! neighbour's cities in the first sixty turns, when the handicap is still
//! measured in a few dozen yields and a city is two Archers and a Warrior
//! away.
//!
//! Two live measurements say why the shipped controller cannot do that on
//! its own, and both are answered here rather than assumed away:
//!
//! 1. **We do not trade well enough to improvise a war.** The ledger's
//!    combat rate is 0.45 kills per loss, and the empire has lost 65 cities
//!    against 2 taken. A war that opens because the empire-wide power ratio
//!    happened to clear 1.32× is a war fought at that rate.
//! 2. **We die to what we never saw.** 234 of 304 unit deaths carry
//!    `no_visible_threat`: the killing blow came from a tile the unit could
//!    not see when it chose to stand there. That is not a tactics problem in
//!    the fight; it is a tile-choice problem on the march.
//!
//! ## What the gene does, in the order it happens
//!
//! Every step is inert with the flag off — each entry point returns before
//! reading the board — so the controller is byte-identical off.
//!
//! 1. **The target** ([`AdvancedAi::conquest_target`]). Among rivals we have
//!    MET, a city whose centre we have EXPLORED, within
//!    [`CONQUEST_REACH_TILES`] of our capital, whose owner has at most
//!    [`CONQUEST_MAX_RIVAL_CITIES`] cities we know of. The rival's capital
//!    is preferred, then the lightest visible garrison. Re-evaluated every
//!    turn until the force commits. Fog-honest by construction: the scan
//!    reads `players[pid].explored` for what cities exist — the idiom
//!    `opening_archery_goal` uses for a barbarian camp — and the turn-start
//!    visibility frame for what stands on them. It never reads a city or a
//!    defender the seat has not seen.
//! 2. **The reservation** ([`AdvancedAi::conquest_reservation`]). While a
//!    target stands and the standard turn is before
//!    [`CONQUEST_COMMIT_DEADLINE`], the CAPITAL's production is reserved for
//!    [`CONQUEST_RANGED`] ranged bodies and [`CONQUEST_MELEE`] melee bodies,
//!    and — when the best shooter the empire can train is a range-one
//!    Slinger — the research picker chases the node that upgrades it, by the
//!    same beeline `early-archers` uses. The reservation is a real
//!    reservation, not a bid: while it is unfilled the capital's Settler arm
//!    is deferred outright, the way `threatened_recovery_holds_settlers`
//!    defers it. It never defers the FIRST Settler (an empire of one city
//!    has nothing to conquer with) and it never fires while the capital is
//!    `threatened` — the defence sentinel outranks the whole opening.
//! 3. **Assembly and declaration**
//!    ([`AdvancedAi::conquest_declaration`]). The force gathers at a rally
//!    tile [`CONQUEST_RALLY_MIN`]–[`CONQUEST_RALLY_MAX`] tiles from the
//!    target on our side of it. When at least [`CONQUEST_ASSEMBLY_SHARE`] of
//!    the force stands within [`CONQUEST_ASSEMBLY_RADIUS`] of the rally AND
//!    the city's own bill is covered, the cheapest legal war is declared
//!    through the shipped `preferred_war_opening` (a casus belli if one is
//!    free, else a surprise war) and the campaign is handed to
//!    `city_campaign.rs` with the city pinned: `campaign_target` and
//!    `campaign_objective_city` are what `assess` reads, so from that moment
//!    the whole shipped army machinery — force groups, staging ring, siege
//!    train, pillage — is aimed at this city and nothing re-aims it.
//! 4. **The vision guard** ([`AdvancedAi::conquest_blind_tile_penalty`]). A
//!    unit of the strike force does not end its move on a tile whose 1-ring
//!    holds a tile it cannot see, unless a friendly unit stands beside it.
//!    This is the `no_visible_threat` answer, and it is deliberately scoped
//!    to the force: it is a term in the deployed mover's tile score, the
//!    same seam `close-as-a-body` uses, and it is worth
//!    [`CONQUEST_BLIND_TILE_PENALTY`] — more than any one tile of objective
//!    progress can pay, less than a certain death.
//! 5. **After the first capture** ([`AdvancedAi::conquest_continuation`]).
//!    While the war's own kills per loss is at least
//!    [`CONQUEST_KILLS_PER_LOSS_FLOOR`], the campaign extends to the next
//!    known city of the same rival. Below it, the shipped peace desk is
//!    asked for terms. The rate is counted by this module from the engine's
//!    `kills` counter and our own roster, because the `kills_per_loss` the
//!    operator reads is computed after the fact by `tools/live_ledger.py`
//!    and does not exist inside the simulator.
//!
//! Abandonment: if the bill is still not covered
//! [`CONQUEST_ABANDON_TURNS`] standard turns after the force first
//! assembled, the reservation is released, the target is dropped, and the
//! reason is journalled.
//!
//! ## What it deliberately does not do
//!
//! It does not move a unit itself (step 4 is a score term, not an order); it
//! does not choose the tactics of the assault (that is `city_campaign`,
//! `siege_train` and the battle planner); it does not raise the empire's
//! military target or change any other city's production; and it never
//! declares on a rival the shipped `campaign_target_legal` mask refuses.

use super::city_campaign::{CampaignPlan, CAMPAIGN_MIN_BODIES};
use super::{AdvancedAi, EmpireCounts};
use crate::game::{Action, ActionFamilies, Game};
use crate::name::Name;
use crate::rules::UnitSpec;
use crate::think;
use crate::world::TileBits;
use crate::Pos;
use std::collections::BTreeSet;

/// How far from our capital a target city may stand, in the wrapped world
/// distance every campaign reach in this controller is measured in
/// (`CAMPAIGN_REACH`, `CAMPAIGN_V2_REACH`, `rival_is_in_campaign_reach`).
/// Twelve is `CAMPAIGN_V2_REACH`: the distance at which a force can reach
/// the city without pulling the field army across a frontier.
pub(crate) const CONQUEST_REACH_TILES: i32 = 12;

/// The most cities a rival may be KNOWN to hold and still be an opening
/// target. Beyond three the neighbour is no longer a small empire whose
/// capital is a decisive prize; it is a war, and this gene is an opening.
pub(crate) const CONQUEST_MAX_RIVAL_CITIES: usize = 3;

/// The standard turn by which the force must have committed. Sixty is the
/// end of the window in which the rung's handicap is still small and a city
/// is still defended by one or two Ancient bodies; it is also the turn the
/// opening band (`4–6 cities @ t60`) is measured at, so the reservation
/// cannot run past the expansion decision it competes with.
pub(crate) const CONQUEST_COMMIT_DEADLINE: u32 = 60;

/// Ranged bodies the capital reserves. Three shooters take a city's hit
/// points down without ever standing in the counter-attack.
pub(crate) const CONQUEST_RANGED: usize = 3;

/// Melee bodies the capital reserves. Two: one to take the centre and one to
/// replace it, which is exactly `CAMPAIGN_SPARE_BODIES` over the minimum a
/// capture needs.
pub(crate) const CONQUEST_MELEE: usize = 2;

/// The reservation never defers a Settler while the empire holds fewer than
/// this many cities. One city is not an empire that can spare its capital's
/// production, and the first Settler is the one every recorded win came
/// from (`docs/eval` — the opening band).
pub(crate) const CONQUEST_FIRST_SETTLER_CITIES: usize = 2;

/// Nearest and furthest a rally tile may stand from the target city. Two is
/// inside the siege ring's own `CAMPAIGN_RING`; three is one march step
/// outside it, so the force closes the last tile together.
pub(crate) const CONQUEST_RALLY_MIN: i32 = 2;
/// See [`CONQUEST_RALLY_MIN`].
pub(crate) const CONQUEST_RALLY_MAX: i32 = 3;

/// The share of the force that must stand within
/// [`CONQUEST_ASSEMBLY_RADIUS`] of the rally before the war opens.
pub(crate) const CONQUEST_ASSEMBLY_SHARE: f64 = 0.8;
/// How close to the rally a body counts as assembled.
pub(crate) const CONQUEST_ASSEMBLY_RADIUS: i32 = 2;

/// Standard turns after the force first assembled that the opening waits for
/// a bill it can cover. Twenty is `CAMPAIGN_PATIENCE`: the same patience the
/// shipped campaign gives an unlaunched plan.
pub(crate) const CONQUEST_ABANDON_TURNS: u32 = 20;

/// The war's own kills per loss at or above which the campaign extends to
/// the rival's next city. One is break-even, and the live rate is 0.45: a
/// campaign trading at par is already twice the empire's ordinary rate.
pub(crate) const CONQUEST_KILLS_PER_LOSS_FLOOR: f64 = 1.0;

/// What a tile whose 1-ring holds something the unit cannot see costs a
/// strike-force body in the deployed mover's score. The score pays
/// `objective_progress × progress` — about 3 — per tile closer, and charges
/// up to about 15 for standing in a visible enemy's reach. Forty is chosen
/// to outrank any single tile of progress and the cohesion term together,
/// and to stay finite so a force with no seen tile to stand on still moves.
pub(crate) const CONQUEST_BLIND_TILE_PENALTY: f64 = 40.0;

/// What the reserved body is worth in the capital's production ranking while
/// the reservation is open. Above the Builder (260–295) and the opening
/// Monument (240) and below any Settler with a site (920 and up): the
/// reservation is enforced by DEFERRING the Settler arm, not by outbidding
/// it, so this term only has to beat the ordinary infrastructure it is
/// displacing.
pub(crate) const CONQUEST_RESERVATION_BASE: f64 = 400.0;
/// For every further reserved body still missing, so the capital finishes
/// the force instead of alternating with the next Builder.
pub(crate) const CONQUEST_RESERVATION_PER_MISSING: f64 = 60.0;

/// What a node on the way to the strike force's shooter is worth in
/// `tech_value` while the reservation is open and the best shooter the
/// empire can train is a range-one Slinger. The same weight
/// `early-archers` pays its own beeline.
pub(crate) const CONQUEST_RESEARCH: f64 = 90.0;

/// The opening this controller has committed to.
///
/// Everything the gene remembers between turns lives here, so the flag being
/// off leaves exactly one `None` behind and no other state.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ConquestOpening {
    /// The rival whose city this is.
    pub(crate) target: usize,
    /// The city to take.
    pub(crate) city: u32,
    /// The turn the target was first named.
    pub(crate) opened: u32,
    /// The tile the force gathers on, on our side of the city.
    pub(crate) rally: Pos,
    /// The strike force: the bodies this opening is spending.
    pub(crate) force: BTreeSet<u32>,
    /// The turn the force first stood assembled at the rally.
    pub(crate) assembled: Option<u32>,
    /// The turn war was declared under this opening.
    pub(crate) declared: Option<u32>,
    /// Our `kills` counter when the war opened; the war's kills are read
    /// against it.
    pub(crate) kills_at_war: i64,
    /// Bodies of ours lost since the war opened.
    pub(crate) losses: u32,
    /// Cities taken under this opening.
    pub(crate) taken: usize,
}

impl ConquestOpening {
    /// The war's own kills per loss so far. A war with no losses yet trades
    /// at its kill count, and a war with neither is at the floor: an opening
    /// that has not yet paid anything is not evidence against itself.
    pub(crate) fn kills_per_loss(&self, g: &Game, pid: usize) -> f64 {
        let kills = g.players[pid]
            .counters
            .get("kills")
            .copied()
            .unwrap_or(0)
            .saturating_sub(self.kills_at_war)
            .max(0) as f64;
        if self.losses == 0 {
            return kills.max(CONQUEST_KILLS_PER_LOSS_FLOOR);
        }
        kills / self.losses as f64
    }
}

/// One candidate target, ranked. Lower is better in every field, in order.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct TargetRank {
    /// `0` for the rival's capital, `1` for any other city. The capital is
    /// the one city whose loss can end a small neighbour outright.
    not_the_capital: u8,
    /// The visible garrison standing on or beside the city, in whole
    /// strength points, so the ranking is a total order and does not depend
    /// on floating-point ties.
    garrison: i64,
    /// Distance from our capital, so a tie goes to the nearer city.
    distance: i32,
    /// The city id, so the whole ranking is deterministic.
    city: u32,
}

impl AdvancedAi {
    // ------------------------------------------------------------------
    // 1. the target
    // ------------------------------------------------------------------

    /// Our capital, or `None` before it is founded.
    fn conquest_capital(g: &Game, pid: usize) -> Option<u32> {
        g.player_city_ids(pid)
            .into_iter()
            .find(|cid| g.cities[cid].is_capital)
    }

    /// Whether this seat has seen the tile at all. The fog-honest reading of
    /// "a city we know about": the same `players[pid].explored` test
    /// `opening_archery_goal` uses to decide whether a barbarian camp is
    /// known. A city on a tile we have never walked past does not exist for
    /// this gene, however plainly `g.cities` lists it.
    fn conquest_explored(g: &Game, pid: usize, pos: Pos) -> bool {
        g.players[pid].explored.contains(&pos)
    }

    /// The cities of `rival` this seat knows about.
    fn conquest_known_cities(g: &Game, pid: usize, rival: usize) -> Vec<u32> {
        let mut known: Vec<u32> = g
            .cities
            .values()
            .filter(|city| city.owner == rival && Self::conquest_explored(g, pid, city.pos))
            .map(|city| city.id)
            .collect();
        known.sort_unstable();
        known
    }

    /// The strength of the rival bodies we can SEE on or beside the city.
    /// Not the city's own strength and not a remembered garrison: the
    /// question this ranks is which of two known cities looks lighter from
    /// where we stand, and a defender in the fog is not an observation.
    fn conquest_visible_garrison(g: &Game, pid: usize, city: u32, visible: &TileBits) -> f64 {
        let Some(city) = g.cities.get(&city) else {
            return 0.0;
        };
        g.units
            .values()
            .filter(|unit| unit.owner == city.owner)
            .filter(|unit| g.rules.units[unit.kind].class == "military")
            .filter(|unit| g.wdist(unit.pos, city.pos) <= 1)
            .filter(|unit| g.sees(visible, unit.pos) && g.unit_visible_to(unit.id, pid))
            .map(|unit| crate::game::effective_strength(g.unit_strength(unit, true), unit.hp))
            .sum()
    }

    /// ⭐ THE TARGET: the city this opening is for, or `None`.
    ///
    /// A met rival, a city we have explored within [`CONQUEST_REACH_TILES`]
    /// of our capital, an owner with at most [`CONQUEST_MAX_RIVAL_CITIES`]
    /// known cities, and the shipped legality mask
    /// (`campaign_target_legal`: alive, not a friend or ally, not a
    /// city-state under a suzerain we may not offend). Ranked by
    /// [`TargetRank`]: the capital first, then the lightest visible
    /// garrison, then the nearest, then the lowest id.
    pub(crate) fn conquest_target(&self, g: &Game, pid: usize) -> Option<(usize, u32)> {
        if !self.early_conquest_opening {
            return None;
        }
        let capital = Self::conquest_capital(g, pid)?;
        let home = g.cities[&capital].pos;
        let visible = self.battlefront_visibility(g, pid);
        let mut best: Option<(TargetRank, usize)> = None;
        for rival in 0..g.players.len() {
            let player = &g.players[rival];
            if rival == pid || !player.alive || player.is_minor || player.is_barbarian {
                continue;
            }
            if !g.has_met(pid, rival) || !self.campaign_target_legal(g, pid, rival) {
                continue;
            }
            let known = Self::conquest_known_cities(g, pid, rival);
            if known.is_empty() || known.len() > CONQUEST_MAX_RIVAL_CITIES {
                continue;
            }
            for city in known {
                let pos = g.cities[&city].pos;
                let distance = g.wdist(home, pos);
                if distance > CONQUEST_REACH_TILES {
                    continue;
                }
                let rank = TargetRank {
                    not_the_capital: u8::from(!g.cities[&city].is_capital),
                    garrison: Self::conquest_visible_garrison(g, pid, city, &visible) as i64,
                    distance,
                    city,
                };
                if best.as_ref().is_none_or(|(held, _)| rank < *held) {
                    best = Some((rank, rival));
                }
            }
        }
        best.map(|(rank, rival)| (rival, rank.city))
    }

    // ------------------------------------------------------------------
    // 2. the reservation
    // ------------------------------------------------------------------

    /// Whether the reservation is open at all: the gene is on, an opening
    /// stands, the war has not opened yet, and the standard turn is before
    /// the commit deadline.
    fn conquest_reservation_open(&self, g: &Game) -> bool {
        self.early_conquest_opening
            && self.conquest_opening.as_ref().is_some_and(|opening| {
                opening.declared.is_none()
                    && g.turn < g.standard_duration(CONQUEST_COMMIT_DEADLINE)
            })
    }

    /// A body the strike force's ranged half counts: a land, non-siege
    /// military unit that shoots. A Slinger counts here where it does not
    /// count for `early-archers` — a range-one shot is a poor city defence
    /// but it is a real body in a five-unit column, and the research credit
    /// below is what turns it into an Archer.
    pub(crate) fn conquest_ranged_body(spec: &UnitSpec) -> bool {
        spec.class == "military"
            && matches!(spec.domain.as_deref(), None | Some("land"))
            && !spec.siege
            && spec.has_ranged_attack()
    }

    /// A body the strike force's melee half counts: a land military unit
    /// that can take a city centre. A `recon` body is excluded — a Scout
    /// cannot capture and the census already files it under melee, which is
    /// exactly the miscount `early-contact-window` had to work around.
    pub(crate) fn conquest_melee_body(spec: &UnitSpec) -> bool {
        spec.class == "military"
            && matches!(spec.domain.as_deref(), None | Some("land"))
            && spec.promotion_class != "recon"
            && spec.is_melee_capable()
            && !spec.has_ranged_attack()
    }

    /// The reserved bodies still missing, as `(ranged, melee)`, read off the
    /// caller's own empire census so the arm allocates nothing new and two
    /// cities reviewed in the same turn cannot both start the same body.
    ///
    /// The census files a Scout under `melee`, so the melee half subtracts
    /// the Scouts back out; the ranged half is the census's land shooters,
    /// which is every land ranged body including a Slinger.
    pub(super) fn conquest_reservation_shortfall(counts: &EmpireCounts) -> (usize, usize) {
        let melee = counts.melee.saturating_sub(counts.scouts);
        (
            CONQUEST_RANGED.saturating_sub(counts.ranged),
            CONQUEST_MELEE.saturating_sub(melee),
        )
    }

    /// `early-conquest-opening`: what training `spec` in `cid` is worth on
    /// top of the military arm's own sum. Zero with the gene off, outside
    /// the reservation window, in any city but the capital, while the city
    /// is threatened, for anything but a reserved body, and once the force
    /// is complete.
    ///
    /// `threatened` is the caller's own defence sentinel — a barbarian
    /// alarm, the plan's threatened city, or a city hit in the last four
    /// turns. The opening never outranks it: a capital under attack builds
    /// what defends it.
    pub(super) fn conquest_reservation(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        spec: &UnitSpec,
        counts: &EmpireCounts,
        threatened: bool,
    ) -> f64 {
        if threatened || !self.conquest_reservation_open(g) {
            return 0.0;
        }
        if Self::conquest_capital(g, pid) != Some(cid) {
            return 0.0;
        }
        let (ranged, melee) = Self::conquest_reservation_shortfall(counts);
        let wanted = if Self::conquest_ranged_body(spec) {
            ranged
        } else if Self::conquest_melee_body(spec) {
            melee
        } else {
            0
        };
        if wanted == 0 {
            return 0.0;
        }
        CONQUEST_RESERVATION_BASE
            + CONQUEST_RESERVATION_PER_MISSING * (ranged + melee).saturating_sub(1) as f64
    }

    /// `early-conquest-opening`: whether the capital defers its Settler
    /// while the reservation is unfilled.
    ///
    /// This is what makes the reservation a reservation rather than a bid.
    /// It is deliberately as narrow as `threatened_recovery_holds_settlers`:
    /// the capital only, while a target stands, before the deadline, while
    /// bodies are actually missing, never while the city is threatened, and
    /// never while the empire holds fewer than
    /// [`CONQUEST_FIRST_SETTLER_CITIES`] cities — the first Settler is never
    /// deferred, because an empire of one city has nothing to conquer with.
    pub(super) fn conquest_defers_the_settler(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        counts: &EmpireCounts,
        city_count: usize,
        threatened: bool,
    ) -> bool {
        if threatened || !self.conquest_reservation_open(g) {
            return false;
        }
        if Self::conquest_capital(g, pid) != Some(cid) {
            return false;
        }
        if city_count < CONQUEST_FIRST_SETTLER_CITIES {
            return false;
        }
        let (ranged, melee) = Self::conquest_reservation_shortfall(counts);
        ranged + melee > 0
    }

    /// The node that upgrades a range-one shooter into a real one, while the
    /// reservation is open and the empire's best shooter is still a Slinger.
    /// `None` with the gene off, outside the window, once the node is held,
    /// and whenever the empire can already train a range-two shooter.
    fn conquest_research_node(&self, g: &Game, pid: usize) -> Option<Name> {
        if !self.conquest_reservation_open(g) {
            return None;
        }
        let node = Self::early_archers_node(g, pid)?;
        (!g.players[pid].techs.contains(&node)).then_some(node)
    }

    /// `early-conquest-opening`: what `tech` is worth in `tech_value` for
    /// leading to the strike force's shooter. Zero with the gene off,
    /// outside the reservation window, and once the node is held.
    pub(crate) fn conquest_research_value(&self, g: &Game, pid: usize, tech: &str) -> f64 {
        let Some(node) = self.conquest_research_node(g, pid) else {
            return 0.0;
        };
        if !self.tech_leads_to(g, tech, &node) {
            return 0.0;
        }
        CONQUEST_RESEARCH
    }

    // ------------------------------------------------------------------
    // 3. assembly and declaration
    // ------------------------------------------------------------------

    /// The rally tile: a dry, passable tile [`CONQUEST_RALLY_MIN`] to
    /// [`CONQUEST_RALLY_MAX`] from the target, nearest our capital — "on our
    /// side" is exactly that, and it needs no separate geometry. `None` when
    /// the ring around the city is all water or impassable.
    pub(crate) fn conquest_rally_tile(g: &Game, home: Pos, city: Pos) -> Option<Pos> {
        g.wdisk(city, CONQUEST_RALLY_MAX)
            .into_iter()
            .filter(|pos| (CONQUEST_RALLY_MIN..=CONQUEST_RALLY_MAX).contains(&g.wdist(*pos, city)))
            .filter(|pos| {
                g.map
                    .get(*pos)
                    .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
            })
            .min_by_key(|pos| (g.wdist(*pos, home), *pos))
    }

    /// The strike force: our field army, nearest the rally first, capped at
    /// the bodies this opening reserved. Deterministic — ties break on unit
    /// id — so the same force is named every turn a unit has not moved.
    fn conquest_force(&self, g: &Game, pid: usize, rally: Pos) -> BTreeSet<u32> {
        let mut army: Vec<u32> = self.campaign_field_army(g, pid);
        army.sort_by_key(|uid| (g.wdist(g.units[uid].pos, rally), *uid));
        army.truncate(CONQUEST_RANGED + CONQUEST_MELEE);
        army.into_iter().collect()
    }

    /// The share of the force standing within [`CONQUEST_ASSEMBLY_RADIUS`]
    /// of the rally. Zero for an empty force: nothing is assembled.
    pub(crate) fn conquest_assembled_share(g: &Game, opening: &ConquestOpening) -> f64 {
        if opening.force.is_empty() {
            return 0.0;
        }
        let up = opening
            .force
            .iter()
            .filter_map(|uid| g.units.get(uid))
            .filter(|unit| g.wdist(unit.pos, opening.rally) <= CONQUEST_ASSEMBLY_RADIUS)
            .count();
        up as f64 / opening.force.len() as f64
    }

    /// Whether the force can take the city.
    ///
    /// The preview is the shipped `campaign_city_requirement` — the
    /// defenders within `CAMPAIGN_DEFENDER_RADIUS`, the city's own strength,
    /// its walls at `WALL_STRENGTH_PER_100_HP`, every one of them moved by
    /// the tech edge, times `CAMPAIGN_SUPERIORITY`. That is this
    /// controller's own city preview, and it is deliberately used in place
    /// of the host's `SimulateAttackInto` reading: `Game::host_preview` is a
    /// live-mirror field that does not exist in a simulated game, and it was
    /// measured to over-predict our damage by about 9 HP over 700 strikes.
    /// A bill that is 1.5× the defence is the honest version of the same
    /// question.
    pub(crate) fn conquest_preview_takes_the_city(
        &self,
        g: &Game,
        pid: usize,
        opening: &ConquestOpening,
    ) -> bool {
        let Some(appraisal) = self.appraise_neighbour(g, pid, opening.target) else {
            return false;
        };
        if !g
            .cities
            .get(&opening.city)
            .is_some_and(|city| city.owner == opening.target)
        {
            return false;
        }
        let force: Vec<u32> = opening.force.iter().copied().collect();
        let strength = Self::campaign_strength_of(g, &force);
        let average_body = if force.is_empty() {
            return false;
        } else {
            strength / force.len() as f64
        };
        let requirement =
            self.campaign_city_requirement(g, pid, opening.city, &appraisal, average_body);
        let has_capturer = force
            .iter()
            .any(|uid| g.rules.units[g.units[uid].kind].is_melee_capable());
        requirement.holdable
            && has_capturer
            && force.len() >= CAMPAIGN_MIN_BODIES
            && strength + 1e-9 >= requirement.strength
    }

    // ------------------------------------------------------------------
    // 4. the vision guard
    // ------------------------------------------------------------------

    /// Whether a friendly unit of ours stands beside `tile`, excluding the
    /// unit that is choosing it.
    fn conquest_friend_beside(g: &Game, pid: usize, uid: u32, tile: Pos) -> bool {
        g.units.values().any(|other| {
            other.owner == pid && other.id != uid && g.wdist(other.pos, tile) == 1
        })
    }

    /// `early-conquest-opening`: what standing on `tile` costs a strike-force
    /// body in the deployed mover's one-ply score.
    ///
    /// 234 of 304 live unit deaths carry `no_visible_threat`: the blow came
    /// out of a tile the unit could not see. A tile whose own 1-ring holds
    /// an unseen tile is a tile something can be standing next to unseen, so
    /// it is charged [`CONQUEST_BLIND_TILE_PENALTY`] — unless a friendly
    /// body stands beside it, which is the screen that makes the same tile
    /// survivable.
    ///
    /// Zero with the gene off, for any unit that is not in this opening's
    /// force, and for a tile whose ring is wholly seen. Scoped to the force
    /// on purpose: a scout's job is to stand where it cannot see.
    pub(crate) fn conquest_blind_tile_penalty(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        tile: Pos,
        visible: &TileBits,
    ) -> f64 {
        if !self.early_conquest_opening {
            return 0.0;
        }
        if !self
            .conquest_opening
            .as_ref()
            .is_some_and(|opening| opening.force.contains(&uid))
        {
            return 0.0;
        }
        let blind = g.nbrs(tile).iter().any(|pos| !g.sees(visible, *pos));
        if !blind || Self::conquest_friend_beside(g, pid, uid, tile) {
            return 0.0;
        }
        CONQUEST_BLIND_TILE_PENALTY
    }

    // ------------------------------------------------------------------
    // the lifecycle
    // ------------------------------------------------------------------

    /// Whether this opening owns the shared campaign plan this turn, so
    /// `maintain_city_campaign` leaves it alone. Exactly `false` with the
    /// gene off.
    pub(crate) fn conquest_owns_the_campaign(&self) -> bool {
        self.early_conquest_opening
            && self
                .conquest_opening
                .as_ref()
                .is_some_and(|opening| opening.declared.is_some())
    }

    /// Write the opening's city into the shared campaign plan, which is what
    /// `assess` reads through `campaign_target` and
    /// `campaign_objective_city`. This is the whole handoff: from here the
    /// shipped force groups, staging ring, siege train and pillage step are
    /// aimed at this city and nothing re-aims them.
    fn conquest_pin_the_campaign(&mut self, g: &Game, pid: usize) {
        let Some(opening) = self.conquest_opening.as_ref() else {
            return;
        };
        let force: Vec<u32> = opening.force.iter().copied().collect();
        let strength = Self::campaign_strength_of(g, &force);
        let bodies = force.len().max(CAMPAIGN_MIN_BODIES);
        let _ = pid;
        self.campaign = Some(CampaignPlan {
            target: opening.target,
            cities: vec![opening.city],
            requirement: strength,
            bodies,
            planned: opening.opened,
            declared: opening.declared,
            taken: opening.taken,
        });
    }

    /// Count the bodies of ours that left the board since the last turn
    /// boundary, and refresh the roster. Only the strike force is counted:
    /// this rate is the campaign's, not the empire's.
    fn conquest_count_losses(&mut self, g: &Game) {
        let Some(opening) = self.conquest_opening.as_mut() else {
            return;
        };
        if opening.declared.is_none() {
            return;
        }
        let lost = opening
            .force
            .iter()
            .filter(|uid| !g.units.contains_key(uid))
            .count();
        opening.losses = opening.losses.saturating_add(lost as u32);
        opening.force.retain(|uid| g.units.contains_key(uid));
    }

    /// Drop the opening and journal why.
    fn conquest_release(&mut self, g: &Game, pid: usize, why: &str) {
        let Some(opening) = self.conquest_opening.take() else {
            return;
        };
        let _ = pid;
        think!(self.journal(), Military, Strategy,
               "Releasing the conquest opening on {}", g.players[opening.target].civ;
               "{why}");
    }

    /// ⭐ Start of turn: keep the opening honest.
    ///
    /// Runs before `maintain_city_campaign` so the pinned plan is in place
    /// before the shipped campaign maintenance reads it, and before `assess`
    /// so the strategic plan aims at the pinned city on the same turn the
    /// war opens. Exact no-op with the gene off.
    pub(crate) fn maintain_conquest_opening(&mut self, g: &mut Game, pid: usize) {
        if !self.early_conquest_opening {
            self.conquest_opening = None;
            return;
        }
        self.conquest_count_losses(g);
        if let Some(opening) = self.conquest_opening.as_ref() {
            let target_alive = g
                .players
                .get(opening.target)
                .is_some_and(|player| player.alive);
            let still_theirs = g
                .cities
                .get(&opening.city)
                .is_some_and(|city| city.owner == opening.target);
            if !target_alive {
                self.conquest_release(g, pid, "the rival is no longer in the game");
                return;
            }
            if !still_theirs {
                self.conquest_after_a_capture(g, pid);
                return;
            }
            if opening.declared.is_none() {
                if let Some(assembled) = opening.assembled {
                    if g.turn.saturating_sub(assembled)
                        >= g.standard_duration(CONQUEST_ABANDON_TURNS)
                    {
                        self.conquest_release(
                            g,
                            pid,
                            "the force has stood at the rally for the whole patience window \
                             without covering the city's bill",
                        );
                        return;
                    }
                }
                if g.turn >= g.standard_duration(CONQUEST_COMMIT_DEADLINE)
                    && opening.assembled.is_none()
                {
                    self.conquest_release(
                        g,
                        pid,
                        "the commit deadline passed before the force ever assembled",
                    );
                    return;
                }
            }
        }
        if self.conquest_opening.is_none() {
            self.conquest_open(g, pid);
        }
        self.conquest_refresh_force(g, pid);
        if self.conquest_owns_the_campaign() {
            self.conquest_pin_the_campaign(g, pid);
        }
    }

    /// Name a target and open. Nothing happens once the commit deadline has
    /// passed: an opening is an opening.
    fn conquest_open(&mut self, g: &mut Game, pid: usize) {
        if g.turn >= g.standard_duration(CONQUEST_COMMIT_DEADLINE) {
            return;
        }
        let Some((target, city)) = self.conquest_target(g, pid) else {
            return;
        };
        let Some(capital) = Self::conquest_capital(g, pid) else {
            return;
        };
        let home = g.cities[&capital].pos;
        let Some(rally) = Self::conquest_rally_tile(g, home, g.cities[&city].pos) else {
            return;
        };
        think!(self.journal(), Military, Strategy,
               "Opening a conquest against {}", g.players[target].civ;
               "{} is {} tiles from the capital and they hold {} known cit{}; \
                the capital reserves {} shooters and {} melee bodies and rallies at the ring",
               g.cities[&city].name,
               g.wdist(home, g.cities[&city].pos),
               Self::conquest_known_cities(g, pid, target).len(),
               if Self::conquest_known_cities(g, pid, target).len() == 1 { "y" } else { "ies" },
               CONQUEST_RANGED, CONQUEST_MELEE;
               rally);
        self.conquest_opening = Some(ConquestOpening {
            target,
            city,
            opened: g.turn,
            rally,
            force: BTreeSet::new(),
            assembled: None,
            declared: None,
            kills_at_war: 0,
            losses: 0,
            taken: 0,
        });
    }

    /// Re-name the force from the board and record the turn it first stood
    /// assembled. The force is fixed once the war opens: a war's losses are
    /// counted against the bodies that started it.
    fn conquest_refresh_force(&mut self, g: &Game, pid: usize) {
        let Some(opening) = self.conquest_opening.as_ref() else {
            return;
        };
        if opening.declared.is_some() {
            return;
        }
        let rally = opening.rally;
        let force = self.conquest_force(g, pid, rally);
        let Some(opening) = self.conquest_opening.as_mut() else {
            return;
        };
        opening.force = force;
        if opening.assembled.is_none()
            && Self::conquest_assembled_share(g, opening) >= CONQUEST_ASSEMBLY_SHARE
            && !opening.force.is_empty()
        {
            opening.assembled = Some(g.turn);
        }
    }

    /// ⭐ The declaration, and the peace that closes the campaign.
    ///
    /// Called from `advanced_diplomacy` beside the shipped campaign's own
    /// peace desk. Returns whether it spent this turn's one declaration.
    /// Exact no-op with the gene off.
    pub(crate) fn conquest_declaration(&mut self, g: &mut Game, pid: usize) -> bool {
        if !self.early_conquest_opening {
            return false;
        }
        let Some(opening) = self.conquest_opening.clone() else {
            return false;
        };
        if opening.declared.is_some() {
            self.conquest_peace(g, pid, &opening);
            return false;
        }
        if opening.assembled.is_none() {
            return false;
        }
        let share = Self::conquest_assembled_share(g, &opening);
        if share < CONQUEST_ASSEMBLY_SHARE {
            return false;
        }
        if !self.conquest_preview_takes_the_city(g, pid, &opening) {
            think!(self.journal(), Military, Detail,
                   "Holding the conquest force at the rally";
                   "{:.0}% of the force is up, but the city's bill is not covered yet",
                   share * 100.0);
            return false;
        }
        if !self.campaign_target_legal(g, pid, opening.target) {
            return false;
        }
        let Some(action) = self.conquest_war_opening(g, pid, opening.target) else {
            return false;
        };
        if g.apply(pid, &action).is_err() {
            return false;
        }
        let kills = g.players[pid]
            .counters
            .get("kills")
            .copied()
            .unwrap_or(0);
        if let Some(opening) = self.conquest_opening.as_mut() {
            opening.declared = Some(g.turn);
            opening.kills_at_war = kills;
        }
        think!(self.journal(), Military, Decision,
               "Declaring war on {}", g.players[opening.target].civ;
               "{:.0}% of the strike force stands at the rally and the preview covers {}'s bill",
               share * 100.0,
               g.cities.get(&opening.city).map(|city| city.name.clone())
                   .unwrap_or_else(|| String::from("the objective")));
        self.conquest_pin_the_campaign(g, pid);
        true
    }

    /// The cheapest legal war: a casus belli when one happens to be free,
    /// otherwise the surprise war. Exactly `raid_opening`'s rule, and for
    /// the same reason — `preferred_war_opening` can answer with a
    /// `Denounce` (the Formal War clock), which is not a declaration and
    /// must not be mistaken for one by a force that is already assembled.
    fn conquest_war_opening(&self, g: &Game, pid: usize, target: usize) -> Option<Action> {
        if let Some(action) = self.preferred_war_opening(g, pid, target) {
            if matches!(action, Action::DeclareWarWithCasusBelli { .. }) {
                return Some(action);
            }
        }
        g.legal_actions_within(pid, ActionFamilies::DIPLOMACY)
            .into_iter()
            .find(|action| matches!(action, Action::DeclareWar { player } if *player == target))
    }

    /// After the first capture: extend to the rival's next known city while
    /// the war is paying, otherwise ask for terms.
    fn conquest_after_a_capture(&mut self, g: &mut Game, pid: usize) {
        let Some(opening) = self.conquest_opening.clone() else {
            return;
        };
        let ours = g
            .cities
            .get(&opening.city)
            .is_some_and(|city| city.owner == pid);
        if !ours {
            self.conquest_release(g, pid, "the objective city changed hands to a third party");
            return;
        }
        let taken = opening.taken + 1;
        let rate = opening.kills_per_loss(g, pid);
        if rate < CONQUEST_KILLS_PER_LOSS_FLOOR {
            think!(self.journal(), Military, Strategy,
                   "Closing the conquest against {}", g.players[opening.target].civ;
                   "the campaign has taken {} cit{} but is trading at {:.2} kills per loss, \
                    under the {:.1} floor",
                   taken,
                   if taken == 1 { "y" } else { "ies" },
                   rate, CONQUEST_KILLS_PER_LOSS_FLOOR);
            self.conquest_sue_for_peace(g, pid, opening.target);
            self.conquest_release(g, pid, "the campaign stopped paying and asked for terms");
            return;
        }
        let next = Self::conquest_known_cities(g, pid, opening.target)
            .into_iter()
            .filter(|cid| {
                g.cities
                    .get(cid)
                    .is_some_and(|city| city.owner == opening.target)
            })
            .min_by_key(|cid| (g.wdist(g.cities[cid].pos, opening.rally), *cid));
        let Some(next) = next else {
            think!(self.journal(), Military, Strategy,
                   "The conquest has taken every city it knows of from {}",
                   g.players[opening.target].civ;
                   "trading at {:.2} kills per loss; asking for terms", rate);
            self.conquest_sue_for_peace(g, pid, opening.target);
            self.conquest_release(g, pid, "no further known city of the rival remains");
            return;
        };
        think!(self.journal(), Military, Strategy,
               "The conquest continues to {}", g.cities[&next].name;
               "trading at {:.2} kills per loss, at or above the {:.1} floor",
               rate, CONQUEST_KILLS_PER_LOSS_FLOOR);
        if let Some(opening) = self.conquest_opening.as_mut() {
            opening.taken = taken;
            opening.city = next;
        }
        self.conquest_pin_the_campaign(g, pid);
    }

    /// The peace desk this gene shares with the shipped campaign: one offer
    /// per rival, never repeated while one is pending.
    fn conquest_sue_for_peace(&mut self, g: &mut Game, pid: usize, target: usize) {
        if !g.is_at_war(pid, target) || self.peace_offers.contains(&target) {
            return;
        }
        let pending = g.pending_deals.iter().any(|deal| {
            deal.peace
                && ((deal.from == pid && deal.to == target)
                    || (deal.from == target && deal.to == pid))
                && deal.expires >= g.turn
        });
        if pending {
            return;
        }
        self.peace_offers.insert(target);
        think!(self.journal(), Diplomacy, Decision,
               "Offering peace to {}", g.players[target].civ;
               "the conquest opening has stopped paying for itself");
        let _ = g.apply(
            pid,
            &Action::ProposeDeal {
                player: target,
                give_gold: 0.0,
                request_gold: 0.0,
                open_borders: false,
                friendship: false,
                peace: true,
                alliance: None,
            },
        );
    }

    /// A war that stopped paying is closed even before a capture.
    fn conquest_peace(&mut self, g: &mut Game, pid: usize, opening: &ConquestOpening) {
        if opening.taken == 0 || opening.kills_per_loss(g, pid) >= CONQUEST_KILLS_PER_LOSS_FLOOR {
            return;
        }
        self.conquest_sue_for_peace(g, pid, opening.target);
    }
}

#[cfg(test)]
mod tests;
