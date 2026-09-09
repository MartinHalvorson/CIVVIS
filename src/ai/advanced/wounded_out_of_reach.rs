//! `wounded-out-of-reach`: a unit the next blow could remove leaves the reach
//! of whatever can strike it, and a shooter or scout does not end the turn
//! inside a raider's reach without a melee unit beside it.
//!
//! Measured on the 32 live ledger runs of 2026-08-30..09-01 that reached turn
//! 100 (`docs/LIVE_TACTICS.md`, *The evacuation lands*): 461 of our units
//! died in combat, 408 of them to barbarians. 352 were at or below 50 HP
//! when the killing blow landed and 334 had been hit on an earlier turn and
//! left where they stood; 135 were ranged units a melee unit walked onto,
//! and 80 were scouts. The controller's own recovery
//! (`BasicAi::healing_step`, `retreat_step`) already withdraws a unit at or
//! below `withdraw_hp` and evacuates one whose *expected* incoming damage
//! reaches its hit points. What it leaves standing:
//!
//! - a unit in the band between the mean blow and the top of the roll —
//!   `damage()` is `30·e^((att−def)/25)` times a `U(0.8, 1.2)` roll
//!   (`game.rs`), so a unit at 70 HP facing a mean 63 is "safe" to the mean
//!   and dead to the roll;
//! - a shooter or scout at full health with a melee raider two tiles away,
//!   which the recovery does not consider wounded and the attack scan does
//!   not consider its problem;
//! - a unit whose attacker is in the fog: `enemy_attack_envelopes` reads
//!   visible units only, where `barbarian_reach` remembers a raider for
//!   `HOSTILE_MEMORY_TURNS` and projects it from where it was last seen.
//!
//! This step runs ahead of the recovery on the advanced path and answers
//! those three. It triggers only on a tile something can strike, when the
//! unit is under the withdrawal line, or the roll-top total of everything
//! that reaches the tile meets its hit points, or it is an unscreened
//! shooter. It stands down when one attack this turn would kill the last
//! thing that reaches it. It moves only to a strictly better refuge: a tile
//! nothing reaches first, then the least incoming, then a garrison, then (for
//! a shooter) a melee neighbour between it and the threat, then healing,
//! then the nearest own city. Byte-identical with the gene off: the step
//! returns `None` before reading the board.

use super::civilian_safety::{BarbarianReach, HOSTILE_MEMORY_TURNS, REACH_SCAN_RADIUS};
use super::AdvancedAi;
use crate::ai::{AttackEnvelopes, BasicAi, COMBAT_ROLL_MAX};
use crate::game::{Action, ActionFamilies, Game};
use crate::reasoning::plain;
use crate::think;
use crate::Pos;
use std::cmp::Ordering;
use std::collections::BTreeSet;

/// `withdraw_hp`: the line the controller's own recovery uses. Kept as a
/// constant here so this step reads the same line the recovery does without
/// reaching into its weights.
pub(super) const WOUNDED_LINE: f64 = 45.0;

/// One tile the unit could stand on, priced for the withdrawal.
#[derive(Clone, Copy, Debug)]
struct Refuge {
    pos: Pos,
    /// Nothing visible or remembered can strike this tile next turn.
    clear: bool,
    /// The roll-top total of everything visible that reaches it.
    incoming: f64,
    /// Nonpositive distance to the nearest remembered firing envelope's edge.
    /// When none of the reachable tiles escapes it, less negative is better.
    remembered_clearance: i32,
    /// A City Center or Encampment: the blow lands on the district.
    garrison: bool,
    /// A friendly melee unit stands beside it, no farther from the nearest
    /// threat than it is. Always `false` for a unit that is not a shooter,
    /// so it never orders a melee unit's tiles.
    screened: bool,
    healing: i32,
    city_distance: i32,
}

/// A last-seen ranged unit can fire across a shoreline. The civilian capture
/// envelope answers where it can stand, which misses that danger on water.
/// These projections contain only observed positions and static unit rules.
struct RememberedRangedReach(Vec<(Pos, i32)>);

impl RememberedRangedReach {
    fn margin(&self, g: &Game, pos: Pos) -> i32 {
        self.0
            .iter()
            .map(|(from, radius)| g.wdist(*from, pos) - radius)
            .min()
            .unwrap_or(i32::MAX)
    }
}

impl AdvancedAi {
    /// The withdrawal, or `None` when the gene is off, the unit is not a
    /// land or sea combat unit with movement, it is garrisoned, nothing can
    /// strike its tile, none of the three triggers hold, or one attack
    /// would clear the reach.
    pub(super) fn wounded_out_of_reach_step(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !self.wounded_out_of_reach || g.is_arena() {
            return None;
        }
        let unit = g.units.get(&uid)?;
        let spec = &g.rules.units[unit.kind];
        if unit.owner != pid
            || unit.moves_left <= 0.0
            || spec.class != "military"
            || spec.domain.as_deref() == Some("air")
            || (!spec.is_melee_capable() && !spec.has_ranged_attack())
        {
            return None;
        }
        let here = unit.pos;
        let hp = f64::from(unit.hp);
        let kind = unit.kind;
        let shooter = (spec.has_ranged_attack() && !spec.is_melee_capable())
            || spec.promotion_class == "recon";
        if g.city_at(here).is_some() || g.encampment_at(here).is_some() {
            return None;
        }
        let envelopes = self.base.enemy_attack_envelopes(g, pid);
        let raiders = self.barbarian_reach(g, pid, here, REACH_SCAN_RADIUS);
        let threats = Self::threat_positions(g, &envelopes);
        let remembered_fire = self.remembered_ranged_reach(g, pid);
        let holding = self.refuge_at(
            g,
            pid,
            uid,
            here,
            &envelopes,
            &raiders,
            &remembered_fire,
            &threats,
            shooter,
        );
        if holding.clear {
            return None;
        }
        let wounded = hp <= WOUNDED_LINE;
        let roll_top_lethal = holding.incoming >= hp;
        let exposed_shooter = shooter && !holding.screened;
        if !(wounded || roll_top_lethal || exposed_shooter) {
            return None;
        }
        if self.attack_clears_the_reach(g, pid, uid) {
            return None;
        }
        let why = if roll_top_lethal {
            "the top of the roll on everything that reaches its tile meets its hit points"
        } else if wounded {
            "it is under the withdrawal line on a tile a hostile can strike"
        } else {
            "it is a shooter with no melee unit beside it inside a hostile's reach"
        };
        let best = g
            .reachable(uid)
            .into_iter()
            .filter(|pos| *pos != here && g.can_stop(uid, *pos))
            .map(|pos| {
                self.refuge_at(
                    g,
                    pid,
                    uid,
                    pos,
                    &envelopes,
                    &raiders,
                    &remembered_fire,
                    &threats,
                    shooter,
                )
            })
            .max_by(Self::refuge_cmp);
        match best {
            Some(best) if Self::refuge_cmp(&best, &holding).is_gt() => {
                think!(self.journal(), Military, Detail, "{} steps out of reach", plain(&kind);
                       "{why}; {here:?} takes {:.0} at the top of the roll, {:?} {}",
                       holding.incoming, best.pos,
                       if best.clear { "is out of every reach it can see or remembers" }
                       else { "is the least exposed tile it can reach" };
                       best.pos);
                let moved = self.base.move_to_evacuation_tile(g, pid, uid, best.pos);
                Some(moved || self.base.fortify_or_stop(g, pid, uid))
            }
            _ => {
                think!(self.journal(), Military, Detail, "{} holds inside a hostile's reach", plain(&kind);
                       "{why}, and no tile it can reach is better than {here:?}";
                       here);
                Some(self.base.fortify_or_stop(g, pid, uid))
            }
        }
    }

    /// Where every visible unit that owns an attack envelope stands.
    fn threat_positions(g: &Game, envelopes: &AttackEnvelopes) -> Vec<Pos> {
        envelopes
            .iter()
            .filter_map(|(enemy, _)| g.units.get(enemy).map(|unit| unit.pos))
            .collect()
    }

    /// Hex distance from `pos` to the nearest thing that could strike it:
    /// a visible envelope owner or a remembered raider.
    fn threat_distance(g: &Game, pos: Pos, threats: &[Pos], raiders: &BarbarianReach) -> i32 {
        threats
            .iter()
            .map(|threat| g.wdist(*threat, pos))
            .min()
            .unwrap_or(i32::MAX)
            .min(raiders.nearest(g, pos))
    }

    /// A friendly melee unit stands beside `pos`, no farther from the nearest
    /// threat than `pos` is: a raider walking onto the shooter walks into it.
    fn shooter_screened(
        g: &Game,
        pid: usize,
        pos: Pos,
        threats: &[Pos],
        raiders: &BarbarianReach,
    ) -> bool {
        let own = Self::threat_distance(g, pos, threats, raiders);
        if own == i32::MAX {
            return false;
        }
        g.nbrs(pos).into_iter().any(|neighbour| {
            g.unit_ids_at(neighbour).iter().any(|other| {
                let other = &g.units[other];
                let spec = &g.rules.units[other.kind];
                other.owner == pid
                    && spec.class == "military"
                    && spec.is_melee_capable()
                    && Self::threat_distance(g, neighbour, threats, raiders) <= own
            })
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn refuge_at(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        pos: Pos,
        envelopes: &AttackEnvelopes,
        raiders: &BarbarianReach,
        remembered_fire: &RememberedRangedReach,
        threats: &[Pos],
        shooter: bool,
    ) -> Refuge {
        let garrison = g.city_at(pos).is_some() || g.encampment_at(pos).is_some();
        let incoming =
            BasicAi::incoming_damage(g, pid, uid, pos, envelopes).total * COMBAT_ROLL_MAX;
        let remembered_margin = remembered_fire.margin(g, pos);
        let clear = incoming <= 1e-9 && !raiders.covers(g, pos) && remembered_margin > 0;
        let city_distance = g
            .cities
            .values()
            .filter(|city| city.owner == pid)
            .map(|city| g.wdist(city.pos, pos))
            .min()
            .unwrap_or(i32::MAX);
        Refuge {
            pos,
            clear,
            incoming,
            remembered_clearance: remembered_margin.min(0),
            garrison,
            screened: shooter && Self::shooter_screened(g, pid, pos, threats, raiders),
            healing: g.healing_location(pid, pos).rate(),
            city_distance,
        }
    }

    /// Greater is the better refuge.
    fn refuge_cmp(left: &Refuge, right: &Refuge) -> Ordering {
        left.clear
            .cmp(&right.clear)
            .then_with(|| right.incoming.total_cmp(&left.incoming))
            .then(left.garrison.cmp(&right.garrison))
            .then(left.screened.cmp(&right.screened))
            .then(left.remembered_clearance.cmp(&right.remembered_clearance))
            .then(left.healing.cmp(&right.healing))
            .then(right.city_distance.cmp(&left.city_distance))
            .then_with(|| right.pos.cmp(&left.pos))
    }

    /// One attack this turn kills the last thing that could reach the unit,
    /// visible or remembered, and the unit survives it: the attack scan's
    /// decision, not a withdrawal.
    fn attack_clears_the_reach(&self, g: &Game, pid: usize, uid: u32) -> bool {
        g.legal_actions_within(pid, ActionFamilies::UNITS)
            .into_iter()
            .filter(|action| {
                matches!(
                    action,
                    Action::Attack { unit, .. }
                        | Action::Ranged { unit, .. }
                        | Action::PriorityTarget { unit, .. }
                        if *unit == uid
                )
            })
            .any(|action| {
                let mut future = g.clone();
                if future.apply(pid, &action).is_err() {
                    return false;
                }
                let Some(survivor) = future.units.get(&uid) else {
                    return false;
                };
                let after = survivor.pos;
                let envelopes = self.base.enemy_attack_envelopes(&future, pid);
                !BasicAi::anything_can_reach(&future, pid, after, &envelopes)
                    && self
                        .remembered_ranged_reach(&future, pid)
                        .margin(&future, after)
                        > 0
                    && !self
                        .barbarian_reach(&future, pid, after, REACH_SCAN_RADIUS)
                        .covers(&future, after)
            })
    }
    /// Keep the capture memory's four-turn lifetime and one extra hex of
    /// uncertainty per elapsed turn, then add the gun's firing range. Do not
    /// read a hidden unit's current position, HP, or promotions. Visible units
    /// are handled by exact attack envelopes rather than counted twice here.
    fn remembered_ranged_reach(&self, g: &Game, pid: usize) -> RememberedRangedReach {
        if !(self.hostile_memory || self.hostile_memory_2 || self.live_settler_capture_lessons) {
            return RememberedRangedReach(Vec::new());
        }
        let visible = self.battlefront_visibility(g, pid);
        let current: BTreeSet<i64> = g
            .units
            .values()
            .filter(|unit| {
                unit.owner != pid
                    && g.is_at_war(pid, unit.owner)
                    && g.sees(&visible, unit.pos)
                    && g.unit_visible_to(unit.id, pid)
            })
            .map(|unit| super::hostile_memory_key(g, unit))
            .collect();
        let projections = self
            .hostile_last_seen
            .iter()
            .filter_map(|(key, record)| {
                if current.contains(key)
                    || record.when > g.turn
                    || g.turn - record.when > HOSTILE_MEMORY_TURNS
                    || record.owner >= g.players.len()
                    || record.owner == pid
                    || !g.is_at_war(pid, record.owner)
                {
                    return None;
                }
                let spec = &g.rules.units[record.kind];
                if spec.class != "military"
                    || !spec.has_ranged_attack()
                    || spec.domain.as_deref() == Some("air")
                {
                    return None;
                }
                let radius = spec.moves.ceil() as i32 + (g.turn - record.when) as i32 + spec.range;
                Some((record.pos, radius.max(1)))
            })
            .collect();
        RememberedRangedReach(projections)
    }
}
