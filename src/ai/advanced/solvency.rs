//! Two opt-in genes against the bankruptcy spiral.
//!
//! `solvent-or-hold`: while income is not positive nothing may raise the
//! army's upkeep or trade a protection away. The Domination modernizer keeps
//! the upkeep-reserve floor and skips an upgrade whose added upkeep would
//! take income below zero; the policy pass keeps an amenity card (Liberalism,
//! Retainers, …) or an upkeep card (Conscription, Levée en Masse) unless the
//! card replacing it is of the same kind.
//!
//! `bankruptcy-exits-the-drain`: while the treasury holds less than one turn
//! of the deficit, the empire leaves the drain itself. The policy pass puts
//! the gold cards that pay first, an extra surplus-sale pass sells spare
//! luxury and strategic copies every turn, and if income is still negative
//! the cheapest units to lose are disbanded until it is not — never a siege
//! gun, an air unit, a unit an active siege needs, or a city's garrison.
//!
//! **Why (10-14, two Emperor passers lost after the t150 gate).** With gold at
//! 0 and income negative the host charges `amenities_bankruptcy` in every city
//! (1 + one per 10 Gold of deficit): 20 → 30 → 50 across 083713Z's ten cities
//! and 60 → 120 across 100910Z's twelve. Every city went Unhappy, the yield
//! penalty cost 40-90 Gold a turn, and the host disbanded the COSTLIEST units
//! first (`Game::settle_gold_budget`): 29 → 15 and 38 → 14 units. A +216 Gold
//! sale at 083713Z t161 lifted the drain the same turn and took the Unhappy
//! cities from 10 to 1. Income had turned negative through upkeep nobody
//! priced: 083713Z's unit bill went 30 → 70 (t128 → t145) on 24 → 30 units as
//! "Modernizing a standing Domination unit … keep 30 Gold" read only the
//! treasury, and while broke the policy pass traded Liberalism (−10
//! amenities) for a conquest card.

use super::*;

/// Surplus sales a drained turn adds to the ordinary pass.
pub(crate) const DRAIN_SALES_PER_TURN: usize = 2;

/// Units the drain may disband in one turn; the next turn re-reads income.
pub(crate) const DRAIN_DISBANDS_PER_TURN: usize = 3;

/// A gold card estimated below this many Gold a turn is not slotted for the drain.
pub(crate) const DRAIN_CARD_MIN_GOLD: f64 = 2.0;

impl AdvancedAi {
    /// `solvent-or-hold`: income is not positive.
    pub(super) fn solvency_guard(&self, g: &Game, pid: usize) -> bool {
        self.solvent_or_hold && g.players[pid].gold_per_turn <= 0.0
    }

    /// The protection a card gives under `solvent-or-hold`: "amenity" for a
    /// positive amenity effect, "upkeep" for a unit maintenance discount.
    pub(super) fn solvency_card_kind(g: &Game, card: &str) -> Option<&'static str> {
        let spec = g.rules.policies.get(card)?;
        if spec
            .effects
            .iter()
            .any(|(effect, value)| effect.contains("amenit") && *value > 0.0)
        {
            return Some("amenity");
        }
        (spec
            .effects
            .get("unit_maintenance_discount")
            .copied()
            .unwrap_or(0.0)
            > 0.0)
            .then_some("upkeep")
    }

    /// Whether `solvent-or-hold` keeps `current` against `incoming`, with the
    /// incoming card's own estimate. Test convenience for
    /// [`Self::solvency_keeps_card_priced`].
    #[cfg(test)]
    pub(super) fn solvency_keeps_card(
        &self,
        g: &Game,
        pid: usize,
        current: &str,
        incoming: &str,
    ) -> bool {
        self.solvency_keeps_card_priced(g, pid, current, incoming, None)
    }

    /// Whether `solvent-or-hold` keeps `current` against `incoming`. A card
    /// of the same kind may take its slot, and so may a gold card that pays
    /// for what it gives up: the deficit, against an amenity card (income
    /// back above zero ends the bankruptcy drain, worth far more than one
    /// card's amenities), or the card's own upkeep discount, against an
    /// upkeep card. The 10-14 replays held Merchant Confederation (about 41
    /// Gold a turn) behind Civil Prestige without this. `priced` is the
    /// incoming card's Gold a turn when the policy pass already knows it
    /// (Economic Union's wartime marginal: adjacency Gold this estimator
    /// cannot see, which the 10-14 100910Z replay held behind Civil Prestige
    /// at t192); otherwise [`Self::card_gold_estimate`].
    pub(super) fn solvency_keeps_card_priced(
        &self,
        g: &Game,
        pid: usize,
        current: &str,
        incoming: &str,
        priced: Option<f64>,
    ) -> bool {
        if !self.solvency_guard(g, pid) {
            return false;
        }
        let Some(kind) = Self::solvency_card_kind(g, current) else {
            return false;
        };
        if Self::solvency_card_kind(g, incoming) == Some(kind) {
            return false;
        }
        let gold = priced.unwrap_or_else(|| Self::card_gold_estimate(g, pid, incoming));
        match kind {
            "amenity" => gold < DRAIN_CARD_MIN_GOLD.max(-g.players[pid].gold_per_turn),
            _ => gold <= Self::upkeep_card_value(g, pid, current),
        }
    }

    /// The Gold a turn a card's unit maintenance discount saves on our army.
    fn upkeep_card_value(g: &Game, pid: usize, card: &str) -> f64 {
        let discount = g
            .rules
            .policies
            .get(card)
            .and_then(|spec| spec.effects.get("unit_maintenance_discount").copied())
            .unwrap_or(0.0);
        let paying = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let spec = &g.rules.units[g.units[uid].kind];
                spec.class == "military" && spec.maintenance > 0.0
            })
            .count() as f64;
        discount * paying
    }

    /// The Gold a turn a card is estimated to pay: trade Gold by our routes,
    /// Gold per envoy, capital Gold. Zero for a card whose Gold this cannot
    /// estimate or that costs amenities.
    pub(super) fn card_gold_estimate(g: &Game, pid: usize, card: &str) -> f64 {
        let Some(spec) = g.rules.policies.get(card) else {
            return 0.0;
        };
        if spec
            .effects
            .iter()
            .any(|(effect, value)| effect.contains("amenit") && *value < 0.0)
        {
            return 0.0;
        }
        let effect = |key: &str| spec.effects.get(key).copied().unwrap_or(0.0);
        let routes = g.routes.iter().filter(|route| route.owner == pid).count() as f64;
        let envoys: f64 = g.players[pid]
            .envoys
            .iter()
            .map(|(_, placed)| (*placed).max(0) as f64)
            .sum();
        let capital = g
            .player_city_ids(pid)
            .into_iter()
            .any(|cid| g.cities[&cid].is_capital);
        effect("trade_gold") * routes
            + effect("gold_per_envoy") * envoys
            + if capital { effect("capital_gold") } else { 0.0 }
    }

    /// The Gold a turn upgrading `uid` to `target` adds to the unit bill, from
    /// the ruleset's maintenance and the formation multiplier.
    pub(super) fn upgrade_upkeep_delta(g: &Game, uid: u32, target: Name) -> f64 {
        let Some(unit) = g.units.get(&uid) else {
            return 0.0;
        };
        let formation = match unit.formation {
            1 => 1.5,
            2.. => 2.0,
            _ => 1.0,
        };
        let old = g.rules.units[unit.kind].maintenance;
        let new = g
            .rules
            .units
            .get(&target)
            .map_or(old, |spec| spec.maintenance);
        (new - old) * formation
    }

    /// `bankruptcy-exits-the-drain`: income is negative and the treasury holds
    /// less than one turn of it.
    pub(super) fn drained(&self, g: &Game, pid: usize) -> bool {
        let player = &g.players[pid];
        self.bankruptcy_exits_the_drain
            && player.gold_per_turn < 0.0
            && player.gold < -player.gold_per_turn
    }

    /// The gold cards worth slotting while drained, best first, with their
    /// estimated Gold a turn: trade Gold by our routes, capital Gold, Gold per
    /// envoy. Cards whose Gold this cannot estimate, or that cost amenities,
    /// are left to the ordinary portfolio.
    pub(super) fn drain_gold_cards(&self, g: &Game, pid: usize) -> Vec<(&'static str, f64)> {
        if !self.drained(g, pid) {
            return Vec::new();
        }
        let available: HashSet<Name> = g.available_policies(pid).into_iter().collect();
        let mut cards: Vec<(&'static str, f64)> = g
            .rules
            .policies
            .iter()
            .filter(|(name, _)| {
                available.contains(*name) || g.players[pid].policies.contains(*name)
            })
            .map(|(name, _)| {
                (
                    name.as_str(),
                    Self::card_gold_estimate(g, pid, name.as_str()),
                )
            })
            .filter(|(_, gold)| *gold >= DRAIN_CARD_MIN_GOLD)
            .collect();
        cards.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(b.0)));
        cards
    }

    /// `bankruptcy-exits-the-drain`, after diplomacy's ordinary trade and
    /// before production and movement: sell surplus copies, then disband the
    /// cheapest units to lose until income stops running negative. Exact
    /// no-op with the gene off or while the treasury covers the deficit.
    pub(super) fn exit_the_drain(&self, g: &mut Game, pid: usize) {
        if !self.drained(g, pid) {
            return;
        }
        let culture_threats = self.culture_trade_threats(g, pid);
        let science_threats = self.science_threat_seats(g, pid);
        for _ in 0..DRAIN_SALES_PER_TURN {
            if !self.drained(g, pid) {
                break;
            }
            let best = g
                .quick_deals(pid)
                .into_iter()
                .filter(|deal| {
                    deal.direction == "sell"
                        && matches!(deal.category.as_str(), "luxury" | "strategic")
                        && deal.my_value >= 2.0
                        && deal.partner_value >= 2.0
                        && self.culture_deal_allowed(deal, &culture_threats)
                        && self.science_denial_deal_allowed(deal, &science_threats)
                })
                .max_by(|a, b| {
                    a.my_value
                        .total_cmp(&b.my_value)
                        .then(b.partner.cmp(&a.partner))
                        .then(b.item.cmp(&a.item))
                });
            let Some(deal) = best else {
                break;
            };
            let (item, partner) = (deal.item.clone(), deal.partner);
            let (lump, per_turn) = (deal.request.gold, deal.request.gold_per_turn);
            let (gold, income) = (g.players[pid].gold, g.players[pid].gold_per_turn);
            if !self.base.close_quick_deal(g, pid, deal) {
                break;
            }
            think!(self.journal(), Diplomacy, Decision,
                "Selling {} to {} to leave the bankruptcy drain",
                crate::reasoning::plain(&item), g.players[partner].civ;
                "bankruptcy-exits-the-drain: {gold:.0} Gold at {income:+.1} a turn; \
                 asking about {lump:.0} Gold and {per_turn:+.1} a turn");
        }
        if !self.drained(g, pid) {
            return;
        }
        // A gold card slotted this turn pays from the next: the host's income
        // does not show it yet, so wait a turn before disbanding for it.
        if self.drain_card_turn.get() == Some(g.turn) {
            think!(self.journal(), Military, Detail,
                "Waiting a turn before disbanding";
                "bankruptcy-exits-the-drain: a gold card slotted this turn pays from the next");
            return;
        }
        if self.drain_disband_turn.get() == Some(g.turn) {
            think!(self.journal(), Military, Detail,
                "Waiting for refreshed income before another bankruptcy batch";
                "a cleanup batch already disbanded units this turn; keep the surviving army until the next turn's income observation");
            return;
        }
        let mut income = g.players[pid].gold_per_turn;
        let mut candidates: Vec<(f64, u32, f64)> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter_map(|uid| {
                let unit = &g.units[&uid];
                let spec = &g.rules.units[unit.kind];
                let upkeep = g.unit_gold_maintenance_cost(pid, unit);
                let garrison = g
                    .city_at(unit.pos)
                    .is_some_and(|cid| g.cities[&cid].owner == pid);
                (spec.class == "military"
                    && !spec.siege
                    && spec.domain.as_deref() != Some("air")
                    && upkeep > 0.0
                    && !garrison
                    && !self.siege_needs_the_unit(g, pid, uid))
                .then(|| {
                    let power = spec.strength.max(spec.ranged_attack_strength())
                        * f64::from(unit.hp.max(1))
                        / 100.0;
                    (power / upkeep, uid, upkeep)
                })
            })
            .collect();
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (_, uid, upkeep) in candidates.into_iter().take(DRAIN_DISBANDS_PER_TURN) {
            if income >= 0.0 {
                break;
            }
            let kind = g.units[&uid].kind;
            if g.apply(pid, &Action::DisbandUnit { unit: uid }).is_err() {
                continue;
            }
            self.drain_disband_turn.set(Some(g.turn));
            income += upkeep;
            think!(self.journal(), Military, Decision,
                "Disbanding a {} to leave the bankruptcy drain", crate::reasoning::plain(kind.as_str());
                "bankruptcy-exits-the-drain: {:.0} Gold, income {:+.1} after its {upkeep:.0} upkeep; \
                 the host would disband the costliest units first",
                g.players[pid].gold, income);
        }
    }
}

#[cfg(test)]
mod tests;
