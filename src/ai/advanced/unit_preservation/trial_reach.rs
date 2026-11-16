//! A retreat's trial stands share the enemy reaches the move cannot touch.
//!
//! `preserve_unit_actions` picks a withdrawn unit's stand by trying each one:
//! it copies the board, moves the unit (and a companion Settler) there, and
//! forecasts the replies. Each forecast flooded every visible hostile twice
//! over — its strike reach, then its strike reach again from every tile it
//! can reach (`DangerField::second_turn`) — and nothing carried from one
//! trial to the next, although a trial differs from the board it was taken
//! from only where our units moved. On a late production-genome screen that
//! was 9% of all samples (`preserve_unit_actions` 15.5%, `strike_reach_of`
//! 11.4%).
//!
//! [`TrialReach`] reads each hostile once on the loop's starting board and
//! records what the answer read: every tile its floods relaxed, and how far
//! past them its attack looks. A trial keeps the answer unless something it
//! read changed:
//!
//! - **a unit that differs** between the boards (moved, added, removed, or
//!   any field) on or next to a relaxed tile. The floods read other units
//!   only there: stacking on the tile itself, and from the ring around it,
//!   zone of control and the hostile's own support units. A melee target is a
//!   stand's neighbour, and its test reads only that tile. Units do not block
//!   line of sight.
//! - **a map write** (`TileGrid::writes_since`) within the farthest relaxed
//!   tile plus the attack range and one: tile costs, borders, cliffs and
//!   rivers along the floods and one step beyond, and line of sight out to
//!   the range — the radius the per-enemy envelopes use (#5361), one wider.
//! - **anything global**: the turn, the belligerence or city fingerprint the
//!   envelopes key on, a fighter's patrol (it blocks entry anywhere), or a map
//!   history that cannot be followed back. Then every hostile is read fresh.
//!
//! The second turn is kept by piece rather than whole (see [`SecondTurn`]):
//! where the hostile can stand on a fresh turn, and its strike reach from each
//! stand. A whole two-turn reach covers so much ground that a retreat near the
//! front touched it on nearly every trial; each piece reads a small patch, and
//! a trial re-reads only the pieces whose patch it touched. A hostile that
//! itself differs between the boards keeps no piece.
//!
//! Air units are always read fresh: their disk follows the operation origin,
//! not a flood. Nothing is kept on a mirrored board, whose strike reach is
//! the host's. `CIVVIS_TRIAL_REACH_AUDIT=1` re-reads every kept answer and
//! reports any that differ, which is how a missing read gets named.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use super::super::battle_planner::{
    fresh_turn_stands, hostile_in_field, mirrored_board, strike_reach_from, strike_reach_of,
    strike_reach_reading, widen_to_the_second_turn, DangerField,
};
use crate::ai::BasicAi;
use crate::game::Game;
use crate::Pos;

fn audit_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("CIVVIS_TRIAL_REACH_AUDIT").is_some())
}

/// What one reading of a hostile read off the board.
#[derive(Clone)]
struct Reads {
    /// Every relaxed tile and the ring around it, ascending: where a unit
    /// that differs can change the answer.
    tiles: Vec<Pos>,
    centre: Pos,
    /// How far from `centre` a map write can change the answer.
    radius: i32,
}

impl Reads {
    fn of(g: &Game, centre: Pos, relaxed: &[Pos], range: i32) -> Self {
        let farthest = relaxed
            .iter()
            .map(|tile| g.wdist(centre, *tile))
            .max()
            .unwrap_or(0);
        let mut tiles = Vec::with_capacity(relaxed.len() * 7);
        for tile in relaxed {
            tiles.push(*tile);
            tiles.extend(g.nbrs(*tile));
        }
        tiles.sort_unstable();
        tiles.dedup();
        Reads {
            tiles,
            centre,
            radius: farthest + range.max(1) + 1,
        }
    }

    fn hold(&self, g: &Game, change: &Change) -> bool {
        change
            .units
            .iter()
            .all(|tile| self.tiles.binary_search(tile).is_err())
            && change
                .written
                .iter()
                .all(|tile| g.wdist(*tile, self.centre) > self.radius)
    }
}

/// How a trial board differs from the base board.
struct Change {
    /// Where a unit that differs stood on either board, ascending.
    units: Vec<Pos>,
    /// Every tile written since the base board, ascending.
    written: Vec<Pos>,
}

/// One hostile, read on the base board.
struct Hostile {
    /// `Game::attack_reach` and `strike_reach_of`: what `with_reach` reads.
    flood: Vec<Pos>,
    strike: Vec<Pos>,
    first: Reads,
    /// Its second turn, by piece. Filled the first time a trial asks.
    second: Option<SecondTurn>,
}

/// One hostile's second turn on the base board, kept by piece, because a
/// retreat trial moves one of our units and touches few of them: where the
/// hostile can stand on a fresh turn, and its strike reach from each of those
/// stands, each beside what it read. A hostile near the front sees a trial
/// touch some of its stands every time, and the rest are kept.
struct SecondTurn {
    /// `fresh_turn_stands` and what that flood read.
    stands: Vec<Pos>,
    stands_read: Reads,
    /// `strike_reach_from` each stand, and what it read.
    from: BTreeMap<Pos, (Vec<Pos>, Reads)>,
}

/// The hostiles' reaches on the board a retreat loop starts from, kept for
/// the trial boards it takes from that board. See the module header.
pub(super) struct TrialReach {
    pid: usize,
    base: Game,
    /// `base`, for standing hostiles on their second-turn stands.
    probe: Game,
    stamp: (u32, u64, u64),
    chain: u64,
    keep: bool,
    /// `None` for a hostile that is read fresh on every trial (an air unit,
    /// or one the base board does not hold).
    hostiles: BTreeMap<u32, Option<Hostile>>,
    /// How many hostile reaches a trial kept (first and second turn).
    #[cfg(test)]
    pub(super) kept: usize,
}

impl TrialReach {
    pub(super) fn new(g: &Game, pid: usize) -> Self {
        Self {
            pid,
            base: g.speculative_clone(),
            probe: g.speculative_clone(),
            stamp: Self::stamp(g),
            chain: g.map.tiles.write_chain(),
            keep: !mirrored_board(g, pid),
            hostiles: BTreeMap::new(),
            #[cfg(test)]
            kept: 0,
        }
    }

    fn stamp(g: &Game) -> (u32, u64, u64) {
        (
            g.turn,
            BasicAi::envelope_belligerence_fingerprint(g),
            BasicAi::envelope_city_fingerprint(g),
        )
    }

    /// How `trial` differs from the base board, or `None` when that cannot be
    /// told or the difference can reach anything.
    fn change(&self, trial: &Game) -> Option<Change> {
        if !self.keep || Self::stamp(trial) != self.stamp {
            return None;
        }
        let mut written = trial.map.tiles.writes_since(self.chain)?;
        written.sort_unstable();
        written.dedup();
        let mut units = Vec::new();
        let mut matched = 0usize;
        for (id, now) in trial.units.iter() {
            match self.base.units.get(id) {
                Some(was) => {
                    matched += 1;
                    if std::ptr::eq(was, now) || was == now {
                        continue;
                    }
                    if was.air_patrol || now.air_patrol {
                        return None;
                    }
                    units.push(was.pos);
                    units.push(now.pos);
                }
                None => {
                    if now.air_patrol {
                        return None;
                    }
                    units.push(now.pos);
                }
            }
        }
        if matched != self.base.units.len() {
            for (id, was) in self.base.units.iter() {
                if !trial.units.contains_key(id) {
                    if was.air_patrol {
                        return None;
                    }
                    units.push(was.pos);
                }
            }
        }
        units.sort_unstable();
        units.dedup();
        Some(Change { units, written })
    }

    /// One hostile's first-turn reaches on the base board, read once.
    fn hostile(&mut self, id: u32) -> Option<&mut Hostile> {
        if !self.hostiles.contains_key(&id) {
            let read = self.read(id);
            self.hostiles.insert(id, read);
        }
        self.hostiles.get_mut(&id).and_then(Option::as_mut)
    }

    fn read(&self, id: u32) -> Option<Hostile> {
        let g = &self.base;
        let unit = g.units.get(&id)?;
        if g.rules.units[unit.kind].domain.as_deref() == Some("air") {
            return None;
        }
        let (flood, mut relaxed) = g.attack_reach_from_flood(id);
        relaxed.push(unit.pos);
        let strike = strike_reach_reading(g, self.pid, id, Some(&mut relaxed));
        Some(Hostile {
            flood,
            strike,
            first: Reads::of(g, unit.pos, &relaxed, g.unit_attack_range(id)),
            second: None,
        })
    }

    /// One hostile's second turn on the base board, read once, by piece.
    fn second(&mut self, id: u32) -> Option<&SecondTurn> {
        self.hostile(id)?;
        let Self {
            pid,
            base,
            probe,
            hostiles,
            ..
        } = self;
        let hostile = hostiles.get_mut(&id)?.as_mut()?;
        if hostile.second.is_none() {
            let centre = base.units[&id].pos;
            let range = base.unit_attack_range(id);
            let mut relaxed = Vec::new();
            let stands = fresh_turn_stands(probe, id, Some(&mut relaxed));
            let stands_read = Reads::of(base, centre, &relaxed, range);
            let from = stands
                .iter()
                .map(|stand| {
                    let mut read = Vec::new();
                    let strike = strike_reach_from(probe, *pid, id, *stand, Some(&mut read));
                    (*stand, (strike, Reads::of(base, *stand, &read, range)))
                })
                .collect();
            hostile.second = Some(SecondTurn {
                stands,
                stands_read,
                from,
            });
        }
        hostile.second.as_ref()
    }

    /// `DangerField::with_reach(trial, pid, true)`, keeping each hostile's
    /// reaches where the trial cannot have changed them.
    pub(super) fn first_field(&mut self, trial: &Game) -> DangerField {
        let pid = self.pid;
        let change = self.change(trial);
        let ids: Vec<u32> = trial
            .units
            .values()
            .filter(|unit| hostile_in_field(trial, pid, unit))
            .map(|unit| unit.id)
            .collect();
        let mut hostiles = Vec::with_capacity(ids.len());
        for id in ids {
            let kept = change.as_ref().and_then(|change| {
                let hostile = self.hostile(id)?;
                hostile
                    .first
                    .hold(trial, change)
                    .then(|| (hostile.flood.clone(), hostile.strike.clone()))
            });
            let fresh = || (trial.attack_reach(id), strike_reach_of(trial, pid, id));
            let (flood, strike) = match kept {
                Some(kept) if audit_enabled() => {
                    let fresh = fresh();
                    if fresh != kept {
                        eprintln!(
                            "trial-reach: STALE first-turn reach, seat {pid} hostile {id} turn {}: \
                             kept flood {} strike {}, fresh flood {} strike {}",
                            trial.turn,
                            kept.0.len(),
                            kept.1.len(),
                            fresh.0.len(),
                            fresh.1.len()
                        );
                    }
                    fresh
                }
                Some(kept) => {
                    #[cfg(test)]
                    {
                        self.kept += 1;
                    }
                    kept
                }
                None => fresh(),
            };
            hostiles.push((id, flood, Some(strike)));
        }
        DangerField::from_hostiles(trial, pid, hostiles)
    }

    /// `DangerField::second_turn(trial, pid)`, from `first` (this trial's
    /// [`Self::first_field`]), keeping each hostile's second-turn reach where
    /// the trial cannot have changed it.
    pub(super) fn second_field(&mut self, first: &DangerField, trial: &Game) -> DangerField {
        let pid = self.pid;
        let change = self.change(trial);
        let mut probe: Option<Game> = None;
        DangerField::second_turn_from(first, trial, |id, reach| {
            // The pieces are read around each stand, not around the hostile,
            // so a hostile that itself differs keeps none of them.
            let unchanged = match (trial.units.get(&id), self.base.units.get(&id)) {
                (Some(now), Some(was)) => std::ptr::eq(now, was) || now == was,
                _ => false,
            };
            let kept = match change.as_ref().filter(|_| unchanged) {
                Some(change) => self
                    .second(id)
                    .map(|second| assemble(second, change, trial, &mut probe, pid, id, reach)),
                None => None,
            };
            match kept {
                Some((widened, _kept)) if audit_enabled() => {
                    widen_to_the_second_turn(trial_probe(&mut probe, trial), pid, id, reach);
                    if *reach != widened {
                        eprintln!(
                            "trial-reach: STALE second-turn reach, seat {pid} hostile {id} turn {}: \
                             kept {} tiles, fresh {}",
                            trial.turn,
                            widened.len(),
                            reach.len()
                        );
                    }
                }
                Some((widened, _kept)) => {
                    #[cfg(test)]
                    {
                        self.kept += _kept;
                    }
                    *reach = widened;
                }
                None => widen_to_the_second_turn(trial_probe(&mut probe, trial), pid, id, reach),
            }
        })
    }
}

/// The trial's own copy for whatever must be read fresh on it, made once.
fn trial_probe<'p>(probe: &'p mut Option<Game>, trial: &Game) -> &'p mut Game {
    probe.get_or_insert_with(|| trial.speculative_clone())
}

/// `widen_to_the_second_turn(trial, ..)` from `reach`, assembled from the
/// base board's pieces that the trial left alone and fresh reads of the rest:
/// the stands, if their flood held, and each stand's strike reach, if its read
/// held. Returns the widened reach and how many pieces were kept.
fn assemble(
    second: &SecondTurn,
    change: &Change,
    trial: &Game,
    probe: &mut Option<Game>,
    pid: usize,
    id: u32,
    reach: &[Pos],
) -> (Vec<Pos>, usize) {
    let fresh_stands;
    let stands: &[Pos] = if second.stands_read.hold(trial, change) {
        &second.stands
    } else {
        fresh_stands = fresh_turn_stands(trial_probe(probe, trial), id, None);
        &fresh_stands
    };
    let mut kept = usize::from(std::ptr::eq(stands, second.stands.as_slice()));
    let mut widened = reach.to_vec();
    for stand in stands {
        match second
            .from
            .get(stand)
            .filter(|(_, read)| read.hold(trial, change))
        {
            Some((strike, _)) => {
                kept += 1;
                widened.extend_from_slice(strike);
            }
            None => widened.extend(strike_reach_from(
                trial_probe(probe, trial),
                pid,
                id,
                *stand,
                None,
            )),
        }
    }
    widened.sort_unstable();
    widened.dedup();
    (widened, kept)
}
