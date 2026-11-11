//! `science-denial-trains-spies`: a space race is answered with enough Spies.
//!
//! `science-denial-every-pad` steers the Spies we have onto every pad of a
//! decisive space racer, but it cannot steer Spies we do not have. Live
//! Emperor G406 (civvis-20261008T140813Z, pin 900d977d6) passed the
//! production gate, took seven cities, and lost to Khmer's Science Victory at
//! turn 215: Khmer landed the Moon by 185, the Mars base by 190 and the
//! Exoplanet Expedition about 208, from five Spaceports. Our Spy capacity read
//! 5 the whole time and we held 0-1 Spies from turn 175 to 215: the only Spy
//! orders after turn 179 were the one agent's Gain Sources, two Disrupt
//! Rocketry runs (186, 204) and a Foment Unrest at 190 — which was the right
//! call, the pad it had just disrupted reading pillaged from 190. Two Spies
//! were queued on turn 177 and none after, while the empire made 628
//! Production a turn at 200: four Spies in parallel in the top cities is
//! about six turns.
//!
//! Against a decisive space racer (`decisive_space_racers`, the every-pad
//! trigger: two space projects landed or a science race at 80%), this claims
//! a queue for one Spy at a time — the host trains them one by one — until
//! held reaches the smaller of our Spy capacity and the racers' pads plus
//! one, in the top-Production city whose queue is idle or holds routine
//! work: never a
//! threatened city, never a defensive building, a military unit or Settler,
//! a Wonder, a project or a repair. The switch banks the displaced item's
//! progress. It runs after the defence, religion, settlement, siege and
//! backlog claims and before the routine ones, and the end-of-turn defence
//! redirects still have the last word.
//!
//! The strategic governor retains the selected Spy while that same unmet
//! race applies. Native Deity Pericles 015051Z/t136 selected a Spy against
//! Khmer after its Satellite, then ordinary review replaced it; Khmer won
//! Science at t189. The earlier claim alone did not reserve production.
//! Siege handling, upkeep/recovery and the final near-done handoff keep
//! their existing priority. The delegated governor already keeps a busy queue.

use super::*;

/// Turns since a city was last attacked under which it keeps its queue.
const SPY_CLAIM_ATTACK_MEMORY: u32 = 4;

impl AdvancedAi {
    /// The Spies a denial needs against `racers`: the smaller of our
    /// capacity and their explored Spaceports plus one.
    pub(crate) fn denial_spies_needed(g: &Game, pid: usize, racers: &BTreeSet<usize>) -> usize {
        let explored = &g.players[pid].explored;
        let pads: usize = racers
            .iter()
            .flat_map(|rival| g.player_city_ids(*rival))
            .map(|cid| {
                g.cities[&cid]
                    .districts
                    .iter()
                    .filter(|(district, pad)| {
                        g.district_family(**district) == "spaceport" && explored.contains(*pad)
                    })
                    .count()
            })
            .sum();
        (g.spy_capacity(pid).max(0) as usize).min(pads + 1)
    }

    /// The Spies the network trains to: [`Self::denial_spies_needed`], or
    /// under `space-denial-spies-from-rocketry` our whole Spy capacity, so the
    /// agents are trained before the racer's pads stand.
    pub(crate) fn denial_spies_wanted(
        &self,
        g: &Game,
        pid: usize,
        racers: &BTreeSet<usize>,
    ) -> usize {
        let needed = Self::denial_spies_needed(g, pid, racers);
        if self.space_denial_spies_from_rocketry {
            needed.max(g.spy_capacity(pid).max(0) as usize)
        } else {
            needed
        }
    }

    /// A running, legal Spy still requested by the existing denial policy.
    /// A quote for a paused or new Spy does not reserve the active queue.
    pub(super) fn denial_spy_queue_committed(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        if !self.science_denial_trains_spies
            || !matches!(item, Item::Unit { unit } if unit == "spy")
            || self.spy_losses_pause_training(g)
        {
            return false;
        }
        let Some(city) = g.cities.get(&cid).filter(|city| city.owner == pid) else {
            return false;
        };
        if city.queue.first() != Some(item)
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0
                && g.turn.saturating_sub(city.last_attacked) <= SPY_CLAIM_ATTACK_MEMORY)
            || !Self::production_commitment_is_legal(g, pid, cid, item)
        {
            return false;
        }
        // `denial-spies-fill-every-slot`: the stand-in racer and the whole
        // capacity. Exactly the denial's own reading with that gene off.
        let racers = self.denial_training_racers(g, pid);
        !racers.is_empty() && g.spy_agents(pid) < self.denial_training_target(g, pid, &racers)
    }

    /// See the module: claim queues for the Spies a decisive space race
    /// needs. Exact no-op while the gene is off.
    pub(super) fn claim_queues_for_denial_spies(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        // `denial-spies-count-their-losses`: the claim stands down while recent
        // Spies keep dying.
        if !self.science_denial_trains_spies || self.spy_losses_pause_training(g) {
            return;
        }
        // `denial-spies-fill-every-slot`: from the first slot, a stand-in
        // racer. Exactly `denial_spy_racers` with that gene off.
        let racers = self.denial_training_racers(g, pid);
        if racers.is_empty() {
            return;
        }
        let spy = Item::Unit {
            unit: crate::name!("spy"),
        };
        let city_ids = g.player_city_ids(pid);
        let queued = city_ids
            .iter()
            .filter(|cid| g.cities[cid].queue.contains(&spy))
            .count();
        let held = g.spy_agents(pid);
        let needed = self.denial_training_target(g, pid, &racers);
        // One Spy in training at a time, in the best city: the host trains
        // them one by one. Live G411 (civvis-20261008T151915Z) sent a second
        // (and third) Spy order on the same turn five times — turns 161, 171,
        // 175, 194 and 210, with capacity 3-5 and 0-3 held — and every one
        // after the first came back refused, leaving its city producing
        // nothing or its old build, while the first trained. Spies already
        // in training count, so the claim waits for the one in the queue.
        // `denial-spies-fill-every-slot`: one more per pass while the host's
        // menu still offers a Spy, which counts those in training; see
        // `advanced/denial_spies_fill_every_slot.rs`.
        let deficit = self.denial_training_claims(g, held, queued, needed);
        if deficit == 0 {
            return;
        }
        let mut candidates: Vec<(f64, u32)> = city_ids
            .iter()
            .copied()
            .filter(|cid| {
                let city = &g.cities[cid];
                let attacked = city.last_attacked > 0
                    && g.turn.saturating_sub(city.last_attacked) <= SPY_CLAIM_ATTACK_MEMORY;
                plan.threatened_city != Some(*cid)
                    && !attacked
                    // `launch-pad-holds-its-queue`: the first pad keeps its
                    // city; the Spy trains in the next-best one.
                    && !self.launch_pad_holds_city(g, pid, *cid)
                    && city
                        .queue
                        .first()
                        .is_none_or(|head| Self::spy_displaces(g, head))
                    && g.can_produce(pid, *cid, &spy)
            })
            .map(|cid| (g.city_yields(cid).production, cid))
            .collect();
        // `racer-keeps-the-spies`: say so when every city is refused a Spy
        // below our capacity. Exact no-op with that gene off.
        if self.racer_keeps_the_spies && candidates.is_empty() {
            let refused_everywhere = city_ids.iter().all(|cid| !g.can_produce(pid, *cid, &spy));
            self.racer_spy_note_held_slots(g, pid, held, needed, refused_everywhere);
        }
        self.rank_spy_producers(g, &mut candidates);
        // `denial-spies-fill-every-slot`: the host's soonest Spy first. Exact
        // no-op with that gene off.
        self.denial_fill_rank_by_quote(g, &mut candidates);
        let rival = racers
            .iter()
            .next()
            .map(|rival| g.players[*rival].civ.to_string())
            .unwrap_or_default();
        for (production, city) in candidates.into_iter().take(deficit) {
            let city_name = g.cities[&city].name.clone();
            if g.apply(
                pid,
                &Action::Produce {
                    city,
                    item: spy.clone(),
                },
            )
            .is_ok()
            {
                *g.players[pid]
                    .counters
                    .entry("denial_spies_trained".to_string())
                    .or_insert(0) += 1;
                think!(self.journal(), Military, Decision,
                       "{city_name} trains a Spy against {rival}'s space race";
                       "science-denial-trains-spies: {held} held and {queued} queued against \
                        {needed} needed; {production:.0} Production a turn here");
            }
        }
    }
}

#[cfg(test)]
mod tests;
