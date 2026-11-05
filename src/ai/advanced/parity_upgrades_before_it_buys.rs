//! `parity-upgrades-before-it-buys`: when `border-parity` would buy a unit to
//! keep up with the strongest bordering major, it upgrades the army it
//! already has first, never buys a unit an era-pair behind the world, and
//! stops buying once parity is out of reach.
//!
//! ## Why
//!
//! Live Lv 8 Deity Korea Online, games played out (operator, 11-05 ~03:1xZ):
//! three eliminations since 02:01Z, all obsolete armies, not posture.
//! `civvis-20261105T044106Z` lost its capital at t103 holding 10 Archers and
//! 7 Warriors (best strength 20) against Greece's 50; `044845Z` and `033244Z`
//! fell to Mongol (65) and Aztec (86) lines against our 55. Machinery and
//! Iron Working were researched by t100 in every one, yet the eliminated
//! games upgraded 3-4 units each (the survivors 11-26). The Gold went to
//! `border_parity_purchase`, which buys the first affordable of its best
//! ranged or melee unit above its 100-Gold reserve: in 044106Z an 80-Gold
//! Warrior at t36, 43, 52, 62 and 66 ("to hold parity with Canada", 112
//! against 237 power, then 267 against 595), 400 Gold on Warriors while ten
//! Archers waited with Crossbows unlocked.
//!
//! ## What the gene does
//!
//! Inside `border_parity_purchase`, once a bordering major out-powers us:
//! 1. the army is modernised first (`BasicAi::modernize_army`, best strength
//!    gained per Gold, down to the parity reserve); a turn that upgraded
//!    buys nothing;
//! 2. with the rival at [`PARITY_HOPELESS_RATIO`] times our power or more,
//!    nothing is bought: parity is out of reach and the treasury is kept;
//! 3. a unit [`PARITY_BUY_ERAS_BEHIND`] or more eras behind the world era is
//!    never bought.
//! `Kind::OptIn`; off, the purchase is exactly the old one.
//!
//! ## Version three
//!
//! `parity-upgrades-before-it-buys-3`: versions one and two read a unit's age
//! against `g.world_era`, which at Online speed is a clock (live 2026-11-05:
//! era 2 at turn 51, 3 at 71, 4 at 91, 5 at 111, 6 at 131, 7 at 151, whatever
//! anyone had researched). So the `obsolete` bar refused every Ancient unit
//! from turn 51 and every Renaissance unit from 111: the parity path never
//! bought (0 parity purchases live) and version two refused nearly every gold
//! military purchase, early-war defenders included (10+ war turns before turn
//! 100: median 0 buys and 2.5 cities at turn 100 against 1 and 5 without it,
//! n=4 vs 6; disarmed 2026-11-05). Version three carries version one and
//! measures a unit against the newest technology we know
//! (`BasicAi::obsolete_by_our_techs`, three eras: a Science beeline reaches one
//! high-era technology long before the rest of the tree), on the parity path
//! and in `BasicAi::buy_gold_military`.

use super::*;

/// A rival this many times our military power is out of reach for a
/// purchase: buying units chases a number we cannot meet.
pub(crate) const PARITY_HOPELESS_RATIO: f64 = 2.0;
/// A unit this many eras behind the world era is not bought for parity.
pub(crate) const PARITY_BUY_ERAS_BEHIND: usize = crate::ai::OBSOLETE_BUY_ERAS_BEHIND;

impl AdvancedAi {
    /// Upgrades bought with parity's Gold, down to its reserve. The count of
    /// units upgraded; 0 with the gene off.
    pub(super) fn parity_upgrades_first(
        &self,
        g: &mut Game,
        pid: usize,
        rival: usize,
        ours: f64,
        theirs: f64,
    ) -> usize {
        if !self.parity_upgrades_on() {
            return 0;
        }
        let bank = g.players[pid].gold;
        let taken = crate::ai::BasicAi::modernize_army(g, pid, BORDER_PARITY_RESERVE, 0.0);
        if taken > 0 && self.journal().wants(crate::reasoning::Level::Decision) {
            let spent = (bank - g.players[pid].gold).max(0.0);
            think!(self.journal(), Economy, Decision,
                   "Upgrading {taken} unit{} to hold parity with {}",
                   if taken == 1 { "" } else { "s" }, g.players[rival].civ;
                   "parity-upgrades-before-it-buys: {spent:.0} Gold on the army we hold before a \
                    new unit; {ours:.0} power against their {theirs:.0}");
        }
        taken
    }

    /// Why the parity purchase of `unit` is barred, or `None`. Always `None`
    /// with the gene off.
    pub(super) fn parity_buy_barred(
        &self,
        g: &Game,
        pid: usize,
        unit: &str,
        ours: f64,
        theirs: f64,
    ) -> Option<&'static str> {
        if !self.parity_upgrades_on() {
            return None;
        }
        if theirs >= PARITY_HOPELESS_RATIO * ours.max(1.0) {
            return Some("out_of_reach");
        }
        // `parity-upgrades-before-it-buys-3` reads our own technology; the
        // world era versions one and two read is a clock at Online speed.
        let obsolete = if self.parity_upgrades_before_it_buys_3 {
            crate::ai::BasicAi::obsolete_by_our_techs(g, pid, unit)
        } else {
            g.unit_era(unit) + PARITY_BUY_ERAS_BEHIND <= g.world_era
        };
        obsolete.then_some("obsolete")
    }

    /// Whether any version of the gene is on.
    fn parity_upgrades_on(&self) -> bool {
        self.parity_upgrades_before_it_buys
            || self.parity_upgrades_before_it_buys_2
            || self.parity_upgrades_before_it_buys_3
    }
}

impl AdvancedAi {
    /// `parity-upgrades-before-it-buys-2`: the base controller's wartime and
    /// declaration-window defender purchases (`BasicAi::buy_gold_military`)
    /// take the same `obsolete` bar. Set each turn before the base turn runs.
    pub(super) fn obsolete_military_buys_barred(&self) -> bool {
        self.parity_upgrades_before_it_buys_2
    }

    /// `parity-upgrades-before-it-buys-3`: the same two purchase paths, with
    /// a unit obsolete by our own technology rather than the world era's
    /// clock. Set each turn before the base turn runs.
    pub(super) fn obsolete_buys_read_our_techs(&self) -> bool {
        self.parity_upgrades_before_it_buys_3
    }
}

#[cfg(test)]
mod tests;
