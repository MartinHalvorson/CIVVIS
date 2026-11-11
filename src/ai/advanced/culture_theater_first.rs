//! `culture-seat-theater-first`: on a seat aiming at a Culture victory, every
//! unthreatened city puts its first Theater Square (the civilization's
//! replacement: Greece's Acropolis) at the head of its queue once the district
//! is unlocked, and the free-slot Campus claim yields that city's slot to it.
//!
//! **Why (live Prince Pericles, 2026-10-30, the first two Culture seats).**
//! - Drama and Poetry came at t62 and t63, but the first Acropolis was started
//!   only at t110 and t121: 50-60 turns of an unlocked culture district never
//!   built while the Campus claims (`campus-claims-the-free-slot-2`,
//!   `campus-before-the-army-2` and the rest, all forced on the domination
//!   lineage and none gated on the victory target) took every first slot.
//!   civvis-20261030T043401Z stood at 5 / 7 / 10 Campuses and 0 / 0 / 3
//!   Acropoleis at t80 / t100 / t138, with 0 Great Works at t138.
//! - Tourism ran 8 a turn from t60 to t120 in both games, against rivals
//!   already hosting visitors. 043401Z still won at t188, but 044531Z lost
//!   a Culture race to France at t228 with our tourism at 317 and rising:
//!   France held 24 visiting tourists at t150 when we had 0, and the
//!   Theater chain started 50 turns late is the gap.
//!
//! **What the gene does.** For a Culture seat, a city with no Theater Square
//! standing or queued that can finish one in time
//! ([`BasicAi::first_theater_item`]) claims it: an idle queue, or one over a
//! deferrable head that keeps its progress. A Settler, Walls, a wonder, a
//! project, a repair, the faith defence's Holy Site, Shrine or Temple, an
//! Aqueduct, anything finishing this turn, a military unit while a major is
//! at war with us, and the queues other claims hold keep their place. The
//! claim runs after the Campus claims; the free-slot Campus claim yields a
//! city this claim wants ([`AdvancedAi::culture_theater_item`]), and the
//! delegated governor's rescoring leaves the claimed Theater alone
//! ([`AdvancedAi::culture_theater_holds`]).
//!
//! Off, or for a seat not aiming at Culture, nothing changes.
//!
//! **Version two, `culture-seat-theater-first-2`, balances the claim against
//! the Campuses.** A city claims its first Theater Square only while the
//! empire's Theater Squares, standing or queued, number fewer than its
//! Campuses (at least one: the first Theater always goes ahead, which keeps
//! version one's fix); otherwise the free-slot Campus claim keeps that city's
//! slot. The two settle near one each. Live King culture seats today stood
//! at 2-4 Campuses against 4-5 Theaters at t100 (063953Z 2/4, 081602Z 2/4,
//! 091422Z 4/5, 112906Z 3/5) and 2-7 Campuses on 7-10 cities at t150, sat
//! 5-10 technologies behind the rival mean, and lost mostly to a rival
//! Science finish at t222-244. Small cities hold one or two district slots,
//! so a Theater first in every city leaves each Campus waiting for the
//! second. Enabling one version turns the other off.
//!
//! The first district must also finish its first Amphitheater. Native Deity
//! Pericles 20261106T211946Z had three Acropoleis at turn 60, five at 100,
//! and no Amphitheater or Tourism through turn 100. The ordinary governor
//! kept selecting infrastructure; the first Amphitheater arrived after 120.
//! Both versions therefore reserve one safe, fastest build for the empire's
//! first Amphitheater, leaving settlers, defenses and imminent completions
//! under the same protections as the district claim. Additional Amphitheaters
//! may be built in parallel after the first: a safe, solvent city must
//! finish within fourteen Online turns, with at least twenty-eight more turns to use it.
//! Native Deity 20261107T040449Z and 044901Z had five/four Acropoleis
//! at turn 100 but only one Amphitheater through 125; infrastructure kept
//! displacing the remaining writing slots, Culture income and Writer points.
//!
//! The first Art Museum receives the same reservation after Humanism. Native
//! Deity Pericles 20261106T233736Z still had no art slots at turn 162 despite
//! 1,555 Faith and 472 Artist points: patronage correctly refused an unusable
//! creator. One legal Art Museum opens three slots and the Artist economy; an
//! Archaeological Museum does not supply compatible art slots.
//! A reserved University keeps its queue ahead of that first Museum unless a
//! recruited cultural person needs the Museum's slot chain. Native Deity
//! Pericles 20261109T005826Z/t101 claimed a four-turn University and replaced
//! it with a Museum in the same pass; the University first stood at t129.

use super::*;

pub(super) type CultureSiteReservation = (
    Arc<BTreeMap<u32, BTreeSet<String>>>,
    Arc<BTreeMap<u32, BTreeSet<Name>>>,
);

impl AdvancedAi {
    /// Buy one native-quoted hill when ownership alone prevents a Greek
    /// Culture city's first Acropolis. The next host frame, rather than this
    /// suitability hint, must authorize district construction.
    pub(super) fn culture_acropolis_site_purchase(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<CultureSiteReservation> {
        if !self.theater_first_active()
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
            || (self.culture_seat_theater_first_2 && !Self::theaters_trail_campuses(g, pid))
            || g.players[pid].civ != "Greece"
            || !g.players[pid]
                .civics
                .contains(&crate::name!("drama_poetry"))
            || g.players[pid].gold_per_turn < 0.0
            || g.players.iter().any(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && !other.is_free_city
                    && g.is_at_war(pid, other.id)
            })
        {
            return None;
        }
        let reserve = self.purchase_upkeep_floor(
            g,
            pid,
            self.base
                .reserve_floor
                .max(self.threatened_city_gold_floor(g, pid, plan)),
        );

        let acropolis = crate::name!("acropolis");
        let mut choice: Option<(f64, u32, Pos)> = None;
        for cid in g.player_city_ids(pid) {
            let city = &g.cities[&cid];
            // The picker can reject a slow build despite a legal owned site.
            // A positive native placement offer needs no additional hill.
            let native_owned_site = g
                .host_district_plots
                .get(&cid)
                .and_then(|plots| plots.get(&acropolis))
                .is_some_and(|sites| !sites.is_empty());
            if plan.threatened_city == Some(cid)
                || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
                || !g.city_accepts_new_district_site(city, acropolis)
                || city.queue.iter().any(|item| {
                    matches!(item, Item::District { district, .. }
                    if g.district_family(*district) == "theater_square")
                })
                || native_owned_site
                || BasicAi::first_theater_item(g, pid, cid).is_some()
            {
                continue;
            }
            let Some(quotes) = g.host_plot_purchases.get(&cid) else {
                continue;
            };
            for quote in quotes {
                if quote.can_have_acropolis != Some(true) {
                    continue;
                }
                let Some(cost) = g.plot_purchase_cost(pid, cid, quote.pos) else {
                    continue;
                };
                if g.players[pid].gold - cost + f64::EPSILON < reserve {
                    continue;
                }
                let candidate = (cost, cid, quote.pos);
                if choice.is_none_or(|old| {
                    candidate
                        .0
                        .total_cmp(&old.0)
                        .then_with(|| candidate.1.cmp(&old.1))
                        .then_with(|| candidate.2.cmp(&old.2))
                        .is_lt()
                }) {
                    choice = Some(candidate);
                }
            }
        }
        let (cost, city, pos) = choice?;
        if g.apply(pid, &Action::BuyPlot { city, pos, cost }).is_err() {
            return None;
        }
        think!(self.journal(), Cities, Decision,
            "{} buys a native Acropolis site", g.cities[&city].name;
            "culture-seat-theater-first: {cost:.0} Gold at {pos:?}; placement waits for the host's build menu");
        let held = (
            Arc::clone(&g.blocked_production),
            Arc::clone(&g.blocked_districts),
        );
        let mut production = (*held.0).clone();
        let mut districts = (*held.1).clone();
        let actor = &g.cities[&city];
        for (district, spec) in &g.rules.districts {
            if !spec.specialty
                || g.district_family(*district) == "theater_square"
                || actor.owned_tiles.iter().any(|position| {
                    g.map.tiles[position]
                        .district_foundation
                        .as_ref()
                        .is_some_and(|foundation| {
                            g.district_family(foundation.district) == g.district_family(*district)
                        })
                })
            {
                continue;
            }
            districts.entry(city).or_default().insert(*district);
            production
                .entry(city)
                .or_default()
                .insert(format!("district:{district}"));
        }
        g.blocked_districts = Arc::new(districts);
        g.replace_blocked_production(production);
        Some(held)
    }

    pub(super) fn release_culture_acropolis_site(
        g: &mut Game,
        held: Option<CultureSiteReservation>,
    ) {
        if let Some((production, districts)) = held {
            g.blocked_districts = districts;
            g.replace_blocked_production((*production).clone());
        }
    }

    /// Either version is on.
    fn theater_first_active(&self) -> bool {
        self.culture_seat_theater_first || self.culture_seat_theater_first_2
    }

    /// Version two's gate: the empire's Theater Squares, standing or queued,
    /// number fewer than its Campuses, or than one.
    pub(super) fn theaters_trail_campuses(g: &Game, pid: usize) -> bool {
        let holds = |city: &crate::game::City, family: &str| {
            g.city_has_district_family(city, Name::new(family))
                || city.queue.iter().any(|item| {
                    matches!(item, Item::District { district, .. }
                        if g.district_family(*district) == family)
                })
        };
        let (mut theaters, mut campuses) = (0usize, 0usize);
        for city in g.cities.values().filter(|city| city.owner == pid) {
            theaters += usize::from(holds(city, "theater_square"));
            campuses += usize::from(holds(city, "campus"));
        }
        theaters < campuses.max(1)
    }

    /// The first Theater Square `cid` would claim and its build turns: a
    /// Culture seat's unthreatened city of ours with none standing or queued.
    /// `None` with the gene off or for another victory target.
    pub(super) fn culture_theater_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<(Item, f64)> {
        if !self.theater_first_active()
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
        {
            return None;
        }
        if self.culture_seat_theater_first_2 && !Self::theaters_trail_campuses(g, pid) {
            return None;
        }
        let city = g.cities.get(&cid)?;
        if city.owner != pid
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
        {
            return None;
        }
        let queued = city.queue.iter().any(|item| {
            matches!(item, Item::District { district, .. }
                if g.district_family(*district) == "theater_square")
        });
        if queued {
            return None;
        }
        let item = BasicAi::first_theater_item(g, pid, cid)?;
        let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
            g.item_cost_for(pid, &item) / g.city_yields(cid).production.max(0.5)
        });
        Some((item, turns))
    }

    /// Whether `item` at the head of `cid` is the Theater Square this gene
    /// claims: the delegated governor's rescoring leaves it alone. Always
    /// false with the gene off.
    pub(super) fn culture_theater_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        if !self.theater_first_active()
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
        {
            return false;
        }
        if self.culture_faith_source_holds(g, pid, cid, plan, item) {
            return true;
        }
        match item {
            Item::Building { building } => {
                [crate::name!("amphitheater"), crate::name!("art_museum")]
                    .into_iter()
                    .any(|family| {
                        g.building_is_family(*building, family)
                            && (self.culture_needs_first_building(g, pid, family)
                                || (family == "amphitheater"
                                    && Self::culture_followup_amphitheater_turns(
                                        g, pid, cid, plan,
                                    )
                                    .is_some()))
                    })
                    && g.cities.get(&cid).is_some_and(|city| {
                        city.owner == pid
                            && plan.threatened_city != Some(cid)
                            && city.queue.first() == Some(item)
                    })
            }
            Item::District { district, .. } => {
                g.district_family(*district) == "theater_square"
                    && g.cities.get(&cid).is_some_and(|city| {
                        city.owner == pid
                            && plan.threatened_city != Some(cid)
                            && !g.city_has_district_family(city, crate::name!("theater_square"))
                    })
            }
            _ => false,
        }
    }

    /// Whether the queue head `item` may wait for the Theater Square.
    pub(super) fn culture_theater_may_defer(
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
        at_major_war: bool,
    ) -> bool {
        if Self::live_gp_cultural_building_commitment(g, pid, cid, item) {
            return false;
        }
        let remaining =
            g.item_cost_for_city(pid, cid, item) - g.item_invested_production(cid, item);
        if remaining <= g.city_yields(cid).production {
            return false;
        }
        match item {
            Item::Unit { unit } => {
                if *unit == "settler" {
                    return false;
                }
                match g.rules.units.get(unit) {
                    Some(spec) if spec.class == "military" => !at_major_war,
                    Some(_) => matches!(unit.as_str(), "builder" | "scout" | "trader"),
                    None => false,
                }
            }
            Item::Building { building } => g.rules.buildings.get(building).is_some_and(|spec| {
                !spec.wonder
                    && !matches!(
                        building.as_str(),
                        "walls" | "medieval_walls" | "renaissance_walls" | "shrine" | "temple"
                    )
            }),
            Item::District { district, .. } => {
                let family = g.district_family(*district);
                family != crate::name!("aqueduct")
                    && family != crate::name!("holy_site")
                    && family != crate::name!("theater_square")
            }
            _ => false,
        }
    }

    /// See the module: every eligible city of a Culture seat claims its first
    /// Theater Square. Exact no-op with the gene off or for another target.
    pub(super) fn claim_culture_theaters(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if !self.theater_first_active()
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
        {
            return;
        }
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_barbarian
                && !other.is_minor
                && g.is_at_war(pid, other.id)
        });
        for cid in g.player_city_ids(pid) {
            let Some((theater, turns)) = self.culture_theater_item(g, pid, cid, plan) else {
                continue;
            };
            let head = g.cities[&cid].queue.first().cloned();
            if let Some(item) = &head {
                if self.early_settler_floor_holds(g, pid, cid, plan)
                    || self.bound_granary_holds(g, pid, cid, plan, item)
                    || self.capital_monument_holds(g, pid, cid, plan, item)
                    || self.culture_monument_finishes_first(g, pid, cid, plan, item)
                    // See `advanced/acropolis_before_the_deterrent.rs`.
                    // `culture-seat-builds-past-the-deterrent`: see
                    // `advanced/culture_past_the_deterrent.rs`.
                    || (self.deterrent_holds_against_culture(g, pid, item)
                        && !self.quick_theater_passes_the_deterrent(g, turns))
                    || self.idle_route_trader_holds(g, pid, cid, plan, item)
                    || self.backlog_builder_holds(g, pid, cid, plan, item)
                    || self.capped_aqueduct_holds(g, pid, cid, plan, item)
                    || !Self::culture_theater_may_defer(g, pid, cid, item, at_major_war)
                {
                    continue;
                }
            }
            let name = g.cities[&cid].name.clone();
            if g.apply(
                pid,
                &Action::Produce {
                    city: cid,
                    item: theater.clone(),
                },
            )
            .is_ok()
            {
                think!(self.journal(), Cities, Decision,
                       "{name} claims its {} for the Culture victory", Self::plain_item(&theater);
                       "culture-seat-theater-first: {turns:.1} turns, ahead of {}",
                       head.map_or_else(|| "an idle queue".to_string(), |item| format!(
                           "{} (it keeps its progress)", Self::plain_item(&item))));
            }
        }
        for building in [crate::name!("amphitheater"), crate::name!("art_museum")] {
            self.claim_first_culture_building(g, pid, plan, at_major_war, building);
        }
        self.claim_culture_faith_source(g, pid, plan);
    }

    fn culture_needs_first_building(&self, g: &Game, pid: usize, family: Name) -> bool {
        !g.cities.values().any(|city| {
            city.owner == pid
                && city
                    .buildings
                    .iter()
                    .any(|building| g.building_is_family(*building, family))
        })
    }

    /// Complete existing theaters without diverting slow, insolvent or
    /// locally threatened cities from their ordinary recovery work.
    fn culture_followup_amphitheater_turns(
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<f64> {
        let city = g.cities.get(&cid)?;
        if city.owner != pid
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            || (g.players[pid].gold_per_turn < 0.0
                && g.players[pid].gold < 100.0 + 25.0 * g.player_city_ids(pid).len() as f64)
            || g.units.values().any(|unit| {
                unit.owner != pid
                    && g.is_at_war(pid, unit.owner)
                    && g.rules.units[&unit.kind].class == "military"
                    && g.wdist(city.pos, unit.pos) <= 6
            })
        {
            return None;
        }
        let item = Item::Building {
            building: crate::name!("amphitheater"),
        };
        let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
            (g.item_cost_for_city(pid, cid, &item) - g.item_invested_production(cid, &item))
                .max(0.0)
                / g.city_yields(cid).production.max(0.5)
        });
        let cap = g.standard_duration(20) as f64;
        (turns.is_finite()
            && turns <= cap
            && g.max_turns.saturating_sub(g.turn) as f64 > turns + cap * 2.0)
            .then_some(turns)
    }

    fn claim_first_culture_building(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
        at_major_war: bool,
        building: Name,
    ) {
        let first = self.culture_needs_first_building(g, pid, building);
        if !first && building != "amphitheater" {
            return;
        }
        let item = Item::Building { building };
        // Reserve only one empire-opening building. Once Writing slots exist,
        // another city's construction must not serialize the whole empire's
        // Writer points and Culture income. Each followup still passes its own
        // bounded build-time, solvency, survival and queue-commitment checks.
        if first
            && g.cities
                .values()
                .any(|city| city.owner == pid && city.queue.contains(&item))
        {
            return;
        }
        let waiting_for_art_slots = g.players[pid]
            .live_great_person_activation_needs
            .iter()
            .any(|need| {
                BasicAi::live_great_person_work(need).is_some_and(|work| {
                    BasicAi::live_great_person_work_chain(work).contains(&"art_museum")
                        && BasicAi::live_great_person_work_stage_needed(g, pid, work, "art_museum")
                })
            });
        let choice = g
            .player_city_ids(pid)
            .into_iter()
            .filter_map(|cid| {
                let city = &g.cities[&cid];
                if city.queue.contains(&item)
                    || plan.threatened_city == Some(cid)
                    || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
                    || !g.can_produce(pid, cid, &item)
                {
                    return None;
                }
                if let Some(head) = city.queue.first() {
                    // The Campus claim already selected this short University.
                    // Another city can open the first art slots, or this city
                    // can do so after its University. A physical person's
                    // missing activation chain retains its immediate priority.
                    let university_held = building == "art_museum"
                        && !waiting_for_art_slots
                        && matches!(head, Item::Building { building }
                            if g.building_is_family(*building, crate::name!("university")))
                        && self.campus_building_claim_holds(g, pid, cid, plan, head);
                    if self.early_settler_floor_holds(g, pid, cid, plan)
                        || self.bound_granary_holds(g, pid, cid, plan, head)
                        || self.capital_monument_holds(g, pid, cid, plan, head)
                        || self.culture_monument_finishes_first(g, pid, cid, plan, head)
                        || self.deterrent_holds_against_culture(g, pid, head)
                        || self.idle_route_trader_holds(g, pid, cid, plan, head)
                        || self.backlog_builder_holds(g, pid, cid, plan, head)
                        || self.capped_aqueduct_holds(g, pid, cid, plan, head)
                        || university_held
                        || !Self::culture_theater_may_defer(g, pid, cid, head, at_major_war)
                    {
                        return None;
                    }
                }
                let turns = if first {
                    g.host_production_turns(cid, &item).unwrap_or_else(|| {
                        (g.item_cost_for_city(pid, cid, &item)
                            - g.item_invested_production(cid, &item))
                        .max(0.0)
                            / g.city_yields(cid).production.max(0.5)
                    })
                } else {
                    Self::culture_followup_amphitheater_turns(g, pid, cid, plan)?
                };
                Some((turns, cid))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        if let Some((turns, cid)) = choice {
            if g.apply(pid, &Action::Produce { city: cid, item }).is_ok() {
                think!(self.journal(), Cities, Decision,
                    "{} claims {} for the Culture victory", g.cities[&cid].name, building;
                    "culture-seat-theater-first: compatible work slots in {turns:.1} turns (first: {first})");
            }
        }
    }
}

#[cfg(test)]
mod tests;
