//! `watched-capital-trains-its-defender`: while the capital watch of
//! `capital-walls-before-the-surprise` reads a met major at
//! [`WATCHED_CAPITAL_TRAIN_RATIO`] times our power or more (one of its
//! soldiers seen within
//! [`capital_walls_before_the_surprise::CAPITAL_WATCH_TILES`] of our original
//! capital in the watch window), the capital stands unwalled and fewer than
//! [`capital_walls_before_the_surprise::CAPITAL_GARRISON`] of our soldiers
//! stand beside it, the capital's queue goes to its strongest land defender.
//! The walls gene's own claim, a land soldier already in training, a head that
//! finishes this turn and a Settler within
//! [`capital_walls_before_the_surprise::CAPITAL_WALLS_SETTLER_TURNS`] keep the
//! queue; the displaced item keeps its progress.
//!
//! **Why (Tournament 8 review, 2026-11-26/27, 6 of 40 Lv 8 Deity games after
//! `watched-capital-keeps-its-defender-gold` was armed).** The Gold gene was
//! on and fired in every one of these games, but the reserve it held (70-160
//! Gold) was never in the bank: the treasury read 10-45 Gold. The watch had
//! read for 2-14 turns before each war while the capital's queue went to
//! civilian work: `civvis-t8-20261127T050644Z` (Ethiopia) read Sumeria at
//! 4.8x with 2 defenders missing from t9, and Addis Ababa built its Holy Site,
//! a Monument and a Trader until Sumeria declared at t23; Adwa fell at t25.
//! `T161152Z` (Russia) read the Mapuche at 3.7x with 2 missing at t21, and
//! Moscow started a 17-turn Monument that turn ("2 military for 2 cities
//! against a target of 1.0 each"); Moscow fell at t47. `T162313Z`
//! (Byzantium) read the Cree at 6.8x at t23, war at t25, Constantinople fell
//! at t29 with one Warrior inside. Walls need Masonry and 40 Production; a
//! Warrior or Slinger is a few turns of the capital's own Production and needs
//! no Gold.
//!
//! **What it does.** [`AdvancedAi::claim_watched_capital_defender`] runs after
//! the walls gene's claim. It sets one queue head and nothing else: no
//! research, no Gold, no unit moves. The claim lapses with the watch, when the
//! Walls stand, when the garrison is full, or when our own soldiers bring the
//! ratio under the bar. Off, it is an exact no-op.

use super::*;
use capital_walls_before_the_surprise::{
    CAPITAL_GARRISON, CAPITAL_GARRISON_TILES, CAPITAL_WALLS_SETTLER_TURNS, CAPITAL_WATCH_TILES,
};

/// The watched major's power over ours at which the capital trains its own
/// defender. The live games read 2.9-7.4x before the war.
pub(crate) const WATCHED_CAPITAL_TRAIN_RATIO: f64 = 2.5;

impl AdvancedAi {
    /// The strongest land soldier (no scout) `capital` can train, the cheaper
    /// on a tie, then by name.
    fn watched_capital_trainee(g: &Game, pid: usize, capital: u32) -> Option<Name> {
        let mut best: Option<(f64, f64, Name)> = None;
        for (name, spec) in &g.rules.units {
            let item = Item::Unit { unit: *name };
            if spec.class != "military"
                || spec
                    .domain
                    .as_deref()
                    .is_some_and(|domain| domain != "land")
                || spec.promotion_class == "recon"
                || !(spec.is_melee_capable() || spec.has_ranged_attack())
                || !g.can_produce(pid, capital, &item)
            {
                continue;
            }
            let power = spec.strength.max(spec.ranged_attack_strength());
            let cost = g.item_cost_for(pid, &item);
            if best.as_ref().is_none_or(|(old_power, old_cost, old)| {
                power > *old_power + 1e-9
                    || ((power - old_power).abs() <= 1e-9
                        && (cost, name.as_str()) < (*old_cost, old.as_str()))
            }) {
                best = Some((power, cost, *name));
            }
        }
        best.map(|(_, _, name)| name)
    }

    /// The production claim: the capital's strongest land defender while the
    /// watch reads at [`WATCHED_CAPITAL_TRAIN_RATIO`], the capital is unwalled
    /// and its garrison is short. Exact no-op with the gene off.
    pub(super) fn claim_watched_capital_defender(&self, g: &mut Game, pid: usize) {
        if !self.watched_capital_trains_its_defender || self.base.minor || self.base.barb {
            return;
        }
        let Some(capital) = Self::capital_watch_city(g, pid) else {
            return;
        };
        if g.city_max_wall_hp(&g.cities[&capital]) > 0 {
            return;
        }
        let Some((rival, ratio)) = self.capital_watched_major_power(g, pid) else {
            return;
        };
        if ratio < WATCHED_CAPITAL_TRAIN_RATIO {
            return;
        }
        let garrison = Self::capital_garrison(g, pid, capital);
        if garrison >= CAPITAL_GARRISON {
            return;
        }
        let head = g.cities[&capital].queue.first().cloned();
        if let Some(head) = &head {
            if self.capital_walls_hold(g, pid, capital, head)
                || matches!(head, Item::Unit { unit } | Item::Formation { unit, .. }
                    if g.rules.units.get(unit).is_some_and(|spec| spec.class == "military"
                        && spec.domain.as_deref().is_none_or(|domain| domain == "land")))
            {
                return;
            }
            let production = g.city_yields(capital).production.max(0.1);
            let remaining = g.item_remaining_cost_for_city(pid, capital, head);
            let turns = g
                .host_production_turns(capital, head)
                .filter(|turns| turns.is_finite() && *turns > 0.0)
                .unwrap_or(remaining / production);
            if remaining <= production + f64::EPSILON
                || (matches!(head, Item::Unit { unit } if unit == "settler")
                    && turns <= CAPITAL_WALLS_SETTLER_TURNS)
            {
                return;
            }
        }
        let Some(unit) = Self::watched_capital_trainee(g, pid, capital) else {
            return;
        };
        let item = Item::Unit { unit };
        if g.apply(
            pid,
            &Action::Produce {
                city: capital,
                item: item.clone(),
            },
        )
        .is_err()
        {
            return;
        }
        let turns = g.host_production_turns(capital, &item).unwrap_or_else(|| {
            g.item_remaining_cost_for_city(pid, capital, &item)
                / g.city_yields(capital).production.max(0.1)
        });
        think!(self.journal(), Military, Decision,
            "{} trains its defender ({}) before {} can surprise it",
            g.cities[&capital].name, crate::reasoning::plain(unit.as_str()), g.players[rival].civ;
            "watched-capital-trains-its-defender: {ratio:.1}x our power and its soldiers stood \
             within {CAPITAL_WATCH_TILES} tiles of the unwalled capital; {garrison} of \
             {CAPITAL_GARRISON} of ours within {CAPITAL_GARRISON_TILES}; {turns:.1} turns{}",
            head.map_or(String::new(), |item| format!(
                "; {} keeps its progress", Self::plain_item(&item))));
    }
}

#[cfg(test)]
mod tests;
