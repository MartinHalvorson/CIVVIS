//! `edge-unit-takes-the-soft-city`: when the seat can field a melee unit
//! clearly stronger than a nearby rival city, it raises that unit early,
//! masses a few, takes the city, and stops when the edge is gone.
//!
//! ## Why (live Deity, 96 Rome Score games of 2026-11-12/13)
//!
//! A fight is decided by the strength gap before contact (damage is about
//! 30·e^(0.04·gap)). The nearest rival major city to any of ours, an
//! expansion city five or six tiles away at population 3-4, defends at 25 at
//! turn 40 (p25 22, p75 27), 27.5 at turn 50, 39 at turn 60 and 45 at turn
//! 70, and stands walled in 14% / 18% / 34% / 46% of games. A Roman Legion
//! (strength 40, Iron Working, 10 Iron) is +15 over that city at turn 40 (97%
//! of games at +5 or better) and +12.5 at turn 50 (75%); the window is half
//! shut at turn 60 and shut at turn 70. The seat knows Iron Working in 33% of
//! games at turn 40 and 64% at turn 50, holds 10 Iron in 1% and 9%, and owns
//! a Legion in 0% and 1%. Its best land unit is 20 at turn 40 and 25 at turn
//! 50. The shipped opening (`early_conquest.rs`) never opens on the live seat
//! (`early-conquest-stands-down`), and its 28 live Emperor declarations,
//! four ranged and two melee bodies with no strength edge, took no city.
//!
//! ## The rule
//!
//! 1. **Edge unit** ([`edge_unit`]): the strongest land melee unit (military,
//!    no ranged strength, no bombard) the seat can train now or after at most
//!    [`EDGE_TECHS_AHEAD`] more technologies, its civilization's unique unit
//!    standing in for the unit it replaces (Rome trains the Legion). It is
//!    chosen the first turn the gene reads the board and held for the game
//!    while it stays within reach, so a Writing seat does not trade a
//!    half-researched Legion for a Medieval Pikeman three technologies off.
//! 2. **Soft city** ([`AdvancedAi::edge_soft_city`]): a met rival major's
//!    city whose centre we have explored, within [`EDGE_REACH_TILES`] of one
//!    of our cities, a legal war target (`campaign_target_legal`: never a
//!    friend or ally), where the edge unit's strength less the city's defence
//!    (`Game::city_strength`) is at least [`EDGE_MIN`]. Behind standing walls
//!    it also needs a breaker the walls admit that we can train; without one
//!    it is not soft.
//! 3. **Window**: opens on a soft city up to turn [`EDGE_LAST_TURN`] on Online
//!    (scaled by speed as `city_state_harvest::harvest_floor` scales), with at
//!    least [`EDGE_MIN_CITIES`] cities, until this game's one attempt is spent.
//!    A city named by then stays the target while it stays soft (version 2:
//!    an unwalled one while the capture forecast with the edge units owned,
//!    posed beside it, reads it taken, or while the edge is
//!    [`EDGE_UNWALLED_FLOOR`] or better before [`EDGE_MUSTER_BODIES`] are
//!    owned; see `edge_keep_unwalled`), up to turn
//!    [`EDGE_MUSTER_LAST_TURN`]: in round 0 (16 paired worlds with the Legion's
//!    Iron waived) 13 worlds held 3-6 Legions only by turns 44-55, and a
//!    window shut at 55 closed on every one of them still walking. While it
//!    is open:
//!    - **Research** ([`AdvancedAi::edge_unit_research_goal`]): when the edge
//!      unit needs a strategic resource we neither hold nor earn, the
//!      technology that reveals it comes first (Bronze Working for Iron). The
//!      edge unit's own technology becomes the goal only once a source is
//!      visible inside our borders or within [`EDGE_SOURCE_TILES`] of one of
//!      our cities; with none, the window waits and says so, before the
//!      Science is spent. A unit that needs no resource skips this.
//!    - **Resource**: a Builder improves a source inside our borders first
//!      ([`AdvancedAi::edge_unit_builder_step`]), and a source one tile
//!      outside them is bought.
//!    - **Force** ([`AdvancedAi::claim_edge_unit_queue`]): edge units, up to
//!      [`EDGE_FORCE_MAX`] owned and queued, while the resource allows, in the
//!      capital and the two other cities nearest the target (version 2; one
//!      before the unit is trainable), ahead of a deferrable item, plus one
//!      breaker when the walls stand. A Gold upgrade of an owned unit into the
//!      edge unit goes ahead of the treasury floor, down to 0 Gold while
//!      income is not negative, and its bill is held back from other
//!      purchases while the window is open. A city's first Settler is never
//!      deferred.
//!    - **Muster**: at the start of each turn the edge units (and the
//!      breaker) walk with every move to a free stand
//!      [`EDGE_STAND_MIN_TILES`]-[`EDGE_MUSTER_TILES`] tiles from the city and
//!      hold it fortified, so no later step of the turn walks them home (round
//!      0: with the unit pass's staging step alone no muster formed in any
//!      world). The objective board's Reserve gathers there too, as it does
//!      at the shipped opening's rally. Version 2: an edge unit under
//!      [`EDGE_HEAL_HP`] heals first, in place or in a city of ours within
//!      [`EDGE_HEAL_CITY_TILES`].
//!    - **Declare**: once [`EDGE_MUSTER_BODIES`] edge units at
//!      [`EDGE_MUSTER_HP`] or more stand within [`EDGE_MUSTER_TILES`] of the
//!      city (and a breaker behind walls), no other major is at war with us,
//!      and the capture forecast (`assault_forecast_at_peace`, calibrated)
//!      reads the city taken within [`EDGE_FORECAST_TURNS`] turns at
//!      `DECLARE_P` or more, the seat declares through the shipped
//!      `preferred_war_opening`. On a major with no casus belli that opening
//!      denounces first and offers the Formal War five standard turns later,
//!      so the denouncement goes out as soon as [`EDGE_DENOUNCE_BODIES`] edge
//!      unit is owned (version 2: the first, so its five turns run during the
//!      muster). From the declaration the plan's target is the city
//!      ([`AdvancedAi::apply_edge_unit_to_strategy`]).
//!    - **After a capture**: on to the same owner's next city if it is within
//!      [`EDGE_CONTINUE_TILES`] of the force and the edge is still
//!      [`EDGE_CONTINUE_MIN`]; otherwise peace is asked. Peace is also asked
//!      after [`EDGE_WAR_PATIENCE`] war turns without a capture and when
//!      fewer than [`EDGE_FORCE_FLOOR`] edge units remain.
//! 4. **Journal**: every stage writes a `Military` (or `Research`) line whose
//!    detail begins `edge-unit-takes-the-soft-city: <stage>:`, and bumps an
//!    `edge_unit:<stage>` counter on the seat (on the board the seat plans on;
//!    a simulated seat plans on a decision view, so read the journal).
//!
//! Off, every entry point returns before it reads the board.

use super::city_state_harvest::harvest_floor;
use super::declaration_needs_a_capture::DECLARE_P;
use super::*;
use crate::think;
use std::collections::{BTreeSet, HashSet};

/// Strength the edge unit must hold over the city's defence to name it.
pub(crate) const EDGE_MIN: f64 = 8.0;
/// Strength over the next city's defence to go on after a capture.
pub(crate) const EDGE_CONTINUE_MIN: f64 = 5.0;
/// Tiles from the nearest of our cities to a soft city.
pub(crate) const EDGE_REACH_TILES: i32 = 10;
/// Technologies the edge unit may still be away.
pub(crate) const EDGE_TECHS_AHEAD: usize = 3;
/// The last Online turn the window names a soft city on.
pub(crate) const EDGE_LAST_TURN: u32 = 55;
/// The last Online turn a city named by [`EDGE_LAST_TURN`] is mustered on
/// and declared on, while it stays soft. Round 0 (16 paired worlds, the Iron
/// waived): 13 worlds held 3-6 Legions by turn 44-55, none had three at the
/// city when the window shut at turn 56.
pub(crate) const EDGE_MUSTER_LAST_TURN: u32 = 70;
/// Cities the seat holds before the window opens.
pub(crate) const EDGE_MIN_CITIES: usize = 2;
/// Edge units owned and queued that the window trains up to.
pub(crate) const EDGE_FORCE_MAX: usize = 6;
/// Edge units that must stand at the city before the declaration.
pub(crate) const EDGE_MUSTER_BODIES: usize = 3;
/// Health an edge unit at the city needs to count.
pub(crate) const EDGE_MUSTER_HP: i32 = 70;
/// Tiles from the city an edge unit counts at.
pub(crate) const EDGE_MUSTER_TILES: i32 = 4;
/// The nearest stand the force takes before the war (inside it is the
/// rival's ring of two, closed at peace).
pub(crate) const EDGE_STAND_MIN_TILES: i32 = 3;
/// Turns within which the forecast must read the city taken.
pub(crate) const EDGE_FORECAST_TURNS: usize = 4;
/// Tiles from one of our cities a resource source may lie and count.
pub(crate) const EDGE_SOURCE_TILES: i32 = 3;
/// Tiles from the force the next city may stand after a capture.
pub(crate) const EDGE_CONTINUE_TILES: i32 = 8;
/// `edge-unit-opens-from-the-start`: the last Online turn a city is named
/// on, and the last a muster may form by.
pub(crate) const EDGE_START_NAME_TURN: u32 = 45;
/// See [`EDGE_START_NAME_TURN`].
pub(crate) const EDGE_START_MUSTER_TURN: u32 = 60;
/// Declarations that never reached the board the window asks again.
pub(crate) const EDGE_REDECLARE_MAX: u32 = 3;
/// Health below which an edge unit beside the city does not strike it.
pub(crate) const EDGE_STRIKE_MIN_HP: i32 = 35;
/// War turns without a capture before peace is asked.
pub(crate) const EDGE_WAR_PATIENCE: u32 = 20;
/// Edge units below which the war asks for peace.
pub(crate) const EDGE_FORCE_FLOOR: usize = 2;
/// Version 2: the edge an unwalled named city may fall to and stay the
/// target while fewer than [`EDGE_MUSTER_BODIES`] edge units are owned (live
/// 205538Z: Mbamba went from +23 to +6 while the force was raised).
pub(crate) const EDGE_UNWALLED_FLOOR: f64 = -4.0;
/// Version 2: cities that train the edge unit at once once it is trainable:
/// the capital and the two nearest the target.
pub(crate) const EDGE_TRAINING_CITIES: usize = 3;
/// Version 2: edge units owned before the denouncement goes out, so its five
/// turns run during the muster.
pub(crate) const EDGE_DENOUNCE_BODIES: usize = 1;
/// Version 2: an edge unit under this health heals before it marches (live
/// 205538Z: the upgraded Warrior walked at 50 health from turn 36 to 48).
pub(crate) const EDGE_HEAL_HP: i32 = EDGE_MUSTER_HP;
/// Version 2: a wounded edge unit within this many tiles of a city of ours
/// heals in that city.
pub(crate) const EDGE_HEAL_CITY_TILES: i32 = 2;
/// Version 2, `edge-unit-opens-from-the-start`: Online turns the resource
/// check repeats after the revealing technology lands with no source in
/// reach (live 205538Z: none at Bronze Working, turn 19; Iron in stock from
/// about turn 23).
pub(crate) const EDGE_START_RECHECK_TURNS: u32 = 10;
/// Builders walk this far to a source.
const EDGE_BUILDER_STEPS: usize = 8;
/// Every line's detail starts with this.
const TAG: &str = "edge-unit-takes-the-soft-city";

/// The edge unit and what research still stands before it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EdgeUnit {
    pub(crate) unit: Name,
    pub(crate) strength: f64,
    /// The technologies still missing, prerequisites first, its own last.
    pub(crate) missing: Vec<Name>,
}

/// A city the edge unit outclasses.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SoftCity {
    pub(crate) rival: usize,
    pub(crate) city: u32,
    pub(crate) defence: f64,
    pub(crate) edge: f64,
    /// Tiles from the nearest of the positions the scan was asked from.
    pub(crate) tiles: i32,
    pub(crate) walls: bool,
    pub(crate) breaker: Option<Name>,
}

/// The soft city the window is aimed at. Live, the mirror's city ids are not
/// stable from one frame to the next, so the city is re-read from its tile.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EdgeTarget {
    pub(crate) player: usize,
    pub(crate) city: u32,
    pub(crate) pos: Pos,
    pub(crate) owner_civ: String,
}

/// What the window remembers between turns. `None` with the gene off.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct EdgeWindow {
    pub(crate) unit: Option<Name>,
    pub(crate) target: Option<EdgeTarget>,
    /// The turn the window first opened.
    pub(crate) opened: Option<u32>,
    /// The research goal this turn.
    pub(crate) goal: Option<Name>,
    pub(crate) declared: Option<u32>,
    pub(crate) last_capture: Option<u32>,
    pub(crate) taken: u32,
    /// The game's one attempt is over.
    pub(crate) spent: bool,
    /// Why the window is not open this turn; `None` while it is.
    pub(crate) waiting: Option<String>,
    /// The last hold at the muster, journalled once.
    pub(crate) held: Option<String>,
    /// The force last journalled, as (owned, queued).
    pub(crate) force_noted: (usize, usize),
    /// A resource source was seen out of reach at its revealing technology.
    pub(crate) no_source_noted: bool,
    /// `edge-unit-opens-from-the-start`: whether the opening runs from turn
    /// one (the edge unit is the civilization's own unique unit), decided on
    /// the first turn; `Some(false)` once it stands down.
    pub(crate) start: Option<bool>,
    /// The from-the-start opening found its resource in reach (or needs
    /// none): the Builder, the bodies and the held Gold follow.
    pub(crate) start_supplied: bool,
    /// The turn three edge units first stood at the city.
    pub(crate) mustered: Option<u32>,
    /// Declarations that never reached the board, asked again.
    pub(crate) redeclared: u32,
    /// Version 2, from the start: the turn the resource check first found no
    /// source in reach at its revealing technology; the check repeats for
    /// [`EDGE_START_RECHECK_TURNS`] from it.
    pub(crate) recheck_since: Option<u32>,
    /// Version 2: how the named city is kept below the naming edge
    /// (`"edge"` or `"forecast"`), journalled once for each.
    pub(crate) kept: Option<&'static str>,
    /// Version 2: edge units journalled as healing before the march.
    pub(crate) healing: BTreeSet<u32>,
}

impl EdgeWindow {
    /// Open, not yet at war, attempt not spent.
    fn mustering(&self) -> bool {
        !self.spent && self.opened.is_some() && self.waiting.is_none() && self.declared.is_none()
    }

    /// Open (mustering or at war) and not spent.
    fn live(&self) -> bool {
        !self.spent && self.opened.is_some() && self.waiting.is_none()
    }

    /// `edge-unit-opens-from-the-start`: the opening runs before any city is
    /// named (the caller checks the flag).
    fn preparing(&self) -> bool {
        self.start == Some(true) && !self.spent && self.declared.is_none()
    }
}

/// The technologies `pid` still needs before `tech`, prerequisites first and
/// `tech` last; empty when it is known.
pub(crate) fn missing_techs(g: &Game, pid: usize, tech: Name) -> Vec<Name> {
    fn visit(g: &Game, known: &BTreeSet<Name>, tech: Name, out: &mut Vec<Name>) {
        if known.contains(&tech) || out.contains(&tech) {
            return;
        }
        if let Some(spec) = g.rules.techs.get(&tech) {
            for required in &spec.requires {
                visit(g, known, *required, out);
            }
        }
        out.push(tech);
    }
    let mut out = Vec::new();
    visit(g, &g.players[pid].techs, tech, &mut out);
    out
}

/// `unit` as an edge unit for `pid`, or `None` when it is not one: a land
/// melee unit (military, no ranged strength, no bombard, not siege, not
/// recon) `pid` may train now or after at most [`EDGE_TECHS_AHEAD`] more
/// technologies. A unique unit stands in for the unit it replaces; a civic
/// it needs must be known; an obsolete unit is out.
pub(crate) fn edge_candidate(g: &Game, pid: usize, unit: Name) -> Option<EdgeUnit> {
    let spec = g.rules.units.get(&unit)?;
    let player = &g.players[pid];
    if !spec.buildable
        || spec.siege
        || spec.bombard_strength > 0.0
        || !AdvancedAi::conquest_melee_body(spec)
        || spec
            .unique_to
            .as_deref()
            .is_some_and(|civ| !g.owns_civ_unique(pid, civ))
        || g.player_unit_replacement(pid, unit) != unit
        || spec
            .civic
            .is_some_and(|civic| !player.civics.contains(&civic))
        || spec
            .obsolete_tech
            .is_some_and(|tech| player.techs.contains(&tech))
    {
        return None;
    }
    let missing = spec
        .tech
        .map(|tech| missing_techs(g, pid, tech))
        .unwrap_or_default();
    if missing.len() > EDGE_TECHS_AHEAD || (missing.is_empty() && g.unit_is_obsolete(pid, unit)) {
        return None;
    }
    Some(EdgeUnit {
        unit,
        strength: spec.strength,
        missing,
    })
}

/// The edge unit: the strongest [`edge_candidate`]. Ties go to fewer
/// technologies, then the cheaper, then the name.
///
/// The window asks this on the first turn the gene reads the board and
/// holds the answer for the game while it stays a candidate. Asked later,
/// a seat that knows Writing finds the Pikeman (45, Military Tactics) or the
/// Man-at-Arms three technologies away and would leave the Legion's
/// research half done for a Medieval unit that cannot arrive by turn 55.
pub(crate) fn edge_unit(g: &Game, pid: usize) -> Option<EdgeUnit> {
    let mut best: Option<(EdgeUnit, f64)> = None;
    for (name, spec) in g.rules.units.iter() {
        let Some(candidate) = edge_candidate(g, pid, *name) else {
            continue;
        };
        let missing = &candidate.missing;
        let better = best.as_ref().is_none_or(|(held, cost)| {
            if (spec.strength - held.strength).abs() > 1e-9 {
                return spec.strength > held.strength;
            }
            if missing.len() != held.missing.len() {
                return missing.len() < held.missing.len();
            }
            if (spec.cost - *cost).abs() > 1e-9 {
                return spec.cost < *cost;
            }
            name.as_str() < held.unit.as_str()
        });
        if better {
            best = Some((candidate, spec.cost));
        }
    }
    best.map(|(unit, _)| unit)
}

/// The strategic resource `unit` costs to train, and how much.
pub(crate) fn edge_resource(g: &Game, unit: Name) -> Option<(Name, f64)> {
    let spec = g.rules.units.get(&unit)?;
    let resource = spec.requires_resource?;
    (spec.resource_cost > 0.0).then_some((resource, spec.resource_cost))
}

/// Whether `pid` holds enough `resource` for one more unit or earns it.
fn edge_supplied(g: &Game, pid: usize, resource: Name, cost: f64) -> bool {
    g.strategic_stockpile(pid, resource) + 1e-9 >= cost
        || g.strategic_resource_rate(pid, resource.as_str()) > 0.0
}

/// The visible sources of `resource` `pid` could work: inside our borders
/// (`true`), or on unowned ground within [`EDGE_SOURCE_TILES`] of one of our
/// cities (`false`). Fog-honest: the resource must be revealed to us and the
/// tile explored.
pub(crate) fn edge_sources(g: &Game, pid: usize, resource: Name) -> Vec<(Pos, bool)> {
    if !g.resource_visible_to(pid, resource.as_str()) {
        return Vec::new();
    }
    let explored = &g.players[pid].explored;
    let mut seen: BTreeSet<Pos> = BTreeSet::new();
    let mut out = Vec::new();
    for cid in g.player_city_ids(pid) {
        let city = &g.cities[&cid];
        let tiles = g
            .wdisk(city.pos, EDGE_SOURCE_TILES)
            .into_iter()
            .chain(city.owned_tiles.iter().copied());
        for pos in tiles {
            if !seen.insert(pos) {
                continue;
            }
            let Some(tile) = g.map.get(pos) else {
                continue;
            };
            if tile.resource != Some(resource) || !explored.contains(&pos) {
                continue;
            }
            match tile
                .owner_city
                .and_then(|owner| g.cities.get(&owner))
                .map(|city| city.owner)
            {
                Some(owner) if owner == pid => out.push((pos, true)),
                None => out.push((pos, false)),
                Some(_) => {}
            }
        }
    }
    out
}

/// The breaker a walled `cid` admits that `pid` can train by technology now.
fn edge_breaker(g: &Game, pid: usize, cid: u32) -> Option<Name> {
    let player = &g.players[pid];
    ["battering_ram", "siege_tower"]
        .into_iter()
        .find_map(|kind| {
            let spec = g.rules.units.get(kind)?;
            let open = spec.buildable
                && spec.tech.is_none_or(|tech| player.techs.contains(&tech))
                && spec
                    .civic
                    .is_none_or(|civic| player.civics.contains(&civic))
                && !spec
                    .obsolete_tech
                    .is_some_and(|tech| player.techs.contains(&tech))
                && g.city_allows_siege_support(cid, kind);
            open.then(|| Name::new(kind))
        })
}

fn is_breaker(kind: &str) -> bool {
    matches!(kind, "battering_ram" | "siege_tower")
}

/// Whether one of our bodies may stand on `pos` before the war: dry and
/// passable for it, no city, and not behind a border closed to us.
fn edge_stand(g: &Game, pid: usize, uid: Option<u32>, pos: Pos) -> bool {
    let Some(tile) = g.map.get(pos) else {
        return false;
    };
    if g.rules.is_water(tile)
        || !g.rules.is_passable(tile)
        || g.city_at(pos).is_some()
        || uid.is_some_and(|uid| !g.unit_can_traverse(uid, pos))
    {
        return false;
    }
    tile.owner_city
        .and_then(|city| g.cities.get(&city))
        .map(|city| city.owner)
        .is_none_or(|owner| owner == pid || g.has_open_borders(pid, owner))
}

/// The living major whose civilization is `civ`.
fn player_by_civ(g: &Game, civ: &str) -> Option<usize> {
    g.players
        .iter()
        .find(|player| player.alive && !player.is_minor && player.civ == civ)
        .map(|player| player.id)
}

impl AdvancedAi {
    fn edge_count(g: &mut Game, pid: usize, key: &str) {
        *g.players[pid]
            .counters
            .entry(format!("edge_unit:{key}"))
            .or_insert(0) += 1;
    }

    /// Edge units of `unit` owned and queued in our cities.
    fn edge_force(g: &Game, pid: usize, unit: Name) -> (usize, usize) {
        let owned = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| g.units[uid].kind == unit)
            .count();
        let queued = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| {
                g.cities[&cid]
                    .queue
                    .iter()
                    .filter(|item| matches!(item, Item::Unit { unit: queued } if *queued == unit))
                    .count()
            })
            .sum();
        (owned, queued)
    }

    /// One city of `rival`, read as a soft city for a unit of `strength`
    /// from `from` (the nearest position counts) within `reach`. No edge
    /// floor here; walls need a breaker we can train.
    fn edge_read_city(
        &self,
        g: &Game,
        pid: usize,
        strength: f64,
        cid: u32,
        from: &[Pos],
        reach: i32,
    ) -> Option<SoftCity> {
        let city = g.cities.get(&cid)?;
        let rival = city.owner;
        let player = g.players.get(rival)?;
        if rival == pid
            || !player.alive
            || player.is_minor
            || player.is_barbarian
            || player.is_free_city
            || !g.has_met(pid, rival)
            || !g.players[pid].explored.contains(&city.pos)
            || (!g.is_at_war(pid, rival) && !self.campaign_target_legal(g, pid, rival))
        {
            return None;
        }
        let tiles = from.iter().map(|pos| g.wdist(*pos, city.pos)).min()?;
        if tiles > reach {
            return None;
        }
        let defence = g.city_strength(cid);
        let walls = city.wall_hp > 0;
        let breaker = if walls {
            Some(edge_breaker(g, pid, cid)?)
        } else {
            None
        };
        Some(SoftCity {
            rival,
            city: cid,
            defence,
            edge: strength - defence,
            tiles,
            walls,
            breaker,
        })
    }

    /// ⭐ THE SOFT CITY: the best rival major's city (of `owner` only, when
    /// given) that a unit of `strength` outclasses by `min_edge` or more,
    /// within `reach` of `from`. The largest edge first, then the nearest,
    /// then the lowest id.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn edge_soft_city(
        &self,
        g: &Game,
        pid: usize,
        strength: f64,
        min_edge: f64,
        from: &[Pos],
        reach: i32,
        owner: Option<usize>,
    ) -> Option<SoftCity> {
        let mut best: Option<SoftCity> = None;
        for city in g.cities.values() {
            if owner.is_some_and(|owner| owner != city.owner) {
                continue;
            }
            let Some(soft) = self.edge_read_city(g, pid, strength, city.id, from, reach) else {
                continue;
            };
            if soft.edge + 1e-9 < min_edge {
                continue;
            }
            let better = best.as_ref().is_none_or(|held| {
                if (soft.edge - held.edge).abs() > 1e-9 {
                    soft.edge > held.edge
                } else {
                    (soft.tiles, soft.city) < (held.tiles, held.city)
                }
            });
            if better {
                best = Some(soft);
            }
        }
        best
    }

    /// The capital and the other cities of ours nearest `target`, `count` in
    /// all: where the force is trained.
    fn edge_training_cities(g: &Game, pid: usize, target: Pos, count: usize) -> Vec<u32> {
        let capital = Self::conquest_capital(g, pid);
        let mut others: Vec<u32> = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| Some(*cid) != capital)
            .collect();
        others.sort_by_key(|cid| (g.wdist(g.cities[cid].pos, target), *cid));
        let mut cities: Vec<u32> = capital.into_iter().collect();
        let room = count.saturating_sub(cities.len());
        cities.extend(others.into_iter().take(room));
        cities
    }

    /// Version 2: whether the edge unit (or its breaker) may take the head
    /// of `cid`'s queue from `item`, keeping its progress. Never an item
    /// that finishes this turn, a military unit, Walls, a wonder, a project,
    /// or a District with work in it; never a city's first Settler: the
    /// capital's went to the second city, so only the capital's Settler is
    /// deferred, once the empire holds [`EDGE_MIN_CITIES`] cities, and a
    /// Settler anywhere else is read as that city's first. Live 205538Z:
    /// the claim took only idle queues, and the two cities it trained in
    /// were idle on four turns between turns 33 and 44.
    fn edge_may_displace(g: &Game, pid: usize, cid: u32, item: &Item) -> bool {
        let remaining =
            g.item_cost_for_city(pid, cid, item) - g.item_invested_production(cid, item);
        if remaining <= g.city_yields(cid).production {
            return false;
        }
        match item {
            Item::Unit { unit } if *unit == "settler" => {
                g.cities[&cid].is_capital && g.player_city_ids(pid).len() >= EDGE_MIN_CITIES
            }
            Item::Unit { unit } => matches!(unit.as_str(), "builder" | "scout" | "trader"),
            Item::Building { building } => g.rules.buildings.get(building).is_some_and(|spec| {
                !spec.wonder
                    && !matches!(
                        building.as_str(),
                        "walls" | "medieval_walls" | "renaissance_walls"
                    )
            }),
            Item::District { .. } => g.item_invested_production(cid, item) <= 0.0,
            _ => false,
        }
    }

    /// Version 2: the capture forecast the declaration reads
    /// (`assault_forecast_at_peace`, `calibrated_within`
    /// [`EDGE_FORECAST_TURNS`]) for `units` at their current health, posed
    /// beside the city: on a copy of the board each is placed on a free
    /// land tile next to it, then two and three tiles out. Returns the
    /// calibrated and the raw chance. `None` when no forecast reads.
    fn edge_posed_forecast(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        units: &[u32],
    ) -> Option<(f32, f32)> {
        let center = g.cities.get(&cid)?.pos;
        let mut board = g.clone();
        let free =
            |board: &Game, pos: Pos| {
                board.map.get(pos).is_some_and(|tile| {
                    !board.rules.is_water(tile) && board.rules.is_passable(tile)
                }) && board.city_at(pos).is_none()
                    && board.unit_ids_at(pos).iter().all(|uid| units.contains(uid))
            };
        let mut spots: Vec<Pos> = Vec::new();
        for ring in 1..=EDGE_STAND_MIN_TILES {
            let mut tiles = board.wring(center, ring);
            tiles.sort_unstable();
            spots.extend(tiles.into_iter().filter(|pos| free(&board, *pos)));
        }
        for (uid, spot) in units.iter().zip(spots) {
            if board.units.contains_key(uid) {
                board.relocate(*uid, spot);
            }
        }
        let forecast = self.assault_forecast_at_peace(&board, pid, cid, units)?;
        let city = &board.cities[&cid];
        let walls = board.city_max_wall_hp(city).max(city.wall_hp);
        Some((
            forecast.calibrated_within(EDGE_FORECAST_TURNS, walls),
            forecast.within(EDGE_FORECAST_TURNS),
        ))
    }

    /// Version 2: whether an unwalled named city below [`EDGE_MIN`] stays
    /// the target. With [`EDGE_MUSTER_BODIES`] or more edge units owned, while
    /// the forecast with them beside it ([`Self::edge_posed_forecast`]) reads
    /// it taken at `DECLARE_P` or more; with fewer, while the edge is
    /// [`EDGE_UNWALLED_FLOOR`] or better. `Ok` names how it is kept and the
    /// mode; `Err` why it is not.
    fn edge_keep_unwalled(
        &self,
        g: &Game,
        pid: usize,
        soft: &SoftCity,
        unit: Name,
    ) -> Result<(&'static str, String), String> {
        let owned: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| g.units[uid].kind == unit)
            .collect();
        let name = &g.cities[&soft.city].name;
        let unit_name = plain(unit.as_str());
        if owned.len() < EDGE_MUSTER_BODIES {
            let line = format!(
                "{name} unwalled at defence {:.0}, edge {:+.0}, with {} of {} {unit_name} owned (the floor is {:+.0} until {} are owned)",
                soft.defence,
                soft.edge,
                owned.len(),
                EDGE_MUSTER_BODIES,
                EDGE_UNWALLED_FLOOR,
                EDGE_MUSTER_BODIES
            );
            return if soft.edge + 1e-9 >= EDGE_UNWALLED_FLOOR {
                Ok(("edge", line))
            } else {
                Err(line)
            };
        }
        let Some((chance, raw)) = self.edge_posed_forecast(g, pid, soft.city, &owned) else {
            return Err(format!(
                "{name} unwalled at defence {:.0}, edge {:+.0}: no forecast reads",
                soft.defence, soft.edge
            ));
        };
        let line = format!(
            "{name} unwalled at defence {:.0}, edge {:+.0}: the forecast with the {} {unit_name} owned beside it reads {:.0}% within {} turns (raw {:.0}%)",
            soft.defence,
            soft.edge,
            owned.len(),
            chance * 100.0,
            EDGE_FORECAST_TURNS,
            raw * 100.0
        );
        if chance >= DECLARE_P {
            Ok(("forecast", line))
        } else {
            Err(format!("{line}, short of {:.0}%", DECLARE_P * 100.0))
        }
    }

    /// Journal why the window is not open, once for each new reason.
    fn edge_wait(&mut self, window: &mut EdgeWindow, why: String) {
        if window.waiting.as_deref() != Some(why.as_str()) {
            think!(self.journal(), Military, Decision,
                   "Edge unit window waits";
                   "{TAG}: waiting: {why}");
        }
        if window.declared.is_none() && window.target.is_some() {
            // The staged force and the Reserve stop pointing at the city.
            self.force_groups_dirty = true;
        }
        window.waiting = Some(why);
        // The from-the-start opening keeps its research goal while it waits.
        if !(self.edge_unit_opens_from_the_start && window.preparing()) {
            window.goal = None;
        }
        window.target = None;
    }

    /// Close the window for this game.
    fn edge_close(&mut self, g: &mut Game, pid: usize, window: &mut EdgeWindow, why: &str) {
        window.spent = true;
        window.goal = None;
        Self::edge_count(g, pid, "closed");
        think!(self.journal(), Military, Strategy,
               "Edge unit window closes";
               "{TAG}: closed: {why}");
        if window.declared.is_some() {
            self.plan = None;
        }
        self.force_groups_dirty = true;
    }

    /// ⭐ Start of turn: open, keep or close the window; its research goal,
    /// resource purchase, upgrades, muster and declaration; at war, the
    /// capture, the continuation and the peace. Exact no-op with the gene
    /// off.
    pub(crate) fn maintain_edge_unit_window(&mut self, g: &mut Game, pid: usize) {
        if !self.edge_unit_takes_the_soft_city
            || g.players[pid].is_minor
            || g.players[pid].is_barbarian
        {
            return;
        }
        let mut window = self.edge_unit_window.take().unwrap_or_default();
        if !window.spent {
            if window.declared.is_some() {
                self.edge_war(g, pid, &mut window);
            } else {
                self.edge_peace(g, pid, &mut window);
            }
        }
        self.edge_unit_window = Some(window);
        // Version 2: the claim chain (`run_idle_queue_claims`) runs only once
        // the opening book is done, and live 205538Z's book held the capital
        // to turn 37. While it runs, the window claims idle queues here, at
        // the start of the turn; the book fills only an idle queue.
        if self.base.book_pos < 4 {
            self.edge_claim_queues(g, pid, false);
        }
    }

    /// The window before the war.
    fn edge_peace(&mut self, g: &mut Game, pid: usize, window: &mut EdgeWindow) {
        // The edge unit is chosen once and held while it stays a candidate.
        let held = window.unit.and_then(|unit| edge_candidate(g, pid, unit));
        let Some(edge) = held.or_else(|| edge_unit(g, pid)) else {
            self.edge_wait(window, "no land melee unit within reach".to_string());
            return;
        };
        if window.unit != Some(edge.unit) {
            think!(self.journal(), Military, Decision,
                   "Edge unit: {}", plain(edge.unit.as_str());
                   "{TAG}: unit: {} ({:.0}), {} technolog{} away",
                   plain(edge.unit.as_str()), edge.strength, edge.missing.len(),
                   if edge.missing.len() == 1 { "y" } else { "ies" });
            window.unit = Some(edge.unit);
            window.target = None;
        }
        // `edge-unit-opens-from-the-start`: decided on the first turn, for a
        // civilization whose edge unit is its own unique unit.
        if window.start.is_none() {
            let unique = g
                .rules
                .units
                .get(&edge.unit)
                .is_some_and(|spec| spec.unique_to.is_some());
            let on = self.edge_unit_opens_from_the_start && unique;
            window.start = Some(on);
            if on {
                Self::edge_count(g, pid, "start");
                think!(self.journal(), Military, Strategy,
                       "Edge unit opening from the start: {}", plain(edge.unit.as_str());
                       "{TAG}: start: research goes to {} from turn {} ({} technolog{} away)",
                       plain(edge.unit.as_str()), g.turn, edge.missing.len(),
                       if edge.missing.len() == 1 { "y" } else { "ies" });
            }
        }
        let start = self.edge_unit_opens_from_the_start && window.start == Some(true);
        let (name_by, muster_by) = if start {
            (EDGE_START_NAME_TURN, EDGE_START_MUSTER_TURN)
        } else {
            (EDGE_LAST_TURN, EDGE_MUSTER_LAST_TURN)
        };
        let naming_last = harvest_floor(g, name_by);
        let muster_last = harvest_floor(g, muster_by);
        let naming = g.turn <= naming_last;
        let named = window.opened.is_some() && window.target.is_some();
        if g.turn > muster_last || (!naming && !named) {
            if window.opened.is_some() || start {
                let why = match (g.turn > muster_last, start) {
                    (true, true) => format!(
                        "the force is not at the city by turn {muster_last}; the opening stands down and the {} stay as defence",
                        plain(edge.unit.as_str())
                    ),
                    (true, false) => {
                        format!("turn {} is past the muster's last turn, {muster_last}", g.turn)
                    }
                    (false, true) => format!(
                        "no soft city within {EDGE_REACH_TILES} tiles is known by turn {naming_last}; the opening stands down"
                    ),
                    (false, false) => format!(
                        "turn {} is past the window's last turn to name a city, {naming_last}, and none is named",
                        g.turn
                    ),
                };
                self.edge_close(g, pid, window, &why);
            } else {
                window.spent = true;
            }
            return;
        }
        // From the start, the path to the unit is researched before any city
        // is named; the resource decides at its revealing technology.
        let start_goal = if start {
            self.edge_start_goal(g, pid, window, &edge)
        } else {
            None
        };
        let start = self.edge_unit_opens_from_the_start && window.start == Some(true);
        let ours: Vec<Pos> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .collect();
        if ours.len() < EDGE_MIN_CITIES {
            self.edge_wait(window, format!("fewer than {EDGE_MIN_CITIES} cities"));
            if start {
                self.edge_prepare(g, pid, window, &edge, start_goal);
            }
            return;
        }
        // Keep the named city while it stays soft; otherwise the best one.
        // Version 2: behind walls the named city keeps the naming rule (a
        // breaker its walls admit, and the edge); unwalled it stays the
        // target below the edge while the capture forecast holds (live
        // 205538Z: Mbamba, never walled, was dropped at edge +6 two turns
        // after the third Legion).
        let named_city = window
            .target
            .as_ref()
            .filter(|_| window.unit == Some(edge.unit))
            .and_then(|target| g.city_at(target.pos));
        let mut lapsed: Option<String> = None;
        let kept = match named_city {
            None => None,
            Some(cid) => {
                match self.edge_read_city(g, pid, edge.strength, cid, &ours, EDGE_REACH_TILES) {
                    Some(soft) if soft.edge + 1e-9 >= EDGE_MIN => {
                        window.kept = None;
                        Some(soft)
                    }
                    Some(soft) if !soft.walls => {
                        match self.edge_keep_unwalled(g, pid, &soft, edge.unit) {
                            Ok((mode, why)) => {
                                if window.kept != Some(mode) {
                                    window.kept = Some(mode);
                                    Self::edge_count(g, pid, "kept");
                                    think!(self.journal(), Military, Decision,
                                           "Edge unit keeps its target below the edge";
                                           "{TAG}: kept: {why}; it stays the target");
                                }
                                Some(soft)
                            }
                            Err(why) => {
                                lapsed = Some(why);
                                None
                            }
                        }
                    }
                    Some(soft) => {
                        lapsed = Some(format!(
                            "{} walled at defence {:.0}, edge {:+.0}, under {:+.0}",
                            g.cities[&cid].name, soft.defence, soft.edge, EDGE_MIN
                        ));
                        None
                    }
                    None => {
                        let city = &g.cities[&cid];
                        lapsed = Some(if city.wall_hp > 0 {
                            format!(
                                "{} walled, and no breaker its walls admit can be trained",
                                city.name
                            )
                        } else {
                            format!("{} is no longer a legal target in reach", city.name)
                        });
                        None
                    }
                }
            }
        };
        // Past the naming turn only the city already named is mustered on.
        let Some(soft) = kept.or_else(|| {
            naming
                .then(|| {
                    self.edge_soft_city(
                        g,
                        pid,
                        edge.strength,
                        EDGE_MIN,
                        &ours,
                        EDGE_REACH_TILES,
                        None,
                    )
                })
                .flatten()
        }) else {
            if !naming {
                let why = match lapsed {
                    Some(why) => {
                        format!(
                            "past the naming turn the named city is no longer the target: {why}"
                        )
                    }
                    None => "past the naming turn the named city is gone".to_string(),
                };
                self.edge_close(g, pid, window, &why);
                return;
            }
            self.edge_wait(window, "no soft city in reach".to_string());
            if start {
                self.edge_prepare(g, pid, window, &edge, start_goal);
            }
            return;
        };
        let own_tech = edge.missing.last().copied();
        let resource = edge_resource(g, edge.unit);
        let unsupplied = resource.filter(|(res, cost)| !edge_supplied(g, pid, *res, *cost));
        let goal = match unsupplied {
            Some((res, _)) => {
                let reveal = g.rules.resources.get(&res).and_then(|spec| spec.tech);
                match reveal.filter(|tech| !g.players[pid].techs.contains(tech)) {
                    // Pull the revealing technology forward; decide there.
                    Some(reveal) => Some(reveal),
                    None => {
                        if edge_sources(g, pid, res).is_empty() {
                            if !window.no_source_noted {
                                window.no_source_noted = true;
                                Self::edge_count(g, pid, "no_source");
                            }
                            self.edge_wait(
                                window,
                                format!(
                                    "no {} source visible inside our borders or within {} tiles of a city of ours",
                                    plain(res.as_str()),
                                    EDGE_SOURCE_TILES
                                ),
                            );
                            return;
                        }
                        own_tech
                    }
                }
            }
            None => own_tech,
        };
        let target = EdgeTarget {
            player: soft.rival,
            city: soft.city,
            pos: g.cities[&soft.city].pos,
            owner_civ: g.players[soft.rival].civ.clone(),
        };
        let renamed = window.target.as_ref().map(|held| held.pos) != Some(target.pos)
            || window.unit != Some(edge.unit);
        window.waiting = None;
        if window.opened.is_none() {
            window.opened = Some(g.turn);
            Self::edge_count(g, pid, "opened");
        }
        if renamed {
            self.force_groups_dirty = true;
            window.kept = None;
            think!(self.journal(), Military, Strategy,
                   "Edge unit window opens on {}", g.cities[&soft.city].name;
                   "{TAG}: opened: {} ({:.0}) against {} of {} at defence {:.0}, edge +{:.0}, {} tiles from our nearest city, walls {}{}; {} technolog{} to go",
                   plain(edge.unit.as_str()), edge.strength,
                   g.cities[&soft.city].name, target.owner_civ, soft.defence, soft.edge, soft.tiles,
                   if soft.walls { "standing" } else { "none" },
                   soft.breaker.map(|b| format!(" (breaker: {})", plain(b.as_str()))).unwrap_or_default(),
                   edge.missing.len(), if edge.missing.len() == 1 { "y" } else { "ies" };
                   target.pos);
        }
        window.unit = Some(edge.unit);
        window.target = Some(target);
        self.edge_set_goal(g, pid, window, goal, edge.unit);
        if let Some((res, _)) = unsupplied {
            self.edge_buy_source(g, pid, res);
        }
        if edge.missing.is_empty() {
            self.edge_upgrades(g, pid, edge.unit);
        }
        let force = Self::edge_force(g, pid, edge.unit);
        if force != window.force_noted {
            window.force_noted = force;
            think!(self.journal(), Military, Decision,
                   "Edge unit force: {} owned, {} queued", force.0, force.1;
                   "{TAG}: force: {} {} owned and {} queued of {}",
                   force.0, plain(edge.unit.as_str()), force.1, EDGE_FORCE_MAX);
        }
        if edge.missing.is_empty() {
            self.edge_muster(g, pid, window, &soft, edge.unit);
            if window.declared.is_none() {
                let city_pos = g.cities[&soft.city].pos;
                self.edge_walk_the_force(g, pid, window, edge.unit, city_pos);
            }
        }
    }

    /// Set the research goal, journalling a new one.
    fn edge_set_goal(
        &mut self,
        g: &mut Game,
        pid: usize,
        window: &mut EdgeWindow,
        goal: Option<Name>,
        unit: Name,
    ) {
        if goal != window.goal {
            if let Some(goal) = goal {
                Self::edge_count(g, pid, "goal");
                think!(self.journal(), Research, Decision,
                       "Edge unit research goal: {}", plain(goal.as_str());
                       "{TAG}: research: {} toward {}", plain(goal.as_str()), plain(unit.as_str()));
            }
        }
        window.goal = goal;
    }

    /// `edge-unit-opens-from-the-start`: the research goal before any city is
    /// named. The technology that reveals the unit's resource first (Mining
    /// then Bronze Working for Iron); there, a source in reach makes the
    /// unit's own technology the goal, and none stands the opening down
    /// (`window.start` false) with research back to normal.
    fn edge_start_goal(
        &mut self,
        g: &mut Game,
        pid: usize,
        window: &mut EdgeWindow,
        edge: &EdgeUnit,
    ) -> Option<Name> {
        let own = edge.missing.last().copied();
        let unsupplied =
            edge_resource(g, edge.unit).filter(|(res, cost)| !edge_supplied(g, pid, *res, *cost));
        let Some((res, _)) = unsupplied else {
            // Version 2: a supply that appears during the recheck (a stock or
            // an income) commits the opening too.
            if let Some(since) = window.recheck_since.filter(|_| !window.start_supplied) {
                if let Some((res, _)) = edge_resource(g, edge.unit) {
                    Self::edge_count(g, pid, "start_recheck_found");
                    think!(self.journal(), Military, Decision,
                           "Edge unit opening from the start: {} supplied", plain(res.as_str());
                           "{TAG}: start: {} is supplied {} turn(s) into the recheck; {} is the goal, up to {} bodies train to upgrade and Gold is held for the upgrades",
                           plain(res.as_str()), g.turn.saturating_sub(since),
                           plain(own.map_or("the unit", |tech| tech.as_str())),
                           EDGE_MUSTER_BODIES);
                }
            }
            window.start_supplied = true;
            return own;
        };
        let reveal = g.rules.resources.get(&res).and_then(|spec| spec.tech);
        if let Some(reveal) = reveal.filter(|tech| !g.players[pid].techs.contains(tech)) {
            return Some(reveal);
        }
        let reveal_name = plain(reveal.map_or("its technology", |tech| tech.as_str()));
        if edge_sources(g, pid, res).is_empty() {
            // Version 2: the check repeats every turn for
            // EDGE_START_RECHECK_TURNS before the opening stands down (live
            // 205538Z: none in reach at Bronze Working on turn 19, Iron in
            // stock from about turn 23). No Science is spent meanwhile.
            window.start_supplied = false;
            let first = window.recheck_since.is_none();
            let since = *window.recheck_since.get_or_insert(g.turn);
            let until = since + harvest_floor(g, EDGE_START_RECHECK_TURNS);
            if g.turn < until {
                if first {
                    Self::edge_count(g, pid, "start_recheck");
                    think!(self.journal(), Military, Decision,
                           "Edge unit opening from the start checks again";
                           "{TAG}: start: recheck: no {} source in reach at {}; checking every turn until turn {} before the opening stands down",
                           plain(res.as_str()), reveal_name, until);
                }
                return None;
            }
            window.start = Some(false);
            Self::edge_count(g, pid, "start_closed");
            think!(self.journal(), Military, Strategy,
                   "Edge unit opening from the start stands down";
                   "{TAG}: start: no {} source in reach at {} or in the {} turns after; research returns to normal",
                   plain(res.as_str()), reveal_name, until - since);
            return None;
        }
        if !window.start_supplied {
            let found = window
                .recheck_since
                .map(|since| {
                    format!(
                        " ({} turn(s) into the recheck)",
                        g.turn.saturating_sub(since)
                    )
                })
                .unwrap_or_default();
            if window.recheck_since.is_some() {
                Self::edge_count(g, pid, "start_recheck_found");
            }
            think!(self.journal(), Military, Decision,
                   "Edge unit opening from the start: {} in reach", plain(res.as_str());
                   "{TAG}: start: a {} source is in reach{found}; {} is the goal, a Builder improves the source first, up to {} bodies train to upgrade and Gold is held for the upgrades",
                   plain(res.as_str()),
                   plain(own.map_or("the unit", |tech| tech.as_str())),
                   EDGE_MUSTER_BODIES);
        }
        window.start_supplied = true;
        own
    }

    /// `edge-unit-opens-from-the-start`, before a city is named: the goal,
    /// the source bought, the upgrades. (The Builder and the bodies come from
    /// their own seams.)
    fn edge_prepare(
        &mut self,
        g: &mut Game,
        pid: usize,
        window: &mut EdgeWindow,
        edge: &EdgeUnit,
        goal: Option<Name>,
    ) {
        if !window.preparing() {
            return;
        }
        self.edge_set_goal(g, pid, window, goal, edge.unit);
        if window.start_supplied {
            if let Some((res, _)) = edge_resource(g, edge.unit)
                .filter(|(res, cost)| !edge_supplied(g, pid, *res, *cost))
            {
                self.edge_buy_source(g, pid, res);
            }
        }
        if edge.missing.is_empty() {
            self.edge_upgrades(g, pid, edge.unit);
        }
    }

    /// The Gold held back from every other purchase (`purchase_upkeep_floor`)
    /// and from `one-upgrade-lifts-every-city`'s other upgrades: the upgrade
    /// bill of up to [`EDGE_MUSTER_BODIES`] owned units that upgrade into the
    /// edge unit, no more than the force still has room for. Held while the
    /// window is open (version 2; live 205538Z spent 75 of 76 Gold on one
    /// upgrade with nothing held for the next), and, with
    /// `edge-unit-opens-from-the-start`, while the opening is live and its
    /// resource is in reach. Zero with the gene off.
    pub(super) fn edge_upgrade_gold_held(&self, g: &Game, pid: usize) -> f64 {
        if !self.edge_unit_takes_the_soft_city {
            return 0.0;
        }
        let from_the_start = self.edge_unit_opens_from_the_start;
        let Some(window) = self
            .edge_unit_window
            .as_ref()
            .filter(|w| w.live() || (from_the_start && w.preparing() && w.start_supplied))
        else {
            return 0.0;
        };
        let Some(unit) = window.unit else {
            return 0.0;
        };
        let (owned, queued) = Self::edge_force(g, pid, unit);
        let room = EDGE_FORCE_MAX
            .saturating_sub(owned + queued)
            .min(EDGE_MUSTER_BODIES);
        if room == 0 {
            return 0.0;
        }
        let mut bills: Vec<f64> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter_map(|uid| {
                let kind = g.units[&uid].kind;
                let to = g.rules.units.get(&kind)?.upgrade_to?;
                (g.player_unit_replacement(pid, to) == unit)
                    .then(|| g.unit_upgrade_price_in_formation(pid, kind, unit, 0))
                    .flatten()
                    .map(|(gold, _)| gold)
            })
            .collect();
        bills.sort_by(f64::total_cmp);
        bills.into_iter().take(room).sum()
    }

    /// Version 2: the edge unit `pid`'s window holds Gold for, while it
    /// holds any ([`Self::edge_upgrade_gold_held`]). `one-upgrade-lifts-every-city`
    /// reads it: an upgrade into this unit is never held back by the hold.
    pub(super) fn edge_unit_held_for(&self, g: &Game, pid: usize) -> Option<Name> {
        if self.edge_upgrade_gold_held(g, pid) <= 0.0 {
            return None;
        }
        self.edge_unit_window.as_ref().and_then(|w| w.unit)
    }

    /// The muster's reach: [`EDGE_MUSTER_TILES`], or two more when no free
    /// stand is left 3-4 tiles from `city` (the ring the walk falls back
    /// to).
    fn edge_muster_reach(g: &Game, pid: usize, city: Pos) -> i32 {
        let open = (EDGE_STAND_MIN_TILES..=EDGE_MUSTER_TILES).any(|tiles| {
            g.wring(city, tiles)
                .into_iter()
                .any(|pos| edge_stand(g, pid, None, pos))
        });
        if open {
            EDGE_MUSTER_TILES
        } else {
            EDGE_MUSTER_TILES + 2
        }
    }

    /// The force walks to its stand at the start of the turn, with every
    /// move it has, and holds it fortified (a fortified unit has no moves
    /// left, so no later step of the turn walks it home). Round 0: with the
    /// staging step alone, 13 worlds held 3-6 Legions by turn 44-55 and no
    /// muster formed in any. Not while a city of ours is threatened or a
    /// major is at war with us. Version 2: an edge unit under
    /// [`EDGE_HEAL_HP`] heals first ([`Self::edge_heal_step`]).
    fn edge_walk_the_force(
        &mut self,
        g: &mut Game,
        pid: usize,
        window: &mut EdgeWindow,
        unit: Name,
        city: Pos,
    ) {
        let at_war = g.players.iter().any(|player| {
            player.id != pid
                && player.alive
                && !player.is_minor
                && !player.is_barbarian
                && g.is_at_war(pid, player.id)
        });
        if at_war || self.threatened_city(g, pid).is_some() {
            return;
        }
        let guards = self.all_reserved_civilian_guards();
        let withdraw = self.base.w.withdraw_hp;
        // Heal before the march: a wounded edge unit of the force holds (or
        // steps into a city of ours two tiles off) and does not walk. One at
        // or below the withdraw line is the unit pass's recovery, as before.
        window.healing.retain(|uid| {
            g.units.get(uid).is_some_and(|body| {
                body.owner == pid && body.kind == unit && body.hp < EDGE_HEAL_HP
            })
        });
        let wounded: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let body = &g.units[uid];
                body.kind == unit
                    && body.hp < EDGE_HEAL_HP
                    && body.hp as f64 > withdraw
                    && !guards.contains(uid)
                    && !g.is_embarked(body)
            })
            .collect();
        for uid in wounded.iter().copied() {
            let (hp, from) = (g.units[&uid].hp, g.units[&uid].pos);
            let Some(haven) = Self::edge_heal_step(g, pid, uid) else {
                continue;
            };
            if window.healing.insert(uid) {
                Self::edge_count(g, pid, "heal");
                let place = match haven {
                    Some(cid) => format!("in {}", g.cities[&cid].name),
                    None => "in place".to_string(),
                };
                think!(self.journal(), Military, Decision,
                       "Edge unit heals before the march";
                       "{TAG}: heal: {} {uid} at {hp} health heals {place} before it marches ({} tiles from the target); it walks at {}",
                       plain(unit.as_str()), g.wdist(from, city), EDGE_HEAL_HP;
                       from);
            }
        }
        let force: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let body = &g.units[uid];
                (body.kind == unit || is_breaker(body.kind.as_str()))
                    && !wounded.contains(uid)
                    && !guards.contains(uid)
                    && !g.is_embarked(body)
                    && body.hp as f64 > withdraw
            })
            .collect();
        for uid in force {
            // One step a call; a body has at most a few moves.
            for _ in 0..6 {
                if Self::edge_stage_unit(g, pid, uid, city) != Some(true) {
                    break;
                }
            }
        }
    }

    /// One step of a force body toward a free stand
    /// [`EDGE_STAND_MIN_TILES`]-[`EDGE_MUSTER_TILES`] tiles from `city`
    /// (outside a border closed to us; 3-6 tiles when none is free), or, on
    /// the stand, fortify. `Some(true)` when it stepped, `Some(false)` when it
    /// holds, `None` when it has no move or no stand is reachable.
    fn edge_stage_unit(g: &mut Game, pid: usize, uid: u32, city: Pos) -> Option<bool> {
        let body = g.units.get(&uid)?;
        if body.moves_left <= 0.0 {
            return None;
        }
        let here = body.pos;
        let stands = |g: &Game, far: i32| -> HashSet<Pos> {
            g.wdisk(city, far)
                .into_iter()
                .filter(|pos| {
                    g.wdist(*pos, city) >= EDGE_STAND_MIN_TILES
                        && edge_stand(g, pid, Some(uid), *pos)
                        && (*pos == here || g.unit_ids_at(*pos).is_empty())
                })
                .collect()
        };
        let mut goals = stands(&*g, EDGE_MUSTER_TILES);
        if goals.is_empty() {
            goals = stands(&*g, EDGE_MUSTER_TILES + 2);
        }
        if goals.contains(&here) {
            let _ = g.apply(pid, &Action::Fortify { unit: uid });
            return Some(false);
        }
        let next = g
            .route_step_to_any(uid, &goals)
            .filter(|next| g.can_move(uid, *next))?;
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

    /// Version 2: one wounded edge unit heals before it marches. It steps,
    /// with every move it has, into the nearest city of ours within
    /// [`EDGE_HEAL_CITY_TILES`] when there is one, and holds fortified there
    /// or where it stands, so no later step of the turn walks it (live
    /// 205538Z: the upgraded Warrior, at 50 health, walked from turn 36 to
    /// 48 and never healed). Returns the city it heals in, `Some(None)` in
    /// place, `None` for a unit gone.
    fn edge_heal_step(g: &mut Game, pid: usize, uid: u32) -> Option<Option<u32>> {
        let here = g.units.get(&uid)?.pos;
        let haven = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| g.wdist(g.cities[cid].pos, here) <= EDGE_HEAL_CITY_TILES)
            .min_by_key(|cid| (g.wdist(g.cities[cid].pos, here), *cid));
        if let Some(cid) = haven {
            let city = g.cities[&cid].pos;
            for _ in 0..6 {
                let Some(body) = g.units.get(&uid) else {
                    break;
                };
                if body.pos == city || body.moves_left <= 0.0 {
                    break;
                }
                let Some(next) = g
                    .route_step(uid, city, 0)
                    .filter(|next| g.can_move(uid, *next))
                else {
                    break;
                };
                if g.apply(
                    pid,
                    &Action::Move {
                        unit: uid,
                        to: next,
                    },
                )
                .is_err()
                {
                    break;
                }
            }
        }
        if g.units.get(&uid).is_some_and(|body| body.moves_left > 0.0) {
            let _ = g.apply(pid, &Action::Fortify { unit: uid });
        }
        Some(haven)
    }

    fn edge_journal_denounce(&mut self, g: &mut Game, pid: usize, rival: usize) {
        Self::edge_count(g, pid, "denounced");
        let wait = g.standard_duration(5);
        think!(self.journal(), Military, Strategy,
               "Edge unit denounces {}", g.players[rival].civ;
               "{TAG}: denounced: {} so the Formal War is legal in {} turns",
               g.players[rival].civ, wait);
    }

    /// Journal a hold at the muster once for each new reason.
    fn edge_hold(
        &mut self,
        g: &mut Game,
        pid: usize,
        window: &mut EdgeWindow,
        key: &str,
        why: String,
    ) {
        if window.held.as_deref() != Some(why.as_str()) {
            Self::edge_count(g, pid, key);
            think!(self.journal(), Military, Decision,
                   "Edge unit holds the declaration";
                   "{TAG}: hold: {why}");
            window.held = Some(why);
        }
    }

    /// ⭐ The muster and the declaration.
    fn edge_muster(
        &mut self,
        g: &mut Game,
        pid: usize,
        window: &mut EdgeWindow,
        soft: &SoftCity,
        unit: Name,
    ) {
        let city_pos = g.cities[&soft.city].pos;
        // Once the first edge unit exists, the shipped opening's denouncement
        // goes out, so the Formal War is legal by the time the force musters
        // (`preferred_war_opening` on a major with no casus belli denounces
        // and waits five standard turns). A war action it offers now is left
        // for the muster. Version 2: at the first unit, not the third (live
        // 205538Z denounced at turn 54 with the third Legion; the window
        // closed at 56 with the Formal War still three turns off).
        let raised = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| g.units[uid].kind == unit)
            .count()
            >= EDGE_DENOUNCE_BODIES;
        let denounced = g.players[pid]
            .denounced_until
            .get(&soft.rival)
            .is_some_and(|until| *until > g.turn);
        if raised && !denounced && !g.is_at_war(pid, soft.rival) {
            if let Some(action @ Action::Denounce { .. }) =
                self.preferred_war_opening(g, pid, soft.rival)
            {
                if g.apply(pid, &action).is_ok() {
                    self.edge_journal_denounce(g, pid, soft.rival);
                }
            }
        }
        // At the city: within EDGE_MUSTER_TILES, or within the stand ring the
        // walk falls back to when the rival's border closes the 3-4 ring
        // (round 1: Legions held 5-6 tiles out on a closed ring and never
        // counted).
        let reach = Self::edge_muster_reach(g, pid, city_pos);
        let near: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let body = &g.units[uid];
                !g.is_embarked(body) && g.wdist(body.pos, city_pos) <= reach
            })
            .collect();
        let bodies = near
            .iter()
            .filter(|&&uid| g.units[&uid].kind == unit && g.units[&uid].hp >= EDGE_MUSTER_HP)
            .count();
        if bodies >= EDGE_MUSTER_BODIES && window.mustered.is_none() {
            window.mustered = Some(g.turn);
            Self::edge_count(g, pid, "mustered");
            think!(self.journal(), Military, Strategy,
                   "Edge unit force musters at {}", g.cities[&soft.city].name;
                   "{TAG}: muster: {bodies} {} at {}+ health within {reach} tiles of {}",
                   plain(unit.as_str()), EDGE_MUSTER_HP, g.cities[&soft.city].name;
                   city_pos);
        }
        if bodies < EDGE_MUSTER_BODIES {
            let owned: Vec<u32> = g
                .player_unit_ids(pid)
                .into_iter()
                .filter(|uid| g.units[uid].kind == unit)
                .collect();
            if owned.len() >= EDGE_MUSTER_BODIES {
                // Where the rest of the force stands, so a muster that never
                // forms says why.
                let mut away: Vec<(i32, i32)> = owned
                    .iter()
                    .filter(|uid| !near.contains(uid) || g.units[uid].hp < EDGE_MUSTER_HP)
                    .map(|uid| (g.wdist(g.units[uid].pos, city_pos), g.units[uid].hp))
                    .collect();
                away.sort_unstable();
                let why = format!(
                    "{bodies} of {} {} at {} with {}+ health; the others at {}",
                    owned.len(),
                    plain(unit.as_str()),
                    g.cities[&soft.city].name,
                    EDGE_MUSTER_HP,
                    away.iter()
                        .map(|(tiles, hp)| format!("{tiles} tiles {hp} hp"))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                self.edge_hold(g, pid, window, "held_bodies", why);
            }
            return;
        }
        let breakers = near
            .iter()
            .filter(|&&uid| {
                let kind = g.units[&uid].kind;
                is_breaker(kind.as_str()) && g.city_allows_siege_support(soft.city, kind.as_str())
            })
            .count();
        if soft.walls && breakers == 0 {
            let why = format!(
                "{bodies} at {}, walls stand and no breaker is with them",
                g.cities[&soft.city].name
            );
            self.edge_hold(g, pid, window, "held_breaker", why);
            return;
        }
        let other_war = g
            .players
            .iter()
            .find(|player| {
                player.id != pid
                    && player.id != soft.rival
                    && player.alive
                    && !player.is_minor
                    && !player.is_barbarian
                    && g.is_at_war(pid, player.id)
            })
            .map(|player| player.civ.clone());
        if let Some(other) = other_war {
            let why = format!("at war with {other}");
            self.edge_hold(g, pid, window, "held_war", why);
            return;
        }
        let pool: Vec<u32> = near
            .iter()
            .copied()
            .filter(|uid| {
                let spec = &g.rules.units[g.units[uid].kind];
                !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && ((spec.class == "military"
                        && (spec.is_melee_capable() || spec.has_ranged_attack()))
                        || is_breaker(g.units[uid].kind.as_str()))
            })
            .collect();
        let Some(forecast) = self.assault_forecast_at_peace(g, pid, soft.city, &pool) else {
            self.edge_hold(g, pid, window, "held_forecast", "no forecast".to_string());
            return;
        };
        let city = &g.cities[&soft.city];
        let walls = g.city_max_wall_hp(city).max(city.wall_hp);
        let chance = forecast.calibrated_within(EDGE_FORECAST_TURNS, walls);
        let raw = forecast.within(EDGE_FORECAST_TURNS);
        if chance < DECLARE_P {
            let why = format!(
                "{bodies} at {} with {} in the pool; the forecast reads {:.0}% within {} turns (raw {:.0}%), short of {}",
                city.name,
                pool.len(),
                chance * 100.0,
                EDGE_FORECAST_TURNS,
                raw * 100.0,
                forecast.short_of.map_or("nothing named", |short| short.as_str())
            );
            self.edge_hold(g, pid, window, "held_forecast", why);
            return;
        }
        let name = city.name.clone();
        let Some(action) = self.preferred_war_opening(g, pid, soft.rival) else {
            let why = format!(
                "the forecast reads {:.0}% at {name}, and the shipped opening offers no declaration on {} yet (a denouncement waits out its Formal War)",
                chance * 100.0,
                g.players[soft.rival].civ
            );
            self.edge_hold(g, pid, window, "held_opening", why);
            return;
        };
        if matches!(action, Action::Denounce { .. }) {
            // The shipped opening on a major with no casus belli: denounce
            // now, declare the Formal War when it is legal.
            if g.apply(pid, &action).is_ok() {
                self.edge_journal_denounce(g, pid, soft.rival);
            }
            let why = format!(
                "the forecast reads {:.0}% at {name}; denounced {} at the muster, the Formal War follows",
                chance * 100.0,
                g.players[soft.rival].civ
            );
            self.edge_hold(g, pid, window, "held_opening", why);
            return;
        }
        if g.apply(pid, &action).is_err() || !g.is_at_war(pid, soft.rival) {
            let why = format!(
                "the declaration on {} was refused",
                g.players[soft.rival].civ
            );
            self.edge_hold(g, pid, window, "held_refused", why);
            return;
        }
        window.declared = Some(g.turn);
        window.held = None;
        Self::edge_count(g, pid, "declared");
        think!(self.journal(), Military, Strategy,
               "Edge unit declares on {} for {}", g.players[soft.rival].civ, name;
               "{TAG}: declared: {bodies} {} at {}+ health within {} tiles{}, {} in the pool; forecast {:.0}% within {} turns (raw {:.0}%), edge {:+.0}",
               plain(unit.as_str()), EDGE_MUSTER_HP, reach,
               if soft.walls { format!(" and {breakers} breaker(s)") } else { String::new() },
               pool.len(), chance * 100.0, EDGE_FORECAST_TURNS, raw * 100.0, soft.edge;
               city_pos);
        self.plan = None;
        self.force_groups_dirty = true;
    }

    /// The window at war: the capture, the continuation, the peace.
    fn edge_war(&mut self, g: &mut Game, pid: usize, window: &mut EdgeWindow) {
        let (Some(mut target), Some(declared), Some(unit)) =
            (window.target.clone(), window.declared, window.unit)
        else {
            self.edge_close(g, pid, window, "the war lost its target");
            return;
        };
        if let Some(rival) = player_by_civ(g, &target.owner_civ) {
            target.player = rival;
        }
        let rival = target.player;
        let owner = g.city_at(target.pos).and_then(|cid| {
            target.city = cid;
            g.cities.get(&cid).map(|city| city.owner)
        });
        if !g.players.get(rival).is_some_and(|player| player.alive) {
            self.edge_close(g, pid, window, "the rival is gone");
            return;
        }
        if !g.is_at_war(pid, rival) {
            // A declaration logged on the seat's decision view can miss the
            // board (round 3: three of eight, the batch cut short by an
            // earlier refused order); with no war on record since, the
            // window asks again.
            let began = g.wars.values().any(|war| {
                war.started >= declared
                    && ((war.aggressor == pid && war.defender == rival)
                        || (war.aggressor == rival && war.defender == pid))
            });
            if !began && window.redeclared < EDGE_REDECLARE_MAX {
                window.redeclared += 1;
                window.declared = None;
                window.held = None;
                Self::edge_count(g, pid, "declare_lost");
                think!(self.journal(), Military, Strategy,
                       "Edge unit declaration did not reach the board";
                       "{TAG}: hold: the declaration of turn {declared} on {} is not on the board; asking again ({} of {})",
                       target.owner_civ, window.redeclared, EDGE_REDECLARE_MAX);
                self.plan = None;
                return;
            }
            if g.turn > declared {
                self.edge_close(g, pid, window, "the war has ended");
                return;
            }
        }
        let force: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| g.units[uid].kind == unit)
            .collect();
        let name = g
            .cities
            .get(&target.city)
            .map(|city| city.name.clone())
            .unwrap_or_default();
        if owner == Some(rival) {
            let since = g
                .turn
                .saturating_sub(window.last_capture.unwrap_or(declared));
            if since >= EDGE_WAR_PATIENCE {
                let why = format!("{since} war turns without a capture at {name}");
                self.edge_sue(g, pid, window, rival, &why);
                return;
            }
            if force.len() < EDGE_FORCE_FLOOR {
                let why = format!("{} {} left", force.len(), plain(unit.as_str()));
                self.edge_sue(g, pid, window, rival, &why);
                return;
            }
            // The assault: breakers and edge units step beside the city and
            // the edge units strike it (round 3: five declared wars stood
            // 10-20 turns within four tiles with no unit beside the city).
            let (blows, before, after) = self.edge_press_the_assault(g, pid, unit, target.city);
            if blows > 0 {
                Self::edge_count(g, pid, "assault");
                think!(self.journal(), Military, Decision,
                       "Edge unit strikes {}", name;
                       "{TAG}: assault: {blows} blow(s) on {name}, health {before} to {after}";
                       target.pos);
            }
            // One line a turn of what the assault does to the city.
            if let Some(city) = g.cities.get(&target.city) {
                let beside = force
                    .iter()
                    .filter(|uid| {
                        g.units
                            .get(uid)
                            .is_some_and(|body| g.wdist(body.pos, target.pos) <= 1)
                    })
                    .count();
                let close = force
                    .iter()
                    .filter(|uid| {
                        g.units
                            .get(uid)
                            .is_some_and(|body| g.wdist(body.pos, target.pos) <= EDGE_MUSTER_TILES)
                    })
                    .count();
                think!(self.journal(), Military, Decision,
                       "Edge unit siege at {}", name;
                       "{TAG}: siege: {} health {} walls {}; {} {} beside it, {} within {}, war turn {}",
                       name, city.hp, city.wall_hp, beside, plain(unit.as_str()), close,
                       EDGE_MUSTER_TILES, g.turn.saturating_sub(declared);
                       target.pos);
            }
            window.target = Some(target);
            return;
        }
        if owner == Some(pid) {
            window.taken += 1;
            window.last_capture = Some(g.turn);
            Self::edge_count(g, pid, "captured");
            think!(self.journal(), Military, Strategy,
                   "Edge unit takes {}", name;
                   "{TAG}: captured: {} from {}, {} taken, {} {} left",
                   name, target.owner_civ, window.taken, force.len(), plain(unit.as_str());
                   target.pos);
        } else {
            think!(self.journal(), Military, Strategy,
                   "Edge unit target changed hands";
                   "{TAG}: lost the target: {} is no longer {}'s", name, target.owner_civ);
        }
        let strength = g.rules.units.get(&unit).map_or(0.0, |spec| spec.strength);
        let from: Vec<Pos> = force.iter().map(|uid| g.units[uid].pos).collect();
        let next = self.edge_soft_city(
            g,
            pid,
            strength,
            EDGE_CONTINUE_MIN,
            &from,
            EDGE_CONTINUE_TILES,
            Some(rival),
        );
        let Some(next) = next.filter(|_| force.len() >= EDGE_FORCE_FLOOR) else {
            let why = format!(
                "no city of {} within {} tiles of the force with an edge of {:.0} or more ({} {} left)",
                target.owner_civ,
                EDGE_CONTINUE_TILES,
                EDGE_CONTINUE_MIN,
                force.len(),
                plain(unit.as_str())
            );
            self.edge_sue(g, pid, window, rival, &why);
            return;
        };
        Self::edge_count(g, pid, "continued");
        let next_city = &g.cities[&next.city];
        think!(self.journal(), Military, Strategy,
               "Edge unit goes on to {}", next_city.name;
               "{TAG}: continue: {} of {}, edge +{:.0}, {} tiles from the force",
               next_city.name, target.owner_civ, next.edge, next.tiles;
               next_city.pos);
        window.target = Some(EdgeTarget {
            player: rival,
            city: next.city,
            pos: next_city.pos,
            owner_civ: target.owner_civ,
        });
        if window.last_capture.is_none() {
            window.last_capture = Some(g.turn);
        }
        self.plan = None;
    }

    /// Press the assault on `cid` at the start of the turn: each breaker of
    /// ours steps to a free tile beside the city, then each edge unit with a
    /// move steps beside it and, at [`EDGE_STRIKE_MIN_HP`] or more, strikes
    /// it. Returns the blows struck and the city's health before and after.
    fn edge_press_the_assault(
        &mut self,
        g: &mut Game,
        pid: usize,
        unit: Name,
        cid: u32,
    ) -> (usize, i32, i32) {
        let Some(city) = g.cities.get(&cid) else {
            return (0, 0, 0);
        };
        let (pos, before) = (city.pos, city.hp);
        let mut bodies: Vec<(bool, i32, u32)> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter_map(|uid| {
                let body = &g.units[&uid];
                let breaker = is_breaker(body.kind.as_str());
                (body.kind == unit || breaker).then_some((!breaker, -body.hp, uid))
            })
            .collect();
        // Breakers first, then the healthiest.
        bodies.sort_unstable();
        let mut blows = 0;
        for (striker, _, uid) in bodies {
            for _ in 0..6 {
                let Some(body) = g.units.get(&uid) else {
                    break;
                };
                if body.moves_left <= 0.0 || g.is_embarked(body) {
                    break;
                }
                if !g.cities.get(&cid).is_some_and(|city| city.owner != pid) {
                    break;
                }
                let here = body.pos;
                if g.wdist(here, pos) <= 1 {
                    let strikes = striker && body.hp >= EDGE_STRIKE_MIN_HP;
                    if strikes
                        && g.apply(
                            pid,
                            &Action::Attack {
                                unit: uid,
                                target: pos,
                            },
                        )
                        .is_ok()
                    {
                        blows += 1;
                    }
                    break;
                }
                let beside: HashSet<Pos> = g
                    .wring(pos, 1)
                    .into_iter()
                    .filter(|tile| {
                        g.map
                            .get(*tile)
                            .is_some_and(|t| !g.rules.is_water(t) && g.rules.is_passable(t))
                            && g.city_at(*tile).is_none()
                            && g.unit_ids_at(*tile)
                                .iter()
                                .all(|other| g.units[other].owner == pid)
                    })
                    .collect();
                let Some(next) = g
                    .route_step_to_any(uid, &beside)
                    .filter(|next| g.can_move(uid, *next))
                else {
                    break;
                };
                if g.apply(
                    pid,
                    &Action::Move {
                        unit: uid,
                        to: next,
                    },
                )
                .is_err()
                {
                    break;
                }
            }
        }
        let after = g.cities.get(&cid).map_or(0, |city| city.hp);
        (blows, before, after)
    }

    /// Ask `rival` for peace and close the window.
    fn edge_sue(
        &mut self,
        g: &mut Game,
        pid: usize,
        window: &mut EdgeWindow,
        rival: usize,
        why: &str,
    ) {
        Self::edge_count(g, pid, "peace");
        think!(self.journal(), Military, Strategy,
               "Edge unit asks {} for peace", g.players[rival].civ;
               "{TAG}: peace: {why}");
        let pending = g.pending_deals.iter().any(|deal| {
            deal.peace
                && ((deal.from == pid && deal.to == rival)
                    || (deal.from == rival && deal.to == pid))
                && deal.expires >= g.turn
        });
        if g.is_at_war(pid, rival) && !pending && !self.peace_offers.contains(&rival) {
            self.peace_offers.insert(rival);
            let _ = g.apply(
                pid,
                &Action::ProposeDeal {
                    player: rival,
                    give_gold: 0.0,
                    request_gold: 0.0,
                    open_borders: false,
                    friendship: false,
                    peace: true,
                    alliance: None,
                },
            );
        }
        self.edge_close(g, pid, window, why);
    }

    /// A source of `resource` one tile outside our borders is bought, down
    /// to an empty treasury while income is not negative, when no source
    /// lies inside them.
    fn edge_buy_source(&mut self, g: &mut Game, pid: usize, resource: Name) {
        let sources = edge_sources(g, pid, resource);
        if sources.iter().any(|(_, inside)| *inside) {
            return;
        }
        let income = g.players[pid].gold_per_turn;
        if !income.is_finite() || income < 0.0 {
            return;
        }
        let gold = g.players[pid].gold;
        let mut best: Option<(f64, u32, Pos)> = None;
        for (pos, _) in sources {
            for cid in g.player_city_ids(pid) {
                let Some(cost) = g.plot_purchase_cost(pid, cid, pos) else {
                    continue;
                };
                if cost > gold + 1e-9 {
                    continue;
                }
                let better = best.as_ref().is_none_or(|(held, city, at)| {
                    if (cost - *held).abs() > 1e-9 {
                        cost < *held
                    } else {
                        (cid, pos) < (*city, *at)
                    }
                });
                if better {
                    best = Some((cost, cid, pos));
                }
            }
        }
        let Some((cost, city, pos)) = best else {
            return;
        };
        if g.apply(pid, &Action::BuyPlot { city, pos, cost }).is_err() {
            return;
        }
        Self::edge_count(g, pid, "tile_bought");
        think!(self.journal(), Military, Decision,
               "{} buys the {} at {:?}", g.cities[&city].name, plain(resource.as_str()), pos;
               "{TAG}: resource: bought a {} source for {:.0} Gold, {:.0} left",
               plain(resource.as_str()), cost, g.players[pid].gold;
               pos);
    }

    /// Gold upgrades of owned units into the edge unit, the cheapest first,
    /// ahead of the treasury floor and down to an empty treasury while the
    /// income is not negative, up to the force's size.
    fn edge_upgrades(&mut self, g: &mut Game, pid: usize, unit: Name) {
        let income = g.players[pid].gold_per_turn;
        if !income.is_finite() || income < 0.0 {
            return;
        }
        loop {
            let (owned, queued) = Self::edge_force(g, pid, unit);
            if owned + queued >= EDGE_FORCE_MAX {
                return;
            }
            let treasury = g.players[pid].gold;
            let best = g
                .player_unit_ids(pid)
                .into_iter()
                .filter_map(|uid| {
                    let (to, gold, _) = g.unit_gold_upgrade_offer(pid, uid)?;
                    (to == unit && gold <= treasury + 1e-9).then_some((gold, uid))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            let Some((gold, uid)) = best else {
                return;
            };
            let from = g.units[&uid].kind;
            if g.apply(pid, &Action::UpgradeUnit { unit: uid }).is_err() {
                return;
            }
            Self::edge_count(g, pid, "upgraded");
            think!(self.journal(), Military, Decision,
                   "Edge unit upgrade: {} to {}", plain(from.as_str()), plain(unit.as_str());
                   "{TAG}: force: upgraded a {} to {} for {:.0} Gold of {:.0} ({:+.0} a turn)",
                   plain(from.as_str()), plain(unit.as_str()), gold, treasury, income);
        }
    }

    /// `edge-unit-takes-the-soft-city`: the research goal while the window
    /// is open and the edge unit is still behind research: the technology
    /// that reveals its resource, then its own. `None` with the gene off.
    pub(super) fn edge_unit_research_goal(&self, g: &Game, pid: usize) -> Option<Name> {
        if !self.edge_unit_takes_the_soft_city {
            return None;
        }
        let from_the_start = self.edge_unit_opens_from_the_start;
        let window = self
            .edge_unit_window
            .as_ref()
            .filter(|w| w.mustering() || (from_the_start && w.preparing()))?;
        window
            .goal
            .filter(|goal| !g.players[pid].techs.contains(goal))
    }

    /// `edge-unit-takes-the-soft-city`: the capital and the cities nearest
    /// the target train the next edge unit (or the breaker the walls ask
    /// for) while the window is open. Exact no-op with the gene off.
    ///
    /// Version 2 (live 205538Z: one Legion owned and nothing queued from
    /// turn 33 to 44 with Iron in stock): while the window is open and the
    /// unit is trainable, [`EDGE_TRAINING_CITIES`] cities train it at once,
    /// and it takes the head of their queues from a deferrable item
    /// ([`Self::edge_may_displace`]) instead of waiting for an idle one. Other
    /// claims leave it there (`first_capital_training_holds` reads
    /// [`Self::edge_unit_training_holds`]).
    pub(super) fn claim_edge_unit_queue(&mut self, g: &mut Game, pid: usize) {
        self.edge_claim_queues(g, pid, true);
    }

    /// Version 2: whether `item` at the head of `cid`'s queue is the edge
    /// force's unit or its breaker in one of the training cities, while the
    /// window is open; the claims that respect a reserved army build leave
    /// it in place. `false` with the gene off.
    pub(super) fn edge_unit_training_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
    ) -> bool {
        if !self.edge_unit_takes_the_soft_city {
            return false;
        }
        let Some(window) = self.edge_unit_window.as_ref().filter(|w| w.live()) else {
            return false;
        };
        let (Some(unit), Some(target)) = (window.unit, window.target.as_ref()) else {
            return false;
        };
        let Item::Unit { unit: kind } = item else {
            return false;
        };
        (*kind == unit || is_breaker(kind.as_str()))
            && Self::edge_training_cities(g, pid, target.pos, EDGE_TRAINING_CITIES).contains(&cid)
    }

    /// The claim. `preempt` lets the edge unit take the head of a queue
    /// from a deferrable item; off, only idle queues are filled (the turn
    /// start while the opening book runs, whose own items are left alone).
    fn edge_claim_queues(&mut self, g: &mut Game, pid: usize, preempt: bool) {
        if !self.edge_unit_takes_the_soft_city {
            return;
        }
        let from_the_start = self.edge_unit_opens_from_the_start;
        let Some(window) = self
            .edge_unit_window
            .as_ref()
            .filter(|w| w.live() || (from_the_start && w.preparing() && w.start_supplied))
        else {
            return;
        };
        let Some(unit) = window.unit else {
            return;
        };
        let live = window.live();
        let target = window.target.clone();
        // Before a city is named the force gathers at the capital.
        let Some(anchor) = target
            .as_ref()
            .map(|target| target.pos)
            .or_else(|| Self::conquest_capital(g, pid).map(|cid| g.cities[&cid].pos))
        else {
            return;
        };
        let breaker = target
            .as_ref()
            .and_then(|target| {
                g.city_at(target.pos)
                    .filter(|cid| g.cities[cid].owner == target.player)
            })
            .filter(|cid| g.cities[cid].wall_hp > 0)
            .and_then(|cid| edge_breaker(g, pid, cid));
        // Version 2: three cities at once while the window is open and the
        // unit's technology is known; before, the capital and one other.
        let trainable = g
            .rules
            .units
            .get(&unit)
            .and_then(|spec| spec.tech)
            .is_none_or(|tech| g.players[pid].techs.contains(&tech));
        let count = if live && trainable {
            EDGE_TRAINING_CITIES
        } else {
            2
        };
        for cid in Self::edge_training_cities(g, pid, anchor, count) {
            let head = g.cities[&cid].queue.first().cloned();
            if let Some(Item::Unit { unit: kind }) = &head {
                if *kind == unit || is_breaker(kind.as_str()) {
                    continue;
                }
            }
            let displaced = match &head {
                None => None,
                Some(item) if preempt && live && Self::edge_may_displace(g, pid, cid, item) => {
                    Some(item.clone())
                }
                Some(_) => continue,
            };
            let (owned, queued) = Self::edge_force(g, pid, unit);
            let breakers_held = Self::conquest_breakers_held(g, pid);
            let mut item: Option<Item> = None;
            if let Some(kind) = breaker.filter(|_| breakers_held == 0) {
                let ram = Item::Unit { unit: kind };
                if g.can_produce(pid, cid, &ram) {
                    item = Some(ram);
                }
            }
            if item.is_none() && owned + queued < EDGE_FORCE_MAX {
                let body = Item::Unit { unit };
                if g.can_produce(pid, cid, &body) {
                    item = Some(body);
                }
            }
            // `edge-unit-opens-from-the-start`: while the edge unit cannot be
            // trained, bodies that upgrade into it, up to the muster's three,
            // in an idle queue only.
            if item.is_none() && from_the_start && displaced.is_none() {
                let feeders = Self::edge_feeders(g, pid, unit);
                if owned + queued + feeders.0 + feeders.1 < EDGE_MUSTER_BODIES {
                    if let Some(feeder) = Self::edge_feeder_kind(g, pid, cid, unit) {
                        item = Some(Item::Unit { unit: feeder });
                    }
                }
            }
            let Some(item) = item else {
                continue;
            };
            if g.apply(
                pid,
                &Action::Produce {
                    city: cid,
                    item: item.clone(),
                },
            )
            .is_err()
            {
                continue;
            }
            Self::edge_count(g, pid, "trained");
            let ahead = displaced
                .as_ref()
                .map(|item| {
                    format!(
                        ", ahead of {} (it keeps its progress)",
                        Self::plain_item(item)
                    )
                })
                .unwrap_or_default();
            think!(self.journal(), Military, Decision,
                   "{} trains {} for the edge force", g.cities[&cid].name, Self::plain_item(&item);
                   "{TAG}: force: {} {} owned and {} queued of {} before this one{ahead}",
                   owned, plain(unit.as_str()), queued, EDGE_FORCE_MAX);
        }
    }

    /// Whether a unit of `kind` upgrades into `unit` (a Warrior into the
    /// Legion, through the Swordsman the Legion replaces).
    fn edge_feeds(g: &Game, pid: usize, kind: Name, unit: Name) -> bool {
        g.rules
            .units
            .get(&kind)
            .and_then(|spec| spec.upgrade_to)
            .is_some_and(|to| g.player_unit_replacement(pid, to) == unit)
    }

    /// Units that upgrade into `unit`, owned and queued.
    fn edge_feeders(g: &Game, pid: usize, unit: Name) -> (usize, usize) {
        let owned = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| Self::edge_feeds(g, pid, g.units[uid].kind, unit))
            .count();
        let queued = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| {
                g.cities[&cid]
                    .queue
                    .iter()
                    .filter(|item| {
                        matches!(item, Item::Unit { unit: kind } if Self::edge_feeds(g, pid, *kind, unit))
                    })
                    .count()
            })
            .sum();
        (owned, queued)
    }

    /// The cheapest unit `cid` can train that upgrades into `unit`.
    fn edge_feeder_kind(g: &Game, pid: usize, cid: u32, unit: Name) -> Option<Name> {
        g.rules
            .units
            .iter()
            .filter(|(name, _)| Self::edge_feeds(g, pid, **name, unit))
            .filter(|(name, _)| g.can_produce(pid, cid, &Item::Unit { unit: **name }))
            .min_by(|a, b| a.1.cost.total_cmp(&b.1.cost).then(a.0.cmp(b.0)))
            .map(|(name, _)| *name)
    }

    /// `edge-unit-takes-the-soft-city`: a Builder improves a source of the
    /// edge unit's resource inside our borders before anything else, while
    /// the window is open and nothing supplies it. `None` with the gene off.
    pub(super) fn edge_unit_builder_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !self.edge_unit_takes_the_soft_city {
            return None;
        }
        let from_the_start = self.edge_unit_opens_from_the_start;
        let unit = self
            .edge_unit_window
            .as_ref()
            .filter(|w| w.live() || (from_the_start && w.preparing() && w.start_supplied))
            .and_then(|w| w.unit)?;
        let (resource, _) = edge_resource(g, unit)?;
        if g.strategic_resource_rate(pid, resource.as_str()) > 0.0
            || !g.resource_visible_to(pid, resource.as_str())
        {
            return None;
        }
        let builder = g.units.get(&uid)?;
        if builder.owner != pid
            || builder.kind != "builder"
            || builder.charges <= 0
            || builder.moves_left <= 0.0
            || builder.linked_to.is_some()
            || self.builder_support.contains_key(&uid)
        {
            return None;
        }
        let current = builder.pos;
        let improvement = Name::new(&g.rules.resources[resource].improvement);
        let visible = g.player_vision_frame(pid);
        let mut jobs = Vec::new();
        for (pos, inside) in edge_sources(g, pid, resource) {
            if !inside {
                continue;
            }
            let tile = &g.map.tiles[&pos];
            let repair = tile.pillaged && tile.improvement == Some(improvement);
            if (tile.improvement == Some(improvement) && !repair)
                || tile.flooded
                || tile.submerged
                || self.builder_targets.iter().any(|(other, at)| {
                    *other != uid
                        && *at == pos
                        && g.units.get(other).is_some_and(|unit| {
                            unit.owner == pid && unit.kind == "builder" && unit.charges > 0
                        })
                })
            {
                continue;
            }
            if !repair && !g.valid_improvements(pid, pos).contains(&improvement) {
                continue;
            }
            let Some(steps) = g
                .route_distance(uid, pos, 0)
                .filter(|steps| *steps <= EDGE_BUILDER_STEPS)
            else {
                continue;
            };
            if self.settlement_tile_risk_with_support(g, pid, Some(uid), pos, &visible, false) > 0.0
            {
                continue;
            }
            jobs.push((steps, pos, repair));
        }
        jobs.sort_unstable();
        for (_, pos, repair) in jobs {
            let acted = if current == pos {
                let action = if repair {
                    Action::RepairImprovement { unit: uid }
                } else {
                    Action::Improve {
                        unit: uid,
                        improvement,
                    }
                };
                g.apply(pid, &action).is_ok()
            } else {
                let Some(next) = g.route_step(uid, pos, 0).filter(|next| {
                    g.can_move(uid, *next)
                        && self.settlement_tile_risk_with_support(
                            g,
                            pid,
                            Some(uid),
                            *next,
                            &visible,
                            false,
                        ) <= 0.0
                }) else {
                    continue;
                };
                self.base.path_move(g, pid, uid, next)
            };
            if !acted {
                continue;
            }
            if current == pos {
                self.builder_targets.remove(&uid);
                Self::edge_count(g, pid, "improved");
            } else {
                self.builder_targets.insert(uid, pos);
            }
            think!(self.journal(), Military, Decision,
                   "Builder works the edge unit's {}", plain(resource.as_str());
                   "{TAG}: resource: builder {} {} the {} at {:?}",
                   uid, if current == pos { "improves" } else { "walks to" },
                   plain(resource.as_str()), pos;
                   pos);
            return Some(true);
        }
        None
    }

    /// The rival and city tile a peacetime body of the edge force stages
    /// on: an edge unit, or the breaker, of ours, not a civilian's guard,
    /// above the withdraw line, while the window musters.
    fn edge_staging_target(&self, g: &Game, pid: usize, uid: u32) -> Option<(usize, Pos)> {
        let window = self.edge_unit_window.as_ref().filter(|w| w.mustering())?;
        let target = window.target.as_ref()?;
        let unit = g.units.get(&uid)?;
        if unit.owner != pid
            || unit.hp as f64 <= self.base.w.withdraw_hp
            || g.is_embarked(unit)
            || (Some(unit.kind) != window.unit && !is_breaker(unit.kind.as_str()))
            || g.is_at_war(pid, target.player)
            || !self.campaign_target_legal(g, pid, target.player)
            || self.all_reserved_civilian_guards().contains(&uid)
        {
            return None;
        }
        let cid = g.city_at(target.pos)?;
        (g.cities[&cid].owner == target.player).then_some((target.player, target.pos))
    }

    /// `edge-unit-takes-the-soft-city`: a body of the edge force walks to a
    /// stand [`EDGE_STAND_MIN_TILES`]-[`EDGE_MUSTER_TILES`] tiles from the
    /// city (outside a border closed to us) and holds it. `None` with the
    /// gene off, for any other unit, and when no stand is reachable.
    pub(super) fn edge_unit_staging_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !self.edge_unit_takes_the_soft_city {
            return None;
        }
        let (_, city) = self.edge_staging_target(g, pid, uid)?;
        // Version 2: a wounded edge unit heals before it marches.
        let unit = self.edge_unit_window.as_ref().and_then(|w| w.unit);
        if g.units
            .get(&uid)
            .is_some_and(|body| Some(body.kind) == unit && body.hp < EDGE_HEAL_HP)
        {
            return Self::edge_heal_step(g, pid, uid).map(|_| false);
        }
        Self::edge_stage_unit(g, pid, uid, city)
    }

    /// `edge-unit-takes-the-soft-city`: where the objective board's Reserve
    /// gathers while the force musters and holds at least one edge unit: the
    /// stand nearest our capital [`EDGE_STAND_MIN_TILES`]-[`EDGE_MUSTER_TILES`]
    /// tiles from the city. `None` with the gene off.
    pub(super) fn edge_unit_staging_rally(&self, g: &Game, pid: usize) -> Option<Pos> {
        if !self.edge_unit_takes_the_soft_city {
            return None;
        }
        let window = self.edge_unit_window.as_ref().filter(|w| w.mustering())?;
        let (unit, target) = (window.unit?, window.target.as_ref()?);
        if Self::edge_force(g, pid, unit).0 == 0 || g.is_at_war(pid, target.player) {
            return None;
        }
        let home = Self::conquest_capital(g, pid).map(|cid| g.cities[&cid].pos)?;
        g.wdisk(target.pos, EDGE_MUSTER_TILES)
            .into_iter()
            .filter(|pos| {
                g.wdist(*pos, target.pos) >= EDGE_STAND_MIN_TILES && edge_stand(g, pid, None, *pos)
            })
            .min_by_key(|pos| (g.wdist(*pos, home), *pos))
    }

    /// `edge-unit-takes-the-soft-city`: an edge unit of the mustering force,
    /// which the objective board keeps out of distant skirmish rows as it
    /// keeps the opening's force. `false` with the gene off.
    pub(super) fn edge_unit_force_member(&self, g: &Game, uid: u32) -> bool {
        self.edge_unit_takes_the_soft_city
            && self
                .edge_unit_window
                .as_ref()
                .filter(|w| w.mustering())
                .and_then(|w| w.unit)
                .is_some_and(|unit| g.units.get(&uid).is_some_and(|body| body.kind == unit))
    }

    /// `edge-unit-takes-the-soft-city`: while the window's war runs, the
    /// plan's target is its city, under the lane's own strategy. A Recovery
    /// with a threatened city keeps its own target. No-op with the gene off.
    pub(crate) fn apply_edge_unit_to_strategy(
        &self,
        g: &Game,
        pid: usize,
        plan: &mut StrategicPlan,
    ) {
        if !self.edge_unit_takes_the_soft_city {
            return;
        }
        let Some(window) = self.edge_unit_window.as_ref().filter(|w| w.live()) else {
            return;
        };
        let (Some(_), Some(target)) = (window.declared, window.target.as_ref()) else {
            return;
        };
        if plan.strategy == GrandStrategy::Recovery && plan.threatened_city.is_some() {
            return;
        }
        let Some(cid) = g.city_at(target.pos) else {
            return;
        };
        if g.cities[&cid].owner != target.player || !g.is_at_war(pid, target.player) {
            return;
        }
        plan.target_player = Some(target.player);
        plan.target_city = Some(cid);
    }
}

#[cfg(test)]
mod tests;
