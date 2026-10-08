//! `founder-defends-its-cities`: once a rival faith holds a city of ours, a
//! founding seat spends its Faith on its own faith's defenders now, instead
//! of saving for an Inquisition it cannot reach or counting another faith's
//! Missionary as one of its own.
//!
//! Founding is not the defence on its own. Live Emperor civvis-20261008T100610Z
//! founded Buddhism at turn 45 (Holy Site t22, Shrine t29), then held a
//! Catholic Missionary of ours from turn 46 to 111
//! (`founder-spreads-only-its-faith` holds a foreign charge) while the
//! defensive Missionary cap (two) counted it among the corps: two Missionaries
//! in the field from 50 to 70, one of them Catholic, and no purchase. Quito,
//! the Holy City, was Catholic by 60, every city by 80, and with no city left
//! on our faith nothing could be bought: the bank went 322 (t80), 706 (t90),
//! 892 (t95) and the Catholics won at 111. civvis-20261008T101304Z founded at
//! 40 and logged "Saving for the apostle" from turn 49 to 83 at 2, 31 and 121
//! of the Apostle's 200 Faith, which held every ordinary purchase
//! (`prepare_defensive_inquisition` returns true while it saves), bought three
//! Missionaries all game, and lost to Catholicism at 104 as our cities went
//! from 4 of 4 Buddhist at 80 to 5 of 6 Catholic at 100.
//!
//! Under the gene, while a living rival's founded faith holds the majority of
//! at least one city of ours (and only then: a founder converting others is
//! untouched):
//! - the religious corps counts only units of our own faith;
//! - the defensive Missionary cap is at least one per rival-held city plus
//!   one, up to [`FOUNDER_DEFENCE_MAX_MISSIONARIES`];
//! - the Inquisition's next unit is saved for only while it is within
//!   [`FOUNDER_SAVE_TURNS`] turns of Faith income; beyond that the ordinary
//!   defensive purchases run (`inquisition_first_price` names no price and
//!   `prepare_defensive_inquisition` yields the turn);
//! - the Holy City is the first purchase city, so the source is defended
//!   first.
//!
//! Off: unchanged.

use super::*;

/// Turns of Faith income within which the Inquisition's next unit is still
/// worth saving for, holding every other purchase.
pub(crate) const FOUNDER_SAVE_TURNS: f64 = 5.0;

/// The most defensive Missionaries the gene asks for.
pub(crate) const FOUNDER_DEFENCE_MAX_MISSIONARIES: usize = 4;

impl AdvancedAi {
    /// Our cities whose majority follows a faith a living rival founded.
    pub(super) fn rival_held_cities(g: &Game, pid: usize) -> usize {
        let own = g.players[pid].religion.as_deref();
        g.player_city_ids(pid)
            .into_iter()
            .filter(|cid| {
                g.city_religion(&g.cities[cid]).is_some_and(|faith| {
                    Some(faith) != own
                        && g.players.iter().any(|p| {
                            p.id != pid
                                && p.alive
                                && !p.is_minor
                                && !p.is_barbarian
                                && p.religion.as_deref() == Some(faith)
                        })
                })
            })
            .count()
    }

    /// The gene's condition: we founded a religion, Religious Victory is on,
    /// and a rival faith holds at least one city of ours.
    pub(super) fn founder_defence_live(&self, g: &Game, pid: usize) -> bool {
        self.founder_defends_its_cities
            && g.victory_conditions.religious
            && g.players[pid].religion.is_some()
            && Self::rival_held_cities(g, pid) > 0
    }

    /// The Faith our cities yield a turn.
    fn founder_faith_income(g: &Game, pid: usize) -> f64 {
        g.player_city_ids(pid)
            .into_iter()
            .map(|cid| g.city_yields(cid).faith)
            .sum()
    }

    /// Whether `price` is more than [`FOUNDER_SAVE_TURNS`] turns of Faith
    /// income away, with the gene live.
    pub(super) fn founder_inquisition_out_of_reach(
        &self,
        g: &Game,
        pid: usize,
        price: f64,
    ) -> bool {
        self.founder_defence_live(g, pid)
            && price - g.players[pid].faith
                > FOUNDER_SAVE_TURNS * Self::founder_faith_income(g, pid).max(0.0)
    }

    /// Whether the Inquisition's next unit (`unit`) is out of reach in every
    /// city of our faith that could sell it. No city that could sell it reads
    /// as out of reach too: there is nothing to save for.
    pub(super) fn founder_inquisition_unit_out_of_reach(
        &self,
        g: &Game,
        pid: usize,
        unit: &str,
    ) -> bool {
        if !self.founder_defence_live(g, pid) {
            return false;
        }
        let Some(own) = g.players[pid].religion.as_deref() else {
            return false;
        };
        g.player_city_ids(pid)
            .into_iter()
            .filter(|cid| g.city_religion(&g.cities[cid]) == Some(own))
            .filter_map(|cid| Self::inquisition_unit_price(g, pid, cid, unit))
            .min_by(f64::total_cmp)
            .is_none_or(|price| self.founder_inquisition_out_of_reach(g, pid, price))
    }

    /// Our units of `kind` the religious corps counts: every one with the
    /// gene idle, only those of `religion` while it is live.
    pub(super) fn founder_corps_count(
        &self,
        g: &Game,
        pid: usize,
        kind: &str,
        religion: &str,
    ) -> usize {
        let own_only = self.founder_defence_live(g, pid);
        g.units
            .values()
            .filter(|unit| {
                unit.owner == pid
                    && unit.kind == kind
                    && (!own_only || unit.religion.as_deref() == Some(religion))
            })
            .count()
    }

    /// The defensive Missionary cap: `shipped`, or with the gene live at
    /// least one per rival-held city plus one, up to
    /// [`FOUNDER_DEFENCE_MAX_MISSIONARIES`].
    pub(super) fn founder_defence_missionary_cap(
        &self,
        g: &Game,
        pid: usize,
        shipped: usize,
    ) -> usize {
        if !self.founder_defence_live(g, pid) {
            return shipped;
        }
        shipped.max((Self::rival_held_cities(g, pid) + 1).min(FOUNDER_DEFENCE_MAX_MISSIONARIES))
    }

    /// The cities religious purchases try, in order: ours as listed, with the
    /// Holy City first while the gene is live.
    pub(super) fn founder_purchase_order(&self, g: &Game, pid: usize) -> Vec<u32> {
        let mut cities = g.player_city_ids(pid);
        if self.founder_defence_live(g, pid) {
            if let Some(holy) = g.players[pid].holy_city {
                if let Some(at) = cities.iter().position(|cid| *cid == holy) {
                    let holy = cities.remove(at);
                    cities.insert(0, holy);
                }
            }
        }
        cities
    }
}

#[cfg(test)]
mod tests;
