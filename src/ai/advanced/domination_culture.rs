//! `domination-seat-raises-its-culture`: a Domination seat raises a Monument
//! in every city of two citizens or more ahead of a routine queue, and
//! Pingala takes Connoisseur as her second promotion.
//!
//! **Why (live Deity Gran Colombia, 4-player Tiny Pangaea, Online: the 20
//! games `civvis-20261112T085526Z` to `T124244Z`).** Civics give a
//! Domination seat its governments, its military policy cards and Corps and
//! Armies, and Culture is now the seat's neglected yield:
//!
//! - Culture a turn, median at t50 / t75 / t100: 9.1 / 15.8 / 22.3 against
//!   the best rival's 67 / 142 / 227. Civics 7 / 13 / 18 against 15 / 22 /
//!   29. Classical Republic came at t41, Oligarchy at t61 and Monarchy at t88
//!   (Divine Right t87); Nationalism (Corps) at a median t142, in 8 of 20.
//! - At t100 the seat's Culture is population 14.0, modifiers 3.2, buildings
//!   2.0 (the Palace and about one Monument), worked tiles 1.5 and amenities
//!   -1.2. The empire held a median 1 Monument at t50, t75 and t100 while it
//!   stood at 4, 7 and 9 cities. 112 of 194 cities never had one in the log;
//!   the other own-founded cities got theirs a median 45 turns after
//!   founding (40 after their second citizen), and 3 of 20 capitals had none.
//!   A Monument is 30 Production at Online and +1 Culture, +1 more at full
//!   Loyalty (96% of our non-capital cities).
//! - On the city-turns a city of two citizens went ten turns or more with no
//!   Monument it built a Campus (12%), a Settler (10%), a University (7%), a
//!   Builder (6%), a Library (6%), Walls (5%), a Granary (4%), an Industrial
//!   Zone (3%) and soldiers. Of the heads this gene passes, the first one
//!   each city met was a Granary in 50 cities and a soldier at peace in 55.
//! - `domination-seat-appoints-pingala-first` gives Pingala the first title
//!   and Researcher the second; the third (median t53) went to Magnus (13
//!   games) or Victor (6), and every later title finished their trees: no
//!   Pingala took Connoisseur (+1 Culture per citizen in her city) in any of
//!   the 20 games.
//!
//! Replayed over those logs, a Monument ten turns after each city's second
//! citizen adds a median 6 / 8 / 12 Culture a turn at t50 / t75 / t100 and
//! about 3 civics by t75; read through this gene's passes alone (a Monument a
//! median 15 turns after the second citizen) it adds 4 / 8 / 11. Connoisseur
//! at the third title adds 0 / 8 / 9.8 and about 2 civics by t75. The two
//! add up. The Monuments cost their 30 Production a city out of a Granary
//! or a soldier at peace; Connoisseur costs Magnus one title (his Provision
//! moves from about t65 to about t95; his city trained a median 0.5 Settlers
//! in that window) and Science nothing (Researcher stays her first).
//!
//! **The gene.** While the active victory target is Domination:
//!
//! 1. From the second city on, a city of ours with
//!    [`DOMINATION_MONUMENT_MIN_POP`] citizens or more and no Monument, not
//!    the plan's threatened city, not attacked within
//!    [`super::hint_runs_the_claims::HINT_ATTACK_MEMORY`] turns and, while a
//!    major is at war with us, away from the front (no hostile presses it:
//!    the rear-city test), that can finish its Monument within
//!    `CAPITAL_MONUMENT_MAX_STANDARD` standard turns, puts it at the head of
//!    an idle queue (unless a Settler is due there), of a Granary or of an
//!    Aqueduct, and at peace with no hostile near, of a soldier: the
//!    near-rival deterrent or a routine land unit. The displaced item keeps
//!    its progress.
//! 2. Pingala, seated with Researcher, takes Connoisseur with the next title
//!    before the plan's own roster returns, so Magnus (or Victor) is the
//!    fourth title. A Recovery plan keeps its Victor first. It acts through
//!    `domination-seat-appoints-pingala-first`, which seats her first.
//!
//! A Settler, a Builder, a Trader, Walls, a Campus and its buildings, a
//! Commercial Hub (`commercial-hub-claims-a-peace-queue` would take a
//! Monument's head back), a Holy Site or Shrine (the faith defence re-queues
//! them every frame), a siege reservation's gun, a ship, the first capital's
//! reserved soldier and Scout, a wonder, a project, anything finishing this
//! turn and saved work within
//! [`super::near_done_work_finishes::NEAR_DONE_MAX_TURNS`] turns of done keep
//! their queue. The claim stands aside under the war's hold except for the
//! rear cities, and once `endgame-army-takes-the-queue`'s clock is open. The
//! bound Granary claim, the deterrent and the governor's rescoring then leave
//! the claimed Monument alone ([`AdvancedAi::domination_monument_holds`]).
//! Off, or for another victory target, nothing changes.
//!
//! Grep the journal for `domination-seat-raises-its-culture:`.

use super::capital_monument::{CAPITAL_MONUMENT_MAX_STANDARD, CAPITAL_MONUMENT_MIN_CITIES};
use super::*;

/// Citizens a city holds before it stops for its Monument: a newly founded
/// city works its first tile for growth and its Settler or Builder first.
pub(crate) const DOMINATION_MONUMENT_MIN_POP: i32 = 2;

impl AdvancedAi {
    /// The gene is on and the seat plays the Domination lane.
    fn domination_culture_on(&self, g: &Game) -> bool {
        self.domination_seat_raises_its_culture
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
    }

    /// The Monument `cid` would claim, its build turns, and whether it may go
    /// ahead of a soldier (at peace with no hostile pressing the city).
    /// `None` with the gene off, for another victory target, and for a city
    /// the module leaves alone.
    pub(super) fn domination_monument_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<(Item, f64, bool)> {
        if !self.domination_culture_on(g) {
            return None;
        }
        let city = g.cities.get(&cid)?;
        let memory = super::hint_runs_the_claims::HINT_ATTACK_MEMORY;
        if city.owner != pid
            || city.pop < DOMINATION_MONUMENT_MIN_POP
            || g.player_city_ids(pid).len() < CAPITAL_MONUMENT_MIN_CITIES
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= memory)
        {
            return None;
        }
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_minor
                && !other.is_barbarian
                && g.is_at_war(pid, other.id)
        });
        let pressed = self.hint_city_in_reach_of_war(g, pid, cid);
        if at_major_war && pressed {
            return None;
        }
        let monument = BasicAi::civ_building(g, pid, cid, "monument")?;
        let turns = g.host_production_turns(cid, &monument).unwrap_or_else(|| {
            let remaining = g.item_cost_for_city(pid, cid, &monument)
                - g.item_invested_production(cid, &monument);
            remaining.max(0.0) / g.city_yields(cid).production.max(0.5)
        });
        (turns <= g.standard_duration(CAPITAL_MONUMENT_MAX_STANDARD) as f64).then_some((
            monument,
            turns,
            !at_major_war && !pressed,
        ))
    }

    /// Whether `item` at the head of `cid` is the Monument this gene claims
    /// and the city still wants: the bound Granary claim, the deterrent and
    /// the governor's rescoring leave it alone. Always false with the gene
    /// off.
    pub(super) fn domination_monument_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        self.domination_monument_item(g, pid, cid, plan)
            .is_some_and(|(monument, _, _)| monument == *item)
    }

    /// The head `item` of `cid` the Monument goes ahead of, as the journal
    /// names it; `None` when the head keeps its queue. `army` is whether a
    /// soldier may wait (see [`Self::domination_monument_item`]).
    pub(super) fn domination_monument_passes(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
        army: bool,
    ) -> Option<String> {
        let invested = g.item_invested_production(cid, item);
        let remaining = g.item_cost_for_city(pid, cid, item) - invested;
        if remaining <= g.city_yields(cid).production {
            return None;
        }
        // `near-done-work-finishes` and `paused-district-finishes` put saved
        // work this close to done back at the head; leave it there.
        if invested > 0.0
            && self.production_build_turns(g, pid, cid, item)
                <= super::near_done_work_finishes::NEAR_DONE_MAX_TURNS
        {
            return None;
        }
        match item {
            Item::Unit { unit } => {
                let spec = g.rules.units.get(unit)?;
                if !army
                    || spec.class != "military"
                    || matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    || Self::queued_wall_breaker(g, item)
                    || self.first_capital_training_holds(g, pid, cid, item)
                    || self.first_capital_land_recon_holds(g, pid, cid, plan, item)
                {
                    return None;
                }
                let name = Self::plain_item(item);
                Some(if self.deterrence_unit_holds(g, pid, item) {
                    format!("the near-rival deterrent ({name})")
                } else {
                    format!("a routine {name}")
                })
            }
            Item::Building { building } => {
                let spec = g.rules.buildings.get(building)?;
                (!spec.wonder && spec.replaces.unwrap_or(*building) == "granary")
                    .then(|| "the Granary".to_string())
            }
            Item::District { district, .. } => {
                (g.district_family(*district) == "aqueduct").then(|| "the Aqueduct".to_string())
            }
            _ => None,
        }
    }

    /// See the module: every eligible city of a Domination seat puts its
    /// Monument at the head of an idle or routine queue. Exact no-op with the
    /// gene off or for another target.
    pub(super) fn claim_domination_monuments(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !self.domination_culture_on(g) || self.endgame_army_clock(g, pid).is_some() {
            return;
        }
        let city_ids = g.player_city_ids(pid);
        let n_cities = city_ids.len();
        let settlers = self.counts(g, pid).settlers;
        for cid in city_ids {
            // `rear-cities-grow-through-the-war-hold`: under the war's hold,
            // only a rear city.
            if !self.claim_open_behind_the_front(cid) {
                continue;
            }
            let Some((monument, turns, army)) = self.domination_monument_item(g, pid, cid, plan)
            else {
                continue;
            };
            if self.early_settler_floor_holds(g, pid, cid, plan) {
                continue;
            }
            let head = g.cities[&cid].queue.first().cloned();
            if head.as_ref() == Some(&monument) {
                continue;
            }
            let passed = match &head {
                Some(item) => {
                    match self.domination_monument_passes(g, pid, cid, plan, item, army) {
                        Some(passed) => format!("{passed} (it keeps its progress)"),
                        None => continue,
                    }
                }
                // An idle city with a Settler due leaves its queue to the
                // Settler step.
                None if self.base.settler_due(g, pid, cid, n_cities, settlers) => continue,
                None => "an idle queue".to_string(),
            };
            let (name, pop) = {
                let city = &g.cities[&cid];
                (city.name.clone(), city.pop)
            };
            if g.apply(
                pid,
                &Action::Produce {
                    city: cid,
                    item: monument,
                },
            )
            .is_ok()
            {
                think!(self.journal(), Cities, Decision,
                       "{name} raises its Monument for the civics";
                       "domination-seat-raises-its-culture: {name}'s Monument goes ahead of \
                        {passed}; {pop} citizens, {turns:.1} turns");
            }
        }
    }

    /// Whether Pingala, seated with Researcher on a Domination seat, still
    /// owes Connoisseur: her foundation is not complete until she holds it,
    /// so the next title is hers. False under a Recovery plan, with the gene
    /// off and for another victory target.
    pub(super) fn domination_pingala_owes_connoisseur(
        &self,
        g: &Game,
        pid: usize,
        strategy: GrandStrategy,
    ) -> bool {
        strategy != GrandStrategy::Recovery
            && self.domination_culture_on(g)
            && g.players[pid]
                .governor_roster
                .get("pingala")
                .is_some_and(|state| {
                    state.promotions.contains("researcher")
                        && !state.promotions.contains("connoisseur")
                })
    }

    /// The journal line for a Connoisseur this gene brought forward. Silent
    /// for any other promotion, with the gene off and for another target.
    pub(super) fn explain_domination_connoisseur(
        &self,
        g: &Game,
        pid: usize,
        governor: &str,
        promotion: &str,
    ) {
        if governor != "pingala" || promotion != "connoisseur" || !self.domination_culture_on(g) {
            return;
        }
        let Some(city) = g.players[pid]
            .governor_roster
            .get("pingala")
            .and_then(|state| state.city)
            .and_then(|cid| g.cities.get(&cid))
        else {
            return;
        };
        let (name, pop) = (&city.name, city.pop);
        think!(self.journal(), Government, Decision,
               "Pingala takes Connoisseur in {name}";
               "domination-seat-raises-its-culture: Connoisseur is her second promotion, \
                +1 Culture for each of {pop} citizens; the plan's own Governor follows");
    }
}

#[cfg(test)]
mod tests;
