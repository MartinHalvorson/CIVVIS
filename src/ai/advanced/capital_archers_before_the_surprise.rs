//! `capital-archers-before-the-surprise`: while a met major fields
//! [`CAPITAL_ARCHERS_RATIO`] times our military power and one of its land
//! soldiers has stood within eight tiles of our original capital in the last
//! ten live turns (the capital watch `capital-walls-before-the-surprise`
//! keeps), and that capital has no Walls yet, the capital is armed with
//! shooters before the war instead of during it:
//!
//! 1. **Research.** Archery goes ahead of Masonry while a Slinger of ours
//!    stands within [`CAPITAL_ARCHERS_REACH_TILES`] of the capital or the
//!    capital is training one. It interrupts the research in progress the way
//!    the walls' Masonry does (a Masonry already in progress finishes).
//!    Masonry is next, by the walls gene's own leg.
//! 2. **Upgrade.** Every Slinger within that reach takes its Archer upgrade
//!    the turn the host offers it, at peace as at war, down to the upkeep
//!    reserve, before any purchase can spend the price.
//! 3. **Gold.** Those upgrade bills are held back from every purchase
//!    (`purchase_upkeep_floor`), and the peacetime Trader, Builder and tile
//!    purchases wait: what the treasury holds is for the upgrade and for the
//!    defender the threat purchases buy.
//! 4. **Production.** While fewer than [`CAPITAL_ARCHERS_WANTED`] shooters
//!    of ours stand within [`CAPITAL_ARCHERS_HOME_TILES`] of the capital (a
//!    shooter at its queue's head counts) and the empire holds fewer than
//!    [`CAPITAL_ARCHERS_MAX_OWNED`], the capital trains its best shooter
//!    ahead of the next Settler. A head that finishes this turn, a Settler
//!    within two turns of done, a land soldier in training and the walls
//!    gene's own claim keep the queue.
//!
//! Every leg ends when the capital's Walls stand, when the watch lapses, or
//! when the ratio falls under the bar; the upgrade leg alone keeps going for
//! a Slinger that is still waiting.
//!
//! **Why (live Deity, 6 players, Small Continents, Online; 2026-11-15, 33
//! games).** Four surprise wars opened on our seat before turn 30, by majors
//! at 3.4-5.6 times our power, and two of them went for the capital and took
//! it; no capital that a major struck four times or more was held all day.
//!
//! - `civvis-20261115T093423Z` (Pericles): Persia and Nubia met at t7 at
//!   3.3x and 3.6x, the watch read at t10 and Masonry was researched t10-22.
//!   Nubia declared at t16 and struck Athens 12 times with Warriors from t17.
//!   The walls were started at t22 and stood at 34 of 40 when the seat was
//!   defeated at t26. Archery came at t26, and the Slinger upgraded that turn.
//! - `civvis-20261115T093657Z` (Gilgamesh): Rome met at t10 at 3.5x. Archery
//!   was 14 of 25 Science in when the watch read at t14 and Masonry took the
//!   slot until t24. Rome declared at t23; Uruk fell at t30 with its walls at
//!   35 of 40. Archery came at t26; the one Slinger, at 46 health, upgraded
//!   that turn and killed two Warriors in its last three turns. 80 of 111
//!   Gold had bought a Trader at t20.
//! - `civvis-20261115T121939Z` (Pericles): France met at t18 at 5.3x, its
//!   Slinger stood three tiles from Athens at t20, 105 Gold bought a Builder
//!   at t23, France declared at t26 and took both founded cities.
//!
//! Earlier ladders read the same end: the walls stood at 31, 36 and 22 of 40
//! when the capital fell (`advanced/surprise_watch_latch.rs`). Masonry is 40
//! Science at about 3.3 a turn and the walls 40 Production at about 7.5:
//! 17-18 turns from the warning, against a declaration 6-13 turns after it.
//! Archery is 25 Science, and a Slinger already standing becomes an Archer
//! for 30 Gold the turn it lands: 7-8 turns.
//!
//! **What an Archer changes.** In those two sieges every Warrior that struck
//! the city centre took 19-40 damage from it (20 strikes, a mean of 29) and
//! our Slingers' shots did 18-24 from an adjacent tile. An Archer's shot is
//! about 30 from two tiles away, the city centre included, so two of them
//! finish about one struck Warrior a turn where two Slingers finished three
//! in nine turns (inferred from the logged strengths, not measured).
//!
//! Off: nothing changes. The watch itself is recorded when either this gene,
//! the walls gene or `capital-watch-arms-the-slinger` is on. Grep the journal
//! for `capital-archers-before-the-surprise:`.

use super::capital_walls_before_the_surprise::{CAPITAL_WALLS_SETTLER_TURNS, CAPITAL_WATCH_TILES};
use super::*;

/// A watched major's military power over ours that arms the capital's
/// shooters: every declaration before turn 30 on 2026-11-15 came at 3.4 or
/// more, and the neighbours met by turn 15 that never declared stood at
/// 1.1-3.1 in five of nine.
pub(crate) const CAPITAL_ARCHERS_RATIO: f64 = 2.5;

/// Tiles from the original capital within which a Slinger of ours is one of
/// its shooters for the research, the upgrade and the held Gold: the watch's
/// own radius.
pub(crate) const CAPITAL_ARCHERS_REACH_TILES: i32 = CAPITAL_WATCH_TILES;

/// Shooters of ours the production leg wants within
/// [`CAPITAL_ARCHERS_HOME_TILES`] of the capital.
pub(crate) const CAPITAL_ARCHERS_WANTED: usize = 2;
pub(crate) const CAPITAL_ARCHERS_HOME_TILES: i32 = 3;

/// Land shooters the empire may hold before the production leg stops: the
/// board may walk one away, and the capital does not answer that with a
/// fourth.
pub(crate) const CAPITAL_ARCHERS_MAX_OWNED: usize = 3;

/// Upgrades the Gold leg holds a bill for.
pub(crate) const CAPITAL_ARCHERS_HELD_BILLS: usize = 2;

impl AdvancedAi {
    /// The watched major the gene answers and its power over ours: the gene
    /// on, an original capital of ours without Walls, and the capital watch
    /// reading [`CAPITAL_ARCHERS_RATIO`]. `None` otherwise.
    pub(super) fn capital_archers_threat(&self, g: &Game, pid: usize) -> Option<(usize, f64)> {
        if !self.capital_archers_before_the_surprise {
            return None;
        }
        let capital = Self::capital_watch_city(g, pid)?;
        if g.city_max_wall_hp(&g.cities[&capital]) > 0 {
            return None;
        }
        self.capital_archers_watched(g, pid)
    }

    /// The watch alone, whatever the capital's Walls: the upgrade leg keeps
    /// a waiting Slinger's upgrade after the Walls stand.
    fn capital_archers_watched(&self, g: &Game, pid: usize) -> Option<(usize, f64)> {
        if !self.capital_archers_before_the_surprise {
            return None;
        }
        self.capital_watched_major_power(g, pid)
            .filter(|(_, ratio)| *ratio >= CAPITAL_ARCHERS_RATIO)
    }

    /// A land shooter that is no siege engine: a Slinger, an Archer and the
    /// units that replace them.
    fn capital_archers_shooter(spec: &crate::rules::UnitSpec) -> bool {
        spec.class == "military"
            && matches!(spec.domain.as_deref(), None | Some("land"))
            && !spec.siege
            && spec.has_ranged_attack()
    }

    /// A shooter whose reach is short of the Archer's and whose successor is
    /// this civilization's range-two shooter: the Slinger.
    fn capital_archers_waits_for_its_upgrade(g: &Game, pid: usize, kind: Name) -> bool {
        let spec = &g.rules.units[kind];
        Self::capital_archers_shooter(spec)
            && spec.range < super::early_archers::EARLY_ARCHERS_MIN_RANGE
            && spec.upgrade_to.is_some_and(|target| {
                Self::early_archers_shooter(&g.rules.units[g.player_unit_replacement(pid, target)])
            })
    }

    /// Our Slingers within [`CAPITAL_ARCHERS_REACH_TILES`] of the capital,
    /// the nearest first.
    fn capital_archers_slingers(g: &Game, pid: usize, capital: u32) -> Vec<u32> {
        let home = g.cities[&capital].pos;
        let mut slingers: Vec<(i32, u32)> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let unit = &g.units[uid];
                Self::capital_archers_waits_for_its_upgrade(g, pid, unit.kind)
                    && g.wdist(unit.pos, home) <= CAPITAL_ARCHERS_REACH_TILES
            })
            .map(|uid| (g.wdist(g.units[&uid].pos, home), uid))
            .collect();
        slingers.sort();
        slingers.into_iter().map(|(_, uid)| uid).collect()
    }

    /// Whether the capital's queue head is a Slinger.
    fn capital_archers_slinger_in_training(g: &Game, pid: usize, capital: u32) -> bool {
        matches!(g.cities[&capital].queue.first(),
            Some(Item::Unit { unit }) if Self::capital_archers_waits_for_its_upgrade(g, pid, *unit))
    }

    /// The research leg: the node that unlocks this civilization's range-two
    /// shooter, while the threat reads, the empire lacks it, and a Slinger
    /// stands at the capital or is in training there. Read by
    /// `advanced_research` ahead of the walls' Masonry.
    pub(super) fn capital_archers_research_goal(&self, g: &Game, pid: usize) -> Option<Name> {
        self.capital_archers_threat(g, pid)?;
        let node = Self::early_archers_node(g, pid)?;
        if g.players[pid].techs.contains(&node) {
            return None;
        }
        let capital = Self::capital_watch_city(g, pid)?;
        (!Self::capital_archers_slingers(g, pid, capital).is_empty()
            || Self::capital_archers_slinger_in_training(g, pid, capital))
        .then_some(node)
    }

    /// The research in progress gives way to the leg's node, the way it gives
    /// way to the walls' Masonry (`capital_walls_research_switch`), under the
    /// same host contract: both remaining costs known and the node legal now.
    /// A Masonry in progress, and a node on the way, keep the slot.
    pub(super) fn capital_archers_research_switch(&self, g: &Game, pid: usize) -> Option<Name> {
        let current = g.players[pid].research.as_deref()?;
        let goal = self.capital_archers_research_goal(g, pid)?;
        if current == goal.as_str()
            || current == "masonry"
            || self.tech_leads_to(g, current, goal.as_str())
            || !g.host_research_prerequisites_revealed(goal)
            || g.host_can_research(pid, goal) != Some(true)
            || !g.available_techs(pid).contains(&goal)
            || g.host_remaining_research_cost(pid, Name::new(current))
                .is_none()
            || g.host_remaining_research_cost(pid, goal).is_none()
        {
            return None;
        }
        Some(goal)
    }

    /// What the research journal says when the leg takes the slot.
    pub(super) fn capital_archers_research_reason(&self, g: &Game, pid: usize) -> Option<String> {
        let (rival, ratio) = self.capital_archers_threat(g, pid)?;
        let capital = Self::capital_watch_city(g, pid)?;
        let slingers = Self::capital_archers_slingers(g, pid, capital).len();
        Some(format!(
            "capital-archers-before-the-surprise: Archery before Masonry; {} fields {ratio:.1}x our \
             power, its soldiers stood within {CAPITAL_WATCH_TILES} tiles, and {slingers} Slinger(s) \
             at the capital upgrade the turn it lands",
            g.players[rival].civ
        ))
    }

    /// The Gold leg's held bills: the Archer upgrade of up to
    /// [`CAPITAL_ARCHERS_HELD_BILLS`] Slingers at the capital, while the
    /// threat reads. Read by `purchase_upkeep_floor`. Zero with the gene off.
    pub(super) fn capital_archers_gold_held(&self, g: &Game, pid: usize) -> f64 {
        if self.capital_archers_threat(g, pid).is_none() {
            return 0.0;
        }
        let Some(capital) = Self::capital_watch_city(g, pid) else {
            return 0.0;
        };
        Self::capital_archers_slingers(g, pid, capital)
            .into_iter()
            .take(CAPITAL_ARCHERS_HELD_BILLS)
            .filter_map(|uid| {
                let unit = &g.units[&uid];
                let target = g.rules.units[unit.kind].upgrade_to?;
                let target = g.player_unit_replacement(pid, target);
                g.unit_upgrade_price_in_formation(pid, unit.kind, target, unit.formation)
                    .map(|(gold, _)| gold)
            })
            .sum()
    }

    /// The Gold leg's other half: the peacetime Trader, Builder and tile
    /// purchases wait while the threat reads. False with the gene off.
    pub(super) fn capital_archers_hold_the_chest(&self, g: &Game, pid: usize) -> bool {
        let Some((rival, ratio)) = self.capital_archers_threat(g, pid) else {
            return false;
        };
        think!(self.journal(), Economy, Detail,
            "The treasury waits on the capital's defence";
            "capital-archers-before-the-surprise: {} fields {ratio:.1}x our power at the unwalled \
             capital's door; no Trader, Builder or tile is bought, {:.0} Gold is kept for the Archer \
             upgrade and the defender",
            g.players[rival].civ, g.players[pid].gold);
        true
    }

    /// The upgrade leg: every Slinger at the capital takes the Archer upgrade
    /// the host offers, down to the upkeep reserve. Runs before the turn's
    /// purchases. Returns how many upgraded; zero with the gene off.
    pub(super) fn capital_archers_upgrade(&self, g: &mut Game, pid: usize) -> usize {
        let Some((rival, ratio)) = self.capital_archers_watched(g, pid) else {
            return 0;
        };
        let Some(capital) = Self::capital_watch_city(g, pid) else {
            return 0;
        };
        let mut upgraded = 0;
        for uid in Self::capital_archers_slingers(g, pid, capital) {
            // The offer honours the host's own verdict, territory, movement,
            // successor, price and material bill.
            let Some((target, price, resources)) = g.unit_gold_upgrade_offer(pid, uid) else {
                continue;
            };
            if !Self::early_archers_shooter(&g.rules.units[target])
                || resources != 0.0
                || !price.is_finite()
                || price < 0.0
            {
                continue;
            }
            let delta = Self::upgrade_upkeep_delta(g, uid, target);
            let bill = g.unit_gold_maintenance(pid);
            let reserve =
                super::gold_and_cards::UPKEEP_RESERVE_TURNS * (bill + delta).max(0.0);
            let gold = g.players[pid].gold;
            if !delta.is_finite() || !bill.is_finite() || !gold.is_finite() || gold - price < reserve
            {
                continue;
            }
            let from = g.units[&uid].kind;
            if g.apply(pid, &Action::UpgradeUnit { unit: uid }).is_err() {
                continue;
            }
            upgraded += 1;
            think!(self.journal(), Military, Decision,
                "The capital's {} becomes its {}",
                crate::reasoning::plain(from.as_str()), crate::reasoning::plain(target.as_str());
                "capital-archers-before-the-surprise: {} fields {ratio:.1}x our power and its \
                 soldiers stood within {CAPITAL_WATCH_TILES} tiles; {price:.0} of {gold:.0} Gold, \
                 {reserve:.0} kept for upkeep",
                g.players[rival].civ);
        }
        upgraded
    }

    /// Our shooters within [`CAPITAL_ARCHERS_HOME_TILES`] of the capital, and
    /// those the empire holds.
    fn capital_archers_census(g: &Game, pid: usize, capital: u32) -> (usize, usize) {
        let home = g.cities[&capital].pos;
        let mut near = 0;
        let mut owned = 0;
        for unit in g.units.values() {
            if unit.owner == pid && Self::capital_archers_shooter(&g.rules.units[unit.kind]) {
                owned += 1;
                if g.wdist(unit.pos, home) <= CAPITAL_ARCHERS_HOME_TILES {
                    near += 1;
                }
            }
        }
        (near, owned)
    }

    /// The best shooter `capital` can train: the longest reach, then the
    /// strongest shot, then the cheapest.
    fn capital_archers_trainee(g: &Game, pid: usize, capital: u32) -> Option<Name> {
        g.rules
            .units
            .iter()
            .filter(|(name, spec)| {
                Self::capital_archers_shooter(spec)
                    && g.can_produce(pid, capital, &Item::Unit { unit: **name })
            })
            .max_by(|(a_name, a), (b_name, b)| {
                a.range
                    .cmp(&b.range)
                    .then(
                        a.ranged_attack_strength()
                            .total_cmp(&b.ranged_attack_strength()),
                    )
                    .then(b.cost.total_cmp(&a.cost))
                    .then(b_name.as_str().cmp(a_name.as_str()))
            })
            .map(|(name, _)| *name)
    }

    /// Whether the production leg still wants `item` at the head of `cid`'s
    /// queue: a shooter in the watched, unwalled capital while it is short of
    /// them.
    pub(super) fn capital_archers_hold(&self, g: &Game, pid: usize, cid: u32, item: &Item) -> bool {
        let Item::Unit { unit } = item else {
            return false;
        };
        if Self::capital_watch_city(g, pid) != Some(cid)
            || !g
                .rules
                .units
                .get(unit)
                .is_some_and(Self::capital_archers_shooter)
            || self.capital_archers_threat(g, pid).is_none()
        {
            return false;
        }
        let (near, owned) = Self::capital_archers_census(g, pid, cid);
        near < CAPITAL_ARCHERS_WANTED && owned < CAPITAL_ARCHERS_MAX_OWNED
    }

    /// The production leg, after the walls gene's own claim: the capital's
    /// best shooter ahead of the next Settler while it is short of them.
    /// Exact no-op with the gene off.
    pub(super) fn claim_capital_archers_before_the_surprise(&self, g: &mut Game, pid: usize) {
        let Some((rival, ratio)) = self.capital_archers_threat(g, pid) else {
            return;
        };
        let Some(capital) = Self::capital_watch_city(g, pid) else {
            return;
        };
        let head = g.cities[&capital].queue.first().cloned();
        if let Some(head) = &head {
            // The walls gene's claim, a land soldier in training (a shooter
            // among them) and work about to finish keep the queue.
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
        let (near, owned) = Self::capital_archers_census(g, pid, capital);
        if near >= CAPITAL_ARCHERS_WANTED || owned >= CAPITAL_ARCHERS_MAX_OWNED {
            return;
        }
        let Some(unit) = Self::capital_archers_trainee(g, pid, capital) else {
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
            "{} trains a {} before {} can surprise it",
            g.cities[&capital].name, crate::reasoning::plain(unit.as_str()), g.players[rival].civ;
            "capital-archers-before-the-surprise: {ratio:.1}x our power and its soldiers stood \
             within {CAPITAL_WATCH_TILES} tiles; {near} of {CAPITAL_ARCHERS_WANTED} shooters within \
             {CAPITAL_ARCHERS_HOME_TILES} tiles; {turns:.1} turns{}",
            head.map_or(String::new(), |item| format!(
                "; {} keeps its progress", Self::plain_item(&item))));
    }
}

#[cfg(test)]
mod tests;
