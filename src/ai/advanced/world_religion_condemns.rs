//! `world-religion-condemns-the-faith`: World Congress option B on the faith
//! taking the world, and our soldiers condemn its units at peace.
//!
//! `WC_RES_WORLD_RELIGION` option B reads "All players may condemn units of
//! the chosen Religion, and doing so grants 25 Diplomatic Favor". `legal` and
//! `do_condemn_heretic` have honoured it all along, but `condemn_step` only
//! looked for an owner at war, and the ballot never asked for it: option A in
//! all 74 sessions recorded from 11-01 to 11-05, each won by rivals' free
//! votes (5 at the Deity median).
//!
//! Why it matters: a seat with no faith has nothing else that touches a rival
//! Missionary at peace. Live Deity civvis-20261104T213449Z lost to Orthodoxy
//! at t173. Orthodoxy held all 9 of our cities from t100 or earlier, we held
//! no religious unit and 869 Faith that could buy none (no Shrine in 44 of 44
//! Deity games at t100-140), and the game ended two turns after the Ottomans,
//! the last holdout, flipped. A religious victory needs every civilization,
//! so a civilization that never flips is the block.
//!
//! The chance: in 14 Deity games we saw a median 48 rival religious units a
//! game, a median 223 unit-turns of them within two tiles of a combat unit of
//! ours at peace. The session falls early (median t72 at Deity), before the
//! leading faith held a majority of our cities in 17 of 31 sessions. Favor is
//! the limit: 37 at the median session against 60 for six votes, so the ballot
//! reaches about 4 sessions in 10.
//!
//! Religious defeats are rare at Deity today (1 in about 100 played-out
//! games), because a rival's Science usually ends the game first. Blocking
//! Science exposes the seat to this path next, and the Favor pays for the
//! ballot in three kills.
//!
//! The ballot lives in the agent (`CivvisWorldReligionCondemn`), leased by a
//! `WORLD_RELIGION_CONDEMNS` row each batch. A founder keeps its option A.
//! `preserve_competing_faith` still spares a faith that holds the leader off.
//! Off, nothing changes.

use super::*;
use crate::game::Unit;

impl AdvancedAi {
    /// Whether the bridge leases the ballot to the agent.
    pub fn world_religion_condemns_the_faith_enabled(&self) -> bool {
        self.world_religion_condemns_the_faith
    }

    /// Whether `target` may be condemned without a war: the gene is on and
    /// World Religion's option B names its faith.
    pub(super) fn world_religion_condemns(&self, g: &Game, target: &Unit) -> bool {
        self.world_religion_condemns_the_faith
            && target
                .religion
                .as_deref()
                .is_some_and(|faith| g.congress_effect_active("world_religion", "B", faith))
    }
}

#[cfg(test)]
mod tests;
