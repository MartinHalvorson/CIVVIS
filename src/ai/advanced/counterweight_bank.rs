//! `counterweight-spends-the-bank`: a faithless Domination seat whose
//! majority follows the threat faith spends its Faith on counterweight
//! Missionaries, enough to break that majority, before anything else.
//!
//! A seat with no religion of its own vetoes a Religious Victory only by
//! keeping half its cities off the winner's faith, and the only lever it has
//! without a war is a counterweight: Missionaries of another faith bought in a
//! city that follows it (`religious_defense`). The source is the hard part
//! (a Holy Site and a Shrine in such a city); once it exists, the Faith
//! should go there. It did not. In the 7 faithless Religious defeats of the
//! October control runs on disk our majority followed the winner's faith for
//! 40 to 115 turns; a counterweight source existed in 4 of them (from turns
//! 99, 129, 158 and 103), and on those turns `religious_defense` kept at most
//! two counterweight Missionaries while the bank paid for Great People,
//! Artillery, Llaneros and a Meeting House. Live Emperor G200
//! (civvis-20261006T061051Z): Buddhism held 8 of our 10 cities and every
//! major but Scotland; Quito's Shrine opened the Confucian source at turn
//! 103 with 565 Faith banked and Missionaries at 70; the seat paid 260 for a
//! Great Scientist, bought two Missionaries, and Mongolia won at 108.
//!
//! Under the gene, while our majority follows the threat:
//! - `religious_defense` keeps [`AdvancedAi::counterweight_cap`]
//!   counterweight Missionaries in the field, enough to turn the cities the
//!   threat's majority over us rests on, one spare, never fewer than shipped;
//! - Great Person patronage and Faith-bought military leave
//!   [`AdvancedAi::counterweight_faith_reserve`] in the bank while a source
//!   can sell them and the cap is not met.
//!
//! Off: unchanged. A founder is untouched (its own faith is its defence).

use super::*;

impl AdvancedAi {
    /// Our cities following `threat` beyond half: how many must leave it for
    /// its majority over us to break. Zero when it holds no majority.
    pub(super) fn counterweight_need(g: &Game, pid: usize, threat: &str) -> usize {
        let cities = g.player_city_ids(pid);
        let following = cities
            .iter()
            .filter(|cid| g.city_religion(&g.cities[cid]) == Some(threat))
            .count();
        if following * 2 > cities.len() {
            following - cities.len() / 2
        } else {
            0
        }
    }

    fn counterweight_bank_applies(&self, g: &Game, pid: usize) -> bool {
        self.counterweight_spends_the_bank
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && g.victory_conditions.religious
            && g.players[pid].religion.is_none()
    }

    /// The counterweight Missionaries `religious_defense` keeps against
    /// `threat`: `shipped` with the gene off or while `threat` holds no
    /// majority over us, else enough to turn the cities that majority rests
    /// on, plus one spare. G200 turn 103: 8 Buddhist cities of 10, so 4.
    pub(super) fn counterweight_cap(
        &self,
        g: &Game,
        pid: usize,
        threat: &str,
        shipped: usize,
    ) -> usize {
        if !self.counterweight_bank_applies(g, pid) {
            return shipped;
        }
        match Self::counterweight_need(g, pid, threat) {
            0 => shipped,
            need => shipped.max(need + 1),
        }
    }

    /// The cheapest counterweight Missionary a city of ours sells now: one
    /// whose majority is not `threat` and is a safe counterfaith, the cities
    /// `religious_defense` buys in.
    fn counterweight_price(&self, g: &Game, pid: usize, threat: &str) -> Option<f64> {
        g.player_city_ids(pid)
            .into_iter()
            .filter(|cid| {
                g.city_religion(&g.cities[cid]).is_some_and(|faith| {
                    faith != threat && self.counterfaith_is_safe(g, pid, faith)
                })
            })
            .filter_map(|cid| g.unit_purchase_cost(pid, cid, "missionary", "faith"))
            .min_by(f64::total_cmp)
    }

    /// Faith the other Faith sinks (Great Person patronage, Faith-bought
    /// military) leave for counterweight Missionaries: the price of those
    /// still short of [`Self::counterweight_cap`], while a source sells them.
    /// Zero with the gene off, without a source, or with the cap met.
    pub(super) fn counterweight_faith_reserve(&self, g: &Game, pid: usize) -> f64 {
        if !self.counterweight_bank_applies(g, pid) {
            return 0.0;
        }
        let Some(threat) = self.adopted_faith_threat(g, pid) else {
            return 0.0;
        };
        let need = Self::counterweight_need(g, pid, &threat);
        if need == 0 {
            return 0.0;
        }
        let veto = self.religious_veto_engaged(g, pid);
        let shipped = 2 + Self::religious_veto_extra_spreaders(veto.as_ref());
        let cap = self.counterweight_cap(g, pid, &threat, shipped);
        let short = cap.saturating_sub(self.religious_defense_missionary_count(g, pid, &threat));
        if short == 0 {
            return 0.0;
        }
        self.counterweight_price(g, pid, &threat)
            .map_or(0.0, |price| price * short as f64)
    }
}

#[cfg(test)]
mod tests;
