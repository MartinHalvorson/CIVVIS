//! A world played on to its last civilization leaves every seat one lane:
//! conquest, fought as a total war. Nothing but the survivor can end the
//! world, so no seat falls back to a defensive plan or chases a victory race
//! that is already decided: each aims at the nearest rival city, and every
//! idle city trains a soldier while the treasury carries the army.
//!
//! **Open.** What still stalls a final war is walls: 56 of 80 siege readings
//! taken in stalled worlds read "nothing to open the walls", one of them a
//! 69-unit train of 5,800 strength held outside a city billed at 552. A rule
//! that trained a siege gun or bomber for every four soldiers took more
//! cities by the same turn in two of four worlds and fewer in the other two,
//! at 1.4 to 1.9 times the time per turn, so it is not here. Sieges that
//! commit without the bill or a breaker measured no better than leaving the
//! doctrine alone.

use super::*;

/// Land soldiers a seat wants per city once only the survivor can win. The
/// ordinary Conquest target is two.
pub(super) const FINAL_WAR_BODIES_PER_CITY: usize = 4;

/// Turns of deficit the treasury must cover for the final war to keep arming
/// on a negative income.
pub(super) const FINAL_WAR_DEFICIT_RUNWAY: f64 = 20.0;

/// Tiles a nearer city must beat the standing objective by before the
/// campaign leaves it.
pub(super) const FINAL_WAR_OBJECTIVE_SLACK: i32 = 3;

/// Whether `pid` is a civilization fighting a world's final war: the world
/// plays to its last civilization and this seat is one of them.
pub(super) fn fights_the_final_war(g: &Game, pid: usize) -> bool {
    g.plays_to_the_last_civ() && !g.players[pid].is_minor && !g.players[pid].is_barbarian
}

impl AdvancedAi {
    /// Once "one more turn" has made the survivor the only possible result,
    /// science, culture, faith and votes can win nothing, so every civilization
    /// plays for conquest for the rest of the game.
    pub(super) fn plays_for_the_last_civ(&mut self, g: &Game, pid: usize) {
        if fights_the_final_war(g, pid) && self.victory_target != Some(VictoryTarget::Domination) {
            self.retarget(VictoryTarget::Domination);
        }
    }

    /// Whether the treasury carries `upkeep` more Gold a turn: income stays
    /// at or above zero, or the bank covers the deficit for
    /// [`FINAL_WAR_DEFICIT_RUNWAY`] turns.
    fn final_war_treasury_carries(g: &Game, pid: usize, upkeep: f64) -> bool {
        let player = &g.players[pid];
        let net = player.gold_per_turn - upkeep;
        net >= 0.0 || player.gold >= FINAL_WAR_DEFICIT_RUNWAY * -net
    }

    /// The land army a seat wants: the shipped target, raised in a final war
    /// to [`FINAL_WAR_BODIES_PER_CITY`] a city while the treasury carries it.
    pub(super) fn final_war_army_target(
        &self,
        g: &Game,
        pid: usize,
        city_count: usize,
        shipped: usize,
    ) -> usize {
        if !fights_the_final_war(g, pid) || !Self::final_war_treasury_carries(g, pid, 0.0) {
            return shipped;
        }
        shipped.max(FINAL_WAR_BODIES_PER_CITY * city_count)
    }

    /// In a final war every idle queue trains the land soldier the scorer
    /// rates best, ahead of the governors, for as long as income (less the
    /// upkeep of the soldiers already at the head of a queue) carries one
    /// more. A queue at work is left alone.
    ///
    /// The governors arm to about two soldiers a city and then build for a
    /// future this world does not have. Measured on the stock site world
    /// before this claim: 96 turns into extra time (seed 14) the strongest
    /// empire banked 3,117 Gold at +1,260 a turn with 19 of its 27 queues
    /// converting reactors, and 146 turns in (seed 23) 21 of an empire's 24
    /// queues stood empty.
    pub(super) fn arm_for_the_final_war(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if !fights_the_final_war(g, pid) {
            return;
        }
        let mut pending: f64 = g
            .player_city_ids(pid)
            .iter()
            .filter_map(|cid| match g.cities[cid].queue.first() {
                Some(Item::Unit { unit }) => Some(g.rules.units[unit].maintenance),
                _ => None,
            })
            .sum();
        for cid in g.player_city_ids(pid) {
            if !g.cities[&cid].queue.is_empty() {
                continue;
            }
            let counts = self.counts(g, pid);
            let mut best: Option<(f64, Item)> = None;
            for item in g.producible_items(pid, cid) {
                let Item::Unit { unit } = &item else { continue };
                let spec = &g.rules.units[unit];
                if spec.class != "military"
                    || spec.domain.as_deref() == Some("sea")
                    || spec.promotion_class == "recon"
                {
                    continue;
                }
                let value = self.production_value(g, pid, cid, &item, plan, &counts);
                if value.is_finite()
                    && value > -1_000.0
                    && best.as_ref().is_none_or(|(old, _)| value > *old)
                {
                    best = Some((value, item));
                }
            }
            let Some((_, item)) = best else { continue };
            let upkeep = match &item {
                Item::Unit { unit } => g.rules.units[unit].maintenance,
                _ => 0.0,
            };
            if !Self::final_war_treasury_carries(g, pid, pending + upkeep) {
                break;
            }
            let name = g.cities[&cid].name.clone();
            let what = Self::plain_item(&item);
            if g.apply(pid, &Action::Produce { city: cid, item }).is_ok() {
                pending += upkeep;
                think!(self.journal(), Cities, Decision,
                    "{name} trains {what} for the final war";
                    "only the last civilization standing wins, so an idle queue arms; income carries {pending:.0} more Gold a turn of soldiers in training");
            }
        }
    }

    /// The final war's objective: the nearest city of a living rival
    /// civilization, one we can march to before one across water, and the
    /// standing objective for as long as nothing is nearer by more than
    /// [`FINAL_WAR_OBJECTIVE_SLACK`]. `None` outside a final war.
    ///
    /// The planner otherwise keeps countering whoever leads a victory race,
    /// and the race that ended the game reads 100% for the rest of it.
    /// Measured on the stock site world (seed 22): all three rivals of the
    /// Science winner named the same one of its cities, and a siege read
    /// "0 of 16 units staged" with a unit of its train 33 tiles out; on seed
    /// 14 the strongest empire's 24-unit train stood 15 to 28 tiles from its
    /// objective.
    pub(super) fn final_war_objective(&self, g: &Game, pid: usize) -> Option<(usize, u32)> {
        if !fights_the_final_war(g, pid) {
            return None;
        }
        let ours: Vec<_> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .collect();
        let mut by_land: BTreeMap<usize, bool> = BTreeMap::new();
        let mut ranked: Vec<(u8, i32, u32, usize)> = Vec::new();
        for city in g.cities.values() {
            let owner = &g.players[city.owner];
            if city.owner == pid
                || !owner.alive
                || owner.is_minor
                || owner.is_barbarian
                || g.same_team(pid, city.owner)
            {
                continue;
            }
            let land = *by_land
                .entry(city.owner)
                .or_insert_with(|| self.rival_reachable_by_land(g, pid, city.owner));
            let reach = ours
                .iter()
                .map(|mine| g.wdist(*mine, city.pos))
                .min()
                .unwrap_or(i32::MAX);
            ranked.push((u8::from(!land), reach, city.id, city.owner));
        }
        let best = ranked.iter().copied().min()?;
        let kept = self
            .plan
            .as_ref()
            .and_then(|plan| plan.target_city)
            .and_then(|prior| {
                ranked.iter().copied().find(|entry| {
                    entry.2 == prior
                        && entry.0 == best.0
                        && entry.1 <= best.1.saturating_add(FINAL_WAR_OBJECTIVE_SLACK)
                })
            });
        let (_, _, city, owner) = kept.unwrap_or(best);
        Some((owner, city))
    }
}

#[cfg(test)]
mod tests;
