//! `counterweight-finishes-one-shrine`: while a rival faith presses our
//! cities, the counterweight's source city gets its Shrine, bought when the
//! treasury or the bank covers it, and nothing displaces it; and the Faith
//! bank stays home for the counterweight instead of Great People, Faith
//! buildings or Faith-bought units.
//!
//! A counterweight Missionary is bought only in a Shrine city that follows
//! the counterfaith. Live Emperor G425 (civvis-20261008T175442Z): Caracas,
//! our one Hindu city, finished its Holy Site at turn 86, and from 86 to 92
//! the sanctuary "started its Shrine for religious defense" 19 times, but
//! every order the host received for Caracas was a Trebuchet: the
//! delegated siege reservation (`reserve_delegated_domination_siege`) ran
//! after the sanctuary and took the freshly queued, unpaid Shrine as a
//! displaceable routine building. With Caracas's queue then held by the
//! siege unit (`sanctuary-yields-a-held-queue`), the sanctuary fell back to
//! other cities and started Holy Sites in five of them between turns 69 and
//! 94. Caracas turned Orthodox at 93 with no Shrine. Meanwhile 394 to 551
//! Faith sat in the bank with nothing defensive to buy, and at turn 93,
//! with Georgia at match point, 450 of it went on "Buying out the merchant
//! race". Georgia won on Religion at 98.
//!
//! Under the gene, while a living rival's founded faith holds a city of ours
//! and stands at the religious early-warning bar or match point
//! (`rival_faith_presses_us`):
//! - a city of ours on a safe counterfaith (our own faith for a founder)
//!   with a finished Holy Site and no Shrine is the one sanctuary
//!   (`counterweight_shrine_city`): the sanctuary always names its Shrine,
//!   a held queue there is taken (a threatened city still is not), the
//!   Shrine is bought with Faith or Gold when either covers it, the siege
//!   reservation leaves that city alone, and while it stands without a
//!   Shrine the sanctuary names no other city;
//! - Great Person patronage, Faith buildings and Faith-bought military
//!   leave the whole bank (`counterweight_bank_held`) while that source is
//!   waiting for its Shrine. Once the Shrine stands, the priced counterweight
//!   Missionary reserve protects recruitment and the surplus can fund the army.
//!
//! Replay note: in G425 the Shrine, held from turn 86 at Caracas's 4 to 7
//! production (35 on Online speed), finishes at the turn 92-93 boundary,
//! the turn Caracas turned Orthodox, and the treasury (34-90 Gold) never
//! reached its price, so G425 itself is likely still lost; a site finished
//! a few turns sooner, or a treasury that covers the Shrine, is the case the
//! gene answers.
//!
//! Off: unchanged.

use super::*;

impl AdvancedAi {
    /// The rival faith pressing us: founded by a living rival, holding at
    /// least one city of ours, and at the religious early-warning bar on its
    /// founder's religion lane or at match point. The faith holding the most
    /// of our cities. `None` with the gene off.
    pub(super) fn rival_faith_presses_us(&self, g: &Game, pid: usize) -> Option<String> {
        if !self.counterweight_finishes_one_shrine || !g.victory_conditions.religious {
            return None;
        }
        let own = g.players[pid].religion.as_deref();
        let living = g
            .players
            .iter()
            .filter(|p| p.alive && !p.is_minor && !p.is_barbarian)
            .count() as i32;
        let match_point = 100 * (living - 1) / living.max(1);
        let early_warning = (100 * (living - 2) / living.max(1))
            .max(50)
            .min(match_point);
        g.players
            .iter()
            .filter(|p| p.id != pid && p.alive && !p.is_minor && !p.is_barbarian)
            .filter_map(|founder| {
                let faith = founder.religion.as_deref()?;
                if Some(faith) == own {
                    return None;
                }
                let held = g
                    .player_city_ids(pid)
                    .into_iter()
                    .filter(|cid| g.city_religion(&g.cities[cid]) == Some(faith))
                    .count();
                (held > 0
                    && (self.lane_progress_table(g, founder.id)[2] >= early_warning
                        || self.faith_at_match_point(g, founder.id)))
                .then(|| (held, faith.to_owned()))
            })
            .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)))
            .map(|(_, faith)| faith)
    }

    /// Hold the bank while an actionable counterweight source awaits its
    /// Shrine. A finished source uses `counterweight_faith_reserve` to protect
    /// the Missionaries it can actually sell. Without an unfinished source,
    /// an unlimited hold cannot buy religious defense and starves the army.
    pub(super) fn counterweight_bank_held(&self, g: &Game, pid: usize) -> f64 {
        if self.counterweight_shrine_city(g, pid).is_some() {
            g.players[pid].faith.max(0.0)
        } else {
            0.0
        }
    }

    fn counterweight_city_has_shrine(g: &Game, cid: u32) -> bool {
        g.cities[&cid]
            .buildings
            .iter()
            .any(|building| g.building_is_family(building, crate::name!("shrine")))
    }

    /// The one sanctuary: a city of ours on a safe counterfaith (our own
    /// faith for a founder) with a finished Holy Site and no Shrine, while a
    /// rival faith presses us. The cheapest Shrine to buy now, else the one
    /// that finishes soonest; ties go to the lower city id.
    pub(super) fn counterweight_shrine_city(&self, g: &Game, pid: usize) -> Option<u32> {
        let threat = self.rival_faith_presses_us(g, pid)?;
        let own = g.players[pid].religion.as_deref();
        let shrine = Item::Building {
            building: crate::name!("shrine"),
        };
        let gold = g.players[pid].gold;
        g.player_city_ids(pid)
            .into_iter()
            .filter(|cid| {
                let city = &g.cities[cid];
                g.city_has_district_family(city, crate::name!("holy_site"))
                    && !Self::counterweight_city_has_shrine(g, *cid)
                    && g.city_religion(city).is_some_and(|faith| match own {
                        Some(own) => faith == own,
                        None => faith != threat && self.counterfaith_is_safe(g, pid, faith),
                    })
            })
            .map(|cid| {
                let bought = g
                    .building_gold_purchase_cost(pid, cid, "shrine")
                    .filter(|price| *price <= gold);
                let turns = g.item_remaining_cost_for_city(pid, cid, &shrine)
                    / g.city_yields(cid).production.max(1.0);
                (bought.is_none(), bought.unwrap_or(0.0), turns, cid)
            })
            .min_by(|a, b| {
                a.0.cmp(&b.0)
                    .then(a.1.total_cmp(&b.1))
                    .then(a.2.total_cmp(&b.2))
                    .then(a.3.cmp(&b.3))
            })
            .map(|(_, _, _, cid)| cid)
    }

    /// Whether the siege reservation must leave `cid` alone: it is the one
    /// sanctuary and its Shrine heads the queue.
    pub(super) fn counterweight_shrine_held(&self, g: &Game, pid: usize, cid: u32) -> bool {
        self.counterweight_finishes_one_shrine
            && matches!(g.cities[&cid].queue.first(),
                Some(Item::Building { building }) if building == "shrine")
            && self.counterweight_shrine_city(g, pid) == Some(cid)
    }

    /// Buy the one sanctuary's Shrine with Faith, then Gold, when either
    /// covers it. True when bought.
    pub(super) fn buy_counterweight_shrine(&self, g: &mut Game, pid: usize, cid: u32) -> bool {
        for currency in ["faith", "gold"] {
            let price = if currency == "faith" {
                g.building_faith_purchase_cost(pid, cid, "shrine")
            } else {
                g.building_gold_purchase_cost(pid, cid, "shrine")
            };
            let Some(price) = price else {
                continue;
            };
            let bank = if currency == "faith" {
                g.players[pid].faith
            } else {
                g.players[pid].gold
            };
            if bank + f64::EPSILON < price {
                continue;
            }
            if g.apply(
                pid,
                &Action::BuyBuilding {
                    city: cid,
                    building: crate::name!("shrine"),
                    currency: currency.to_string(),
                },
            )
            .is_ok()
            {
                think!(self.journal(), Faith, Decision,
                    "{} buys its Shrine for the counterweight", g.cities[&cid].name;
                    "counterweight-finishes-one-shrine: {:.0} {} for the one source of counter-faith Missionaries",
                    price, currency);
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests;
