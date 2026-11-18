//! `science-universities-pass-the-claims`: on a Science seat every Campus
//! city's University (or its civilization's replacement) that finishes within
//! [`SCIENCE_UNIVERSITY_ONLINE_TURNS`] Online turns takes the head of its
//! queue each turn, ahead of the economy items the routine claims hold there:
//! an Industrial Zone, a Workshop, a Commercial Hub, a Market, a Harbor and a
//! Lighthouse.
//!
//! **Why (live Deity, 6-player Continents, Online, the 22 Science-seat games
//! of 2026-11-18 that reached turn 100).** Science per Campus city at t150 was
//! 17-30 where a competent human makes 40-60. The Library was not the gap
//! (a median 3-11 turns after the Campus in every game), nor were Natural
//! Philosophy, Rationalism or Pingala. The University was: Education landed
//! at t64-106, and over the 3,627 city-turns in which a Campus city held its
//! Library and Education but no University (t150 and before), the city was
//! building the University (or Madrasa, Navigation School) on 33% of them
//! (`producing_hash` mapped by the corpus's own completions). The rest went
//! to units (21%: Builders, Traders, Settlers, soldiers, Spies), the
//! Industrial Zone, Workshop, Factory and Power Plant (16%), the Commercial
//! Hub, Market, Harbor and Lighthouse (13%) and the Aqueduct (6%). A
//! University is 125 Production at Online, about ten turns at those cities'
//! median 13 Production, yet it stood a median 9-35 turns after Education and
//! its Library; at t100 a median of 20 Science (a quarter of the empire's 75)
//! sat in Universities not yet built, 15 at t125. The best game per Campus
//! city (Rome `civvis-20261118T123536Z`, 59% of those turns on the
//! University) and the worst (Japan `T192816Z`, 16%) bracket the field.
//!
//! **Why the claim did not take them.** `campus-building-claims-the-queue` is
//! armed live and names the University, but it runs after the Settler,
//! Builder, Granary, deterrent, Trader, Aqueduct, Industrial Zone and Factory
//! claims (`run_idle_queue_claims`), takes one city a turn, and yields to
//! every head one of them holds; `trade-district-follows-the-campus` runs
//! after it and puts a Market or Lighthouse back over a University (63
//! city-turns in 13 games journalled "ahead of university"). The gene that
//! lets a University pass those heads, `campus-and-library-before-the-claims`,
//! reads [`AdvancedAi::conquest_or_score_seat`] and so does nothing on a
//! Science seat; `science-seat-campus-before-the-claims` (PR #5473) covers the
//! first Campus and leaves the University to the claim's own rules.
//!
//! **The gene.** While the active victory target is Science, in a city of
//! ours that is not the plan's threatened city, was not attacked within
//! [`super::hint_runs_the_claims::HINT_ATTACK_MEMORY`] turns and, while a
//! major is at war with us, stands away from the front, a University that
//! `campus-building-claims-the-queue` would claim (its Library standing,
//! Education in, the city not due its Granary) and that finishes within the
//! budget:
//!
//! 1. is claimed in every such city each turn, not one city a turn;
//! 2. goes ahead of an Industrial Zone, a Workshop, a Commercial Hub, a
//!    Market, a Harbor or a Lighthouse though a claim holds it or it has
//!    Production in (the displaced item keeps its progress);
//! 3. then holds its queue: the trade-district claim's Market or Lighthouse
//!    does not pass it, nor do the Granary, backlog Builder, deterrent and
//!    siege-reservation writers ([`AdvancedAi::campus_and_library_holds`]).
//!
//! **What keeps its queue.** A Builder, a Settler, a Trader, a Granary, an
//! Aqueduct, a Neighborhood, Walls, every soldier, a Holy Site or Shrine, a
//! wonder, a project, the Factory and power plants, saved work within two
//! turns of done under `near-done-work-finishes`, and anything finishing this
//! turn. `science-seat-campus-before-the-claims` measured what passing a
//! Builder or Granary costs the empire (Production -9.9 at t100); this gene
//! passes neither. A slower University keeps the claim's old rules. The
//! Library and the Research Lab keep the claim's own rules too. Off, nothing
//! changes.
//!
//! Grep the journal for `science-universities-pass-the-claims:`.

use super::campus_and_library_before_the_claims::ScienceFirstReach;
use super::*;

/// The longest University, in Online turns, that passes the claims: 125
/// Production at Online, so a city making about 8.3 a turn. The live
/// University-ready Campus cities made a median 13 Production (quartiles
/// 9.9 and 19, tenth percentile 7.2); 15 turns opens about four in five of
/// those city-turns and leaves the slowest to the old order.
pub(crate) const SCIENCE_UNIVERSITY_ONLINE_TURNS: f64 = 15.0;

impl AdvancedAi {
    /// [`SCIENCE_UNIVERSITY_ONLINE_TURNS`] at this game's speed: production
    /// quotes scale with the speed's costs.
    pub(super) fn science_university_turns(g: &Game) -> f64 {
        SCIENCE_UNIVERSITY_ONLINE_TURNS * g.game_speed.cost_percent() as f64
            / crate::setup::GameSpeed::Online.cost_percent() as f64
    }

    /// Whether the gene opens `cid`'s Campus building of `rank` (1 the
    /// University; see `campus_building_rank`) `turns` from done. False with
    /// the gene off, for another rank or victory target, for a slow
    /// University, for a city not ours, for the plan's threatened city or one
    /// attacked lately, and for a front city while a major is at war with us.
    pub(super) fn science_university_opens(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        rank: u8,
        turns: f64,
    ) -> bool {
        if !self.science_universities_pass_the_claims
            || rank != 1
            || self.active_victory_target(g) != Some(VictoryTarget::Science)
            || !(turns.is_finite() && turns <= Self::science_university_turns(g))
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
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_minor
                && !other.is_barbarian
                && g.is_at_war(pid, other.id)
        });
        !(at_major_war && self.hint_city_in_reach_of_war(g, pid, cid))
    }

    /// The reach `campus-building-claims-the-queue` reads for a University
    /// this gene opens: the economy's heads only, never a soldier.
    pub(super) fn science_university_reach(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        rank: u8,
        turns: f64,
    ) -> Option<ScienceFirstReach> {
        self.science_university_opens(g, pid, cid, plan, rank, turns)
            .then_some(ScienceFirstReach::Economy)
    }

    /// The head `item` of `cid` that the gene's University goes ahead of, as
    /// the journal names it; `None` when the head keeps its queue.
    pub(super) fn science_university_passes(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
    ) -> Option<String> {
        let invested = g.item_invested_production(cid, item);
        let remaining = g.item_cost_for_city(pid, cid, item) - invested;
        if remaining <= g.city_yields(cid).production {
            return None;
        }
        // `near-done-work-finishes` puts saved work within two turns of done
        // back at the head after every writer; leave it there.
        if self.near_done_work_finishes
            && invested > 0.0
            && self.production_build_turns(g, pid, cid, item)
                <= super::near_done_work_finishes::NEAR_DONE_MAX_TURNS
        {
            return None;
        }
        let name = match item {
            Item::Building { building } => {
                let spec = g.rules.buildings.get(building)?;
                if spec.wonder {
                    return None;
                }
                match spec.replaces.unwrap_or(*building).as_str() {
                    "workshop" => "the Workshop",
                    "market" => "the Market",
                    "lighthouse" => "the Lighthouse",
                    _ => return None,
                }
            }
            Item::District { district, .. } => match g.district_family(*district).as_str() {
                "industrial_zone" => "the Industrial Zone",
                "commercial_hub" => "the Commercial Hub",
                "harbor" => "the Harbor",
                _ => return None,
            },
            _ => return None,
        };
        Some(name.to_string())
    }

    /// Whether `item` at the head of `cid` is the University this gene opened
    /// and the city still wants: the trade-district claim and the writers
    /// that read [`AdvancedAi::campus_and_library_holds`] leave it alone.
    /// Always false with the gene off.
    pub(super) fn science_university_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        self.science_universities_pass_the_claims
            && super::campus_building_claims_the_queue::campus_building_rank(g, item) == Some(1)
            && self
                .campus_building_claim_item(g, pid, cid, plan)
                .is_some_and(|(building, rank, turns)| {
                    building == *item
                        && self.science_university_opens(g, pid, cid, plan, rank, turns)
                })
    }

    /// The journal line for a University the gene put first.
    pub(super) fn explain_science_university_passes(
        &self,
        g: &Game,
        cid: u32,
        building: &Item,
        turns: f64,
        passed: &str,
    ) {
        let name = &g.cities[&cid].name;
        let what = Self::plain_item(building);
        think!(self.journal(), Cities, Decision,
               "{name} puts its {what} first";
               "science-universities-pass-the-claims: {name}'s {what} goes ahead of {passed}; \
                {turns:.1} turns");
    }
}

#[cfg(test)]
mod tests;
