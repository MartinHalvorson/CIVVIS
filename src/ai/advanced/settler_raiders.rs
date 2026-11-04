//! `settler-founds-under-raiders`: a stalled Settler whose only threats are
//! barbarians founds where it stands.
//!
//! When a Settler's stall counter expires (`SETTLER_STALL_LIMIT` turns
//! without progress), `founds_where_it_stands` asks whether the ground it is
//! on would take a city, and refuses when `settlement_tile_risk` passes
//! `SETTLER_STEP_RISK_LIMIT`. A passing barbarian raider is exactly what
//! stalls a Settler, through the safe-step guard, and also what fails that
//! check, so the Settler retargets and stalls again. A Settler a raider
//! reaches is simply lost; a city is not, unless the raiders in reach can
//! bring it down. Live Civilization VI barbarians do take cities: the Deity
//! seat civvis-20261031T002934Z lost Tarsus (3 pop) at turn 42 and Ray at 68
//! to barbarians, at peace. So a city is the safer piece only where the
//! raiders cannot break it, which is the bar `settler_founds_for_shelter`
//! already prices.
//!
//! Live Immortal Pericles seats of 2026-10-30, 21 games, first 100 turns: a
//! Settler lived a median 8 turns (mean 11.4, 51% at 8 or more, tails of
//! 20-58) before founding. Of about 630 "Settler HELD" journal lines, 571 read
//! "the safe-step guard rejected every neighbour". civvis-20261030T175606Z
//! held one Settler from turn 22 to 55 inside a five-by-three box around
//! (44-49, 9-11), retreating from raiders and keeping a blocked site, while
//! the early city floor wanted four cities. At turn 80 the seats the
//! production-share rule kept held about seven cities; those it retired held
//! about five.
//!
//! The stall counter rarely expires live: a held Settler re-targets every few
//! turns and each retarget resets it. Five of those seats logged 8-90
//! safe-step holds and not one "Founding where the settler stands".
//!
//! With the gene on, the tile-risk refusal in `founds_where_it_stands` is
//! waived when every visible hostile military unit within
//! [`RAIDER_REACH`] tiles is a barbarian's, there is at least one, and the
//! projected city outlasts their upper-roll damage with
//! [`RAIDER_CITY_HP_MARGIN`] of its health to spare
//! ([`AdvancedAi::raiders_spare_the_city`]), and a
//! Settler the safe-step guard holds for a second turn running among
//! barbarians only founds where it stands
//! ([`AdvancedAi::settler_founds_among_raiders`]). The Loyalty verdict, the
//! unsupported-frontier guard and the hidden-border bookkeeping around the
//! founding are unchanged.
use super::*;

/// Tiles within which a visible hostile counts toward the threat.
pub(crate) const RAIDER_REACH: i32 = 3;

/// The raiders' summed upper-roll damage must stay under this share of the
/// projected city's health: the shelter fallback's bar.
pub(crate) const RAIDER_CITY_HP_MARGIN: f64 = 0.75;

impl AdvancedAi {
    /// Whether the threat at `here` is barbarians alone: the gene is on, a
    /// visible hostile military unit (not air) stands within [`RAIDER_REACH`],
    /// and every such unit belongs to a barbarian player.
    pub(super) fn settler_threat_is_raiders_only(
        &self,
        g: &Game,
        pid: usize,
        here: Pos,
        visible: &TileBits,
    ) -> bool {
        if !self.settler_founds_under_raiders {
            return false;
        }
        let mut raiders = 0;
        for unit in g.units.values() {
            if unit.owner == pid
                || !g.is_at_war(pid, unit.owner)
                || !g.sees(visible, unit.pos)
                || !g.unit_visible_to(unit.id, pid)
                || g.wdist(unit.pos, here) > RAIDER_REACH
            {
                continue;
            }
            let spec = &g.rules.units[unit.kind];
            if spec.class != "military" || spec.domain.as_deref() == Some("air") {
                continue;
            }
            if !g
                .players
                .get(unit.owner)
                .is_some_and(|owner| owner.is_barbarian)
            {
                return false;
            }
            raiders += 1;
        }
        raiders > 0
    }

    /// Whether a city founded at `here` would outlast the visible raiders in
    /// reach: the city is projected on a cloned board, and every visible
    /// hostile military unit whose move and range reach `here` adds 1.2 times
    /// its expected damage (at most 100), which must stay under
    /// [`RAIDER_CITY_HP_MARGIN`] of the city's health. The same estimate
    /// `settler_founds_for_shelter` makes.
    pub(super) fn raiders_spare_the_city(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        here: Pos,
        visible: &TileBits,
    ) -> bool {
        let mut projected = g.clone();
        if projected
            .apply(pid, &Action::FoundCity { unit: uid })
            .is_err()
        {
            return false;
        }
        let Some(cid) = projected.city_at(here) else {
            return false;
        };
        let hp = f64::from(projected.cities[&cid].hp);
        let defense = projected.city_strength(cid);
        let mut pressure = 0.0;
        for unit in g.units.values() {
            let spec = &g.rules.units[unit.kind];
            if unit.owner == pid
                || !g.is_at_war(pid, unit.owner)
                || !g.sees(visible, unit.pos)
                || !g.unit_visible_to(unit.id, pid)
                || spec.class != "military"
                || spec.domain.as_deref() == Some("air")
                || g.wdist(here, unit.pos) > spec.moves.ceil() as i32 + spec.range.max(1)
            {
                continue;
            }
            let attack = g
                .unit_strength(unit, false)
                .max(g.unit_ranged_attack_strength(unit));
            pressure += (1.2 * crate::game::expected_damage(attack, defense)).min(100.0);
        }
        pressure < hp * RAIDER_CITY_HP_MARGIN
    }

    /// The safe-step guard's hold, answered: a Settler that has made no
    /// progress for a turn already and whose only visible threats are
    /// barbarians founds where it stands, through the same guarded
    /// `founds_where_it_stands` the stall expiry uses (the tile-risk refusal
    /// there gives way to the same raider test). `false` with the gene off.
    pub(super) fn settler_founds_among_raiders(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> bool {
        if !self.settler_founds_under_raiders {
            return false;
        }
        let Some(here) = g.units.get(&uid).map(|unit| unit.pos) else {
            return false;
        };
        let visible = self.battlefront_visibility(g, pid);
        // Name the reason a held Settler stays a Settler, so a live seat can
        // say which bar it failed (the founding itself is journaled by
        // `founds_where_it_stands`).
        let afloat = g.map.get(here).is_some_and(|tile| g.rules.is_water(tile));
        let declined = if afloat {
            Some("it is afloat")
        } else if !self.settler_threat_is_raiders_only(g, pid, here, &visible) {
            Some("a hostile in reach is not a barbarian's, or no hostile is in sight")
        } else if !g.can_found_city(uid) {
            Some("the ground it holds cannot take a city")
        } else if !self.raiders_spare_the_city(g, pid, uid, here, &visible) {
            Some("the raiders in reach would break the city")
        } else {
            None
        };
        if let Some(why) = declined {
            think!(self.journal(), Expansion, Detail,
                   "Settler stays a settler among raiders at {here:?}";
                   "settler-founds-under-raiders: {why}"; here);
            return false;
        }
        let founded = self.founds_where_it_stands(g, pid, uid, here);
        if !founded {
            think!(self.journal(), Expansion, Detail,
                   "Settler stays a settler among raiders at {here:?}";
                   "settler-founds-under-raiders: the stall fallback declined the site \
                    (its Loyalty, frontier or colony guard)"; here);
        }
        founded
    }
}

#[cfg(test)]
mod tests;
