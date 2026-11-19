//! `culture-theater-buildings-pass-the-claims`: on a Culture seat every
//! Theater Square city's next Theater building (its Amphitheater, then its
//! Art or Archaeological Museum, then its Broadcast Center, or the
//! civilization's replacement) that finishes within
//! [`CULTURE_THEATER_BUILDING_ONLINE_TURNS`] Online turns takes the head of
//! its queue each turn, ahead of the Campus, trade, industry, harbour and
//! housing items the routine claims hold there.
//!
//! **Why (live Deity, 6-player Continents, Online, the 30 Culture-seat games
//! of 2026-11-18/19 that reached t150).** Great Works win the Culture race,
//! and their slots and much of their Great Person points come from the
//! Theater's buildings (Amphitheater +1 Writer point, a Museum +1 Writer and
//! +2 Artist points and three slots, a Broadcast Center +1 Artist and +2
//! Musician points). Over the 7,349 city-turns (t175 and before) in which a
//! city held its Amphitheater and Humanism but no Museum, the city was
//! building the Museum on 11.6% of them; a median 30-50 turns passed before
//! it stood, about 485 Writer and Artist points a game by t175, and the strong
//! seats' art slots were full at t150-175 (Gorgo: ten Theater cities, three
//! Museums). The Amphitheater waited too (4,639 city-turns, 27% building it).
//! The Museum's turns went to soldiers (16.0%), Faith sources (13.5%),
//! Builders (7.9%), the Campus, Library and University (12.7%), Spies
//! (5.9%), Neighborhoods (5.6%), the Commercial Hub, Market, Industrial Zone,
//! Workshop, Factory and Harbor (7.8%), the growth buildings (4.1%), Settlers
//! (2.7%), Walls (2.6%) and projects (2.2%). By the journal the Campus heads
//! came from `campus-claims-the-free-slot`, `campus-building-claims-the-queue`
//! and `culture-seat-fills-its-district-slots`, the Neighborhoods from
//! `neighborhood-houses-the-capped-city`, the industry from the
//! Industrial Zone claims. `culture-seat-theater-first` claims only the
//! empire's first Amphitheater and first Art Museum and one follow-up
//! Amphitheater a turn, and runs before those writers.
//!
//! **The gene.** While the active victory target is Culture and the lane is
//! not released, in a city of ours that is not the plan's threatened city,
//! was not attacked within [`super::hint_runs_the_claims::HINT_ATTACK_MEMORY`]
//! turns and, while a major is at war with us, stands away from the front, and
//! while the treasury is not running dry, the next Theater building that
//! finishes within the budget goes to the head of the queue after the
//! routine writers have run, ahead of a Campus, Library or University, a
//! Commercial Hub, Market or Bank, an Industrial Zone, Workshop or Factory, a
//! Harbor, Lighthouse or Shipyard, or a Neighborhood (the displaced item keeps
//! its progress). Everything else keeps its queue: Builders, Settlers,
//! Traders, Spies (the space-denial counter), every soldier and ship,
//! Granaries, Aqueducts and the other growth buildings, Walls, Faith sources,
//! Monuments, wonders, projects, anything finishing this turn and saved work
//! within two turns of done under `near-done-work-finishes`. It runs before
//! `paused-district-finishes`, `near-done-work-finishes` and every defence
//! handoff, which keep the last word. A Museum is the Archaeological one
//! while `culture-digs-one-museum` still wants one, else the Art Museum.
//! Off, nothing changes.
//!
//! Grep the journal for `culture-theater-buildings-pass-the-claims:`.

use super::*;

/// The longest Theater building, in Online turns, that passes the claims. A
/// Museum is 145 Production at Online; the Museum-ready cities made a median
/// 15 Production (quartiles 12 and 21), so 15 turns opens about four in five
/// of those city-turns and leaves the slowest to the old order.
pub(crate) const CULTURE_THEATER_BUILDING_ONLINE_TURNS: f64 = 15.0;

impl AdvancedAi {
    /// [`CULTURE_THEATER_BUILDING_ONLINE_TURNS`] at this game's speed.
    fn culture_theater_building_turns(g: &Game) -> f64 {
        CULTURE_THEATER_BUILDING_ONLINE_TURNS * g.game_speed.cost_percent() as f64
            / crate::setup::GameSpeed::Online.cost_percent() as f64
    }

    /// `cid`'s next Theater building: the Amphitheater, then a Museum, then
    /// the Broadcast Center (each the civilization's own version), if the
    /// city can produce it now.
    pub(super) fn culture_theater_building_next(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
    ) -> Option<Item> {
        let city = g.cities.get(&cid)?;
        let holds = |family: &str| {
            let family = Name::new(family);
            city.buildings
                .iter()
                .any(|building| g.building_is_family(*building, family))
        };
        if !holds("amphitheater") {
            return BasicAi::civ_building(g, pid, cid, "amphitheater");
        }
        if !holds("art_museum") && !holds("archaeological_museum") {
            let dig_first =
                self.dig_museum_bonus(g, pid, crate::name!("archaeological_museum")) > 0.0;
            let (first, second) = if dig_first {
                ("archaeological_museum", "art_museum")
            } else {
                ("art_museum", "archaeological_museum")
            };
            return BasicAi::civ_building(g, pid, cid, first)
                .or_else(|| BasicAi::civ_building(g, pid, cid, second));
        }
        if !holds("broadcast_center") {
            return BasicAi::civ_building(g, pid, cid, "broadcast_center");
        }
        None
    }

    /// Whether the gene opens `cid` for a Theater building `turns` from
    /// done. False with the gene off, off the Culture lane or once it is
    /// lost, for a slow building, for a city not ours, for the plan's
    /// threatened city or one attacked lately, for a front city while a major
    /// is at war with us, and while the treasury runs dry.
    pub(super) fn culture_theater_building_opens(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        turns: f64,
    ) -> bool {
        if !self.culture_theater_buildings_pass_the_claims
            || self.lane_lost
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
            || !(turns.is_finite() && turns <= Self::culture_theater_building_turns(g))
        {
            return false;
        }
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        let memory = super::hint_runs_the_claims::HINT_ATTACK_MEMORY;
        if city.owner != pid
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= memory)
        {
            return false;
        }
        let player = &g.players[pid];
        if player.gold_per_turn < 0.0
            && player.gold < 100.0 + 25.0 * g.player_city_ids(pid).len() as f64
        {
            return false;
        }
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_minor
                && !other.is_barbarian
                && g.is_at_war(pid, other.id)
        });
        !(at_major_war && self.hint_city_in_reach_of_war(g, pid, cid))
    }

    /// The head `item` of `cid` that a Theater building goes ahead of, as the
    /// journal names it; `None` when the head keeps its queue.
    pub(super) fn culture_theater_building_passes(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
    ) -> Option<&'static str> {
        let invested = g.item_invested_production(cid, item);
        let remaining = g.item_cost_for_city(pid, cid, item) - invested;
        if remaining <= g.city_yields(cid).production {
            return None;
        }
        if self.near_done_work_finishes
            && invested > 0.0
            && self.production_build_turns(g, pid, cid, item)
                <= super::near_done_work_finishes::NEAR_DONE_MAX_TURNS
        {
            return None;
        }
        match item {
            Item::Building { building } => {
                let spec = g.rules.buildings.get(building)?;
                if spec.wonder {
                    return None;
                }
                match spec.replaces.unwrap_or(*building).as_str() {
                    "library" => Some("the Library"),
                    "university" => Some("the University"),
                    "market" => Some("the Market"),
                    "bank" => Some("the Bank"),
                    "workshop" => Some("the Workshop"),
                    "factory" => Some("the Factory"),
                    "lighthouse" => Some("the Lighthouse"),
                    "shipyard" => Some("the Shipyard"),
                    _ => None,
                }
            }
            Item::District { district, .. } => match g.district_family(*district).as_str() {
                "campus" => Some("the Campus"),
                "commercial_hub" => Some("the Commercial Hub"),
                "industrial_zone" => Some("the Industrial Zone"),
                "harbor" => Some("the Harbor"),
                "neighborhood" => Some("the Neighborhood"),
                _ => None,
            },
            _ => None,
        }
    }

    /// See the module: every open city's next Theater building takes the
    /// head over the passable heads. Exact no-op with the gene off.
    pub(super) fn claim_culture_theater_buildings(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !self.culture_theater_buildings_pass_the_claims
            || self.lane_lost
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
        {
            return;
        }
        for cid in g.player_city_ids(pid) {
            let Some(building) = self.culture_theater_building_next(g, pid, cid) else {
                continue;
            };
            let Some(head) = g.cities[&cid].queue.first().cloned() else {
                continue;
            };
            if head == building {
                continue;
            }
            let turns = g.host_production_turns(cid, &building).unwrap_or_else(|| {
                let remaining = g.item_cost_for_city(pid, cid, &building)
                    - g.item_invested_production(cid, &building);
                remaining.max(0.0) / g.city_yields(cid).production.max(0.5)
            });
            if !self.culture_theater_building_opens(g, pid, cid, plan, turns) {
                continue;
            }
            let Some(passed) = self.culture_theater_building_passes(g, pid, cid, &head) else {
                continue;
            };
            if g.apply(
                pid,
                &Action::Produce {
                    city: cid,
                    item: building.clone(),
                },
            )
            .is_ok()
            {
                let name = &g.cities[&cid].name;
                let what = Self::plain_item(&building);
                think!(self.journal(), Cities, Decision,
                       "{name} puts its {what} first";
                       "culture-theater-buildings-pass-the-claims: {name}'s {what} goes ahead \
                        of {passed} (it keeps its progress); {turns:.1} turns");
            }
        }
    }
}

#[cfg(test)]
mod tests;
