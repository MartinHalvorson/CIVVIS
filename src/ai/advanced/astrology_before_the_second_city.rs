//! `astrology-before-the-second-city`: while the Prophet race is open for this
//! seat, a lone capital researches Astrology once the opening techs
//! ([`PROPHET_RACE_OPENING_TECHS`]) are in, instead of waiting for the second
//! city that admits it to the race (`religious_opening_viable`, floor
//! [`RELIGIOUS_OPENING_CITIES`]). Only the research fee moves: the Holy Site
//! reservation, the Prophet's price and `pursue_religion` still read the
//! two-city gate, so the district starts the turn the second city stands
//! rather than four to thirteen turns later.
//!
//! **Why (2026-10-14 census, 79 live Emperor GC runs that reached t60).**
//! - **The faith decides the Religious loss.** Faithless seats lost 7 of 40
//!   games to Religion, founders 2 of 39. Every one of the 9 losses came
//!   after a rival faith held our own majority, and that happened in 37 of the
//!   40 faithless games against 15 of the 39 founders'. The t150 gate did not
//!   pay for the faith: 7 of 40 faithless seats passed it, 8 of 39 founders.
//! - **The 2-city floor loses the race.** The third Prophet went at t31-44 in
//!   the races we lost. Our Astrology started at the 3rd to 7th technology
//!   (median 4-5), a few turns after the second city (median t18), and landed
//!   at t24-34. Every early founding (by t45) had Astrology by t16-27.
//! - **G202334Z** (Religious loss to the Mapuche's Zoroastrianism at t113):
//!   18.2 of 30 Prophet points when the third Prophet went at t40. A frame-0
//!   replay at t10 (two techs in, one city) researches Pottery, "the cheapest
//!   step toward irrigation"; Astrology waited for the second city (t18) and
//!   then Archery, and landed at t27. The Holy Site followed at t30. Faithless,
//!   our ten cities were Zoroastrian by t100, and the loss came the turn Nubia,
//!   the last holdout, flipped.
//!
//! **Replay (persistent frame-0 serve, t1-36, each run's own decider).** The
//! host only lists a Holy Site as buildable once Astrology is known, so the
//! counterfactual copies each city's own `DISTRICT_HOLY_SITE` entry back to
//! t14 with the tech. `prophet-builds-its-site` then queues the Holy Site
//! earlier in 8 of 10 lost races (6 of the 7 Religious losses replayed):
//! G083713Z t16 instead of t26, G110038Z t21/28, G113054Z t24/32, G174431Z
//! t22/29, G202334Z t25/27, G203024Z t23/25, G073241Z t24/25, G120424Z
//! t25/26; G161256Z and G100910Z unchanged. What still holds it is the
//! city queue: the early Settler floor, a threatened capital, two cities.
//!
//! **`prophet-race-takes-divine-spark`, the companion lever.** Two turns did
//! not win G202334Z's race on their own (about 23 points at t40); Divine
//! Spark alone (+1 Prophet point a turn on the Holy Site, live from t31)
//! reaches about 29. Together they reach 30 at about t38, two turns before
//! Nubia's Prophet. While the seat runs the race
//! (`AdvancedAi::race_points_wanted`), the pantheon pick therefore ranks
//! Divine Spark right behind Religious Settlements (a free Settler still
//! leads) and ahead of Fertility Rites (`BasicAi::pantheon_for_the_prophet_race`).
//!
//! ⚠ **Its price is Fertility Rites' free Builder**, and the live seat takes
//! Fertility Rites in about 47 of 81 Emperor games (a rival holds Religious
//! Settlements). In G202334Z's t20 frames the capital is 23/55 into a
//! Settler; with the Builder on the board `capital-settler-after-completion`
//! keeps it, and a replay of the next frame with Divine Spark and no Builder
//! re-plans the capital to a Warrior. Divine Spark itself changes nothing
//! there (with the Builder kept the Settler stays). Judge this gene on cities
//! at t60 and the t150 gate as well as on the race; it is armed separately
//! from `astrology-before-the-second-city`, which costs one research slot.
//!
//! The admission is the race's own, with this seat ranked at one city: the
//! lane must enter the race (`prophet_race_enabled_for`, or a rival faith at
//! the bar under `found-against-a-rival-faith`), a Prophet slot must be open,
//! the capital must have a Holy Site placement, and the seat must be among the
//! best uncommitted contenders for the slots left. The defensive arms above it
//! in `advanced_research` (barbarian Archery, walls) still take the slot
//! first.

use super::*;

impl AdvancedAi {
    /// `astrology-before-the-second-city`: whether the research slot takes
    /// Astrology for a one-city empire the Prophet race would admit at two.
    /// False with the gene off, with two cities or more (the race's own gate
    /// answers then), and once Astrology is known.
    pub(super) fn astrology_before_the_second_city_due(&self, g: &Game, pid: usize) -> bool {
        if !self.astrology_before_the_second_city
            || g.player_city_ids(pid).len() != 1
            || g.players[pid].techs.len() < PROPHET_RACE_OPENING_TECHS
            || g.players[pid].techs.contains(&crate::name!("astrology"))
        {
            return false;
        }
        let target = self.victory_target;
        let due = (self.prophet_race_enabled_for(target) || self.faith_veto_due(g, pid))
            && self.prophet_race_open_for(g, pid)
            && self.religious_opening_viable_from(g, pid, 1)
            && !self.skip_prophet_race_2_for(g, pid, target);
        if due {
            think!(self.journal(), Research, Decision, "Astrology before the second city";
                   "astrology-before-the-second-city: {} of {} Prophets claimed; the Holy Site starts the turn the second city stands",
                   g.religions_founded(), g.max_religions());
        }
        due
    }
}

#[cfg(test)]
mod tests;
