//! `deterrence-stops-at-a-garrison`: on the Domination lane,
//! `near-rival-deterrence` against a neighbour that fields
//! [`DETERRENCE_OUT_OF_REACH`] times our power or more trains only a ranged
//! garrison, and only in a city with no ranged land unit of ours on or
//! beside it ([`GARRISON_REACH`]); and it holds a queue against the
//! economy's claims only for such a garrison.
//!
//! **Why (live Deity Gran Colombia, 4-player Pangaea, Online; the 17
//! Domination games of 2026-11-12 00:01Z-06:48Z, turns 40-120).** Of 168
//! land soldiers and guns ordered at peace, 89 were `near-rival-deterrence`
//! (29 trained by the claim, 60 more the units it held against
//! `campus-claims-the-free-slot`). 58 of the 191 deterrent decisions were
//! taken against a rival with twice our power or more, where one more unit
//! cannot reach parity.
//!
//! **Split out of `hopeless-army-yields-the-queue` (11-12, #5169), and not
//! armed.** Armed together on the live seat from 08:52Z to 09:58Z, the five
//! games that carried this cap read a median military power of 143 at t60
//! (126, 282, 143, 172, 123) against 269 over the eleven games before, and
//! three of the last seven were lost to a rival Religious victory at t76-90
//! with the defensive faith war held on its power floor ("a war at under
//! 70% of their power cannot stop it; 239 power against their 367"). Other
//! rules read the seat's total power (the faith war's floors, the
//! counter-war floor), and this cap lowers it, while military unit orders
//! t40-100 did not fall (20 and 17 against a mean 18.2).
//!
//! Off, or on any other lane, nothing changes. Grep the journal for
//! `deterrence-stops-at-a-garrison:`.

use super::*;

/// The neighbour's power against ours from which a deterrent cannot deter:
/// half the rival's army is past what a city floor's queue can answer.
pub(crate) const DETERRENCE_OUT_OF_REACH: f64 = 2.0;

/// How far from its city a ranged land unit of ours stands as its garrison:
/// the city tile, or beside it when a melee unit holds the tile.
pub(crate) const GARRISON_REACH: i32 = 1;

/// A land ranged unit that is not a siege gun: a city's garrison.
fn garrison_shooter(spec: &crate::rules::UnitSpec) -> bool {
    spec.class == "military"
        && !spec.siege
        && spec.has_ranged_attack()
        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
}

impl AdvancedAi {
    /// Whether a deterrent against a neighbour of `theirs` power is out of
    /// reach: the gene is on, the seat plays the Domination lane and the
    /// neighbour fields [`DETERRENCE_OUT_OF_REACH`] times our power or more.
    pub(super) fn deterrence_out_of_reach(&self, g: &Game, pid: usize, theirs: f64) -> bool {
        self.deterrence_stops_at_a_garrison
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && theirs > 0.0
            && theirs >= DETERRENCE_OUT_OF_REACH * g.military_power(pid)
    }

    /// Whether a ranged land unit of ours (not a siege gun) stands on `cid`
    /// or within [`GARRISON_REACH`] of it.
    pub(super) fn city_has_ranged_garrison(g: &Game, pid: usize, cid: u32) -> bool {
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        g.units.values().any(|unit| {
            unit.owner == pid
                && garrison_shooter(&g.rules.units[unit.kind])
                && !g.is_embarked(unit)
                && g.wdist(unit.pos, city.pos) <= GARRISON_REACH
        })
    }

    /// The strongest ranged land unit (not a siege gun) `cid` can train: the
    /// garrison an out-of-reach deterrence still asks for.
    pub(super) fn garrison_shooter_for(g: &Game, pid: usize, cid: u32) -> Option<Name> {
        g.rules
            .units
            .iter()
            .filter(|(kind, spec)| {
                garrison_shooter(spec) && g.can_produce(pid, cid, &Item::Unit { unit: **kind })
            })
            .max_by(|a, b| {
                a.1.ranged_attack_strength()
                    .total_cmp(&b.1.ranged_attack_strength())
                    .then(b.0.cmp(a.0))
            })
            .map(|(kind, _)| *kind)
    }

    /// Whether `item` is the ranged garrison of a city of ours that has none
    /// yet: the one deterrent an out-of-reach deterrence still holds.
    pub(super) fn deterrence_garrison_head(g: &Game, pid: usize, item: &Item) -> bool {
        let Item::Unit { unit } = item else {
            return false;
        };
        g.rules.units.get(unit).is_some_and(garrison_shooter)
            && g.player_city_ids(pid).into_iter().any(|cid| {
                g.cities[&cid].queue.first() == Some(item)
                    && !Self::city_has_ranged_garrison(g, pid, cid)
            })
    }
}

#[cfg(test)]
mod tests;
