//! `plaza-serves-the-scientist-2`: a Science seat's Plaza city builds its next
//! tier building (the Ancestral Hall, then the Intelligence Agency) as soon as
//! it can, ahead of routine work.
//!
//! ## Why
//!
//! Version 1 (`plaza-serves-the-scientist`) chose WHICH tier buildings a
//! Science seat's Plaza takes; nothing made it take them soon. The tier claim
//! of `plaza-in-the-district-list` waits for the Plaza city's queue to stand
//! empty, and on the live seat it rarely does: Builders for the backlog,
//! Settlers and the production hints fill it first. The Intelligence Agency
//! (+1 Spy slot, every offensive operation one level likelier; plaza
//! buildings cannot be bought) stood buildable in the Plaza city and was never
//! built in 4 of 12 Deity Korea science games of 2026-11-05 after version 1
//! was armed (civvis-20261105T131016Z: buildable from turn 88, never queued in
//! 120 turns), and took a median 18 turns where it was. The one Deity win of
//! the lane (civvis-20261105T122513Z) had it at turn 93, 16 turns after it
//! became buildable, and stole 14 Eurekas.
//!
//! ## What the gene does
//!
//! Inert off, and off a Science aim. On, once a turn: the city holding a
//! Government Plaza whose next tier building
//! (`BasicAi::plaza_building_item_for`, the Science list) is buildable and not
//! already queued takes it at the head of its queue, when the head is routine
//! work (`spies-steal-the-lead`'s displacement rule: no defensive building,
//! military unit, Settler, Spy, Wonder or project) and the city is neither the
//! plan's threatened city nor attacked in the last [`PLAZA_TIER_ATTACK_MEMORY`]
//! turns. Version 2 carries version 1: either reads the Science tier list.
//! `Kind::OptIn`. Counter `plaza_tier_claims`.
//!
//! ## Version 3: the Plaza itself
//!
//! `plaza-serves-the-scientist-3` (carries versions 1 and 2) claims the
//! Government Plaza district too. The district's own claim
//! (`plaza-in-the-district-list`) likewise waits for an idle queue, and on the
//! live Indonesia seat of 2026-11-05 none came: of seven full games from
//! 15:19Z, the Plaza stood by turn 54 in two, at turn 177 in one, and never in
//! four (civvis-20261105T165539Z: buildable from turn 37; the capital built
//! Archers, its Campus and Library, a Settler, an Industrial Zone and a
//! Diplomatic Quarter by turn 80, and no Plaza in 174 turns), so neither tier
//! building, nor the Intelligence Agency's Spy slot, ever came. Version 3:
//! once a turn, on a Science aim, with no Plaza standing or at the head of a
//! queue and no Prophet race open, the capital (once its Campus stands; the
//! Campus comes first) takes the Plaza at the head of its queue under the
//! same displacement rule, else the most productive city with a Plaza site
//! and routine work at its head. Counter `plaza_district_claims`.
//!
//! ## The tier building waits for its city's first Campus
//!
//! The displacement rule calls every district routine work, a first Campus
//! included, so in the Plaza city `campus-claims-the-free-slot` put the Campus
//! at the head and this claim put the Ancestral Hall or the Intelligence
//! Agency back over it in the same frame, turn after turn. Live Deity Rome
//! (6-player Continents, Online, the nine Science-target games of 2026-11-13
//! from 15:57Z): seven games journalled "the Plaza's next tier building goes
//! ahead of campus", 60 city-turns in all, and in the seven cities where the
//! Campus was claimed before it was placed it was placed 9, 14, 24, 25, 26,
//! 30 and 32 turns after its first claim. In `civvis-20261113T164001Z` Antium,
//! the second city, claimed its Campus at t34 and placed it at t58; in
//! `161411Z` the Hall went ahead of the capital's own Campus (claimed t33,
//! placed t47). The Plaza district already waits for the capital's Campus
//! ("the Campus comes first"); the tier building now waits the same way: a
//! Campus district at the head of a city that holds no Campus keeps its
//! queue ([`AdvancedAi::plaza_city_owes_its_first_campus`]). Once the Campus
//! stands, or with anything else at the head, nothing changes.

use super::*;

/// Turns since a city was last attacked under which it keeps its queue.
const PLAZA_TIER_ATTACK_MEMORY: u32 = 4;

impl AdvancedAi {
    /// Whether the Science tier list is in force: either version on, on a
    /// Science aim.
    pub(super) fn plaza_serves_science_on(&self) -> bool {
        (self.plaza_serves_the_scientist
            || self.plaza_serves_the_scientist_2
            || self.plaza_serves_the_scientist_3)
            && self.victory_target == Some(VictoryTarget::Science)
    }

    /// Whether a city may give the head of its queue to the Plaza: not the
    /// plan's threatened city, not attacked lately, and routine work (or
    /// nothing) at the head.
    fn plaza_may_claim(&self, g: &Game, plan: &StrategicPlan, cid: u32) -> bool {
        let city = &g.cities[&cid];
        let attacked = city.last_attacked > 0
            && g.turn.saturating_sub(city.last_attacked) <= PLAZA_TIER_ATTACK_MEMORY;
        plan.threatened_city != Some(cid)
            && !attacked
            && city
                .queue
                .first()
                .is_none_or(|head| Self::spy_displaces(g, head))
    }

    /// Version 3 (see the module): the Government Plaza district claims the
    /// capital's queue once its Campus stands, else the most productive city
    /// with a site. Exact no-op with version 3 off or off a Science aim.
    pub(super) fn claim_queue_for_the_plaza_district(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !self.plaza_serves_the_scientist_3
            || self.victory_target != Some(VictoryTarget::Science)
            || !BasicAi::plaza_absent(g, pid)
            || self.base.plaza_prophet_race_open(g, pid)
        {
            return;
        }
        let city_ids = g.player_city_ids(pid);
        let capital = city_ids.iter().copied().find(|cid| {
            let city = &g.cities[cid];
            city.is_capital && city.original_owner == pid
        });
        let campus_first = capital.is_some_and(|cid| {
            !g.city_has_district_family(&g.cities[&cid], crate::name!("campus"))
                || self.capital_builds_its_campus(g, cid)
        });
        let at_capital = capital
            .filter(|cid| !campus_first && self.plaza_may_claim(g, plan, *cid))
            .and_then(|cid| BasicAi::plaza_site_item(g, pid, cid).map(|item| (cid, item)));
        let claim = at_capital.or_else(|| {
            city_ids
                .iter()
                .copied()
                .filter(|cid| Some(*cid) != capital && self.plaza_may_claim(g, plan, *cid))
                .filter_map(|cid| {
                    BasicAi::plaza_site_item(g, pid, cid)
                        .map(|item| (g.city_yields(cid).production, cid, item))
                })
                .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
                .map(|(_, cid, item)| (cid, item))
        });
        let Some((city, item)) = claim else {
            return;
        };
        let displaced = g.cities[&city]
            .queue
            .first()
            .map(Self::plain_item)
            .unwrap_or_else(|| "nothing".to_string());
        if g.apply(
            pid,
            &Action::Produce {
                city,
                item: item.clone(),
            },
        )
        .is_ok()
        {
            *g.players[pid]
                .counters
                .entry("plaza_district_claims".to_string())
                .or_insert(0) += 1;
            think!(self.journal(), Economy, Decision,
                   "{} starts the Government Plaza for the Science plaza", g.cities[&city].name;
                   "plaza-serves-the-scientist-3: the Plaza goes ahead of {displaced}; its \
                    Ancestral Hall and Intelligence Agency (a Spy slot and a level on every \
                    operation) wait on it");
        }
    }

    /// Whether the head of `cid`'s queue is the city's first Campus: a Campus
    /// district in a city that holds none. The tier building does not take
    /// its place; see the module.
    fn plaza_city_owes_its_first_campus(g: &Game, cid: u32) -> bool {
        let city = &g.cities[&cid];
        !g.city_has_district_family(city, crate::name!("campus"))
            && matches!(city.queue.first(), Some(Item::District { district, .. })
                if g.district_family(*district) == "campus")
    }

    /// See the module: the Plaza city's next Science tier building claims its
    /// queue. Exact no-op with version 2 off or off a Science aim.
    pub(super) fn claim_queue_for_the_plaza_tier(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !(self.plaza_serves_the_scientist_2 || self.plaza_serves_the_scientist_3)
            || self.victory_target != Some(VictoryTarget::Science)
        {
            return;
        }
        let claim = g.player_city_ids(pid).into_iter().find_map(|cid| {
            let item = BasicAi::plaza_building_item_for(g, pid, cid, true)?;
            let city = &g.cities[&cid];
            let attacked = city.last_attacked > 0
                && g.turn.saturating_sub(city.last_attacked) <= PLAZA_TIER_ATTACK_MEMORY;
            (!city.queue.contains(&item)
                && plan.threatened_city != Some(cid)
                && !attacked
                && !Self::plaza_city_owes_its_first_campus(g, cid)
                && city
                    .queue
                    .first()
                    .is_none_or(|head| Self::spy_displaces(g, head))
                && g.can_produce(pid, cid, &item))
            .then_some((cid, item))
        });
        let Some((city, item)) = claim else {
            return;
        };
        let displaced = g.cities[&city]
            .queue
            .first()
            .map(Self::plain_item)
            .unwrap_or_else(|| "nothing".to_string());
        if g.apply(
            pid,
            &Action::Produce {
                city,
                item: item.clone(),
            },
        )
        .is_ok()
        {
            *g.players[pid]
                .counters
                .entry("plaza_tier_claims".to_string())
                .or_insert(0) += 1;
            think!(self.journal(), Economy, Decision,
                   "{} starts {} for the Science plaza", g.cities[&city].name, Self::plain_item(&item);
                   "plaza-serves-the-scientist-2: the Plaza's next tier building goes ahead of {displaced}; \
                    the Intelligence Agency is a Spy slot and a level on every operation, and a plaza \
                    building cannot be bought");
        }
    }
}

#[cfg(test)]
mod tests;
