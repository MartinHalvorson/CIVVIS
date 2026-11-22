//! `staged-siege-keeps-its-breakers` (opt-in): while one of our sieges stands
//! in its stage with nothing that can open the walls, a Battering Ram, Siege
//! Tower or siege gun at the head of a city's queue keeps that queue against
//! the strategic scorer, even before any production is in it.
//!
//! ## Why (live Tournament 6/7 and Lv 8 Deity games, 2026-11-21/22)
//!
//! In 11 of 65 wars the seat held 1.5x or more of the rival's military for
//! 13-31 turns (peaks 2.5-3.5x) and those games took 0-2 cities. The siege
//! journal says why: across the recorded games the "Siege of X: stage" lines
//! read "damage ready false" on all but a few turns, most with "breakers: 0
//! gun(s) fit, 0 healing, 0 ram/tower ... nothing to open the walls, so the
//! train holds outside the city's reach". Khmer `civvis-t6-20261122T045704Z`
//! (Lv 7, at war with Zulu t71-155, 324 against 135 military at t100) held
//! its Bulawayo train at the muster line from t88 to t139 while "the walls ask
//! 4 guns"; the Siege Towers it queued never stood: "Bhavapura switches to
//! industrial zone | worth 89, displacing siege tower at 67; 0 banked" (t81),
//! "Mahendraparvata switches to university | worth 99, displacing siege tower
//! at 73" (t84), "Bhavapura switches to campus | worth 168, displacing siege
//! tower at 67" (t87). Zulu then outbuilt the seat and took its capital at
//! t140. The same displacement is journalled 1-13 times a game in 20 of the
//! recorded games.
//!
//! `breaker-keeps-its-queue` (`AdvancedAi::breaker_commitment_holds`) was the
//! guard against exactly this, and could not see it: it holds only a unit
//! with `UnitSpec::siege` (the Catapult line; a Ram and a Tower are `support`
//! class, the engine's own `Game::siege_support_effects` names them by kind)
//! and only once production is invested in it, while every displaced tower
//! above was an unstarted head.
//!
//! ## What the gene does
//!
//! `assess_siege` notes the turn a siege's breach reading leaves the walls
//! shut (`no_breaker`). For that turn and the [`BREAKER_WANTED_TURNS`] after
//! it, `breaker_commitment_holds` also holds a city's head that is a siege-class unit, a Battering Ram or a
//! Siege Tower, invested or not. The guard's own exceptions stand: a city the
//! plan names threatened, or one attacked in the last 4 turns, gives the
//! queue up, and the under-fire defence writer is a separate step.
//!
//! **The window (version of 2026-11-22 14Z).** The first window was that
//! turn and the next. The siege planner writes its stage reading only on the
//! turns it reassesses the siege, so the reading comes in clusters with gaps
//! between them, and over the first armed Tournament 7 games (11 since 11:01Z)
//! 19 breakers were still displaced: 3 within a turn of a reading, 11 within
//! 6, 14 within 10, and 8 of them with the siege reading "nothing to open the
//! walls" again within 10 turns (live Jayavarman `civvis-t7-20261122T112202Z`:
//! readings at t104 and t108, towers displaced at t106 and t107). The window
//! is now six turns.

use super::*;

/// How many turns after a siege last read "nothing to open the walls" its
/// breakers keep their queues. The planner writes that reading only on the
/// turns it reassesses the siege, so a live siege goes several turns between
/// readings, and a turn's production can be decided before the turn's own
/// reading. Six covered 11 of the 19 breakers the one-turn window let go in
/// the first armed live games.
pub(super) const BREAKER_WANTED_TURNS: u32 = 6;

impl AdvancedAi {
    /// `staged-siege-keeps-its-breakers`: a siege's breach reading left the
    /// walls shut this turn. Nothing with the gene off.
    pub(super) fn note_siege_wants_a_breaker(&mut self, g: &Game) {
        if self.staged_siege_keeps_its_breakers {
            self.siege_breaker_wanted_turn = Some(g.turn);
        }
    }

    /// Whether `current`, the head of `cid`'s queue, is a breaker a staged
    /// siege is waiting for and so keeps the queue.
    pub(super) fn staged_siege_holds_the_breaker(
        &self,
        g: &Game,
        cid: u32,
        current: &Item,
        plan: &StrategicPlan,
    ) -> bool {
        if !self.staged_siege_keeps_its_breakers {
            return false;
        }
        let Some(wanted) = self.siege_breaker_wanted_turn else {
            return false;
        };
        if g.turn.saturating_sub(wanted) > BREAKER_WANTED_TURNS {
            return false;
        }
        let Item::Unit { unit } = current else {
            return false;
        };
        let breaker = g.rules.units.get(unit).is_some_and(|spec| spec.siege)
            || matches!(unit.as_str(), "battering_ram" | "siege_tower");
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        breaker
            && plan.threatened_city != Some(cid)
            && !(city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
    }
}

#[cfg(test)]
mod tests;
