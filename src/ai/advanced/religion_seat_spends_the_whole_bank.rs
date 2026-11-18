//! `religion-seat-spends-the-whole-bank`: while a rival still owes our faith a
//! majority, the Faith the corps caps leave in the bank buys spreaders the
//! same turn, in every city that sells one, each the most spread for its
//! price.
//!
//! **Why (live Deity, Religious target, 2026-11-18).** The corps is capped at
//! six Apostles (`religion-seat-fields-apostles`) and six Missionaries, and
//! `religion-seat-spends-the-bank` lifts both caps only while the bank holds
//! two Apostle prices, for one unit a turn, the Apostle first. An Apostle's
//! price climbs with every one bought, so the bank this leaves standing
//! climbs with it. Byzantium civvis-20261118T094102Z bought an Apostle about
//! every second turn from t150 (30 in 59 turns, from 330 Faith each at t130
//! to about 950 at t200) and banked 910 at t150, 1,218 at t160, 1,442 at t170
//! and 1,671 at the end (t208, lost to a rival's Science), one and a half to
//! two and a half Apostle prices throughout. The host then read our faith as
//! the majority in three of five rivals (6 of 11 cities, 6 of 11, 7 of 13)
//! with Egypt at 4 of 12 and the Ottomans at 2 of 8. A Missionary cost 230
//! that turn. Mali civvis-20261118T082214Z ended with 938.
//!
//! **What the gene does**, for a seat aiming at Religion that has founded and
//! is on the offensive, while a living rival major does not follow our faith:
//! once the turn's ordinary purchase is made (or the caps have refused one),
//! what the bank still holds above the pass's reserve buys a spreader in
//! each city of our faith that sells one, one a city. Each is the Apostle or
//! the Missionary that gives the most spread for its price at that city's
//! quote ([`AdvancedAi::spread_per_faith`]: charges times the pressure of one
//! spread, over the price), the Apostle on a tie.
//!
//! It leaves alone what comes before it in the pass: the Holy City's guard,
//! the Inquisition's corps, and the save toward an Apostle the capped corps
//! is still owed (`religion-seat-fields-apostles`), which returns before any
//! purchase. Off, off the offensive, for a seat aiming elsewhere, or once
//! every living rival follows our faith, nothing changes.
//!
//! **What it does not do.** It buys charges; it does not deliver them. The
//! Byzantium game held 86 to 139 unspent charges from t160 to the end (19 of
//! its 23 Apostles had three or more at t208) and spread 1.9 times a turn, so
//! the bank was not what its majorities waited on. By the measure above the
//! Missionary wins at every quote these games show, and a Missionary cannot
//! fight a rival Apostle. Judge it on games: majorities held, units lost.

use super::*;

/// The spreaders the bank may buy, in the order a tie is settled.
const WHOLE_BANK_SPREADERS: [&str; 2] = ["apostle", "missionary"];

impl AdvancedAi {
    /// Living rival majors that do not follow `religion`: the majorities a
    /// Religious Victory is still owed.
    pub(super) fn majorities_owed(g: &Game, pid: usize, religion: &str) -> usize {
        g.players
            .iter()
            .filter(|rival| {
                rival.id != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && !rival.is_free_city
                    && !g.same_team(pid, rival.id)
                    && !g.civ_follows_religion(rival.id, religion)
            })
            .count()
    }

    /// What one Faith buys of `unit` at `price`: its charges times the
    /// pressure of one spread, over the price. Zero for a unit that does not
    /// spread or a price that is not one.
    pub(super) fn spread_per_faith(g: &Game, unit: &str, price: f64) -> f64 {
        match g.rules.units.get(unit) {
            Some(spec) if price > 0.0 => {
                f64::from(spec.charges.max(0)) * spec.religious_spread / price
            }
            _ => 0.0,
        }
    }

    /// Whether the gene acts for this seat this turn. See the module.
    pub(super) fn spends_the_whole_bank(
        &self,
        g: &Game,
        pid: usize,
        offensive: bool,
        religion: &str,
    ) -> bool {
        self.religion_seat_spends_the_whole_bank
            && offensive
            && self.active_victory_target(g) == Some(VictoryTarget::Religion)
            && g.players[pid].religion.as_deref() == Some(religion)
            && Self::majorities_owed(g, pid, religion) > 0
    }

    /// Spend what the bank holds above `reserve`: one spreader in each city
    /// of our faith that sells one, the most spread for its price first.
    /// `sold` is the city the pass has already bought in this turn, if any.
    /// Returns how many it bought. See the module.
    pub(super) fn religion_seat_whole_bank_buys(
        &self,
        g: &mut Game,
        pid: usize,
        offensive: bool,
        religion: &str,
        reserve: f64,
        sold: Option<u32>,
    ) -> usize {
        if !self.spends_the_whole_bank(g, pid, offensive, religion) {
            return 0;
        }
        let mut tried: BTreeSet<u32> = sold.into_iter().collect();
        let mut bought = 0;
        loop {
            let faith = g.players[pid].faith;
            let offer = self
                .founder_purchase_order(g, pid)
                .into_iter()
                .filter(|cid| !tried.contains(cid))
                .filter(|cid| {
                    g.city_religion(&g.cities[cid]) == Some(religion)
                        && !self.founder_purchase_withheld(g, pid, *cid)
                })
                .flat_map(|cid| {
                    WHOLE_BANK_SPREADERS
                        .into_iter()
                        .map(move |unit| (cid, unit))
                })
                .filter_map(|(cid, unit)| {
                    let price = g.unit_purchase_cost(pid, cid, unit, "faith")?;
                    (price > 0.0 && faith + f64::EPSILON >= price + reserve).then(|| {
                        (
                            Self::spread_per_faith(g, unit, price),
                            unit == "apostle",
                            std::cmp::Reverse(cid),
                            unit,
                            price,
                        )
                    })
                })
                .max_by(|left, right| {
                    left.0
                        .total_cmp(&right.0)
                        .then(left.1.cmp(&right.1))
                        .then(left.2.cmp(&right.2))
                });
            let Some((value, _, std::cmp::Reverse(cid), unit, price)) = offer else {
                return bought;
            };
            tried.insert(cid);
            let order = Action::Buy {
                city: cid,
                unit: Name::new(unit),
                formation: 0,
                currency: "faith".to_string(),
            };
            if g.apply(pid, &order).is_ok() {
                bought += 1;
                think!(self.journal(), Faith, Decision,
                    "Faith buys a {} in {}", unit, g.cities[&cid].name;
                    "religion-seat-spends-the-whole-bank: {} rival(s) still owe our faith a majority and {:.0} Faith was banked above the corps caps; this is the most spread for its price on offer ({:.0} Faith, {:.1} pressure a Faith)",
                    Self::majorities_owed(g, pid, religion), faith, price, value);
            }
        }
    }
}

#[cfg(test)]
mod tests;
