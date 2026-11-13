//! `declaration-needs-a-capture` (opt-in): the last gate on a declaration of
//! war on a major. A war we open must be able to take a city: the capture
//! forecast (`advanced/assault_forecast.rs`) of the soldiers already within
//! reach of one of the target's cities has to read a capture inside the
//! window the walls leave, or the declaration is held and the journal names
//! what is short (a taker, a breaker, or bodies).
//!
//! # Why
//!
//! The live census of 2026-11-12/13 (108 Deity games, 195 wars our seat
//! declared on a major; `docs/ASSAULT_FORECAST.md`, "Declarations"):
//!
//! - 143 were faith wars (the `faith-war-before-the-flip` family, the faith
//!   strike, the religious interception). Their point is the spreaders, not
//!   a city. They took 7 cities and lost 6 to their target, and they hold the
//!   conversion: our cities on the target's faith fell by 0.66 (Domination)
//!   and 1.13 (Score) in the 15 turns after one, against a rise of 1.46 and
//!   1.42 after a faith war the seat wanted and held. They are exempt.
//! - The other 48 (science denial, the main path's staged and counter wars,
//!   the open-city front, the launcher war) took no city within 30 turns and
//!   lost 13 to the war's target, and 189 units to its 72. The three the
//!   reader saw read 0%, 6% and 0% at the declaration.
//! - 4 more had no opener line; they are not declarations this gate sees.
//!
//! # The rule
//!
//! - **Pool:** our land soldiers, Battering Rams and Siege Towers within
//!   [`POOL_TURNS`] turns of their moves of one of the target's cities.
//! - **Cities:** the target's cities with a pool member in reach, the
//!   [`MAX_CITIES`] with the most first.
//! - **Reading:** the corps search (`assault_corps_best`) on each, as if the
//!   war began this turn, within [`OPEN_WINDOW`] turns for an open city (the
//!   owner raises walls about seven turns into a war) or [`WALLED_WINDOW`]
//!   for a walled one, calibrated (`calibrated_within`) for its walls.
//! - **Go:** some city reads at least [`DECLARE_P`], and a walled one has a
//!   siege gun, or a Ram or Tower that works on its walls, in the corps.
//! - **Hold:** otherwise; the line names the best city, its reading and what
//!   one more unit would lift it most (`ShortOf`).
//!
//! The faith openers call `preferred_war_opening_ungated`; every other road
//! to a declaration on a major passes here. Off, nothing reads it and every
//! opening is what it was.

use super::assault_forecast::{AssaultCorps, ShortOf, CALIBRATED_FROM, HIGH_BIN_TAKEN};
use super::AdvancedAi;
use crate::game::Game;
use crate::think;
use crate::Pos;
use std::collections::BTreeMap;

/// Calibrated P(capture within the window) a declaration needs.
pub(crate) const DECLARE_P: f32 = 0.5;
/// The window for a city with no walls standing: the owner raised walls 7
/// turns into the war in a replayed live game (100, then 200 ten turns on).
pub(crate) const OPEN_WINDOW: usize = 7;
/// The window for a walled city: the forecast's own go horizon.
pub(crate) const WALLED_WINDOW: usize = 10;
/// A pool member stands within this many turns of its moves of the city.
pub(crate) const POOL_TURNS: f64 = 5.0;
/// The target's cities read, the ones with the most pool members first.
pub(crate) const MAX_CITIES: usize = 2;

/// One city's reading at the gate.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CaptureReading {
    pub(crate) city: u32,
    pub(crate) pos: Pos,
    pub(crate) window: usize,
    /// The city's wall pool.
    pub(crate) walls: i32,
    /// Calibrated P(capture within `window`).
    pub(crate) chance: f32,
    /// The raw reading.
    pub(crate) raw: f32,
    pub(crate) corps: usize,
    pub(crate) pool: usize,
    /// Walls stand and nothing in the corps can open them.
    pub(crate) needs_breaker: bool,
    pub(crate) short_of: Option<ShortOf>,
}

impl CaptureReading {
    fn passes(&self) -> bool {
        self.chance >= DECLARE_P && !self.needs_breaker
    }
}

/// The gate's verdict on one target.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CaptureVerdict {
    Go(CaptureReading),
    /// The best reading, or `None` when no soldier of ours is in reach.
    Hold(Option<CaptureReading>),
}

/// The turn's verdicts, by target, so a turn reads and journals once.
#[derive(Clone, Default)]
pub(crate) struct CaptureMemo {
    turn: u32,
    held: BTreeMap<usize, bool>,
}

/// The raw reading whose calibrated value is `bar` against `walls`.
fn raw_for(bar: f32, walls: i32) -> f32 {
    if bar <= CALIBRATED_FROM {
        return bar;
    }
    let tier = if walls < 50 {
        0
    } else if walls < 150 {
        1
    } else if walls < 250 {
        2
    } else {
        3
    };
    let (_, read_high, taken) = HIGH_BIN_TAKEN[tier];
    let top = taken as f32 / read_high as f32;
    (CALIBRATED_FROM + (bar - CALIBRATED_FROM) * (1.0 - CALIBRATED_FROM) / (top - CALIBRATED_FROM))
        .min(1.0)
}

const SUPPORT_KINDS: [&str; 2] = ["battering_ram", "siege_tower"];

/// A land soldier, Ram or Tower of ours: what a corps can hold.
fn pool_member(g: &Game, pid: usize, uid: u32) -> bool {
    let Some(unit) = g.units.get(&uid) else {
        return false;
    };
    let spec = &g.rules.units[unit.kind];
    unit.owner == pid
        && !g.is_embarked(unit)
        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
        && ((spec.class == "military" && (spec.is_melee_capable() || spec.has_ranged_attack()))
            || SUPPORT_KINDS.contains(&unit.kind.as_str()))
}

/// Whether `uid` can open the walls of `cid`: a land siege gun, or a Ram or
/// Tower the city's walls admit.
fn opens_walls(g: &Game, uid: u32, cid: u32) -> bool {
    let Some(unit) = g.units.get(&uid) else {
        return false;
    };
    let spec = &g.rules.units[unit.kind];
    (spec.siege && !matches!(spec.domain.as_deref(), Some("sea" | "air")))
        || (SUPPORT_KINDS.contains(&unit.kind.as_str())
            && g.city_allows_siege_support(cid, unit.kind.as_str()))
}

impl AdvancedAi {
    /// `declaration-needs-a-capture`: whether the gene holds a declaration of
    /// war on `target` this turn. `false` with the gene off, for a city-state,
    /// the barbarians, a dead major or one we are already at war with.
    pub(super) fn declaration_lacks_a_capture(&self, g: &Game, pid: usize, target: usize) -> bool {
        if !self.declaration_needs_a_capture {
            return false;
        }
        let Some(player) = g.players.get(target) else {
            return false;
        };
        if target == pid
            || player.is_minor
            || player.is_barbarian
            || !player.alive
            || g.is_at_war(pid, target)
        {
            return false;
        }
        {
            let memo = self.declaration_capture_memo.borrow();
            if memo.turn == g.turn {
                if let Some(&held) = memo.held.get(&target) {
                    return held;
                }
            }
        }
        let verdict = self.capture_verdict(g, pid, target);
        let held = !matches!(verdict, CaptureVerdict::Go(_));
        self.journal_capture_verdict(g, target, &verdict);
        let mut memo = self.declaration_capture_memo.borrow_mut();
        if memo.turn != g.turn {
            memo.turn = g.turn;
            memo.held.clear();
        }
        memo.held.insert(target, held);
        held
    }

    /// The gate's reading of a war on `target` begun this turn.
    pub(crate) fn capture_verdict(&self, g: &Game, pid: usize, target: usize) -> CaptureVerdict {
        let ours: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| pool_member(g, pid, *uid))
            .collect();
        // The target's cities with a pool member in reach, most first.
        let mut cities: Vec<(usize, i32, u32, Vec<u32>)> = g
            .player_city_ids(target)
            .into_iter()
            .filter_map(|cid| {
                let pos = g.cities.get(&cid)?.pos;
                let pool: Vec<u32> = ours
                    .iter()
                    .copied()
                    .filter(|uid| {
                        let reach = (POOL_TURNS * g.unit_max_moves(*uid).max(1.0)).ceil() as i32;
                        g.wdist(g.units[uid].pos, pos) <= reach
                    })
                    .collect();
                if pool.is_empty() {
                    return None;
                }
                let near = pool
                    .iter()
                    .map(|uid| g.wdist(g.units[uid].pos, pos))
                    .min()
                    .unwrap_or(i32::MAX);
                Some((pool.len(), near, cid, pool))
            })
            .collect();
        cities.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        cities.truncate(MAX_CITIES);
        let mut best: Option<CaptureReading> = None;
        for (_, _, cid, pool) in cities {
            let Some(reading) = self.capture_reading(g, pid, cid, &pool) else {
                continue;
            };
            if reading.passes() {
                return CaptureVerdict::Go(reading);
            }
            let better = best.as_ref().is_none_or(|held| {
                (!reading.needs_breaker, reading.chance) > (!held.needs_breaker, held.chance)
            });
            if better {
                best = Some(reading);
            }
        }
        // What one more unit would lift the best reading most, read with the
        // short-of reruns only for the line that holds the war.
        if let Some(reading) = best.as_mut() {
            reading.short_of = if reading.needs_breaker {
                Some(ShortOf::Breaker)
            } else {
                self.capture_short_of(g, pid, reading)
            };
        }
        CaptureVerdict::Hold(best)
    }

    fn capture_reading(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        pool: &[u32],
    ) -> Option<CaptureReading> {
        let city = g.cities.get(&cid)?;
        let walls = g.city_max_wall_hp(city).max(city.wall_hp);
        let open = city.wall_hp <= 0;
        let window = if open { OPEN_WINDOW } else { WALLED_WINDOW };
        let AssaultCorps { members, forecast } =
            self.assault_corps_best(g, pid, cid, pool, raw_for(DECLARE_P, walls), window)?;
        let needs_breaker = !open && !members.iter().any(|uid| opens_walls(g, *uid, cid));
        Some(CaptureReading {
            city: cid,
            pos: city.pos,
            window,
            walls,
            chance: forecast.calibrated_within(window, walls),
            raw: forecast.within(window),
            corps: members.len(),
            pool: pool.len(),
            needs_breaker,
            short_of: None,
        })
    }

    /// The full at-peace reading of the best corps, with its short-of reruns.
    fn capture_short_of(&self, g: &Game, pid: usize, reading: &CaptureReading) -> Option<ShortOf> {
        let ours: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| pool_member(g, pid, *uid))
            .filter(|uid| {
                let reach = (POOL_TURNS * g.unit_max_moves(*uid).max(1.0)).ceil() as i32;
                g.wdist(g.units[uid].pos, reading.pos) <= reach
            })
            .collect();
        self.assault_forecast_at_peace(g, pid, reading.city, &ours)
            .and_then(|forecast| forecast.short_of)
    }

    fn journal_capture_verdict(&self, g: &Game, target: usize, verdict: &CaptureVerdict) {
        if !self.journal().wants(crate::reasoning::Level::Decision) {
            return;
        }
        let civ = g.players[target].civ.clone();
        let name = |cid: u32| {
            g.cities
                .get(&cid)
                .map_or_else(String::new, |c| c.name.clone())
        };
        match verdict {
            CaptureVerdict::Go(reading) => {
                think!(self.journal(), Military, Decision,
                       "The war on {} has its capture", civ;
                       "declaration-needs-a-capture: {} reads {:.0}% within {} turns (calibrated; raw {:.0}%), walls {}, a corps of {} of the {} within reach",
                       name(reading.city), reading.chance * 100.0, reading.window, reading.raw * 100.0,
                       reading.walls, reading.corps, reading.pool;
                       reading.pos);
            }
            CaptureVerdict::Hold(None) => {
                think!(self.journal(), Military, Decision,
                       "Holding the war on {} for a capture", civ;
                       "declaration-needs-a-capture: no soldier of ours stands within {} turns of any of its cities; short of: bodies",
                       POOL_TURNS);
            }
            CaptureVerdict::Hold(Some(reading)) => {
                let short = reading.short_of.map_or("unread", ShortOf::as_str);
                let breaker = if reading.needs_breaker {
                    ", and nothing in the corps opens its walls"
                } else {
                    ""
                };
                think!(self.journal(), Military, Decision,
                       "Holding the war on {} for a capture", civ;
                       "declaration-needs-a-capture: the best city, {}, reads {:.0}% within {} turns (calibrated; raw {:.0}%; needs {:.0}%), walls {}, a corps of {} of the {} within reach{breaker}; short of: {short}",
                       name(reading.city), reading.chance * 100.0, reading.window, reading.raw * 100.0,
                       DECLARE_P * 100.0, reading.walls, reading.corps, reading.pool;
                       reading.pos);
            }
        }
    }
}

#[cfg(test)]
mod tests;
