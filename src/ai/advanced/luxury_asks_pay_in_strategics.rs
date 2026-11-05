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

use super::*;

impl AdvancedAi {
    /// Whether `luxury-asks-pay-in-strategics` is on.
    pub fn luxury_asks_pay_in_strategics_enabled(&self) -> bool {
        self.luxury_asks_pay_in_strategics
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
        if !self.luxury_asks_pay_in_strategics {
            return Vec::new();
        }
        Self::barter_spares(g, pid, held)
            .into_iter()
            .filter(|spare| !spare.luxury)
            .collect()
    }
}
