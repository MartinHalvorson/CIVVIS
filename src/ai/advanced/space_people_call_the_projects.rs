//! `space-people-call-the-projects`: while a Great Person who speeds the space
//! race is on offer, a Science seat's Industrial Zones or Campuses run the
//! repeatable district project that earns that class's points.
//!
//! **Why (live Deity, 2026-11-05/06).** Project speed decides the race, and one
//! person sets it. Against its city's Production the near-win 154934Z built all
//! four projects about 2.2 times as fast as the rule (Kwolek), 125414Z went from
//! 1.1 to 2.0 the moment von Braun landed, and 003458Z, 011222Z and 001520Z,
//! with neither, spent 11-13 turns on the Moon alone. Host rows: Kwolek and von
//! Braun are `GREATPERSON_SPACE_RACE_PRODUCTION_RATE_LARGE`, +100% on every
//! space project; Korolev and Sagan grant 1500 and 3000 Production (scaled by
//! speed) into a project.
//!
//! All four are offered in nearly every game, and rivals take them: von Braun
//! went to a rival 15 times of 19 and to us once, Kwolek 23 of 27. He stood on
//! offer for 18-20 turns in 003458Z and 011222Z while the Engineer class earned
//! +10 a turn against a price of 660. Faith cannot bridge that at about 10 a
//! point. But Industrial Zone Logistics and Campus Research Grants pay these
//! same points: 55-69 at t165-180 on live completions, for 138 Production, with
//! 15% back in Gold or Science. Across every offer window of the last 11 games
//! Logistics ran on 0 city-turns, beside 26-73 city-turns of military units and
//! 150-300 empire Production a turn. Taking the person also keeps the doubling
//! from the rival racer who would otherwise hold it.
//!
//! **What the gene does.** On a Science aim, with Rocketry known, the person on
//! offer picks the project: von Braun or Kwolek first, then Korolev or Sagan,
//! the nearest first, and only within [`SPACE_PERSON_MAX_MISSING`] points. Up to
//! [`SPACE_PERSON_CITIES`] of the most productive cities that can run it take it
//! at the head of their queue, no more than the points still missing call for.
//! A city must be safe, and its head must be work no other claim wants
//! ([`AdvancedAi::queue_claim_may_displace`]), and it must not have taken this
//! claim's item away lately ([`AdvancedAi::queue_claim_open`]).
//! [`AdvancedAi::space_person_project_holds`] keeps the project against the
//! governor's rescoring while the call stands. The call ends when the person is
//! recruited, by us or a rival. Off, nothing changes.
//!
//! **Why the two queue rules.** The first live carrier of
//! `sea-scout-claims-the-queue`, civvis-20261106T015754Z, claimed 37 times in a
//! game. At Tuban the Campus building, the Plaza tier, the backlog Builder and
//! the sea scout each retook the head about three times a turn. A claim runs
//! every frame and none of them knows of the others, and "it keeps its progress"
//! takes the loser out of the queue, so it reads as missing and claims again.
//! The Quadrireme never finished and the Intelligence Agency slipped. So a claim
//! leaves other claims' work alone, and a city that took a claim's item away is
//! left alone for [`QUEUE_CLAIM_RETRY`] turns, unless its queue has run dry. A
//! one-frame replay cannot show this: judge carriers on claims per game.

use super::*;

/// Who the space race wants, the class that earns them and the project that
/// earns that class. The two doublings come first; see the module.
const SPACE_PEOPLE: [(&str, &str, &str); 4] = [
    ("engineer", "wernher_von_braun", "industrial_zone_logistics"),
    ("scientist", "stephanie_kwolek", "campus_research_grants"),
    ("engineer", "sergei_korolev", "industrial_zone_logistics"),
    ("scientist", "carl_sagan", "campus_research_grants"),
];

/// The doublings at the head of [`SPACE_PEOPLE`]; a lump grant waits for them.
const SPACE_PEOPLE_DOUBLINGS: usize = 2;

/// Most cities on one call at once. Three projects of about 60 points every
/// four turns plus the class's own income close a typical gap in nine or ten
/// turns, inside most offer windows.
const SPACE_PERSON_CITIES: usize = 3;

/// What one project pays in the window, from live completions of 55-69 points
/// at t165-180. Only sizes how many cities a gap needs.
const SPACE_PERSON_PROJECT_POINTS: f64 = 60.0;

/// The largest gap worth chasing: twelve projects. Sagan at 1,685 with 420
/// points banked is not in reach before a rival takes him.
const SPACE_PERSON_MAX_MISSING: f64 = 720.0;

/// The attack memory a claiming city must be clear of, as the other claims.
const SPACE_PERSON_ATTACK_MEMORY: u32 = 4;

/// How long a city stays out of a claim that it took back, in turns. Long enough
/// for whatever displaced the item to finish, so the claim moves to another city
/// or waits instead of fighting for the head every frame.
const QUEUE_CLAIM_RETRY: u32 = 6;

/// This gene's name in [`AdvancedAi::note_queue_claim`].
const SPACE_PERSON_CLAIM: &str = "space-person";

/// One live call for a space-race person.
struct SpacePersonCall {
    individual: &'static str,
    project: &'static str,
    missing: f64,
}

impl AdvancedAi {
    /// The space-race person this seat should earn now, if any. See the module.
    fn space_person_called(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<SpacePersonCall> {
        if !self.space_people_call_the_projects
            || self.raced_target() != Some(VictoryTarget::Science)
            || plan.strategy == GrandStrategy::Recovery
            || !g.players[pid].techs.contains(&crate::name!("rocketry"))
        {
            return None;
        }
        SPACE_PEOPLE
            .iter()
            .enumerate()
            .filter_map(|(rank, &(kind, individual, project))| {
                let offered = g.great_person_class_offered_now(pid, kind)
                    && g.live_great_person_offer_individual(pid, kind) == Some(individual);
                if !offered {
                    return None;
                }
                let points = g.players[pid].gpp.get(kind).copied().unwrap_or(0.0);
                let missing = (g.gp_cost(pid, kind) - points).max(0.0);
                (missing > f64::EPSILON && missing <= SPACE_PERSON_MAX_MISSING).then_some((
                    rank >= SPACE_PEOPLE_DOUBLINGS,
                    missing,
                    SpacePersonCall {
                        individual,
                        project,
                        missing,
                    },
                ))
            })
            .min_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.total_cmp(&b.1)))
            .map(|(_, _, call)| call)
    }

    /// Whether a queue claim may take the head from `head`: what a steal Spy
    /// would displace, less the work other claims keep there every frame. That
    /// is the Plaza's tiers and the Intelligence Agency above all, the Campus
    /// chain, power, the Spaceport and the backlog Builder. See the module.
    pub(super) fn queue_claim_may_displace(g: &Game, head: &Item) -> bool {
        if !Self::spy_displaces(g, head) {
            return false;
        }
        let claimed_family = |family: Name| {
            family == crate::name!("campus")
                || family == crate::name!("spaceport")
                || family == crate::name!("government_plaza")
        };
        match head {
            Item::Building { building } => {
                !building.as_str().contains("power_plant")
                    && g.rules.buildings.get(building).is_none_or(|spec| {
                        spec.district
                            .is_none_or(|district| !claimed_family(g.district_family(district)))
                    })
            }
            Item::District { district, .. } => !claimed_family(g.district_family(*district)),
            Item::Unit { unit } => *unit != "builder",
            _ => true,
        }
    }

    /// Whether `claim` may take `cid` now: it has not claimed there, or
    /// [`QUEUE_CLAIM_RETRY`] turns have passed, or the queue has since run dry.
    /// Never twice in a turn, whatever the queue reads. See the module.
    pub(super) fn queue_claim_open(&self, g: &Game, claim: &'static str, cid: u32) -> bool {
        self.queue_claim_turns
            .get(&(cid, claim))
            .is_none_or(|turn| {
                g.turn >= turn + QUEUE_CLAIM_RETRY
                    || (g.turn > *turn && g.cities[&cid].queue.is_empty())
            })
    }

    /// Remember that `claim` took `cid` this turn.
    pub(super) fn note_queue_claim(&mut self, g: &Game, claim: &'static str, cid: u32) {
        self.queue_claim_turns.insert((cid, claim), g.turn);
    }

    /// Whether `cid` is safe and can run `item`.
    fn space_person_city_is_clear(
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        cid: u32,
        item: &Item,
    ) -> bool {
        let city = &g.cities[&cid];
        let attacked = city.last_attacked > 0
            && g.turn.saturating_sub(city.last_attacked) <= SPACE_PERSON_ATTACK_MEMORY;
        plan.threatened_city != Some(cid) && !attacked && g.can_produce(pid, cid, item)
    }

    /// Put the called project at the head of the most productive clear cities,
    /// no more of them than the missing points need. See the module.
    pub(super) fn claim_queues_for_the_space_people(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        let Some(call) = self.space_person_called(g, pid, plan) else {
            return;
        };
        let item = Item::Project {
            project: Name::new(call.project),
        };
        let cities = g.player_city_ids(pid);
        let running = cities
            .iter()
            .filter(|cid| g.cities[cid].queue.first() == Some(&item))
            .count();
        let wanted =
            ((call.missing / SPACE_PERSON_PROJECT_POINTS).ceil() as usize).min(SPACE_PERSON_CITIES);
        if running >= wanted {
            return;
        }
        let mut open: Vec<(f64, u32)> = cities
            .into_iter()
            .filter(|cid| {
                let head = g.cities[cid].queue.first();
                head != Some(&item)
                    && head.is_none_or(|head| Self::queue_claim_may_displace(g, head))
                    && self.queue_claim_open(g, SPACE_PERSON_CLAIM, *cid)
                    && Self::space_person_city_is_clear(g, pid, plan, *cid, &item)
            })
            .map(|cid| (g.city_yields(cid).production, cid))
            .collect();
        open.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        for (_, city) in open.into_iter().take(wanted - running) {
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
                self.note_queue_claim(g, SPACE_PERSON_CLAIM, city);
                *g.players[pid]
                    .counters
                    .entry("space_person_project_claims".to_string())
                    .or_insert(0) += 1;
                think!(self.journal(), Economy, Decision,
                       "{} runs {} for {}", g.cities[&city].name,
                       Self::plain_item(&item), crate::reasoning::plain(call.individual);
                       "space-people-call-the-projects: it goes ahead of {displaced}; {:.0} points \
                        short, and a rival who takes this person doubles their own launches",
                       call.missing);
            }
        }
    }

    /// `space-people-call-the-projects`' hold: the called project finishes at
    /// the head of a clear city while the call stands. See the module.
    pub(super) fn space_person_project_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        let Item::Project { project } = item else {
            return false;
        };
        self.space_person_called(g, pid, plan)
            .is_some_and(|call| project == call.project)
            && Self::space_person_city_is_clear(g, pid, plan, cid, item)
    }
}

#[cfg(test)]
mod tests;
