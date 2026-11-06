//! `government-reach-pays-the-switch`: a seat already in the government its
//! lane lists reaches past that list only for a real gain in policy slots.
//!
//! `government-capacity-fallback` was written for a seat stuck on Classical
//! Republic's four slots with Corporate Libertarianism's ten unlocked, and
//! that gain is worth a switch. It also fires for two slots: a Science seat in
//! Communism's eight leaves for ten, late, in the turns that decide the race.
//!
//! Live Deity, 11-04 21Z to 11-05: every switch into Communism raised Science
//! two turns later (17 of 17, median +15%), and every switch into Corporate
//! Libertarianism cut it (5 of 5, t171-193, median -15%, worst -27%; at eight
//! turns the two with a reading stood -23% and -33%). 111305Z, our second
//! closest loss, read "10 policy slots against 8 now; the science plan wants
//! it" at t187 and went 314 to 231. The deck it ordered kept Rationalism and
//! Liberalism, but the next frame held Scripture, Machiavellianism and
//! Diplomatic League in their place, the host priced a further change at 455
//! Gold against 245 in hand, and the deck did not change again until t196.
//! Those five are the seats closest to a win, 122513Z among them, so the
//! sample is small and in-sample, but it is the part of the game every block
//! is buying time for.
//!
//! Under the gene, a seat holding the listed pick takes a richer government
//! only for [`GOVERNMENT_REACH_MIN_GAIN`] slots or more. A seat that does not
//! hold the listed pick is untouched, so the stuck seat the fallback was
//! written for still reaches. Civic research toward the richer government
//! (`government_ladder`) is not changed. Off, nothing changes.

use super::*;

/// The slots a richer government must add before a seat leaves the listed
/// pick it already holds. Communism to Corporate Libertarianism adds two.
pub(crate) const GOVERNMENT_REACH_MIN_GAIN: i64 = 3;

impl AdvancedAi {
    /// Whether the seat keeps the listed pick it holds rather than reach for
    /// `slots` against the pick's `listed`.
    pub(super) fn government_reach_holds(
        &self,
        g: &Game,
        pid: usize,
        pick: Option<&str>,
        slots: i64,
        listed: i64,
    ) -> bool {
        self.government_reach_pays_the_switch
            && pick.is_some()
            && g.players[pid].government.as_deref() == pick
            && slots < listed + GOVERNMENT_REACH_MIN_GAIN
    }
}
