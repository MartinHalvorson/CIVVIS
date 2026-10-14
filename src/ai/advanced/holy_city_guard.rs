//! `founder-holds-the-holy-city`: a founder keeps its Holy City on its own
//! faith. While a rival faith closes on it, our nearest spreader spends a
//! charge there before walking anywhere else, and with no spreader near, the
//! Holy City buys one ahead of the Apostle save.
//!
//! The Holy City is the founder's source: our Missionaries, Apostles and
//! Inquisitors take their purchase city's majority, and the Inquisition needs
//! a Temple city on our faith. Over the 83 live Emperor founder games of
//! October 9-10 (pin d014bd006 and before), the Holy City left our faith in
//! 40; five of the seven Religious defeats lost it first, 27 to 51 turns
//! before the end (civvis-20261010T183646Z t60, 082733Z t69, 094619Z t76,
//! 20261009T215527Z t103, 184140Z t120), against two of the 43 founders that
//! kept it. The flip is rarely seen coming by the shipped warning: the mirror
//! raises a conversion only when the host's clock reads 20 turns or fewer, and
//! the clock reads hundreds of turns until the flip (183646Z: Guayaquil, pop 4,
//! "next" Zoroastrianism at 501 turns on t57 and 359 on t59, Zoroastrian on
//! t61). What the host does show is the faith it names as next: 29 of the 40
//! flips had a rival named in the 15 turns before, and 249 of the 948 Holy
//! City turns showing one flipped within 15. In 183646Z our only Missionary
//! was bought in Guayaquil on t57, stood beside it on t58 and spent its
//! charges in a neighbouring town; Guayaquil flipped two turns later and with
//! it every purchase of our faith.
//!
//! Under the gene, while the Holy City is ours, still on our faith, and a
//! rival faith was named as its next majority (or it needs support by the
//! shipped pressure test) within [`HOLY_CITY_WARNING_STANDARD`] standard
//! turns:
//! - a charged spreader of our faith within [`HOLY_CITY_GUARD_REACH`] tiles
//!   spreads in the Holy City (stepping back to it first), at most once every
//!   [`HOLY_CITY_SPREAD_GAP_STANDARD`] standard turns;
//! - with no such spreader, the Holy City buys a Missionary of our faith before
//!   the Apostle save and the rest of the religious spending, when the bank
//!   covers it.
//!
//! Off: unchanged.

use super::*;

/// Standard turns a rival faith named as the Holy City's next majority keeps
/// the guard up after the host stops naming it: the host's next faith comes and
/// goes from one turn to the next (183646Z: named t57, gone t58, named t59).
pub(crate) const HOLY_CITY_WARNING_STANDARD: u32 = 10;

/// Standard turns between two of our spreads into the Holy City, so one warning
/// does not pour a Missionary's every charge into one city.
pub(crate) const HOLY_CITY_SPREAD_GAP_STANDARD: u32 = 6;

/// The farthest a spreader is called back to the Holy City from, and the radius
/// within which one counts as cover against buying another.
pub(crate) const HOLY_CITY_GUARD_REACH: i32 = 8;

/// The gene's memory, by the Holy City's position (live city ids churn).
#[derive(Clone, Debug, Default)]
pub(crate) struct HolyCityGuard {
    /// The Holy City's position and the last turn a threat to it was seen.
    pub(crate) warned: Option<(Pos, u32)>,
    /// The Holy City's position and the last turn we spread there.
    pub(crate) spread: Option<(Pos, u32)>,
    /// The turn the gene last bought a Missionary. A unit bought this turn
    /// acts on the model's board only; the live seat sees it next frame, so its
    /// spread does not start the gap.
    pub(crate) bought: Option<u32>,
}

impl AdvancedAi {
    /// The Holy City while the gene guards it: ours, on our faith, and a rival
    /// faith named as its next majority (or the shipped pressure test asking
    /// for support) within [`HOLY_CITY_WARNING_STANDARD`] standard turns.
    pub(super) fn holy_city_at_risk(&self, g: &Game, pid: usize) -> Option<u32> {
        if !self.founder_holds_the_holy_city
            || !g.victory_conditions.religious
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
        {
            return None;
        }
        let own = g.players[pid].religion.as_deref()?;
        let cid = g.players[pid].holy_city?;
        let city = g.cities.get(&cid)?;
        if city.owner != pid || g.city_religion(city) != Some(own) {
            return None;
        }
        let mut memory = self.holy_city_guard.borrow_mut();
        let rival_next = city
            .religion_next_seen
            .as_deref()
            .is_some_and(|next| next != own);
        if rival_next || Self::city_needs_religious_support(g, pid, city, own) {
            memory.warned = Some((city.pos, g.turn));
        }
        let window = g.standard_duration(HOLY_CITY_WARNING_STANDARD);
        memory
            .warned
            .is_some_and(|(pos, turn)| pos == city.pos && g.turn.saturating_sub(turn) <= window)
            .then_some(cid)
    }

    /// Whether the Holy City at `pos` is due another spread of ours.
    fn holy_city_spread_due(&self, g: &Game, pos: Pos) -> bool {
        let gap = g.standard_duration(HOLY_CITY_SPREAD_GAP_STANDARD);
        self.holy_city_guard
            .borrow()
            .spread
            .is_none_or(|(at, turn)| at != pos || g.turn.saturating_sub(turn) >= gap)
    }

    /// The spreader's step under the gene: spread in the guarded Holy City, or
    /// walk back to it. `None` leaves the unit to the shipped targets.
    pub(super) fn holy_city_guard_step(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        religion: &str,
    ) -> Option<bool> {
        if g.players[pid].religion.as_deref() != Some(religion) {
            return None;
        }
        let cid = self.holy_city_at_risk(g, pid)?;
        let (pos, name) = {
            let city = &g.cities[&cid];
            (city.pos, city.name.clone())
        };
        if !self.holy_city_spread_due(g, pos) {
            return None;
        }
        let current = g.units.get(&uid)?.pos;
        let distance = g.wdist(current, pos);
        if distance > HOLY_CITY_GUARD_REACH {
            return None;
        }
        // One spreader answers: a nearer charged spreader of ours (the lower
        // id on a tie) takes the Holy City and this one keeps its targets.
        let nearer = g.units.values().any(|unit| {
            unit.id != uid
                && unit.owner == pid
                && unit.charges > 0
                && unit.religion.as_deref() == Some(religion)
                && g.rules.units[unit.kind].religious_spread > 0.0
                && {
                    let other = g.wdist(unit.pos, pos);
                    other < distance || (other == distance && unit.id < uid)
                }
        });
        if nearer {
            return None;
        }
        if distance <= 1 {
            if g.apply(pid, &Action::Spread { unit: uid }).is_ok() {
                let mut memory = self.holy_city_guard.borrow_mut();
                if memory.bought != Some(g.turn) {
                    memory.spread = Some((pos, g.turn));
                }
                drop(memory);
                think!(self.journal(), Faith, Decision,
                    "Spreading {} in the Holy City {}", religion, name;
                    "founder-holds-the-holy-city: a rival faith is named as its next majority, and the Holy City is the source of every unit of our faith");
                return Some(true);
            }
            return None;
        }
        if self.religious_step_toward_range(g, pid, uid, pos, 1) {
            think!(self.journal(), Faith, Detail,
                "{} spreader returns to the Holy City {}", religion, name;
                "founder-holds-the-holy-city: {} tiles out, a rival faith is named as its next majority", distance);
            return Some(true);
        }
        None
    }

    /// The purchase under the gene: a Missionary in the guarded Holy City when
    /// no charged spreader of our faith stands within
    /// [`HOLY_CITY_GUARD_REACH`]. True when one was bought.
    pub(super) fn holy_city_guard_buys(&self, g: &mut Game, pid: usize) -> bool {
        let Some(cid) = self.holy_city_at_risk(g, pid) else {
            return false;
        };
        let Some(own) = g.players[pid].religion.clone() else {
            return false;
        };
        let (pos, name) = {
            let city = &g.cities[&cid];
            (city.pos, city.name.clone())
        };
        if !self.holy_city_spread_due(g, pos) {
            return false;
        }
        let covered = g.units.values().any(|unit| {
            unit.owner == pid
                && unit.charges > 0
                && unit.religion.as_deref() == Some(own.as_str())
                && g.rules.units[unit.kind].religious_spread > 0.0
                && g.wdist(unit.pos, pos) <= HOLY_CITY_GUARD_REACH
        });
        if covered || self.founder_purchase_withheld(g, pid, cid) {
            return false;
        }
        let Some(price) = g.unit_purchase_cost(pid, cid, "missionary", "faith") else {
            return false;
        };
        if g.players[pid].faith + f64::EPSILON < price {
            return false;
        }
        if g.apply(
            pid,
            &Action::Buy {
                city: cid,
                unit: crate::name!("missionary"),
                formation: 0,
                currency: "faith".into(),
            },
        )
        .is_ok()
        {
            self.holy_city_guard.borrow_mut().bought = Some(g.turn);
            think!(self.journal(), Faith, Decision,
                "{} buys a Missionary to hold the Holy City", name;
                "founder-holds-the-holy-city: a rival faith is named as its next majority and no spreader of ours stands within {} tiles; {:.0} of {:.0} Faith", HOLY_CITY_GUARD_REACH, price, g.players[pid].faith + price);
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests;
