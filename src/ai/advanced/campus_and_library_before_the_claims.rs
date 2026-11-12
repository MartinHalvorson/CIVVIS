//! `campus-and-library-before-the-claims`: a Domination seat's first Campus
//! and its Library (then its University) go ahead of the routine claims that
//! keep a city's queue.
//!
//! **Why (live Deity Gran Colombia, 4-player Tiny Pangaea, Online,
//! `civvis-20261112T064834Z`).** The seat held 22 techs at t100 against the
//! best rival's 46. Empire Science was 12.8 at t60, 19.8 at t80 and 39 at
//! t100, against about 190 for the best rival. Science infrastructure came
//! last in every city's queue. The capital placed its Campus at t51 (done
//! t55) and started its Library only at t91 (done t103). Between the two it
//! built a Builder, three Settlers, a Commercial Hub and its Market, a Heavy
//! Chariot, two Catapults (a deterrent and a siege reservation), a Holy Site
//! and Shrine for the counterweight, and an Aqueduct. Every other city got
//! its first Campus at t81-99. `campus-claims-the-free-slot` journalled what
//! held a buildable first Campus: the early Settler floor (91 city-turns),
//! the backlog Builder claim (81), the bound Granary claim (35), the capped
//! city's Aqueduct claim (31), a head it may not defer (Walls, a Ram, a
//! Builder: about 25), near-rival deterrence (15) and the producer Industrial
//! Zone (3).
//!
//! **Why the Library waited.** `campus-building-claims-the-queue` was armed
//! and wrote no line for the capital between t55 and t91. It runs after the
//! Settler, Builder, Granary, deterrent, Aqueduct and Industrial Zone claims
//! and takes only an idle queue or a head none of them holds. The queue was
//! never idle when it ran, because an earlier claim or the production hint
//! had filled it. Every head it found was one it keeps: a Settler, the
//! backlog Builder, a deterrent, a Hub with progress, a Market a turn from
//! done, the faith defence's Holy Site and Shrine, a siege reservation's gun
//! and the Aqueduct claim's Aqueduct.
//!
//! **The gene.** It applies while the active victory target is Domination, in
//! a city that is ours, is not the plan's threatened city, was not attacked
//! within four turns and, while a major is at war with us, stands away from
//! the front (no hostile presses it: `rear-cities-grow-through-the-war-hold`'s
//! rear-city test). There:
//!
//! 1. The first Campus `campus-claims-the-free-slot` (or `-2`) would claim
//!    but for a hold goes ahead of a Builder (the backlog claims' included), a
//!    Granary, an Aqueduct, an Industrial Zone, a Commercial Hub or its
//!    Market. At peace and with no hostile near, it also goes ahead of a
//!    soldier: the deterrent, or a routine unit that is not a siege gun.
//! 2. A standing Campus's Library, and its University once Education is in
//!    when it finishes within [`CAMPUS_AND_LIBRARY_UNIVERSITY_ONLINE_TURNS`]
//!    Online turns, take the head through `campus-building-claims-the-queue`,
//!    which the gene opens for them even with that gene off. They go ahead
//!    of the same items, and of the faith defence's Holy Site and Shrine
//!    outside the Prophet race's site city. At peace and with no hostile
//!    near, they also go ahead of any soldier, a siege reservation's gun
//!    included. Every such city claims each turn, not one city a turn.
//!
//! A Settler, Walls, a wonder, a project, a Trader, a Ram or Siege Tower, a
//! ship, the first capital's reserved soldier and Scout, and anything
//! finishing this turn keep their queue. So
//! does saved work within two turns of done while `near-done-work-finishes`
//! would put it back. A front city at war and a threatened or freshly
//! attacked city keep their queue. The displaced item keeps its progress.
//! The Granary, backlog Builder, deterrent and siege-reservation writers and
//! the governor's rescoring then leave the claimed Campus or Library alone
//! ([`AdvancedAi::campus_and_library_holds`]). Off, nothing changes.
//!
//! Grep the journal for `campus-and-library-before-the-claims:`.

use super::*;

/// The longest University, in Online turns, that goes ahead of the claims.
/// A Library is 45 Production at Online and always qualifies; a University is
/// 125, so a city making less than about 12 a turn keeps the old order.
pub(crate) const CAMPUS_AND_LIBRARY_UNIVERSITY_ONLINE_TURNS: f64 = 10.0;

/// How far the gene may reach into a city's queue this turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ScienceFirstReach {
    /// The economy's heads only: a Builder, a Granary, an Aqueduct, an
    /// Industrial Zone, a Commercial Hub or its Market, and for a Campus
    /// building the faith defence's Holy Site and Shrine.
    Economy,
    /// Those and a soldier too: at peace, with no hostile pressing the city.
    EconomyAndArmy,
}

impl AdvancedAi {
    /// Whether the gene opens `cid` this turn, and how far. `None` with the
    /// gene off, for another victory target, for a city not ours, for the
    /// plan's threatened city or one attacked within
    /// [`super::hint_runs_the_claims::HINT_ATTACK_MEMORY`] turns, and for a
    /// front city while a major is at war with us.
    pub(super) fn campus_and_library_reach(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<ScienceFirstReach> {
        if !self.campus_and_library_before_the_claims
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
        {
            return None;
        }
        let city = g.cities.get(&cid)?;
        let memory = super::hint_runs_the_claims::HINT_ATTACK_MEMORY;
        if city.owner != pid
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
        // The rear-city test: hit within the memory or pressed by a visible
        // or remembered hostile.
        let pressed = self.hint_city_in_reach_of_war(g, pid, cid);
        match (at_major_war, pressed) {
            (true, true) => None,
            (true, false) | (false, true) => Some(ScienceFirstReach::Economy),
            (false, false) => Some(ScienceFirstReach::EconomyAndArmy),
        }
    }

    /// [`CAMPUS_AND_LIBRARY_UNIVERSITY_ONLINE_TURNS`] at this game's speed:
    /// production quotes scale with the speed's costs.
    pub(super) fn campus_and_library_university_turns(g: &Game) -> f64 {
        CAMPUS_AND_LIBRARY_UNIVERSITY_ONLINE_TURNS * g.game_speed.cost_percent() as f64
            / crate::setup::GameSpeed::Online.cost_percent() as f64
    }

    /// Whether a Campus building of `rank` (0 the Library, 1 the University;
    /// see `campus_building_rank`) `turns` from done is one the gene puts
    /// first.
    pub(super) fn campus_and_library_building(g: &Game, rank: u8, turns: f64) -> bool {
        rank == 0
            || (rank == 1
                && turns.is_finite()
                && turns <= Self::campus_and_library_university_turns(g))
    }

    /// The head `item` of `cid` that the gene's Campus (`library` false) or
    /// Campus building (`library` true) goes ahead of, as the journal names
    /// it; `None` when the head keeps its queue. `reach` is
    /// [`Self::campus_and_library_reach`] for the city.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn campus_and_library_passes(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
        reach: ScienceFirstReach,
        library: bool,
    ) -> Option<String> {
        let remaining =
            g.item_cost_for_city(pid, cid, item) - g.item_invested_production(cid, item);
        if remaining <= g.city_yields(cid).production {
            return None;
        }
        // `near-done-work-finishes` puts saved work within two turns of done
        // back at the head after every writer; leave it there.
        if self.near_done_work_finishes
            && g.item_invested_production(cid, item) > 0.0
            && self.production_build_turns(g, pid, cid, item)
                <= super::near_done_work_finishes::NEAR_DONE_MAX_TURNS
        {
            return None;
        }
        let army = reach == ScienceFirstReach::EconomyAndArmy;
        let race_site = || {
            self.base
                .race_site_city(g, pid)
                .is_some_and(|(site, _)| site == cid)
        };
        match item {
            Item::Unit { unit } => {
                if *unit == "builder" {
                    return Some("the Builder".to_string());
                }
                let spec = g.rules.units.get(unit)?;
                if !army
                    || spec.class != "military"
                    || matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    || self.first_capital_training_holds(g, pid, cid, item)
                    || self.first_capital_land_recon_holds(g, pid, cid, plan, item)
                {
                    return None;
                }
                let name = Self::plain_item(item);
                if self.deterrence_unit_holds(g, pid, item) {
                    Some(format!("the near-rival deterrent ({name})"))
                } else if Self::queued_wall_breaker(g, item) {
                    library.then(|| format!("the siege reservation's {name}"))
                } else {
                    Some(format!("a routine {name}"))
                }
            }
            Item::Building { building } => {
                let spec = g.rules.buildings.get(building)?;
                if spec.wonder {
                    return None;
                }
                match spec.replaces.unwrap_or(*building).as_str() {
                    "granary" => Some("the Granary".to_string()),
                    "market" => Some("the Market".to_string()),
                    "shrine" if library && !race_site() => Some("the Shrine".to_string()),
                    _ => None,
                }
            }
            Item::District { district, .. } => match g.district_family(*district).as_str() {
                "commercial_hub" => Some("the Commercial Hub".to_string()),
                "aqueduct" => Some("the Aqueduct".to_string()),
                "industrial_zone" => Some("the Industrial Zone".to_string()),
                "holy_site" if library && !race_site() => Some("the Holy Site".to_string()),
                _ => None,
            },
            _ => None,
        }
    }

    /// `campus-and-library-before-the-claims`: the head of `cid` that its
    /// first Campus goes ahead of though a claim or the deferral rule holds
    /// it. Always `None` with the gene off.
    pub(super) fn first_campus_passes_the_hold(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> Option<String> {
        let reach = self.campus_and_library_reach(g, pid, cid, plan)?;
        self.campus_and_library_passes(g, pid, cid, plan, item, reach, false)
    }

    /// Whether `item` at the head of `cid` is the first Campus, Library or
    /// University this gene put first and the city still wants: the Granary,
    /// backlog Builder, deterrent and siege-reservation writers leave it
    /// alone. Always false with the gene off.
    pub(super) fn campus_and_library_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        if !self.campus_and_library_before_the_claims {
            return false;
        }
        match item {
            Item::District { district, .. } => {
                g.district_family(*district) == "campus"
                    && self.campus_and_library_reach(g, pid, cid, plan).is_some()
                    && self.free_slot_campus_holds(g, pid, cid, plan, item)
            }
            Item::Building { .. } => self
                .campus_building_claim_item(g, pid, cid, plan)
                .is_some_and(|(building, rank, turns)| {
                    building == *item
                        && Self::campus_and_library_building(g, rank, turns)
                        && self.campus_and_library_reach(g, pid, cid, plan).is_some()
                }),
            _ => false,
        }
    }

    /// The journal line for a first Campus the gene put ahead of `passed`.
    pub(super) fn explain_first_campus_passes(&self, g: &Game, cid: u32, turns: f64, passed: &str) {
        let name = &g.cities[&cid].name;
        think!(self.journal(), Cities, Decision,
               "{name} starts its first Campus ahead of {passed}";
               "campus-and-library-before-the-claims: {name}'s first Campus goes ahead of {passed} \
                (it keeps its progress); {turns:.1} turns");
    }

    /// The journal line for a Campus building the gene put first.
    pub(super) fn explain_campus_building_passes(
        &self,
        g: &Game,
        cid: u32,
        building: &Item,
        turns: f64,
        passed: &str,
    ) {
        let name = &g.cities[&cid].name;
        let what = match super::campus_building_claims_the_queue::campus_building_rank(g, building)
        {
            Some(0) => "Library".to_string(),
            Some(1) => "University".to_string(),
            _ => Self::plain_item(building),
        };
        think!(self.journal(), Cities, Decision,
               "{name} puts its {what} first";
               "campus-and-library-before-the-claims: {name}'s {what} goes ahead of {passed}; \
                {turns:.1} turns");
    }
}

#[cfg(test)]
mod tests;
