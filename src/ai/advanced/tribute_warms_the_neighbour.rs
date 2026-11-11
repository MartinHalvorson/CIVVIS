//! `tribute-warms-the-neighbour`: one Gold gift to the nearest stronger
//! neighbour while it is too cold for a friendship, sized to lift its opinion
//! of us past the point where Civilization VI must turn it back to Neutral,
//! and paid from the whole treasury, so the friendship ask can land before
//! its first war.
//!
//! ## Why
//!
//! Live Deity Culture seats (Kristina and Pericles, Online, 6p Continents),
//! the 25 games finished 2026-11-10 20:00Z to 2026-11-11 01:00Z, 14 of them
//! collapsed to two cities or fewer:
//! - The nearest neighbour read Neutral on the turn we met it and Unfriendly
//!   the next in 24 of 24 games, at a visible opinion of −2 to −9. The leader
//!   may pick Unfriendly at +5 or less and must leave it at +5 or more
//!   (`DiplomaticStateTransitions`, `Base/Assets/Gameplay/Data/DiplomaticActions.xml`:
//!   NEUTRAL→UNFRIENDLY `AllowTransitionMax` 5, UNFRIENDLY→NEUTRAL
//!   `RequireTransitionMax` 5). Across all rivals, 53 Unfriendly→Neutral
//!   turns came at a median +6.
//! - The host refuses a Delegation, an Embassy and a friendship to an
//!   Unfriendly leader (shipped Worth 0 against Cost 75 and 100; the agent's
//!   friendship order fails `not_friends`). 347 Delegations went to
//!   Unfriendly rivals in those games and none crossed.
//! - A declared friend never declared war on us (no Declared-Friend→War turn
//!   in about 1,900 rival-turns); every surprise war came from Unfriendly (35),
//!   Denounced (10) or Friendly (4). The first attacker never reached
//!   Friendly before its war in the 13 collapsed games that had one.
//! - A one-sided deal is the lever left: `STANDARD_DIPLOMATIC_ONE_SIDED_TRADES`
//!   gives a point per 10 Gold of trade value (`Leaders.xml`). 15 live
//!   tributes from `tribute-buys-the-friendship` bought about a point less
//!   than their Gold over ten. The three that left the rival at +5 to +9
//!   turned it Neutral the next turn; two went on to a declared friendship
//!   within four turns. The six that left it at +4 or +5 and Unfriendly
//!   bought nothing.
//! - That gene courts the strongest rival, aims at +5 and keeps 100 Gold
//!   back. Before turn 60 every sampled frame of seven collapsed games held
//!   it on the treasury, and it never paid the eventual attacker before its
//!   war. Aimed at +7 with no reserve, a gift one payment could buy (a gap
//!   of ten points) was affordable at least six turns before the first war
//!   against Sumeria (t21, war t41), Persia (t43, t54), America (t33, t39)
//!   and Nubia (t16, t58).
//!
//! ## What the gene does
//!
//! Inert with the flag off. On, the live bridge
//! (`civvis_orders::append_neighbour_tribute_order`) takes the majors at
//! peace with us, stronger than us, with a city within twelve tiles of one of
//! ours, reading Unfriendly or Neutral, with an opinion breakdown and no
//! standing "Favorable trade deals" row, whose gap to +7 one gift can close.
//! The nearest of them is sent one whole gift of ten Gold per point of the
//! gap (`aid_gift` verb `TRIBUTE`, the path `tribute-buys-the-friendship`
//! uses; the mod holds a rival to one tribute per 30 turns). The whole
//! treasury may pay it, less Gold another gift already took this frame; a
//! seat losing Gold per turn sends nothing. Any other diplomatic or trade
//! order to that rival this frame holds it.
//!
//! Once the neighbour reads Friendly, the friendship genes ask as they
//! already do. `Kind::OptIn`.
use super::*;

impl AdvancedAi {
    /// Whether `tribute-warms-the-neighbour` is on, for the live bridge.
    pub fn tribute_warms_the_neighbour_enabled(&self) -> bool {
        self.tribute_warms_the_neighbour
    }
}

#[cfg(test)]
mod tests;
