//! `luxury-asks-pay-in-strategics`: when the luxury swap finds no spare copy
//! that fits, the live bridge asks a partner at peace for a luxury type we
//! hold none of and pays in strategics our army does not need.
//!
//! ## Why
//!
//! Live Lv 8 Deity Korea Online (28 games, 2026-11-04 21Z to 11-05 00Z): the
//! host's "Amenities" yield lines cost 6.7% of all production at turns
//! 90-110, the margin by which the near-miss games fell under the
//! quarter-of-the-leader retire gate. On the luxury slot's turns t50-120 with
//! a city short, `luxury-swap-asks-2` held 74 times, 51 of them for want of
//! any spare copy (most seats hold each type once; one held a single type
//! with 7-8 cities short). Gold alone closed 0 of 16 luxury asks while barter
//! closed 22, and in 56 of the 74 holds the seat stockpiled 10 or more of a
//! strategic the surplus sale sells for Gold anyway.
//!
//! ## What the gene does
//!
//! Bridge-only (`append_luxury_strategic_ask` in `civvis_orders.rs`): on the
//! luxury slot, after the swap holds `no_spare` or `no_fit`, one `buy` order
//! `LUXURY_ANY;RESOURCE_X=n` goes to the partner the week rotates to among
//! those at peace that offer a type we lack, `n` from the strategics held
//! above what our units, their upgrades, their fuel and the barter reserve
//! need ([`AdvancedAi::luxury_ask_strategic_spares`]). `Kind::OptIn`; off,
//! the bridge emits exactly what it did.
//!
//! ## Version two
//!
//! `luxury-asks-pay-in-strategics-2`: in the live Deity games after version
//! one's arming (11-05 02:01Z), 29 of 61 strategic asks died in the Lua buy
//! arm as `barter_short`, before reaching the partner; 24 of the 27 partners
//! asked more than once answered the same way every time, and Horses shorted
//! 22 times in 42 asks against 2 in 10 for Niter (rivals hold Horses at the
//! stockpile cap). Version two skips a partner that answered `barter_short`
//! inside [`LUXURY_BARTER_SHORT_STANDARD`] standard turns (the bridge reads
//! the shorts from the run's log) and puts Horses up only when no other
//! strategic is spare.

use super::*;

/// `luxury-asks-pay-in-strategics-2`: a partner that answered `barter_short`
/// inside this many standard turns is not asked again.
pub(crate) const LUXURY_BARTER_SHORT_STANDARD: u32 = 30;

impl AdvancedAi {
    /// Whether `luxury-asks-pay-in-strategics` is on.
    pub fn luxury_asks_pay_in_strategics_enabled(&self) -> bool {
        self.luxury_asks_pay_in_strategics || self.luxury_asks_pay_in_strategics_2
    }

    /// Whether `luxury-asks-pay-in-strategics-2` is on: the ask skips a
    /// partner the buy arm answered `barter_short` within
    /// [`LUXURY_BARTER_SHORT_STANDARD`] standard turns and keeps Horses back
    /// while another strategic is spare.
    pub fn luxury_asks_pay_in_strategics_2_enabled(&self) -> bool {
        self.luxury_asks_pay_in_strategics_2
    }

    /// The live bridge: `(turn, partner)` of each `barter_short` answer the
    /// run's log holds. Replaces what was there.
    pub fn observe_luxury_barter_shorts(&mut self, shorts: Vec<(u32, i64)>) {
        self.luxury_barter_shorts = shorts;
    }

    /// The host ids of the partners that answered `barter_short` inside the
    /// window ending this turn. Empty with version two off.
    pub fn luxury_barter_shorted_partners(&self, g: &Game) -> BTreeSet<i64> {
        if !self.luxury_asks_pay_in_strategics_2 {
            return BTreeSet::new();
        }
        let window = g.standard_duration(LUXURY_BARTER_SHORT_STANDARD);
        self.luxury_barter_shorts
            .iter()
            .filter(|(turn, _)| *turn <= g.turn && g.turn - *turn < window)
            .map(|(_, partner)| *partner)
            .collect()
    }

    /// The strategics a luxury ask may pay with: the barter spares
    /// (`siege_barter_spares`' own rule, nothing in `held`), luxuries
    /// dropped. Empty with the gene off.
    pub fn luxury_ask_strategic_spares(
        &self,
        g: &Game,
        pid: usize,
        held: &BTreeSet<String>,
    ) -> Vec<BarterSpare> {
        if !self.luxury_asks_pay_in_strategics && !self.luxury_asks_pay_in_strategics_2 {
            return Vec::new();
        }
        Self::barter_spares(g, pid, held)
            .into_iter()
            .filter(|spare| !spare.luxury)
            .collect()
    }
}
