//! `food-focus-grows-the-small-city`: on the native Civ VI seat,
//! `civvis_orders` asks the host's city governor to favour Food in a small
//! city growing at two Food a turn or less, not only in one that has stalled
//! at zero.
//!
//! **The finding (live Immortal 2026-10-28/29, 150 games).**
//! - Our small cities with housing room (population 1-6, at least two
//!   Housing free) grew at a median +2 Food a turn from turn 10 to turn 80.
//!   60-69% ran +2 or less and 80-90% ran +3 or less, the capital included
//!   (median +2 through turn 70, +1 at 80). At Online speed that is one
//!   citizen in about fifteen turns.
//! - Rival capitals stood at a median 4 / 7 / 10 citizens at turns 20 / 40 /
//!   60 against our 3 / 5 / 6. The t80 gap to the Production leader
//!   decomposes as cities 1.2 × citizens per city 1.5 × Production per
//!   citizen 1.9 (handicap 1.6). The middle factor is the largest that is not
//!   the handicap, and the t80 Production share is what retires the game.
//! - The native governor's growth recovery (#4003, `citizen_growth.rs`)
//!   favours Food only at exactly zero surplus, so it held the focus in 4-9%
//!   of those cities. Where it did, the host moved the surplus from a mean
//!   +0.2 to +1.7 for a Production cost of 7.5 → 6.3 (68 focus-on events,
//!   two turns later).
//!
//! **What the gene does.** The same request, read back the same way and
//! released by the same rules (Housing room under two, six citizens, turn
//! 130, or damage), also goes to a city of one to five citizens whose surplus
//! is between zero and two (`WIDE_MAX_POP` / `WIDE_MAX_SURPLUS` in
//! `src/bin/civvis_orders/citizen_growth.rs`), while a richer unworked Food
//! tile is observed. The simulator issues no city-focus orders, so the gene
//! acts only on the native seat.
//!
//! **Version two (in place): the settler pump keeps its Production.** Live,
//! v1 (armed 10-29 05:38Z, pulled 06:42Z) put the capital on the focus for
//! 19-50 of its first 60 turns in 5 of 8 seated games, 6-32 of them while it
//! trained a Settler. 055600Z's capital held it 40 of 60 turns and founded
//! the second city at t36; the seated games stood at 4 cities at t60 against
//! 5 and retired by t95 in 5 of 7 against 27-44%. The widened request no
//! longer goes to a city training a Settler, and it releases a growing
//! city's focus when a Settler starts. A stalled city (zero surplus) keeps
//! #4003's focus.
//!
//! **Version three (in place): the capital is the pump for the whole
//! opening.** Version two reads the queue head, and the capital trains a
//! Scout on turn 1. Russia Deity, Online, 16 live games of 2026-11-10: the
//! capital took the focus on turn 1 in 8 and held it for 4 to 57 of its
//! first 59 turns.
//! - Two held it 51 and 57 turns while `civvis_orders` asked for its release
//!   22 and 24 times. The host's own allocation at the housing cap ran
//!   negative Food, so the agent restored the focus each time
//!   (`food_release_would_starve`, `CivvisCitizenGrowth.read`).
//! - civvis-20261110T115325Z, turn 16, five citizens against six Housing, in
//!   two frames of one turn: focused, tiles of (Food, Production) (2,1) x4
//!   and (3,0) x2 for 5 Production and +4 Food; released, (1,2) x3, (1,3)
//!   and (2,1) x2 for 11 Production and -2 Food. Its release was verified at
//!   turn 52 and the city's Production went from 7 to 16. It ordered its
//!   first Settler at turn 44 and held one city at turn 50.
//! - Worked-tile Production at turn 20 was 5.5 in the 4 capitals holding the
//!   focus against 7.6 in the 9 without it, for 4.5 citizens against 4.0.
//!   Thirteen games is a small sample; the two frames are the mechanism.
//!
//! So through turn 60 (`CAPITAL_PUMP_UNTIL`) the capital is treated as
//! training a Settler: no widened request, and a growing focus is released.
//! A capital stalled at zero surplus keeps #4003's focus.
//!
//! Off, `civvis_orders` asks exactly what it did before.

use super::*;

impl AdvancedAi {
    /// `food-focus-grows-the-small-city`: whether `civvis_orders` widens its
    /// native food-focus request. See the module notes.
    pub fn food_focus_grows_the_small_city(&self) -> bool {
        self.food_focus_grows_the_small_city
    }
}
