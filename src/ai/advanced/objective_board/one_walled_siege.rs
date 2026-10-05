//! `one-walled-siege-at-a-time`: one walled Siege row takes the guns.
//!
//! Every walled Siege row asks for its guns (`siege-needs-a-breaker`), and
//! the allocation answered each in rank order with whatever stood nearest.
//! When the plan's target and the campaign's next city are both walled, the
//! top row recruits guns while the other row's members stay with it on
//! hysteresis, and neither train holds enough to open its walls. Live King
//! civvis-20261005T051413Z (game 102): Toronto (walls 391/400) had held one to
//! five guns on its staging ring since turn 126. At turn 146 walled Uruk
//! joined the board above it. From then on Toronto read "0 gun(s) fit", and
//! Uruk did too, with the army split six and ten between them.
//!
//! With the gene on, while any walled Siege row asks for guns, exactly one
//! walled row is chosen to recruit them. The choice prefers the row with the
//! most guns committed to it: guns of ours inside its staging ring
//! ([`STAGING_FAR`]) or in its force from last turn. Ties go to the row last
//! chosen, then to the most staged strength, then to board rank. The other
//! walled rows are *lower*: they recruit no gun, their land force is cut to
//! a screen ([`SCREEN`]), and the campaign surplus and the rams and towers
//! pass them by. A chosen siege that has guns inside its staging ring keeps
//! the guns, and stays chosen, until it falls or stalls. It stalls after
//! [`STALL_READINGS`] readings with a gun inside its ring and the city not
//! pushed to a new low (hit points plus walls), or once the capture ledger's
//! own stall clock fires on it. It is also released when no gun of ours has
//! reached its ring in [`GUNLESS_TURNS`] standard turns while we field one. A
//! released siege sits out the choice for [`RELEASE_TURNS`] standard turns
//! while another walled row asks.

use std::collections::{BTreeMap, BTreeSet};

use super::super::commitments::{Kind, Owner, Target, CAPTURE_STALL_TURNS, STALL_TURNS};
use super::super::siege_train::{BREAKER_WAIT_TURNS, STAGING_FAR};
use super::{AdvancedAi, ForceDomain, ForceNeed, Objective, ObjectiveKey, ObjectiveKind, TaskForce, UnitFacts};
use crate::game::Game;
use crate::think;
use crate::Pos;

/// What a lower walled row's land force may hold: a melee screen, no gun.
pub(super) const SCREEN: ForceNeed = ForceNeed {
    strength: 0.0,
    melee: 1,
    ranged: 0,
    siege: 0,
    bodies: 2,
};
/// Readings with a fit gun of ours inside the chosen ring and the city not
/// pushed to a new low before the siege is released: the capture ledger's
/// own [`STALL_TURNS`] grace plus its [`CAPTURE_STALL_TURNS`] stand-down.
const STALL_READINGS: u32 = STALL_TURNS + CAPTURE_STALL_TURNS;
/// Standard turns a chosen siege may go with no fit gun of ours inside its
/// staging ring, while we field one, before it is released: the wait
/// `siege-needs-a-breaker` gives a breaker on its way.
const GUNLESS_TURNS: u32 = BREAKER_WAIT_TURNS;
/// Standard turns a released siege sits out the choice.
const RELEASE_TURNS: u32 = BREAKER_WAIT_TURNS;

/// The walled siege the guns serve, kept by the city's position (a live
/// rebuild renumbers every city on almost every turn).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ChosenSiege {
    pub(super) at: Pos,
    /// The turn it was chosen.
    since: u32,
    /// The city's lowest hit points plus walls since then.
    low: i32,
    /// Readings since the last new low with a fit gun of ours inside the
    /// staging ring.
    stalled: u32,
    /// The last turn a fit gun of ours stood inside the staging ring; the
    /// choice's own turn until one does.
    gun_seen: u32,
}

/// This turn's choice, for the allocation.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct OneWalledSiege {
    pub(super) chosen: ObjectiveKey,
    /// Every other walled Siege row: a screen each, no gun.
    pub(super) lower: BTreeSet<ObjectiveKey>,
}

impl OneWalledSiege {
    /// What the `domain` force of the row `key` is asked to hold.
    pub(super) fn need(&self, key: ObjectiveKey, domain: ForceDomain, need: ForceNeed) -> ForceNeed {
        if domain == ForceDomain::Land && self.lower.contains(&key) {
            SCREEN
        } else {
            need
        }
    }

    /// Whether a fit gun is kept off `row`: off a lower walled row always,
    /// and off any other Siege row while the chosen row is short of guns.
    pub(super) fn bars_gun(&self, row: &Objective, chosen_short: bool) -> bool {
        self.lower.contains(&row.key)
            || (row.kind == ObjectiveKind::Siege && row.key != self.chosen && chosen_short)
    }

    /// Whether the chosen row's land force holds fewer fit guns than it asks.
    pub(super) fn chosen_short(
        &self,
        rows: &[Objective],
        forces: &[TaskForce],
        facts: &BTreeMap<u32, UnitFacts>,
    ) -> bool {
        let asks = rows
            .iter()
            .find(|row| row.key == self.chosen)
            .map_or(0, |row| row.requirement.siege);
        let holds = forces
            .iter()
            .filter(|force| force.objective_key == self.chosen && force.domain == ForceDomain::Land)
            .flat_map(|force| force.units.iter())
            .filter(|uid| facts.get(uid).is_some_and(|unit| unit.siege))
            .count();
        holds < asks
    }

    /// Cut last turn's land force of every lower walled row to the screen:
    /// its guns go back to the pool for the chosen row, and so does every
    /// body past [`SCREEN`]'s, melee kept first, then the nearest the city.
    pub(super) fn trim_lower_forces(
        &self,
        g: &Game,
        rows: &[Objective],
        facts: &BTreeMap<u32, UnitFacts>,
        forces: &mut [TaskForce],
    ) {
        for force in forces.iter_mut() {
            if force.domain != ForceDomain::Land || !self.lower.contains(&force.objective_key) {
                continue;
            }
            let Some(at) = rows
                .iter()
                .find(|row| row.key == force.objective_key)
                .map(|row| row.at)
            else {
                continue;
            };
            let mut kept: Vec<u32> = force
                .units
                .iter()
                .copied()
                .filter(|uid| facts.get(uid).is_some_and(|unit| !unit.siege))
                .collect();
            kept.sort_by_key(|uid| {
                let unit = &facts[uid];
                (!unit.melee, g.wdist(unit.pos, at), *uid)
            });
            kept.truncate(SCREEN.bodies);
            force.units.retain(|uid| kept.contains(uid));
        }
    }
}

impl AdvancedAi {
    /// `one-walled-siege-at-a-time`: choose this turn's walled siege. `None`
    /// with the gene off, on an arena, or with no walled Siege row asking for
    /// guns. See the module documentation.
    pub(super) fn one_walled_siege(
        &mut self,
        g: &Game,
        rows: &[Objective],
        pool: &[u32],
        facts: &BTreeMap<u32, UnitFacts>,
        forces: &[TaskForce],
    ) -> Option<OneWalledSiege> {
        if !self.one_walled_siege_at_a_time || g.is_arena() {
            return None;
        }
        let turn = g.turn;
        self.objective_board_state
            .one_siege_released
            .retain(|_, until| turn < *until);
        // (rank, key, city, where)
        let walled: Vec<(usize, ObjectiveKey, u32, Pos)> = rows
            .iter()
            .enumerate()
            .filter_map(|(rank, row)| match row.key {
                ObjectiveKey::Siege(cid)
                    if row.kind == ObjectiveKind::Siege && row.land && row.requirement.siege > 0 =>
                {
                    g.cities.get(&cid).map(|city| (rank, row.key, cid, city.pos))
                }
                _ => None,
            })
            .collect();
        if walled.is_empty() {
            self.objective_board_state.one_siege = None;
            return None;
        }
        let guns: Vec<&UnitFacts> = pool
            .iter()
            .filter_map(|uid| facts.get(uid))
            .filter(|unit| unit.siege && unit.domain == ForceDomain::Land)
            .collect();
        let guns_in_ring = |at: Pos| {
            guns.iter()
                .filter(|unit| g.wdist(unit.pos, at) <= STAGING_FAR)
                .count()
        };
        let reading = |cid: u32| g.cities.get(&cid).map_or(0, |city| city.hp + city.wall_hp);
        let name = |cid: u32| g.cities.get(&cid).map_or_else(String::new, |city| city.name.clone());

        // Last turn's choice: read its clock, release it if it stalled.
        let mut released: Option<(u32, Pos, String)> = None;
        let mut previous: Option<ChosenSiege> = None;
        if let Some(mut record) = self.objective_board_state.one_siege.take() {
            if let Some(&(_, _, cid, at)) = walled.iter().find(|walled| walled.3 == record.at) {
                let present = guns_in_ring(at) > 0;
                let now = reading(cid);
                if now < record.low {
                    record.low = now;
                    record.stalled = 0;
                } else if present {
                    record.stalled += 1;
                }
                if present {
                    record.gun_seen = turn;
                }
                let ledger_stalled = self
                    .commitments
                    .open_for(Kind::Capture, Owner::Empire)
                    .is_some_and(|c| {
                        c.target == Target::City(cid) && c.stalled_streak >= CAPTURE_STALL_TURNS
                    });
                let gunless = !guns.is_empty()
                    && turn.saturating_sub(record.gun_seen) > g.standard_duration(GUNLESS_TURNS);
                let why = if record.stalled >= STALL_READINGS || ledger_stalled {
                    Some(format!(
                        "chosen at turn {}, not pushed below {} in {} readings with a gun on its ring",
                        record.since, record.low, record.stalled
                    ))
                } else if gunless {
                    Some(format!(
                        "chosen at turn {}, no gun of ours on its ring since turn {}",
                        record.since, record.gun_seen
                    ))
                } else {
                    None
                };
                match why {
                    Some(why) => {
                        self.objective_board_state
                            .one_siege_released
                            .insert(at, turn + g.standard_duration(RELEASE_TURNS));
                        released = Some((cid, at, why));
                    }
                    None => previous = Some(record),
                }
            }
        }

        // The choice: a previous siege with a gun on its ring holds;
        // otherwise the most guns committed, the previous, the most staged
        // strength, the board's rank.
        let committed = |key: ObjectiveKey, at: Pos| {
            let mut ids: BTreeSet<u32> = pool
                .iter()
                .copied()
                .filter(|uid| {
                    facts.get(uid).is_some_and(|unit| {
                        unit.siege
                            && unit.domain == ForceDomain::Land
                            && g.wdist(unit.pos, at) <= STAGING_FAR
                    })
                })
                .collect();
            for force in forces
                .iter()
                .filter(|force| force.objective_key == key && force.domain == ForceDomain::Land)
            {
                ids.extend(
                    force
                        .units
                        .iter()
                        .copied()
                        .filter(|uid| facts.get(uid).is_some_and(|unit| unit.siege)),
                );
            }
            ids.len()
        };
        let staged = |at: Pos| -> f64 {
            pool.iter()
                .filter_map(|uid| facts.get(uid))
                .filter(|unit| unit.domain == ForceDomain::Land && g.wdist(unit.pos, at) <= STAGING_FAR)
                .map(|unit| unit.strength)
                .sum()
        };
        let held = previous.and_then(|record| {
            walled
                .iter()
                .find(|walled| walled.3 == record.at && guns_in_ring(record.at) > 0)
                .copied()
        });
        let released_now = &self.objective_board_state.one_siege_released;
        let open: Vec<(usize, ObjectiveKey, u32, Pos)> = walled
            .iter()
            .copied()
            .filter(|walled| !released_now.contains_key(&walled.3))
            .collect();
        let field = if open.is_empty() { &walled } else { &open };
        let was = previous.map(|record| record.at);
        let chosen = held.or_else(|| {
            field.iter().copied().max_by(|a, b| {
                committed(a.1, a.3)
                    .cmp(&committed(b.1, b.3))
                    .then((Some(a.3) == was).cmp(&(Some(b.3) == was)))
                    .then(staged(a.3).total_cmp(&staged(b.3)))
                    .then(b.0.cmp(&a.0))
            })
        })?;
        let (_, key, cid, at) = chosen;
        let fresh = was != Some(at);
        let record = match previous {
            Some(record) if !fresh => record,
            _ => ChosenSiege {
                at,
                since: turn,
                low: reading(cid),
                stalled: 0,
                gun_seen: turn,
            },
        };
        self.objective_board_state.one_siege = Some(record);
        let lower: BTreeSet<ObjectiveKey> = walled
            .iter()
            .map(|walled| walled.1)
            .filter(|other| *other != key)
            .collect();

        if let Some((old, old_at, why)) = released {
            think!(self.journal(), Military, Strategy,
                   "Releasing the walled siege of {}", name(old);
                   "{why}; it sits out the choice for {} turns", g.standard_duration(RELEASE_TURNS); old_at);
        }
        if fresh {
            let others: Vec<String> = walled
                .iter()
                .filter(|walled| walled.1 != key)
                .map(|walled| name(walled.2))
                .collect();
            think!(self.journal(), Military, Strategy,
                   "One walled siege at a time: {}", name(cid);
                   "{} gun(s) committed, {:.0} staged on its ring; screened: {}",
                   committed(key, at), staged(at),
                   if others.is_empty() { "none".to_string() } else { others.join(", ") }; at);
        }
        Some(OneWalledSiege { chosen: key, lower })
    }
}
