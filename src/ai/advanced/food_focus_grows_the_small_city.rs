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
//! Off, `civvis_orders` asks exactly what it did before.

use super::*;

impl AdvancedAi {
    /// `food-focus-grows-the-small-city`: whether `civvis_orders` widens its
    /// native food-focus request. See the module notes.
    pub fn food_focus_grows_the_small_city(&self) -> bool {
        self.food_focus_grows_the_small_city
    }
}
