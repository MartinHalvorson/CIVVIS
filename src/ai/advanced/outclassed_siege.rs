//! `outclassed-siege-stands-down`: a Siege row whose city is held by field
//! units that destroy most of our army with one blow leaves the Objective
//! Board, and the army fights at home instead of feeding its stagers in.
//!
//! Live Immortal Standard `civvis-20261104T012627Z` (Gran Colombia): the
//! Siege of Chucuito sat in "stage, 0 of 16 staged" from turn 222 to 309.
//! Over those turns the army went from 16 to 5. About fifty units died in
//! combat, 44 of the 55 losses HEALTHY land units, each removed by one blow
//! of an AT Crew, a Cuirassier or a Giant Death Robot against our Crossbows,
//! Knights, Bombards and Field Cannons. The board read the war as winnable
//! throughout: `Game::military_power` sums strength times health, so sixteen
//! obsolete units read "743 against their 579". We offered peace 55 times
//! and were refused every time, so the war could not be ended either; only
//! the army's posture was ours to choose. The Siege bill prices the city's
//! own defence, not the units in the field around it, which are the ones
//! that killed the stagers. Live 224444Z (Johannesburg, a city-state harvest
//! target) stalled on the same era gap.
//!
//! The test (`field_outclass`) is plain arithmetic on the engine's own
//! strengths. A blow's damage is `30·e^((A−D)/25)` at its centre roll and
//! the upper roll is `COMBAT_ROLL_MAX` = 1.2 times that, so it removes a
//! full-health unit once `A − D ≥ 25·ln(100/36) ≈ 25.5`. The strongest blow
//! among the city owner's visible land units within [`FIELD_RADIUS`] of the
//! city (melee strength, or ranged strength for a shooter) is set against
//! each of our land military units' defence; when at least half of them sit
//! [`ONE_BLOW_GAP`] or more below it, the field outclasses the army.
//!
//! One reading is not enough: across 126 live siege episodes (11-03/04) a
//! single outclassed reading fell in 69 of them, 7 of our 13 captures among
//! those 69. Once the field has outclassed the army on [`STREAK_TURNS`]
//! consecutive turns (26 episodes, 2 of the 13 captures, and 167 of our
//! units destroyed within six tiles of those cities after that point), the
//! Siege row is left out of the board, and it stays out for
//! [`STAND_DOWN_TURNS`] turns after the last outclassed reading, so the army
//! does not march back the turn the outclassing units fall out of sight. A
//! reading that sees defenders and no longer outclasses the army lifts it at
//! once. Both counts are tactical, turn-by-turn evidence, so they are not
//! scaled by game speed. The memory is keyed by the city's tile because live
//! city ids shift between frames. A stood-down city still covers its neighbourhood against Destroy
//! rows, so the units around it do not draw the army out as a field fight
//! instead. A falling city (walls gone, at most half its health) is never
//! stood down, nor is a city on an arena.
//!
//! `field_outclasses` is the same reading for any caller that names a target
//! (`city-state-harvest` will skip an outclassed city-state with it).
//! `Kind::OptIn`; with the gene off nothing is read or remembered.
use super::AdvancedAi;
use crate::game::Game;
use crate::Pos;

/// The strength gap at which a blow's upper roll removes a full-health
/// unit: `30 · 1.2 · e^(gap/25) ≥ 100` ⇔ `gap ≥ 25.5`.
pub(crate) const ONE_BLOW_GAP: f64 = 25.0;

/// How far from the city the defenders' field is read: the city tile and
/// the rings our stagers stand on.
pub(crate) const FIELD_RADIUS: i32 = 4;

/// Consecutive outclassed turns before a siege stands down.
pub(crate) const STREAK_TURNS: u32 = 4;

/// How long a stood-down siege stays down after its last outclassed reading.
pub(crate) const STAND_DOWN_TURNS: u32 = 10;

/// A falling city's health ceiling: walls gone and at most half of its 200.
const FALLING_CITY_HP: i32 = 100;

/// The field around one city, read against our army.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FieldOutclass {
    /// The strongest blow among the owner's visible land units near the city.
    pub(crate) their_best: f64,
    /// Our land military units read.
    pub(crate) ours: usize,
    /// Of those, the units whose defence sits `ONE_BLOW_GAP` or more below
    /// `their_best`.
    pub(crate) one_blow: usize,
}

impl FieldOutclass {
    /// Half or more of the army is one blow from removal.
    pub(crate) fn outclassed(&self) -> bool {
        self.ours > 0 && self.one_blow * 2 >= self.ours
    }
}

fn land_military(g: &Game, uid: u32) -> bool {
    let unit = &g.units[&uid];
    let spec = &g.rules.units[unit.kind];
    spec.class == "military"
        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
        && (spec.is_melee_capable() || spec.has_ranged_attack())
        && !g.is_embarked(unit)
}

/// The strongest visible blow of `owner`'s land units within
/// [`FIELD_RADIUS`] of `near`, against the defence of each of `ours` (our
/// land military units among them; the rest are ignored). `None` when the
/// owner shows no such unit there.
pub(crate) fn field_outclass(
    g: &Game,
    pid: usize,
    owner: usize,
    near: Pos,
    ours: &[u32],
) -> Option<FieldOutclass> {
    let their_best = g
        .units
        .values()
        .filter(|unit| {
            unit.owner == owner
                && g.wdist(unit.pos, near) <= FIELD_RADIUS
                && land_military(g, unit.id)
                && g.unit_visible_to(unit.id, pid)
        })
        .map(|unit| {
            let spec = &g.rules.units[unit.kind];
            let melee = if spec.is_melee_capable() {
                g.unit_strength(unit, false)
            } else {
                0.0
            };
            let ranged = if spec.has_ranged_attack() {
                g.unit_ranged_strength(unit)
            } else {
                0.0
            };
            melee.max(ranged)
        })
        .fold(None, |best: Option<f64>, blow| {
            Some(best.map_or(blow, |best| best.max(blow)))
        })?;
    let mut read = FieldOutclass {
        their_best,
        ours: 0,
        one_blow: 0,
    };
    for uid in ours {
        if !g.units.get(uid).is_some_and(|unit| unit.owner == pid) || !land_military(g, *uid) {
            continue;
        }
        read.ours += 1;
        if g.unit_strength(&g.units[uid], true) + ONE_BLOW_GAP <= their_best {
            read.one_blow += 1;
        }
    }
    Some(read)
}

/// The same reading against every land military unit of ours: whether the
/// owner's visible units near `near` remove most of our army with one blow.
/// The shared test for a caller that names a target (`city-state-harvest`).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn field_outclasses(g: &Game, pid: usize, owner: usize, near: Pos) -> bool {
    let ours: Vec<u32> = g.player_unit_ids(pid);
    field_outclass(g, pid, owner, near, &ours).is_some_and(|read| read.outclassed())
}

impl AdvancedAi {
    /// Whether the Siege row on `cid` stands down this assessment. Reads the
    /// field with `pool` (the board's available units) as our army and keeps
    /// the city's outclassed streak by its tile: down once the streak reaches
    /// [`STREAK_TURNS`], and for [`STAND_DOWN_TURNS`] after its last turn;
    /// lifted by a reading that sees defenders and is not outclassed. False
    /// with the gene off, on an arena and for a falling city.
    pub(super) fn siege_stands_down(
        &mut self,
        g: &Game,
        pid: usize,
        cid: u32,
        pool: &[u32],
    ) -> bool {
        if !self.outclassed_siege_stands_down || g.is_arena() {
            return false;
        }
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        if city.wall_hp <= 0 && city.hp <= FALLING_CITY_HP {
            self.outclassed_sieges.remove(&city.pos);
            return false;
        }
        let (pos, owner, name) = (city.pos, city.owner, city.name.clone());
        let turn = g.turn;
        match field_outclass(g, pid, owner, pos, pool) {
            Some(read) if read.outclassed() => {
                let (start, last) = match self.outclassed_sieges.get(&pos) {
                    Some(&(start, last)) if turn <= last + 1 => (start, last),
                    _ => (turn, turn),
                };
                self.outclassed_sieges.insert(pos, (start, turn));
                let was_down = (last + 1).saturating_sub(start) >= STREAK_TURNS;
                let down = (turn + 1).saturating_sub(start) >= STREAK_TURNS;
                if down && !was_down {
                    crate::think!(self.journal(), Military, Decision,
                        "Siege of {} stands down: its field outclasses the army", name;
                        "for {} turns running the strongest visible blow near the city ({:.0}) has sat {:.0} or more over the defence of {} of our {} land units, one blow from removal",
                        (turn + 1).saturating_sub(start), read.their_best, ONE_BLOW_GAP, read.one_blow, read.ours;
                        pos);
                }
                down
            }
            Some(_) => {
                self.outclassed_sieges.remove(&pos);
                false
            }
            None => {
                let down = self.siege_stood_down_at(g, pos);
                if self
                    .outclassed_sieges
                    .get(&pos)
                    .is_some_and(|&(_, last)| turn.saturating_sub(last) >= STAND_DOWN_TURNS)
                {
                    self.outclassed_sieges.remove(&pos);
                }
                down
            }
        }
    }

    /// Whether the city at `pos` stands down: its outclassed streak reached
    /// [`STREAK_TURNS`] and its last outclassed turn is under
    /// [`STAND_DOWN_TURNS`] ago. The siege train does not muster for it.
    /// False with the gene off.
    pub(super) fn siege_stood_down_at(&self, g: &Game, pos: Pos) -> bool {
        self.outclassed_siege_stands_down
            && self
                .outclassed_sieges
                .get(&pos)
                .is_some_and(|&(start, last)| {
                    (last + 1).saturating_sub(start) >= STREAK_TURNS
                        && g.turn.saturating_sub(last) < STAND_DOWN_TURNS
                })
    }
}

#[cfg(test)]
mod tests;
