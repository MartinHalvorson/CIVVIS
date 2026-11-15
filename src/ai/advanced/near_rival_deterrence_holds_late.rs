//! `near-rival-deterrence-holds-late`: `near-rival-deterrence` past its
//! opening window. From the window's end to the end of the game, on any seat
//! but a Domination one, while a met major at peace has a city within
//! [`DETERRENCE_NEAR_TILES`] of ours and our military power is below
//! [`LATE_DETERRENCE_RATIO`] of its own, the seat answers cheapest first:
//!
//! 1. **Gold.** The standing army is upgraded down to the wartime reserve
//!    (`WARTIME_UPGRADE_FLOOR`, raised to the upkeep reserve when that gene is
//!    on) instead of the peacetime 100-120, while the income is positive. It
//!    is the shared upgrade loop (`BasicAi::modernize_army_with_filter` with
//!    the air wing's filter), ranked as `modernize-before-spending` ranks it,
//!    and stops once we stand at the target.
//! 2. **Production.** The opening gene's claim, unchanged in what it may take:
//!    one idle or routine queue a turn trains the strongest land unit of the
//!    role the army lacks (`combined_arms_unit`) in the city that finishes it
//!    soonest, within `DETERRENCE_MAX_BUILD`, at most `DETERRENCE_MAX_QUEUED`
//!    in training; the early city floor's Settler, Builders, Walls, wonders,
//!    investments in progress and every claim hold the opening gene respects
//!    keep their queues. On a Culture seat a Theater Square, and the buildings
//!    in one, keep theirs too.
//! 3. **Walls.** The next tier of Walls the city can raise (Ancient, Medieval
//!    at Castles, Renaissance at Siege Tactics) in the city nearest that
//!    rival within [`DETERRENCE_NEAR_TILES`], one at a time
//!    ([`LATE_DETERRENCE_MAX_WALLS`]), on the same queue rules.
//!
//! The units and walls it queues are held against the routine rescoring and
//! the other claims exactly as the opening gene's deterrent is
//! (`deterrence_unit_holds`). `deterrence-trusts-a-friend`,
//! `deterrence-spares-the-faith-defence`, `culture-seat-builds-past-the-deterrent`
//! (and its army floor) read the same neighbour and apply unchanged;
//! `deterrence-stops-at-a-garrison` is a Domination rule and this gene never
//! runs on a Domination seat, whose army has its own plan.
//!
//! **Why (live Deity Pericles, civvis-20261115T075925Z, Culture target, six
//! cities).** `near-rival-deterrence` fired 43 times to turn 79 and never
//! after. Our power against Russia's read 461 / 478 at t90, 502 / 795 at t100
//! and 553 / 1094 at t105; Russia declared at t108 at 0.39 of its power, Mali
//! at t110 (904 power), and five of the six cities fell from t112 to t123.
//! The journal from t96 to t108 holds no unit, upgrade or wall claim, only
//! `garrison-before-the-surprise` walks. On the November Deity ladder (915
//! games reaching t100) 3+ cities were lost in 40% of Culture games, 29% of
//! Science, 28% of Religious and 21% of Score games, and about 70% of those
//! with t50/75/100 city marks had lost nothing by t100: the collapses come
//! after the window closes.
//!
//! **Why that game banked Gold.** Its upgrades did run: `modernize-before-
//! spending` took one at t95, t99, t103 and t108 (journal "Modernizing the
//! army before the treasury is spent"). Each Archer to Crossbowman or Hoplite
//! to Pikeman quote was 120-125 Gold and the peacetime reserve is
//! `PEACETIME_UPGRADE_FLOOR` (100), so the treasury had to reach 220-225
//! before one could go; at about 30 Gold a turn the 126-170 seen at t100-105
//! is the reserve plus less than one quote. The Warriors' Man-at-Arms quote
//! needed 10 Iron the seat never had. Lowering the reserve to the wartime 30
//! buys about one upgrade a few turns earlier, once; the income, not the
//! upgrade path, is what limits Gold here, which is why Production carries
//! most of this gene.
//!
//! **Why 0.8.** The opening gene's 10-08 census (76 live Emperor runs,
//! 6,757 rival-turns) read declarations per 100 rival-turns of 1.27 below
//! half our power, 1.01 at 0.5-0.7, 0.77 at 0.7-0.9, 0.41 at 0.9-1.1. The
//! 11-10 Culture census (`culture-pass-keeps-half-the-army`, 37 live Deity
//! Online games) read 1.46 below 0.45, 0.62 at 0.45-0.7 and 0.14 at 0.7 or
//! more. 0.8 sits past the 0.7 step both tables show for a fifth less army
//! than parity, which the late game's larger rival armies make dear. The 11-13
//! Rome Score census (66 games, 125 declarations) found the ratio barely
//! moving the rate (0.70 / 0.48 / 0.59 / 0.72 per 100 at theirs/ours < 1.5 /
//! 1.5-2.5 / 2.5-4 / 4+) while cities lost per war rose with it (0.52 / 0.96
//! / 1.32), so the army may buy the war's outcome rather than the peace.
//!
//! Off: nothing changes (`deterrence_phase` reads the opening window alone).
//! Grep the journal for `near-rival-deterrence-holds-late:`.

use super::near_rival_deterrence::{
    DETERRENCE_END_STANDARD, DETERRENCE_MAX_BUILD, DETERRENCE_NEAR_TILES, DETERRENCE_RATIO,
    DETERRENCE_START_STANDARD,
};
use super::*;

/// Our power against the near neighbour's the late claim builds toward. See
/// the module: past the 0.7 step in both live censuses, a fifth below parity.
pub(crate) const LATE_DETERRENCE_RATIO: f64 = 0.8;

/// Walls under the claim in training at once.
pub(crate) const LATE_DETERRENCE_MAX_WALLS: usize = 1;

/// The wall tiers, oldest first: the first one a city can produce is its next.
const WALL_TIERS: [&str; 3] = ["walls", "medieval_walls", "renaissance_walls"];

/// Which deterrence gene answers this turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DeterrencePhase {
    /// `near-rival-deterrence`, inside its window.
    Opening,
    /// `near-rival-deterrence-holds-late`, from the window's end.
    Late,
}

impl AdvancedAi {
    /// The deterrence gene that answers on this turn, or `None`. With
    /// `near-rival-deterrence-holds-late` off this is exactly the opening
    /// gene's window.
    pub(super) fn deterrence_phase(&self, g: &Game) -> Option<DeterrencePhase> {
        let end = g.standard_duration(DETERRENCE_END_STANDARD);
        if self.near_rival_deterrence
            && g.turn >= g.standard_duration(DETERRENCE_START_STANDARD)
            && g.turn < end
        {
            return Some(DeterrencePhase::Opening);
        }
        (self.near_rival_deterrence_holds_late
            && g.turn >= end
            && self.victory_target != Some(VictoryTarget::Domination))
        .then_some(DeterrencePhase::Late)
    }

    /// Our power against the neighbour's the answering gene builds toward.
    pub(super) fn deterrence_ratio(&self, g: &Game) -> f64 {
        match self.deterrence_phase(g) {
            Some(DeterrencePhase::Late) => LATE_DETERRENCE_RATIO,
            _ => DETERRENCE_RATIO,
        }
    }

    /// The late gene's neighbour while we stand below its target:
    /// (rival, its power, the target). `None` with the gene off, inside the
    /// opening window, on a Domination seat, with no neighbour, or at target.
    pub(super) fn late_deterrence_shortfall(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<(usize, f64, f64)> {
        if self.deterrence_phase(g) != Some(DeterrencePhase::Late) {
            return None;
        }
        let (rival, theirs) = self.deterrence_threat(g, pid)?;
        let target = LATE_DETERRENCE_RATIO * theirs;
        (g.military_power(pid) < target).then_some((rival, theirs, target))
    }

    /// Whether `cid`'s head is Theater Square work a Culture seat keeps from
    /// the late deterrent: the district (Acropolis included) or a building in
    /// it.
    pub(super) fn late_deterrent_spares_the_theater(&self, g: &Game, cid: u32) -> bool {
        if self.active_victory_target(g) != Some(VictoryTarget::Culture) {
            return false;
        }
        match g.cities.get(&cid).and_then(|city| city.queue.first()) {
            Some(Item::Building { building }) => g
                .rules
                .buildings
                .get(building)
                .is_some_and(|spec| spec.district.as_deref() == Some("theater_square")),
            Some(Item::District { district, .. }) => {
                g.district_family(*district) == crate::name!("theater_square")
            }
            _ => false,
        }
    }

    /// Step 1 of the module: upgrades down to the wartime reserve while the
    /// late gene is short of its target and the income is positive. The
    /// upgrades taken; 0 with the gene off.
    pub(super) fn late_deterrence_upgrades(&self, g: &mut Game, pid: usize) -> usize {
        let Some((rival, theirs, target)) = self.late_deterrence_shortfall(g, pid) else {
            return 0;
        };
        if g.players[pid].gold_per_turn <= 0.0 {
            return 0;
        }
        let floor = self.upgrade_treasury_floor(g, pid, WARTIME_UPGRADE_FLOOR);
        let bank = g.players[pid].gold;
        if bank <= floor {
            return 0;
        }
        let before = g.military_power(pid);
        let taken = BasicAi::modernize_army_with_filter(
            g,
            pid,
            floor,
            VETERAN_UPGRADE_WEIGHT,
            |board, uid, unit, cost| {
                board.military_power(pid) < target
                    && self.air_resource_upgrade_preserves_wing(board, pid, uid, unit, cost)
            },
        );
        if taken > 0 {
            let spent = (bank - g.players[pid].gold).max(0.0);
            think!(self.journal(), Military, Decision,
                "Upgrading {taken} unit{} to hold off {}",
                if taken == 1 { "" } else { "s" }, g.players[rival].civ;
                "near-rival-deterrence-holds-late: {before:.0} power against their {theirs:.0}, \
                 target {target:.0} ({LATE_DETERRENCE_RATIO:.1} of theirs); {spent:.0} Gold, \
                 {floor:.0} kept");
        }
        taken
    }

    /// The next tier of Walls `cid` can raise, if any.
    fn late_deterrence_next_wall(g: &Game, pid: usize, cid: u32) -> Option<Item> {
        WALL_TIERS
            .iter()
            .map(|building| Item::Building {
                building: Name::new(building),
            })
            .find(|wall| g.can_produce(pid, cid, wall))
    }

    fn late_deterrence_wall_item(item: &Item) -> bool {
        matches!(item, Item::Building { building } if WALL_TIERS.contains(&building.as_str()))
    }

    /// Whether the late claim still wants the Walls `item` at the head of
    /// `cid`: the gene answers this turn and we are short of target, or the
    /// walls have started. False with the gene off.
    pub(super) fn late_deterrence_walls_hold(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
    ) -> bool {
        Self::late_deterrence_wall_item(item)
            && self.deterrence_phase(g) == Some(DeterrencePhase::Late)
            && (self.late_deterrence_shortfall(g, pid).is_some()
                || g.item_invested_production(cid, item) > 0.0)
    }

    /// Step 3 of the module: the next tier of Walls in the city nearest the
    /// neighbour, one at a time. Exact no-op with the gene off.
    pub(super) fn claim_late_deterrence_walls(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        let Some((rival, theirs, target)) = self.late_deterrence_shortfall(g, pid) else {
            return;
        };
        let city_ids = g.player_city_ids(pid);
        let walls_queued = city_ids
            .iter()
            .filter(|cid| {
                g.cities[*cid]
                    .queue
                    .first()
                    .is_some_and(Self::late_deterrence_wall_item)
            })
            .count();
        if walls_queued >= LATE_DETERRENCE_MAX_WALLS {
            return;
        }
        let rival_cities: Vec<Pos> = g
            .cities
            .values()
            .filter(|city| city.owner == rival)
            .map(|city| city.pos)
            .collect();
        let max_turns = g.standard_duration(DETERRENCE_MAX_BUILD) as f64;
        let mut best: Option<(i32, f64, u32, Item)> = None;
        for &cid in &city_ids {
            let home = g.cities[&cid].pos;
            let border = rival_cities
                .iter()
                .map(|pos| g.wdist(home, *pos))
                .min()
                .unwrap_or(i32::MAX);
            if border > DETERRENCE_NEAR_TILES
                || self.deterrence_spares_the_city(g, pid, cid, plan)
                || self.late_deterrent_spares_the_theater(g, cid)
            {
                continue;
            }
            let Some(wall) = Self::late_deterrence_next_wall(g, pid, cid) else {
                continue;
            };
            let turns = g
                .host_production_turns(cid, &wall)
                .filter(|turns| turns.is_finite() && *turns > 0.0)
                .unwrap_or_else(|| {
                    g.item_remaining_cost_for_city(pid, cid, &wall)
                        / g.city_yields(cid).production.max(0.5)
                });
            if turns > max_turns {
                continue;
            }
            // Nearest the neighbour first, then the quicker build, then the
            // lower city id.
            if best
                .as_ref()
                .is_none_or(|(old_border, old_turns, old_city, _)| {
                    border < *old_border
                        || (border == *old_border
                            && (turns + 1e-9 < *old_turns
                                || ((turns - old_turns).abs() <= 1e-9 && cid < *old_city)))
                })
            {
                best = Some((border, turns, cid, wall));
            }
        }
        let Some((border, turns, cid, wall)) = best else {
            return;
        };
        let displaced = g.cities[&cid].queue.first().cloned();
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: wall.clone(),
            },
        )
        .is_ok()
        {
            let ours = g.military_power(pid);
            think!(self.journal(), Military, Decision,
                "{} raises its {} against {}", g.cities[&cid].name, Self::plain_item(&wall),
                g.players[rival].civ;
                "near-rival-deterrence-holds-late: {ours:.0} power against their {theirs:.0}, \
                 target {target:.0} ({LATE_DETERRENCE_RATIO:.1} of theirs); their city {border} \
                 tiles off; {turns:.1} turns{}",
                displaced.map_or(String::new(), |item| format!(
                    "; {} keeps its progress", Self::plain_item(&item))));
        }
    }
}

#[cfg(test)]
mod tests;
