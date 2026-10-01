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
//!    target is named before [`CONQUEST_COMMIT_DEADLINE`], the CAPITAL's
//!    production is reserved through that deadline or a minimum preparation
//!    window, whichever is later, for
//!    [`CONQUEST_RANGED`] ranged bodies and [`CONQUEST_MELEE`] melee bodies,
//!    and — when the best shooter the empire can train is a range-one
//!    Slinger — the research picker chases the node that upgrades it, by the
//!    same beeline `early-archers` uses. The reservation is a real
//!    reservation, not a bid: while it is unfilled the capital's Settler arm
//!    is deferred outright, the way `threatened_recovery_holds_settlers`
//!    defers it. It never defers the FIRST Settler (an empire of one city
//!    has nothing to conquer with). A `threatened` capital keeps the
//!    reservation: its bodies are the capital's defence, and a raider beside
//!    the walls is the worst moment to train a Settler instead.
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
//!    progress can pay, less than a certain death. It cannot strand a body:
//!    the charge is one flat number on every blind candidate, so a unit
//!    whose every option is blind still ranks them by the rest of the score
//!    and moves, and a unit's own sight (radius two) means every tile one
//!    step away has a seen ring. The mover only supplies the frame while
//!    `battlefront_observation` is on, which it is in both controllers. A
//!    Settler's bound guard is never in the force, so an escort is never
//!    scored by it.
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
//! reason is journalled. Once the war is open the opening also closes when
//! the war ends by any road (a peace accepted, the rival dead) and when the
//! whole strike force is gone — a dead force sues for terms rather than
//! pinning the campaign to a city nobody is marching on. An opening that
//! got as far as assembling is this game's one attempt: it is never
//! re-opened, so a released reservation cannot come back and hold the
//! Settler again. An opening released before it assembled (the rival died,
//! the city changed hands, the rival became a friend) leaves the door open
//! for another target while the window lasts.
//!
//! The declaration honours the shipped vetoes an elective war honours:
//! `campaign_target_legal` (never a friend or ally), `one-war-at-a-time`'s
//! hold, and `war-needs-a-treasury`'s solvency test. Each of those is its
//! own gene's decision, unchanged here.
//!
//! ## What it deliberately does not do
//!
//! It supplies a staging objective to the existing peacetime mover for its
//! reserved force; it does not choose the tactics of the assault (that is `city_campaign`,
//! `siege_train` and the battle planner); it does not raise the empire's
//! military target or change any other city's production; and it never
//! declares on a rival the shipped `campaign_target_legal` mask refuses.

use super::city_campaign::{CampaignPlan, CAMPAIGN_MIN_BODIES};
use super::{AdvancedAi, EmpireCounts, GrandStrategy, StrategicPlan};
use crate::game::{Action, Game, Item};
use crate::name::Name;
use crate::rules::UnitSpec;
use crate::think;
use crate::world::TileBits;
use crate::Pos;
use std::collections::BTreeSet;

/// How far from our capital a target city may stand, in the wrapped world
/// distance every campaign reach in this controller is measured in
/// (`CAMPAIGN_REACH`, `CAMPAIGN_V2_REACH`, `rival_is_in_campaign_reach`).
///
/// Eighteen, not `CAMPAIGN_V2_REACH`'s twelve. On the live King seat's
/// four-player Tiny Pangaea (60×38) the nearest rival capital was first seen
/// 13–23 tiles from ours in the 2026-09-28/29 runs, so a twelve-tile reach
/// named a target in 4 of 21 games and the strike force the capital had
/// already trained stood at home. In the opening there is no field army to
/// pull across a frontier, and a Gran Colombia archer walks eighteen tiles in
/// six or seven turns.
pub(crate) const CONQUEST_REACH_TILES: i32 = 18;

/// The most cities a rival may be KNOWN to hold and still be an opening
/// target. Beyond four the neighbour is no longer a small empire whose
/// capital is a decisive prize; it is a war, and this gene is an opening.
/// Four, not three: a King rival on Online speed holds three or four cities
/// by the turn its capital is first charted, so three closed the window on
/// the very turn the target became visible.
pub(crate) const CONQUEST_MAX_RIVAL_CITIES: usize = 4;

/// The last standard turn on which a new opening can be named. Sixty is the
/// end of the window in which the rung's handicap is still small and a city
/// is still defended by one or two Ancient bodies. An opening named late in
/// this window may finish its minimum preparation after turn sixty.
pub(crate) const CONQUEST_COMMIT_DEADLINE: u32 = 60;

/// A target first seen near the end of that window still needs time to
/// assemble the five reserved bodies. On Online speed the standard deadline
/// is turn 40: the King Gran Colombia seat named Trà Kiệu on turn 36, then
/// released the opening on turn 40 before any force could assemble. The same
/// window starts when a second city lets the capital reserve production if
/// that happens after the target is named.
pub(crate) const CONQUEST_MIN_PREPARATION_TURNS: u32 = 30;

/// Ranged bodies the capital reserves. Four shooters take a city's hit
/// points down without ever standing in the counter-attack. Three was
/// measured short: on 2026-09-29 (King, Susa) three Archers put 61 damage
/// into the unwalled capital in their one full volley, the city healed
/// 15–20 a turn, and Ancient Walls went up two turns later, after which an
/// Archer's shot did 1–5. A fourth shooter makes it a two-volley city.
pub(crate) const CONQUEST_RANGED: usize = 4;

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

/// A full force that is still marching gets one bounded extension when at
/// least three of five bodies are already on the rally's assembly ring and
/// the other two are within five tiles. In the King Online game on 2026-09-28,
/// the opening expired on turn 40 with three bodies within two tiles and the
/// other two four and five tiles away; expiration diverted the whole column.
pub(crate) const CONQUEST_APPROACHING_SHARE: f64 = 0.6;
pub(crate) const CONQUEST_APPROACHING_RADIUS: i32 = 5;
pub(crate) const CONQUEST_APPROACHING_GRACE_TURNS: u32 = 12;

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

/// Recon bodies the empire keeps while the opening has nothing to aim at.
/// One Scout is what the opening book buys; the second is this gene's.
pub(crate) const CONQUEST_SEARCH_SCOUTS: usize = 2;

/// What the second Scout is worth in the capital's ranking while no target
/// is known. Above the Builder (260–295) and the opening Monument (240),
/// below any Settler with a site (920 and up). The opening is inert until a
/// rival city is charted in reach, and on the live King Tiny Pangaea the one
/// opening Scout met no major until turn 43 (2026-09-30) or 46 (2026-09-29)
/// while four Archers stood at home; `early-contact-window` values a second
/// eye only for unmet city-states, and at a half share below a Builder.
pub(crate) const CONQUEST_SEARCH_SCOUT_VALUE: f64 = 300.0;

/// The slowest a city may train the search Scout and still be asked. An eye
/// that arrives after the window it searches for is no eye: the first live
/// game with the term (2026-09-30) put it in a 0.9-production second city,
/// seventeen turns out, when the capital finished its Settler in four.
pub(crate) const CONQUEST_SEARCH_SCOUT_MAX_TURNS: f64 = 6.0;

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
    /// The turn the empire first had two cities and could reserve the force.
    pub(crate) preparing_since: Option<u32>,
    /// The fixed end of a short extension granted when the full force nears the rally.
    pub(crate) grace_until: Option<u32>,
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

/// `conquest-takes-the-soft-city`: rank the opening's target by what can be
/// taken before what is worth most.
///
/// ## The defect
///
/// [`TargetRank`] ships with `not_the_capital` as its LEADING field, so the
/// rival's capital outranks every other city of theirs whatever is standing in
/// it — the garrison field below it only ever breaks a tie between two cities
/// of the same capital-ness. The opening force is
/// [`CONQUEST_RANGED`] shooters and [`CONQUEST_MELEE`] melee bodies, and the
/// capital is the one city on the board that is reliably defended: it starts
/// with the rival's own Warrior, it is where the free difficulty units spawn,
/// and it is the city an AI reinforces first.
///
/// The record says what that costs. Over 111 recorded Emperor games the empire
/// took **2 cities and lost 65**, at 0.45 kills per loss — and from Emperor
/// upward each rival opens with free Settlers, so a rival holds *more, thinner*
/// cities than the capital-first rule assumes. A target the assembled force
/// cannot take is worth nothing however valuable it is, and
/// `conquest_preview_takes_the_city` then refuses the declaration, so the
/// opening spends its whole reservation and never fires.
///
/// ## What the gene changes
///
/// One thing: [`TargetRank::key`] leads with what the city will COST TO TAKE
/// and demotes capital-ness to the field under it. Same tie-breaks — so among
/// equally defended cities the capital still wins, and the shipped intent
/// survives everywhere the two questions do not conflict.
///
/// The cost is `Game::city_strength` plus the visible garrison, which is the
/// same shape as the `at_city + defenders` term
/// `campaign_city_requirement` builds — the number
/// [`AdvancedAi::conquest_preview_takes_the_city`] already refuses the
/// declaration on. **Selection and admission were asking different questions**:
/// the target was chosen on visible units alone while the gate priced the
/// Palace, the walls and the defence districts, so the opening could reserve
/// the capital's production for sixty turns against a city the gate was always
/// going to refuse. Ranking on the gate's own quantity is what makes the
/// reservation buy something.
///
/// ⚠ `city_strength` is the right reading on the seat this is for: on a live
/// board it answers out of `observed_city_strength`, the strength the mirror
/// read off the city banner. The visible-garrison term keeps its existing
/// fog-honest filter, so no unit this seat cannot see enters the ranking.
///
/// ⚠ **It is deliberately not gated on the difficulty rung**, although the rung
/// is what motivates it. `gene_screen`'s ledger shape runs at **Prince** with no
/// rotation (`difficulty: "prince"`, `difficulty_rotate: ""` in every standard
/// screen), so a gene that is inert below Emperor could never be priced by the
/// ranking and could never be turned on by it — inert by construction, which is
/// the state eight genes were already found in. The rule stands at every rung
/// instead, and the rung is the reason it was looked for.
/// One candidate target, ranked. Lower is better in every field, in order.
///
/// ⚠ The field ORDER is the ranking, and `conquest-takes-the-soft-city`
/// changes which of the first two leads. Read [`TargetRank::key`] rather than
/// comparing these directly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TargetRank {
    /// `0` for the rival's capital, `1` for any other city. The capital is
    /// the one city whose loss can end a small neighbour outright.
    not_the_capital: u8,
    /// The visible garrison standing on or beside the city, in whole
    /// strength points, so the ranking is a total order and does not depend
    /// on floating-point ties.
    garrison: i64,
    /// What this city will cost to take: its own strength — the Palace, the
    /// walls, the defence districts and the terrain under it — plus the
    /// visible garrison, in whole strength points.
    ///
    /// Read from `Game::city_strength`, which on a live seat answers out of
    /// `observed_city_strength`, the strength the mirror actually read off the
    /// city banner. Only `conquest-takes-the-soft-city` consults it.
    defence: i64,
    /// Distance from our capital, so a tie goes to the nearer city.
    distance: i32,
    /// The city id, so the whole ranking is deterministic.
    city: u32,
}

impl TargetRank {
    /// The comparison key.
    ///
    /// Shipped (`soft_first` false) this is exactly the derived ordering the
    /// four fields used to have: capital-ness, then the visible garrison, then
    /// distance, then id — `defence` is not read at all, so the gene off is
    /// byte-identical.
    ///
    /// On, the leading question becomes *what will this cost to take*, with
    /// capital-ness demoted to the field under it. Same tie-breaks, so among
    /// equally defended cities the capital still wins and the shipped intent
    /// survives everywhere the two questions do not conflict.
    fn key(&self, soft_first: bool) -> (i64, i64, i32, u32) {
        let capital = i64::from(self.not_the_capital);
        if soft_first {
            (self.defence, capital, self.distance, self.city)
        } else {
            (capital, self.garrison, self.distance, self.city)
        }
    }
}

impl AdvancedAi {
    /// Carry the opening through a live board rebuild. City positions are
    /// stable across captures; mirror-local city and unit IDs are not.
    pub fn remap_conquest_memory(
        &mut self,
        previous: &Game,
        next: &Game,
        units: &std::collections::BTreeMap<u32, u32>,
    ) {
        let Some(opening) = self.conquest_opening.as_mut() else {
            return;
        };
        let city = previous
            .cities
            .get(&opening.city)
            .and_then(|city| next.city_at(city.pos));
        let Some(city) = city else {
            if self.campaign.as_ref().is_some_and(|campaign| {
                campaign.target == opening.target && campaign.cities == vec![opening.city]
            }) {
                self.campaign = None;
            }
            self.conquest_release(
                next,
                "the objective city is no longer on the observed board",
            );
            return;
        };
        opening.city = city;
        let force: BTreeSet<u32> = opening
            .force
            .iter()
            .filter_map(|unit| units.get(unit).copied())
            .collect();
        if opening.declared.is_some() {
            opening.losses = opening
                .losses
                .saturating_add(opening.force.len().saturating_sub(force.len()) as u32);
        }
        opening.force = force;
        // The next maintenance pass decides captures and peace from the new
        // board. Do not let its shared campaign still point at the old ID.
        if opening.declared.is_some() {
            self.conquest_pin_the_campaign(next);
        }
    }

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
        let soft_first = self.conquest_takes_the_soft_city;
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
                let garrison = Self::conquest_visible_garrison(g, pid, city, &visible);
                let rank = TargetRank {
                    not_the_capital: u8::from(!g.cities[&city].is_capital),
                    garrison: garrison as i64,
                    defence: (g.city_strength(city) + garrison) as i64,
                    distance,
                    city,
                };
                if best
                    .as_ref()
                    .is_none_or(|(held, _)| rank.key(soft_first) < held.key(soft_first))
                {
                    best = Some((rank, rival));
                }
            }
        }
        best.map(|(rank, rival)| (rival, rank.city))
    }

    // ------------------------------------------------------------------
    // 2. the reservation
    // ------------------------------------------------------------------

    /// The original deadline plus preparation time after reservation opens.
    fn conquest_commit_base(g: &Game, opening: &ConquestOpening) -> u32 {
        g.standard_duration(CONQUEST_COMMIT_DEADLINE).max(
            opening
                .opened
                .max(opening.preparing_since.unwrap_or(opening.opened))
                .saturating_add(g.standard_duration(CONQUEST_MIN_PREPARATION_TURNS)),
        )
    }

    /// All five reserved bodies are close enough that another short march
    /// can assemble the column; an incomplete or distant force gets no grace.
    fn conquest_force_approaching(g: &Game, opening: &ConquestOpening) -> bool {
        opening.force.len() == CONQUEST_RANGED + CONQUEST_MELEE
            && Self::conquest_assembled_share(g, opening) >= CONQUEST_APPROACHING_SHARE
            && opening.force.iter().all(|uid| {
                g.units.get(uid).is_some_and(|unit| {
                    g.wdist(unit.pos, opening.rally) <= CONQUEST_APPROACHING_RADIUS
                })
            })
    }

    /// The opening can only be named before the original deadline. Once it
    /// has a real target and can reserve production, it gets a minimum
    /// preparation window. A near-assembled force may receive one fixed,
    /// bounded extension at that deadline.
    fn conquest_commit_due(g: &Game, opening: &ConquestOpening) -> u32 {
        let base = Self::conquest_commit_base(g, opening);
        base.max(opening.grace_until.unwrap_or(base))
    }

    /// Whether the reservation is open at all: the gene is on, an opening
    /// stands, the war has not opened yet, and its preparation has not expired.
    fn conquest_reservation_open(&self, g: &Game) -> bool {
        self.early_conquest_opening
            && self.conquest_opening.as_ref().is_some_and(|opening| {
                opening.declared.is_none() && g.turn < Self::conquest_commit_due(g, opening)
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
    /// the reservation window, in any city but the capital, for anything but
    /// a reserved body, and once the force is complete.
    ///
    /// `threatened` is the caller's own defence sentinel — a barbarian
    /// alarm, the plan's threatened city, or a city hit in the last four
    /// turns — and it no longer suspends the reservation. The reserved bodies
    /// ARE what defends an Ancient capital. Suspending it handed the idle
    /// capital to the Settler arm: live King 2026-09-30T221624Z named
    /// Stockholm on turn 24, a barbarian Horse Archer stood beside Bogotá on
    /// turns 25–27, the capital started a Settler on turn 27, and the force
    /// never assembled before the window closed on turn 44.
    pub(super) fn conquest_reservation(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        spec: &UnitSpec,
        counts: &EmpireCounts,
        threatened: bool,
    ) -> f64 {
        let _ = threatened;
        if !self.conquest_reservation_open(g) {
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

    /// `early-conquest-opening`: what a Scout trained in `cid` is worth while
    /// the opening is still looking for its target. Zero with the gene off,
    /// once an opening stands or has closed, after the commit deadline,
    /// while the city is threatened, once the empire holds
    /// [`CONQUEST_SEARCH_SCOUTS`] Scouts, and whenever a target is already in
    /// reach — the search is for the opening, not for the map — and in any
    /// city that would take longer than [`CONQUEST_SEARCH_SCOUT_MAX_TURNS`].
    /// Any city may train it: the capital's opening is the scripted Settler
    /// book, which never asks this ranking, so a capital-only term would
    /// never be heard.
    /// `counts` is the census with queued bodies, so two cities cannot both
    /// start it.
    pub(super) fn conquest_search_scout_value(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        counts: &EmpireCounts,
        threatened: bool,
    ) -> f64 {
        if !self.early_conquest_opening
            || threatened
            || self.conquest_closed
            || self.conquest_opening.is_some()
            || g.turn >= g.standard_duration(CONQUEST_COMMIT_DEADLINE)
            || counts.scouts >= CONQUEST_SEARCH_SCOUTS
            || g.cities.get(&cid).is_none_or(|city| city.owner != pid)
            || self.conquest_target(g, pid).is_some()
        {
            return 0.0;
        }
        let scout = Item::Unit {
            unit: crate::name!("scout"),
        };
        let turns = g.host_production_turns(cid, &scout).unwrap_or_else(|| {
            g.item_cost_for(pid, &scout) / g.city_yields(cid).production.max(0.5)
        });
        if turns > CONQUEST_SEARCH_SCOUT_MAX_TURNS {
            return 0.0;
        }
        CONQUEST_SEARCH_SCOUT_VALUE
    }

    /// `early-conquest-opening`: where the objective board's Reserve gathers
    /// while an opening is still assembling — its rally — or `None` with the
    /// gene off, with no opening, once the war is open, or past the commit
    /// deadline. A Deter row still outranks it (the caller asks first).
    pub(super) fn conquest_staging_rally(&self, g: &Game) -> Option<Pos> {
        if !self.conquest_reservation_open(g) {
            return None;
        }
        self.conquest_opening.as_ref().map(|opening| opening.rally)
    }

    /// `early-conquest-opening`: whether the capital defers its Settler
    /// while the reservation is unfilled.
    ///
    /// This is what makes the reservation a reservation rather than a bid.
    /// It is deliberately as narrow as `threatened_recovery_holds_settlers`:
    /// the capital only, while a target stands, before the deadline, while
    /// bodies are actually missing — threatened or not, since a threatened
    /// capital is the last place to train a Settler — and never while the
    /// empire holds fewer than
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
        let _ = threatened;
        if !self.conquest_reservation_open(g) {
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

    /// Give the scripted opening the same capital reservation as the utility
    /// governor. Its Settler shortcut never calls `production_value`, so a
    /// missing strike force otherwise waits until the opening book finishes.
    /// Only an idle capital after the first expansion is eligible. Existing
    /// builds and the same local-defense sentinels keep their priority.
    pub(super) fn conquest_opening_production(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        if !self.conquest_reservation_open(g) {
            return false;
        }
        let Some(cid) = Self::conquest_capital(g, pid) else {
            return false;
        };
        let city = &g.cities[&cid];
        if !city.queue.is_empty() {
            return false;
        }
        let threatened = plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            || (self.base.barbarian_tactics_enabled()
                && self.base.barbarian_local_alarm_for_controller(g, pid, cid));
        let counts = self.counts(g, pid);
        if !self.conquest_defers_the_settler(
            g,
            pid,
            cid,
            &counts,
            g.player_city_ids(pid).len(),
            threatened,
        ) {
            return false;
        }
        let mut best: Option<(f64, Item)> = None;
        for item in g.producible_items(pid, cid) {
            let Item::Unit { unit } = &item else { continue };
            if self.conquest_reservation(g, pid, cid, &g.rules.units[unit], &counts, threatened)
                <= 0.0
            {
                continue;
            }
            // Reuse the ordinary scorer's affordability and unit-quality
            // vetoes. A reservation does not license an obsolete or insolvent
            // build that the strategic governor would refuse.
            let value = self.production_value(g, pid, cid, &item, plan, &counts);
            if value.is_finite()
                && value > -1_000.0
                && best.as_ref().is_none_or(|(old, _)| value > *old)
            {
                best = Some((value, item));
            }
        }
        let Some((_, item)) = best else { return false };
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: item.clone(),
            },
        )
        .is_err()
        {
            return false;
        }
        think!(self.journal(), Military, Decision,
            "{} reserves {} for the conquest opening", g.cities[&cid].name, Self::plain_item(&item);
            "the second city is founded and the strike force is incomplete; claim the idle capital before the scripted opening fills it");
        true
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
    /// A guard bound to a Settler or a Builder is not a strike body: the
    /// vision guard would otherwise score an escort's tiles, and the
    /// assembly share would count a unit that is walking the other way.
    fn conquest_force(&self, g: &Game, pid: usize, rally: Pos) -> BTreeSet<u32> {
        let guards = self.all_reserved_civilian_guards();
        let mut army: Vec<u32> = self
            .campaign_field_army(g, pid)
            .into_iter()
            .filter(|uid| !guards.contains(uid))
            .collect();
        army.sort_by_key(|uid| (g.wdist(g.units[uid].pos, rally), *uid));
        army.truncate(CONQUEST_RANGED + CONQUEST_MELEE);
        army.into_iter().collect()
    }

    /// Let the reserved force assemble before the opening can declare.
    /// The general plan may still be Expansion or prefer a different city;
    /// it must not supply this force's pre-war destination. The existing
    /// mover retains legality, recovery, escort and home-defense priority.
    pub(super) fn conquest_staging_objective(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
    ) -> Option<(usize, Pos, Pos)> {
        if !self.early_conquest_opening
            || plan.strategy == GrandStrategy::Recovery
            || plan.threatened_city.is_some()
            || g.players
                .iter()
                .any(|other| other.id != pid && !other.is_barbarian && g.is_at_war(pid, other.id))
        {
            return None;
        }
        let opening = self.conquest_opening.as_ref()?;
        if opening.declared.is_some()
            || !opening.force.contains(&uid)
            || (opening.assembled.is_none() && g.turn >= Self::conquest_commit_due(g, opening))
            || !self.campaign_target_legal(g, pid, opening.target)
            || self.all_reserved_civilian_guards().contains(&uid)
        {
            return None;
        }
        let unit = g.units.get(&uid)?;
        if unit.owner != pid || unit.hp as f64 <= self.base.w.withdraw_hp {
            return None;
        }
        let city = g.cities.get(&opening.city)?;
        (city.owner == opening.target).then_some((opening.target, city.pos, opening.rally))
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
        // The share of the RESERVED force, not of whatever bodies exist yet.
        // Live King 2026-10-01T010043Z named Pella on turn 16 with two bodies
        // trained; both stood at the rally, 2 of 2 read as assembled, the
        // patience window opened that turn and released the opening on turn 30
        // -- this game's one attempt -- before the reserved four shooters were
        // ever built.
        let reserved = (CONQUEST_RANGED + CONQUEST_MELEE).max(opening.force.len());
        up as f64 / reserved as f64
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
        if force.is_empty() {
            return false;
        }
        let strength = Self::campaign_strength_of(g, &force);
        let average_body = strength / force.len() as f64;
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
        g.units
            .values()
            .any(|other| other.owner == pid && other.id != uid && g.wdist(other.pos, tile) == 1)
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

    /// A declared opening keeps its surviving strike force when a new
    /// Settler looks for an escort. Existing civilian guards retain priority;
    /// this reservation applies only to new assignments while the war stands.
    pub(super) fn conquest_unit_committed(&self, g: &Game, pid: usize, uid: u32) -> bool {
        self.conquest_owns_the_campaign()
            && self.conquest_opening.as_ref().is_some_and(|opening| {
                opening.force.contains(&uid)
                    && g.is_at_war(pid, opening.target)
                    && g.units.get(&uid).is_some_and(|unit| unit.owner == pid)
                    && g.cities
                        .get(&opening.city)
                        .is_some_and(|city| city.owner == opening.target)
            })
    }

    /// Write the opening's city into the shared campaign plan, which is what
    /// `assess` reads through `campaign_target` and
    /// `campaign_objective_city`. This is the whole handoff: from here the
    /// shipped force groups, staging ring, siege train and pillage step are
    /// aimed at this city and nothing re-aims them.
    fn conquest_pin_the_campaign(&mut self, g: &Game) {
        let Some(opening) = self.conquest_opening.as_ref() else {
            return;
        };
        let force: Vec<u32> = opening.force.iter().copied().collect();
        let strength = Self::campaign_strength_of(g, &force);
        let bodies = force.len().max(CAMPAIGN_MIN_BODIES);
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

    /// The target declared on us while the force was still gathering. Live
    /// King 2026-09-30T225143Z named Amsterdam on turn 31; the Netherlands
    /// declared on turn 38 with three of our Archers three or four tiles from
    /// the city; the opening kept waiting for an assembly the fighting had
    /// overtaken and released on turn 51 as if no war had ever opened, with
    /// the campaign never pinned to its city. Take the roster once more, mark
    /// the war declared from this turn, and pin the campaign exactly as our
    /// own declaration would.
    fn conquest_adopt_war(&mut self, g: &Game, pid: usize) {
        self.conquest_refresh_force(g, pid);
        let kills = g.players[pid].counters.get("kills").copied().unwrap_or(0);
        let Some(opening) = self.conquest_opening.as_mut() else {
            return;
        };
        opening.declared = Some(g.turn);
        opening.kills_at_war = kills;
        let (target, city, bodies) = (opening.target, opening.city, opening.force.len());
        think!(self.journal(), Military, Strategy,
               "The conquest adopts the war {} opened", g.players[target].civ;
               "they declared before the strike force did; the campaign is pinned to {} \
                with the {} bod{} already raised",
               g.cities.get(&city).map(|city| city.name.clone())
                   .unwrap_or_else(|| String::from("the objective")),
               bodies,
               if bodies == 1 { "y" } else { "ies" });
        self.conquest_pin_the_campaign(g);
    }

    /// Count the bodies of ours that left the board since the last turn
    /// boundary, and refresh the roster. Only the strike force is counted:
    /// this rate is the campaign's, not the empire's. A body that is no
    /// longer in `g.units` is a loss whatever removed it — a kill, a capture,
    /// a disband — because the campaign has lost it either way.
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

    /// Drop the opening and journal why. An opening that got as far as
    /// assembling — or declaring — was this game's one attempt: it closes
    /// the door behind it so the next turn cannot name the same city again,
    /// re-open the reservation, and hold the Settler until the deadline.
    /// One released earlier leaves the door open for another target.
    fn conquest_release(&mut self, g: &Game, why: &str) {
        let Some(opening) = self.conquest_opening.take() else {
            return;
        };
        if opening.assembled.is_some() || opening.declared.is_some() {
            self.conquest_closed = true;
        }
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
            self.conquest_closed = false;
            return;
        }
        self.conquest_count_losses(g);
        if g.player_city_ids(pid).len() >= CONQUEST_FIRST_SETTLER_CITIES {
            if let Some(opening) = self.conquest_opening.as_mut() {
                if opening.preparing_since.is_none() {
                    opening.preparing_since = Some(g.turn);
                }
            }
        }
        if let Some(opening) = self.conquest_opening.as_mut() {
            let base = Self::conquest_commit_base(g, opening);
            if opening.grace_until.is_none()
                && opening.assembled.is_none()
                && g.turn >= base
                && Self::conquest_force_approaching(g, opening)
            {
                opening.grace_until = Some(
                    base.saturating_add(g.standard_duration(CONQUEST_APPROACHING_GRACE_TURNS)),
                );
            }
        }
        // A war the target opened on us before we declared is still this
        // opening's war: adopt it rather than keep waiting for an assembly
        // that the fighting has overtaken.
        let adopt = self.conquest_opening.as_ref().is_some_and(|opening| {
            opening.declared.is_none()
                && g.is_at_war(pid, opening.target)
                && g.players.get(opening.target).is_some_and(|player| player.alive)
                && g.cities
                    .get(&opening.city)
                    .is_some_and(|city| city.owner == opening.target)
        });
        if adopt {
            self.conquest_adopt_war(g, pid);
        }
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
                self.conquest_release(g, "the rival is no longer in the game");
                return;
            }
            if !still_theirs {
                self.conquest_after_a_capture(g, pid);
                return;
            }
            if let Some(declared) = opening.declared {
                // The war is open. It ends by any road — a peace accepted, a
                // truce imposed — and the opening ends with it; and a force
                // that is wholly gone has nothing left to pin the campaign
                // with, so it asks for terms and stands down.
                //
                // Not on the turn it was declared: the host applies the
                // declaration after the order is read back, so the next frame
                // of the SAME turn still exports peace. Live King
                // 2026-10-01T014323Z declared on Norway on turn 45 with 83% of
                // the force at Oslo's rally, read that frame's peace as "the
                // war has ended", and closed the game's one opening -- while
                // the host showed the war from turn 46.
                if g.turn > declared && !g.is_at_war(pid, opening.target) {
                    self.conquest_release(g, "the war has ended");
                    return;
                }
                if opening.force.is_empty() {
                    let target = opening.target;
                    self.conquest_sue_for_peace(g, pid, target);
                    self.conquest_release(g, "the whole strike force is gone");
                    return;
                }
            } else {
                if !self.campaign_target_legal(g, pid, opening.target) {
                    self.conquest_release(g, "the rival is no longer a legal target");
                    return;
                }
                if let Some(assembled) = opening.assembled {
                    if g.turn.saturating_sub(assembled)
                        >= g.standard_duration(CONQUEST_ABANDON_TURNS)
                    {
                        self.conquest_release(
                            g,
                            "the force has stood at the rally for the whole patience window \
                             without covering the city's bill",
                        );
                        return;
                    }
                }
                if g.turn >= Self::conquest_commit_due(g, &opening) && opening.assembled.is_none() {
                    self.conquest_release(
                        g,
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
            self.conquest_pin_the_campaign(g);
        }
    }

    /// Name a target and open. Nothing happens once the commit deadline has
    /// passed, or once an assembled opening has been released: an opening
    /// is an opening, and this game has had its attempt.
    fn conquest_open(&mut self, g: &mut Game, pid: usize) {
        if self.conquest_closed || g.turn >= g.standard_duration(CONQUEST_COMMIT_DEADLINE) {
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
        let known = Self::conquest_known_cities(g, pid, target).len();
        think!(self.journal(), Military, Strategy,
               "Opening a conquest against {}", g.players[target].civ;
               "{} is {} tiles from the capital and they hold {} known cit{}; \
                the capital reserves {} shooters and {} melee bodies and rallies at the ring",
               g.cities[&city].name,
               g.wdist(home, g.cities[&city].pos),
               known,
               if known == 1 { "y" } else { "ies" },
               CONQUEST_RANGED, CONQUEST_MELEE;
               rally);
        self.conquest_opening = Some(ConquestOpening {
            target,
            city,
            opened: g.turn,
            preparing_since: (g.player_city_ids(pid).len() >= CONQUEST_FIRST_SETTLER_CITIES)
                .then_some(g.turn),
            grace_until: None,
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
    ///
    /// The gate, in order: the war is not yet open; the force has assembled
    /// and still stands at the rally; the preview covers the city's bill;
    /// the rival is still a legal target (`campaign_target_legal` — never a
    /// friend or an ally); `one-war-at-a-time` is not holding the
    /// declaration for a war already burning; `war-needs-a-treasury` finds
    /// the treasury able to carry it. Then `raid_opening`: a casus belli if
    /// one is free, else the surprise war, never a `Denounce`.
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
        if self.one_war_holds_declaration(g, pid, opening.target) {
            think!(self.journal(), Military, Detail,
                   "Holding the conquest force at the rally";
                   "one war at a time, and a major war is already being fought");
            return false;
        }
        if !self.war_is_affordable(g, pid) {
            think!(self.journal(), Military, Detail,
                   "Holding the conquest force at the rally";
                   "the treasury cannot carry the war it would open");
            return false;
        }
        let Some(action) = self.raid_opening(g, pid, opening.target) else {
            return false;
        };
        if g.apply(pid, &action).is_err() {
            return false;
        }
        let kills = g.players[pid].counters.get("kills").copied().unwrap_or(0);
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
        self.conquest_pin_the_campaign(g);
        true
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
            self.conquest_release(g, "the objective city changed hands to a third party");
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
            self.conquest_release(g, "the campaign stopped paying and asked for terms");
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
            self.conquest_release(g, "no further known city of the rival remains");
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
        self.conquest_pin_the_campaign(g);
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

    /// Between captures, a war that has stopped paying asks for terms
    /// without waiting for the next city to fall. Only once a city has been
    /// taken: before the first capture the preview is what decided the war,
    /// and the rate — which reads at its kill count with no losses and at
    /// zero after the first loss with no kill — would close every siege on
    /// its first casualty.
    fn conquest_peace(&mut self, g: &mut Game, pid: usize, opening: &ConquestOpening) {
        if opening.taken == 0 || opening.kills_per_loss(g, pid) >= CONQUEST_KILLS_PER_LOSS_FLOOR {
            return;
        }
        self.conquest_sue_for_peace(g, pid, opening.target);
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod staging_tests;

#[cfg(test)]
mod escort_reservation_tests;
