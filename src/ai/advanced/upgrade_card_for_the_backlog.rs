//! `upgrade-card-for-the-backlog`: every victory target other than Domination
//! slots the half-price upgrade card (Professional Army, then Force
//! Modernization) while its land army carries an upgrade backlog the
//! treasury cannot clear.
//!
//! ## Why
//!
//! 142 live frames of 2026-11-12/13 (turns 100 and 150, 86 games, Gran
//! Colombia Domination and Rome Score, Deity Online), each replayed through
//! the live decider on its first frame of the turn (139 of 142 reproduce the
//! live upgrade orders). Of the land units the host offered a successor, the
//! treasury was the binding constraint for 82% (Domination) and 85% (Score):
//! below the host's price outright, or above it by less than the pass's
//! floor. Territory alone held 7% and 3%, strategic material 8% and 10%; a
//! legal, affordable upgrade the decider skipped, under 1%.
//!
//! The Gold was not going elsewhere. Over the twenty turns before each frame
//! upgrades were the largest single use of it (44-49% of the Gold spent on
//! Domination seats, 48-57% on Score seats); discretionary purchases (Builders,
//! Traders, buildings, deals, levies, tribute) were 23-30%. So the army falls
//! behind on income, and the one lever that makes the same Gold buy more is the
//! card that halves every upgrade bill. `domination_upgrade_policy` slots it
//! only for the Domination target: the Score seats held Mercenaries in 48 of
//! the 62 frames and slotted the card in none, against 38 of 62 Domination
//! frames that held the civic.
//!
//! ## What the gene does
//!
//! For any target but Domination (which keeps its own rule): while two or more
//! land combat units hold an unlocked successor at least [`MODERNIZATION_GAP`]
//! stronger, with the material in stock, and their bill exceeds what the
//! treasury can pay above [`PEACETIME_UPGRADE_FLOOR`], the card the seat can
//! slot goes to the front of the policy portfolio, through the same slot the
//! Domination card takes (it displaces an ordinary military card, never
//! Conscription, Levée en Masse, Limitanei or Praetorium). A held card stays
//! while one such unit remains. `None`, and so the portfolio unchanged, with
//! the gene off.
//!
//! ## Measured
//!
//! `world_tests.rs`, 64 paired worlds (out-teched Immortal shape, Score seat,
//! rival chairs under host tactics, 170 turns; on minus off, mean ± se): the
//! card is held 29 ± 3 of the seat's turns, upgrades bought +4.0 ± 0.9 (20.2
//! to 24.1), strength bought +65 ± 15, the upgradeable share at turn 150
//! 0.54 to 0.48 (-0.07 ± 0.02). Units lost -1.1 ± 0.7 (24.6 to 23.5), to
//! majors -0.8 ± 0.7, killed -0.8 ± 1.3, cities lost +0.02 ± 0.07, taken
//! -0.08 ± 0.05. No cost shows at turn 150: Science +0.4 ± 1.8, Culture
//! +0.1 ± 1.0, cities founded +0.16 ± 0.10, score -2.5 ± 3.9; the seat won
//! 37 of 64 games off and 36 on. The combat gain is within noise at this n.

use super::*;

/// Land units with an unlocked successor before the card is wanted.
pub(crate) const UPGRADE_CARD_BACKLOG_UNITS: usize = 2;

impl AdvancedAi {
    /// Land combat units of `pid` holding an unlocked, materially ready
    /// successor at least [`MODERNIZATION_GAP`] stronger, and the Gold bill to
    /// upgrade all of them (the host's quote where the mirror carries one).
    /// Territory and treasury are ignored: this is the backlog the card would
    /// discount, not what the turn can buy.
    pub(super) fn upgrade_card_backlog(&self, g: &Game, pid: usize) -> (usize, f64) {
        let mut units = 0;
        let mut bill = 0.0;
        for uid in g.player_unit_ids(pid) {
            let unit = &g.units[&uid];
            let from = &g.rules.units[unit.kind];
            if from.class != "military"
                || from.promotion_class == "recon"
                || matches!(from.domain.as_deref(), Some("sea" | "air"))
            {
                continue;
            }
            let Some(target) = g.unit_upgrade_target(pid, unit.kind) else {
                continue;
            };
            let to = &g.rules.units[target];
            let gain = to.strength.max(to.ranged_attack_strength())
                - from.strength.max(from.ranged_attack_strength());
            if gain < MODERNIZATION_GAP {
                continue;
            }
            let Some(resources) = g.unit_upgrade_resource_price(pid, uid, target) else {
                continue;
            };
            if to.requires_resource.is_some_and(|resource| {
                g.strategic_stockpile(pid, resource) + f64::EPSILON < resources
            }) {
                continue;
            }
            let Some((modeled, _)) =
                g.unit_upgrade_price_in_formation(pid, unit.kind, target, unit.formation)
            else {
                continue;
            };
            let gold = g
                .host_unit_facts
                .get(&uid)
                .and_then(|facts| facts.upgrade.as_ref())
                .filter(|offer| offer.to == Some(target))
                .and_then(|offer| offer.cost)
                .unwrap_or(modeled);
            units += 1;
            bill += gold;
        }
        (units, bill)
    }

    /// `upgrade-card-for-the-backlog`: the half-price upgrade card a seat
    /// whose target is not Domination wants at the front of its portfolio, or
    /// `None`. Always `None` with the gene off.
    pub(super) fn backlog_upgrade_card(&self, g: &Game, pid: usize) -> Option<&'static str> {
        if !self.upgrade_card_for_the_backlog
            || self.active_victory_target(g) == Some(VictoryTarget::Domination)
        {
            return None;
        }
        let held = g.players[pid]
            .policies
            .iter()
            .any(|card| matches!(card.as_str(), "professional_army" | "force_modernization"));
        let (units, bill) = self.upgrade_card_backlog(g, pid);
        let treasury = g.players[pid].gold;
        let wanted = units >= UPGRADE_CARD_BACKLOG_UNITS
            && bill > (treasury - PEACETIME_UPGRADE_FLOOR).max(0.0);
        if !wanted && !(held && units > 0) {
            return None;
        }
        let available = g.available_policies(pid);
        let card = ["force_modernization", "professional_army"]
            .into_iter()
            .find(|card| {
                let card = Name::new(card);
                available.contains(&card) || g.players[pid].policies.contains(&card)
            })?;
        if !held {
            think!(self.journal(), Policies, Detail,
                   "The half-price upgrade card for the backlog";
                   "upgrade-card-for-the-backlog: {units} land unit(s) hold an unlocked successor, \
                    a {bill:.0} Gold bill against {treasury:.0} Gold; {} halves it",
                   plain(card));
        }
        Some(card)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod world_tests;
