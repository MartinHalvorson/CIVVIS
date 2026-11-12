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
//!    but for a hold goes ahead of a Granary, an Aqueduct, an Industrial
//!    Zone, a Commercial Hub or its Market. At peace and with no hostile
//!    near, it also goes ahead of a soldier: the deterrent, or a routine unit
//!    that is not a siege gun.
//! 2. A standing Campus's Library takes the head through
//!    `campus-building-claims-the-queue`, which the gene opens for it even
//!    with that gene off. It goes ahead of the same items, and of the faith
//!    defence's Holy Site and Shrine outside the Prophet race's site city.
//!    At peace and with no hostile near, it also goes ahead of any soldier,
//!    a siege reservation's gun included. Every such city claims each turn,
//!    not one city a turn.
//! 3. Its University, once Education is in and when it finishes within
//!    [`CAMPUS_AND_LIBRARY_UNIVERSITY_ONLINE_TURNS`] Online turns, takes the
//!    head the same way but passes less: a Commercial Hub or its Market, the
//!    faith defence's Holy Site and Shrine, a wonder with nothing paid and,
//!    at peace with no hostile near, a soldier.
//! 4. A Settler (the early floor's and `holdable-land-keeps-settling`'s
//!    included) with at most half its cost paid waits for the Library from
//!    [`CAMPUS_AND_LIBRARY_SETTLER_MIN_CITIES`] (four) cities and for the
//!    first Campus from [`FIRST_CAMPUS_SETTLER_MIN_CITIES`] (six), counting
//!    founded cities plus Settlers walking. The Settler keeps its progress.
//!    With fewer cities the Settler stays first;
//!    `first-campus-before-the-settler` still covers the empire's very first
//!    Campus.
//! 5. The first Campus goes ahead of a Holy Site or Shrine with nothing paid
//!    outside the Prophet race's site city: the counterweight's and the
//!    adopted faith's sanctuary. A Holy Site with progress finishes first.
//!
//! **A Builder always keeps its queue** against all three: the worked
//! backlog's Builder is what turns land into Production.
//!
//! **A housing-bound city's Granary keeps its queue** too. In a city within
//! one of its housing, the bound Granary `granary-claims-the-bound-queue`
//! claims ([`AdvancedAi::bound_granary_holds`], the same reading as
//! `housing-bound-city-builds-its-granary`) keeps the head against the
//! Library and the University, and against the first Campus when the city is
//! at or over its housing and the Campus has nothing paid (a Campus already
//! started goes on). The capped city's Aqueduct
//! (`aqueduct-claims-the-capped-queue`, [`AdvancedAi::capped_aqueduct_holds`])
//! keeps its head against the Library and the University. The Granary claim
//! runs before both Campus claims, so it takes an idle bound queue first; it
//! leaves a Campus or Library already at the head alone, so the two never
//! trade the head. A city that is not housing-bound is unchanged.
//!
//! **Why points 4 and 5 (live `civvis-20261112T085526Z`, the gene not yet armed,
//! `no-city-limit` armed).** The seat held 5, 7 and 9 cities at t50, t75 and
//! t100 but 0, 0 and 2 Campuses, and 40 Science from 61 citizens at t100. The
//! free-slot claim's holds there, in journal lines (several frames a turn):
//! the early Settler floor 213, a head it may not defer 188 (Holy Site 80,
//! Walls 59, Shrine 27, Trader 6, Siege Tower 6, Monument 4, Settler 2,
//! others 4; by city-turn 33 Holy Site, 17 Walls, 10 Shrine of 70), the
//! backlog Builder 89, the bound Granary 72. The capital trained Settlers at
//! t37, 44, 52, 54, 55 and 56 and a Holy Site at t59 before its Campus at
//! t64. Under `no-city-limit`
//! the sixth Settler costs about 115 Production, more than a Campus and its
//! Library together (about 85). The Holy Sites came from
//! `counterweight-builds-its-source` and the adopted faith's sanctuary, which
//! run before the claims and re-place their item at the head every frame;
//! the free-slot claim's deferral rule never defers a Holy Site or Shrine.
//! Both now leave a city alone while its first Campus or Library holds the
//! head ([`AdvancedAi::campus_and_library_holds`]), as do both Settler
//! claims from the city count that lets the head pass a Settler.
//!
//! **Why the Builder keeps its queue and the University passes less (the
//! armed gene, 11 live games since 10:00Z and a 60-seed paired simulation).**
//! Science rose (t50 24.7 against 13.5, t100 77.0 against 62.7 live; +6.9 at
//! t50 in the simulation) but Production fell: 75 against 106 at t100 live,
//! -14.8 (z -4.4) in the simulation, with 0.62 fewer cities and 4.6 fewer
//! citizens; the first version alone read -5.1 Production, the four-city
//! Settler pass about doubled it. The first claims it made live: the first
//! Campus ahead of the Granary 42, the Settler 22 and the Builder 22; the
//! Library ahead of the Settler 30, the Granary 21 and the Builder 13; the
//! University ahead of the Builder 32. 63% of worked tiles were unimproved at
//! t40.
//!
//! **Why the Granary (17 live games since 11:00Z against 11 before the
//! gene).** Cities with a Granary fell from 23 / 46 / 70% to 2 / 12 / 54% at
//! t40 / t60 / t80, cities within one of their housing rose from 58 to 62% at
//! t60 and 50 to 63% at t80, and Aqueducts from 14 to 4% of cities at t80.
//! Housing averaged 5.2 at t60 against 3.9 citizens, with a mean growth
//! multiplier of 0.66, and 26% of cities stood at or over the cap. The paired
//! simulation read -1.6 citizens at t60 and -4.6 at t100. A Granary is about
//! 33 Production at Online for +2 Housing and +1 Food; a capped city gains
//! nothing from Campus citizens it cannot grow.
//!
//! A Builder, a Settler (below the city count, more than half paid, or before
//! a University), a housing-bound Granary and a capped Aqueduct (above),
//! Walls, a wonder (but an unpaid one before a University), a
//! project, a Trader, a Ram or Siege Tower, a ship, the first capital's
//! reserved soldier and Scout, and anything finishing this turn keep their
//! queue. So does saved work within two turns of done while
//! `near-done-work-finishes` would put it back. A front city at war and a
//! threatened or freshly attacked city keep their queue. The displaced item
//! keeps its progress.
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

/// Founded cities plus Settlers walking from which the Library goes ahead of
/// a Settler (45 Production at Online). Below it the Settler stays first.
pub(crate) const CAMPUS_AND_LIBRARY_SETTLER_MIN_CITIES: usize = 4;

/// Founded cities plus Settlers walking from which the first Campus goes
/// ahead of a Settler. At four the paired simulation read twice the
/// Production cost of the gene without it.
pub(crate) const FIRST_CAMPUS_SETTLER_MIN_CITIES: usize = 6;

/// What the gene puts first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ScienceFirstItem {
    /// The city's first Campus district.
    FirstCampus,
    /// The standing Campus's Library.
    Library,
    /// The standing Campus's University (only when quick).
    University,
}

/// How far the gene may reach into a city's queue this turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ScienceFirstReach {
    /// The economy's heads only: a Granary, an Aqueduct, an Industrial Zone,
    /// a Commercial Hub or its Market, the faith defence's Holy Site and
    /// Shrine, and a Settler (see [`AdvancedAi::campus_and_library_passes`]).
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

    /// Whether the empire holds enough founded cities plus Settlers walking
    /// for `what` to go ahead of a Settler:
    /// [`FIRST_CAMPUS_SETTLER_MIN_CITIES`] for the first Campus,
    /// [`CAMPUS_AND_LIBRARY_SETTLER_MIN_CITIES`] for the Library, never for
    /// the University.
    pub(super) fn campus_and_library_passes_settlers(
        g: &Game,
        pid: usize,
        what: ScienceFirstItem,
    ) -> bool {
        let floor = match what {
            ScienceFirstItem::FirstCampus => FIRST_CAMPUS_SETTLER_MIN_CITIES,
            ScienceFirstItem::Library => CAMPUS_AND_LIBRARY_SETTLER_MIN_CITIES,
            ScienceFirstItem::University => return false,
        };
        g.player_city_ids(pid).len() + Self::early_settler_census(g, pid).0 >= floor
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

    /// The head `item` of `cid` that the gene's `what` goes ahead of, as the
    /// journal names it; `None` when the head keeps its queue. `reach` is
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
        what: ScienceFirstItem,
    ) -> Option<String> {
        let library = what != ScienceFirstItem::FirstCampus;
        // The University passes only a soldier, a Commercial Hub or Market,
        // a Holy Site or Shrine and an unpaid wonder.
        let economy = what != ScienceFirstItem::University;
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
        let invested = g.item_invested_production(cid, item);
        let army = reach == ScienceFirstReach::EconomyAndArmy;
        let race_site = || {
            self.base
                .race_site_city(g, pid)
                .is_some_and(|(site, _)| site == cid)
        };
        match item {
            Item::Unit { unit } => {
                // A Settler with at most half its cost paid waits for the
                // Library from four cities and the first Campus from six. A
                // Builder is a civilian and always keeps its queue.
                if *unit == "settler" {
                    let cost = g.item_cost_for_city(pid, cid, item);
                    return (Self::campus_and_library_passes_settlers(g, pid, what)
                        && invested * 2.0 <= cost)
                        .then(|| {
                            format!("the Settler ({invested:.0} of {cost:.0} Production paid)")
                        });
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
                    return (what == ScienceFirstItem::University && invested <= 0.0)
                        .then(|| format!("the {}", Self::plain_item(item)));
                }
                match spec.replaces.unwrap_or(*building).as_str() {
                    "granary" if economy => Some("the Granary".to_string()),
                    "market" => Some("the Market".to_string()),
                    // A Campus passes only a Shrine with nothing paid.
                    "shrine" if (library || invested <= 0.0) && !race_site() => {
                        Some("the Shrine".to_string())
                    }
                    _ => None,
                }
            }
            Item::District { district, .. } => match g.district_family(*district).as_str() {
                "commercial_hub" => Some("the Commercial Hub".to_string()),
                "aqueduct" if economy => Some("the Aqueduct".to_string()),
                "industrial_zone" if economy => Some("the Industrial Zone".to_string()),
                // A Campus passes only a Holy Site with nothing paid: one
                // with progress finishes first.
                "holy_site" if (library || invested <= 0.0) && !race_site() => {
                    Some("the Holy Site".to_string())
                }
                _ => None,
            },
            Item::Wonder { .. } => (what == ScienceFirstItem::University && invested <= 0.0)
                .then(|| format!("the {}", Self::plain_item(item))),
            _ => None,
        }
    }

    /// Whether `cid` stands at or over its housing.
    pub(super) fn campus_and_library_at_the_cap(g: &Game, cid: u32) -> bool {
        g.cities
            .get(&cid)
            .is_some_and(|city| city.pop as f64 >= g.city_housing(city))
    }

    /// The housing head `item` of `cid` that keeps its queue against `what`
    /// (`science`, the first Campus or the Campus building), as the journal
    /// names it: the bound Granary ([`Self::bound_granary_holds`]) against
    /// the Library and University, and against a first Campus with nothing
    /// paid in a city at or over its housing; the capped Aqueduct
    /// ([`Self::capped_aqueduct_holds`]) against the Library and University.
    /// Always `None` with the gene off.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn campus_and_library_housing_keeps(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
        what: ScienceFirstItem,
        science: &Item,
    ) -> Option<&'static str> {
        if !self.campus_and_library_before_the_claims {
            return None;
        }
        match item {
            Item::Building { .. } if self.bound_granary_holds(g, pid, cid, plan, item) => {
                let keeps = what != ScienceFirstItem::FirstCampus
                    || (Self::campus_and_library_at_the_cap(g, cid)
                        && g.item_invested_production(cid, science) <= 0.0);
                keeps.then_some("Granary")
            }
            Item::District { district, .. }
                if what != ScienceFirstItem::FirstCampus
                    && g.district_family(*district) == "aqueduct"
                    && self.capped_aqueduct_holds(g, pid, cid, plan, item) =>
            {
                Some("Aqueduct")
            }
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
        self.campus_and_library_passes(
            g,
            pid,
            cid,
            plan,
            item,
            reach,
            ScienceFirstItem::FirstCampus,
        )
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

    /// Whether `cid`'s head is the first Campus or Library the gene holds and
    /// both Settler claims leave it alone too: from six cities for the
    /// Campus, four for the Library. Always false with the gene off.
    pub(super) fn campus_and_library_holds_the_settler(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> bool {
        self.campus_and_library_before_the_claims
            && g.cities
                .get(&cid)
                .and_then(|city| city.queue.first())
                .is_some_and(|item| {
                    let what = match item {
                        Item::District { .. } => ScienceFirstItem::FirstCampus,
                        Item::Building { .. }
                            if super::campus_building_claims_the_queue::campus_building_rank(
                                g, item,
                            ) == Some(0) =>
                        {
                            ScienceFirstItem::Library
                        }
                        _ => return false,
                    };
                    Self::campus_and_library_passes_settlers(g, pid, what)
                        && self.campus_and_library_holds(g, pid, cid, plan, item)
                })
    }

    /// Whether the faith writers (`counterweight-builds-its-source`, the
    /// adopted faith's sanctuary) leave `cid` alone this frame: its head is
    /// the first Campus, Library or University the gene holds. Always false
    /// with the gene off.
    pub(super) fn campus_and_library_holds_the_faith_writers(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> bool {
        self.campus_and_library_before_the_claims
            && g.cities
                .get(&cid)
                .and_then(|city| city.queue.first())
                .is_some_and(|item| self.campus_and_library_holds(g, pid, cid, plan, item))
    }

    /// The journal line for a housing head that keeps its queue against
    /// `what` ([`Self::campus_and_library_housing_keeps`]).
    pub(super) fn explain_housing_keeps_the_head(
        &self,
        g: &Game,
        cid: u32,
        kept: &str,
        what: ScienceFirstItem,
    ) {
        let city = &g.cities[&cid];
        let name = &city.name;
        let what = match what {
            ScienceFirstItem::FirstCampus => "first Campus",
            ScienceFirstItem::Library => "Library",
            ScienceFirstItem::University => "University",
        };
        think!(self.journal(), Cities, Detail,
               "{name}'s {kept} keeps its queue at its housing";
               "campus-and-library-before-the-claims: {name}'s {kept} keeps the queue ahead of its \
                {what}; population {} against housing {:.0}", city.pop, g.city_housing(city));
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
