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

    /// Whether `item`, at the head of a queue, is routine work a Spy may
    /// displace.
    fn denial_spy_displaces(g: &Game, item: &Item) -> bool {
        match item {
            Item::Building { building } => g
                .rules
                .buildings
                .get(building)
                .is_some_and(|spec| spec.outer_defense <= 0),
            Item::District { .. } => true,
            Item::Unit { unit } => g.rules.units.get(unit).is_some_and(|spec| {
                spec.class != "military" && *unit != "settler" && *unit != "spy"
            }),
            Item::Formation { .. }
            | Item::Wonder { .. }
            | Item::Repair { .. }
            | Item::Project { .. }
            | Item::Product { .. } => false,
        }
    }

    /// See the module: claim queues for the Spies a decisive space race
    /// needs. Exact no-op while the gene is off.
    pub(super) fn claim_queues_for_denial_spies(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !self.science_denial_trains_spies {
            return;
        }
        let racers = self.decisive_space_racers(g, pid);
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
        let needed = Self::denial_spies_needed(g, pid, &racers);
        // One Spy in training at a time, in the best city: the host trains
        // them one by one. Live G411 (civvis-20261008T151915Z) sent a second
        // (and third) Spy order on the same turn five times — turns 161, 171,
        // 175, 194 and 210, with capacity 3-5 and 0-3 held — and every one
        // after the first came back refused, leaving its city producing
        // nothing or its old build, while the first trained. Spies already
        // in training count, so the claim waits for the one in the queue.
        if queued > 0 || held >= needed {
            return;
        }
        let deficit = 1;
        let mut candidates: Vec<(f64, u32)> = city_ids
            .iter()
            .copied()
            .filter(|cid| {
                let city = &g.cities[cid];
                let attacked = city.last_attacked > 0
                    && g.turn.saturating_sub(city.last_attacked) <= SPY_CLAIM_ATTACK_MEMORY;
                plan.threatened_city != Some(*cid)
                    && !attacked
                    && city
                        .queue
                        .first()
                        .is_none_or(|head| Self::denial_spy_displaces(g, head))
                    && g.can_produce(pid, *cid, &spy)
            })
            .map(|cid| (g.city_yields(cid).production, cid))
            .collect();
        candidates.sort_by(|(left, left_city), (right, right_city)| {
            right.total_cmp(left).then(left_city.cmp(right_city))
        });
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
