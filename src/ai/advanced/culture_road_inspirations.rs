//! `culture-road-earns-its-inspirations`: a Culture seat orders the civics on
//! its beeline by when their Inspirations land, and researches the technology
//! that inspires a near civic of the road to Cold War first.
//!
//! **Why (six live Deity Kristina games, 2026-11-10).** Rock Bands open at
//! Cold War, and every civic of its prerequisite closure (28 civics, 9,098
//! Online Culture) has to be paid for first. An Inspiration pays 40% of its
//! civic, but only if it lands before the civic completes. Of the road civics
//! the seat completed, Inspirations covered 13-18% of the boostable Culture in
//! the two games that reached Ideology (152217Z: 444 of 3,307; 144009Z: 504 of
//! 2,870) and 4-32% across all six: 1,900-2,900 Online Culture lost a game,
//! 20-30 turns of civic research at the seat's t100-150 rate. Most of it is
//! out of reach (Nationalism's casus belli, Diplomatic Service's alliance,
//! Great Merchants, Cold War's Nuclear Fission), but three kinds were missed
//! by order alone:
//!
//! - **Mobilization (3 Corps, 308).** `civic-awaits-its-inspiration` reads a
//!   Corps trigger as "nothing left to earn", so it took Mobilization ahead of
//!   Mass Media while Radio was out of reach: 152217Z formed its third Corps
//!   on the turn Mobilization finished (t141), 144009Z four turns after
//!   (t154 -> t158). Mass Media first loses nothing (Radio landed after it in
//!   both games anyway) and gives the Corps their turns.
//! - **Colonialism (Astronomy, 160).** Astronomy was legal 20-40 turns before
//!   Colonialism started and is on the Culture lane's own Radio beeline, but
//!   the beeline took other steps first: 153619Z finished Colonialism two
//!   turns before Astronomy (t139/t141), 144009Z 22 turns before, 140731Z
//!   never researched it.
//! - **Mass Media (Radio, 308).** 144009Z finished Mass Media and Radio on
//!   the same turn (t160).
//!
//! The rest (Guilds' two Markets 0-14 turns late, Civil Service and
//! Urbanization a size short, Scorched Earth's Field Cannons 13 turns late)
//! are the same shape: a trigger already on its way.
//!
//! **What the gene does**, for an active Culture target only:
//!
//! 1. **Order.** Where `civic-awaits-its-inspiration` sorts a beeline's steps
//!    into "still ours to earn" and "nothing left to earn" by trigger kind, this
//!    reads each step's landing turn ([`AdvancedAi::culture_road_inspiration_eta`]):
//!    a step whose trigger is visibly coming inside [`ROAD_TRIGGER_HORIZON`]
//!    standard turns, and would land after the step finished, waits. Another
//!    step with nothing to wait for goes first, in the beeline's own order;
//!    when every step waits, the one whose trigger lands soonest goes first.
//!    Every step is paid before the goal whatever the order, so the goal's turn
//!    can only move earlier. On the seat it replaces the beeline arm of
//!    `civic-awaits-its-inspiration`; off the beeline that gene still decides.
//! 2. **Research.** A technology that inspires a road civic already open (or
//!    opening when the civic in progress lands), legal now and at most
//!    [`ROAD_TECH_TURNS`] standard turns of Science away, takes the research
//!    slot ahead of the lane's own beeline, behind every war, barbarian,
//!    wall and luxury goal. It must lie on the lane's beeline (Astronomy and
//!    Radio both do) or the Inspiration must be worth
//!    [`ROAD_TECH_VALUE_SHARE`] of its Science, so the opening's Construction
//!    (100 Science for 22 Culture) is not taken for Games and Recreation.
//!
//! Off, nothing changes.

use super::{AdvancedAi, GrandStrategy, VictoryTarget};
use crate::game::{Game, Item};
use crate::name::Name;
use crate::reasoning::plain;
use crate::rules::BoostSpec;
use crate::think;

/// The furthest an Inspiration may be, in standard turns, and still hold its
/// civic back: 20 Online turns, about the time the road's open frontier (two
/// to four civics of five to eight turns each) takes to research.
pub(super) const ROAD_TRIGGER_HORIZON: u32 = 30;

/// The longest the trigger technology may take, in standard turns of the
/// empire's Science (10 Online turns). Astronomy took 7.5 at 140731Z's rate.
pub(super) const ROAD_TECH_TURNS: u32 = 15;

/// Off the lane's beeline, the Inspiration's Culture must reach this share of
/// the technology's Science before the research slot pays for it.
pub(super) const ROAD_TECH_VALUE_SHARE: f64 = 0.5;

/// Live turns per Corps still to form while matched pairs stand ready. The
/// command phase forms them when a pair stands together with its moves, which
/// took 2-5 turns a Corps live: 153619Z went from two to six Corps in t134-148,
/// 152217Z from two to three in t137-142, 144009Z from none to three in
/// t148-158. A high reading only holds the civic back behind another step the
/// road pays for anyway.
const CORPS_TURNS_EACH: f64 = 4.0;

/// The civic whose prerequisite closure is the road.
const ROAD_GOAL: &str = "cold_war";

/// The Culture lane's own technology beeline (`advanced_research`).
const CULTURE_LANE_TECHS: [&str; 3] = ["printing", "radio", "computers"];

/// The civic that unlocks Corps.
const CORPS_CIVIC: &str = "nationalism";

impl AdvancedAi {
    /// On, an active Culture target, and the lane still open.
    pub(super) fn culture_road_active(&self, g: &Game) -> bool {
        self.culture_road_earns_its_inspirations
            && !self.lane_lost
            && self.active_victory_target(g) == Some(VictoryTarget::Culture)
    }

    /// Live turns until `civic`'s Inspiration lands, with its trigger, when
    /// our own play is visibly bringing it inside the horizon. `None` for a
    /// civic with no boost, one in hand, or a trigger nothing in hand advances.
    pub(super) fn culture_road_inspiration_eta(
        &self,
        g: &Game,
        pid: usize,
        civic: &str,
    ) -> Option<(f64, String)> {
        let boost = g.rules.civics.get(civic)?.boost.as_ref()?;
        if Self::boost_in_hand(g, pid, civic, false) {
            return None;
        }
        let trigger = boost.trigger.as_str();
        let eta = if matches!(trigger, "improvements" | "improvement:farm" | "wonders")
            || trigger.starts_with("district:")
        {
            Self::civic_inspiration_eta(g, pid, civic).map(|(eta, _)| eta)
        } else {
            self.culture_road_trigger_eta(g, pid, civic, boost)
        }?;
        (eta <= g.standard_duration(ROAD_TRIGGER_HORIZON) as f64)
            .then(|| (eta, trigger.replace(['_', ':'], " ")))
    }

    fn culture_road_trigger_eta(
        &self,
        g: &Game,
        pid: usize,
        civic: &str,
        boost: &BoostSpec,
    ) -> Option<f64> {
        let trigger = boost.trigger.as_str();
        if let Some(tech) = trigger.strip_prefix("tech:") {
            return self.culture_road_tech_eta(g, pid, tech, civic);
        }
        let (have, need) = g.boost_progress(pid, boost);
        let remaining = need.max(1) - have;
        // Counted here but not credited by the host: the host reads the
        // trigger some other way, so nothing in hand is bringing it.
        if remaining <= 0 {
            return None;
        }
        match trigger {
            "corps" => (g.players[pid].civics.contains(&Name::new(CORPS_CIVIC))
                && Self::corps_pairs(g, pid) >= remaining)
                .then_some(CORPS_TURNS_EACH * remaining as f64),
            "pop" if remaining == 1 => g
                .player_city_ids(pid)
                .into_iter()
                .filter(|cid| i64::from(g.cities[cid].pop) == need - 1)
                .filter_map(|cid| {
                    let city = &g.cities[&cid];
                    let surplus = g.city_yields(cid).food - 2.0 * f64::from(city.pop);
                    (surplus > 0.0).then(|| {
                        ((g.growth_cost(city.pop) - city.food).max(0.0) / surplus)
                            .ceil()
                            .max(1.0)
                    })
                })
                .min_by(f64::total_cmp),
            "specialty_districts" if remaining == 1 => {
                let owned: Vec<Name> = g
                    .player_city_ids(pid)
                    .into_iter()
                    .flat_map(|cid| g.cities[&cid].districts.keys().copied().collect::<Vec<_>>())
                    .map(|district| g.district_family(district))
                    .collect();
                Self::culture_road_queue_turns(g, pid, 1, |item| {
                    matches!(item, Item::District { district, .. } if {
                        let family = g.district_family(*district);
                        !owned.contains(&family)
                            && g.rules
                                .districts
                                .get(family.as_str())
                                .is_some_and(|spec| spec.specialty)
                    })
                })
            }
            _ => {
                if let Some(unit) = trigger.strip_prefix("units_of:") {
                    Self::culture_road_queue_turns(
                        g,
                        pid,
                        remaining,
                        |item| matches!(item, Item::Unit { unit: queued } if *queued == unit),
                    )
                } else if let Some(building) = trigger.strip_prefix("building:") {
                    Self::culture_road_queue_turns(g, pid, remaining, |item| {
                        matches!(item, Item::Building { building: queued }
                            if *queued == building
                                || g.building_is_family(*queued, Name::new(building)))
                    })
                } else {
                    None
                }
            }
        }
    }

    /// Pairs of our same-kind military units still outside a formation: the
    /// Corps the command phase can form next.
    pub(super) fn corps_pairs(g: &Game, pid: usize) -> i64 {
        let mut kinds: std::collections::BTreeMap<Name, i64> = Default::default();
        for unit in g.units.values() {
            if unit.owner != pid || unit.formation != 0 {
                continue;
            }
            let Some(spec) = g.rules.units.get(unit.kind.as_str()) else {
                continue;
            };
            if spec.class == "military" && spec.domain.as_deref() != Some("air") {
                *kinds.entry(unit.kind).or_default() += 1;
            }
        }
        kinds.values().map(|count| count / 2).sum()
    }

    /// Turns until `count` of our cities finish a queue head `wanted` names,
    /// read from the host's own count when the export carried it.
    fn culture_road_queue_turns(
        g: &Game,
        pid: usize,
        count: i64,
        wanted: impl Fn(&Item) -> bool,
    ) -> Option<f64> {
        let mut turns: Vec<f64> = g
            .player_city_ids(pid)
            .into_iter()
            .filter_map(|cid| {
                let item = g.cities[&cid].queue.first()?;
                if !wanted(item) {
                    return None;
                }
                let turns = g.host_production_turns(cid, item).unwrap_or_else(|| {
                    (g.item_cost_for_city(pid, cid, item) - g.item_invested_production(cid, item))
                        .max(0.0)
                        / g.city_yields(cid).production.max(0.5)
                });
                Some(turns.ceil().max(1.0))
            })
            .collect();
        turns.sort_by(f64::total_cmp);
        usize::try_from(count)
            .ok()
            .filter(|count| *count >= 1)
            .and_then(|count| turns.get(count - 1).copied())
    }

    /// Live turns until `tech` is known, when it is in research or will be
    /// taken for `civic`'s Inspiration ([`Self::culture_road_tech_due`]).
    fn culture_road_tech_eta(&self, g: &Game, pid: usize, tech: &str, civic: &str) -> Option<f64> {
        let player = &g.players[pid];
        if player.techs.contains(&Name::new(tech)) || !g.rules.techs.contains_key(tech) {
            return None;
        }
        let rate = Self::research_rate(g, pid, true);
        let left = |node: &str, progress: f64| {
            ((g.tech_cost(node) - progress).max(0.0) / rate)
                .ceil()
                .max(1.0)
        };
        if player.research.as_deref() == Some(tech) {
            return Some(left(tech, player.research_progress));
        }
        if !self.culture_road_tech_due(g, pid, tech, civic) {
            return None;
        }
        let current = player
            .research
            .as_deref()
            .filter(|&current| g.rules.techs.contains_key(current))
            .map_or(0.0, |current| left(current, player.research_progress));
        Some(current + left(tech, 0.0))
    }

    /// Is `tech` legal now, close, and worth the research slot for `civic`'s
    /// Inspiration: on the Culture lane's own beeline, or worth
    /// [`ROAD_TECH_VALUE_SHARE`] of its Science?
    fn culture_road_tech_due(&self, g: &Game, pid: usize, tech: &str, civic: &str) -> bool {
        if !g.available_techs(pid).iter().any(|legal| *legal == tech) {
            return false;
        }
        let rate = Self::research_rate(g, pid, true);
        if g.tech_cost(tech) / rate > g.standard_duration(ROAD_TECH_TURNS) as f64 {
            return false;
        }
        let on_lane = CULTURE_LANE_TECHS
            .into_iter()
            .filter(|goal| !g.players[pid].techs.contains(&Name::new(goal)))
            .any(|goal| self.tech_leads_to(g, tech, goal));
        on_lane
            || Self::boost_frac(g, civic, false) * g.civic_cost(civic)
                >= ROAD_TECH_VALUE_SHARE * g.tech_cost(tech)
    }

    /// Is `civic` on the road and open now, or opening when the civic in
    /// progress lands?
    fn culture_road_civic_near(&self, g: &Game, pid: usize, civic: &str) -> bool {
        let player = &g.players[pid];
        let name = Name::new(civic);
        if player.civics.contains(&name) || !self.civic_leads_to(g, civic, ROAD_GOAL) {
            return false;
        }
        player.civic.as_deref() == Some(civic)
            || g.rules.civics[civic].requires.iter().all(|req| {
                player.civics.contains(req) || player.civic.as_deref() == Some(req.as_str())
            })
    }

    /// The technology that inspires the most valuable near road civic, with
    /// that civic, when [`Self::culture_road_tech_due`] admits it. The research
    /// slot's goal on the seat; `None` with the gene off.
    pub(super) fn culture_road_trigger_tech(&self, g: &Game, pid: usize) -> Option<(Name, Name)> {
        if !self.culture_road_active(g) {
            return None;
        }
        g.rules
            .civics
            .iter()
            .filter(|(civic, _)| self.culture_road_civic_near(g, pid, civic.as_str()))
            .filter(|(civic, _)| !Self::boost_in_hand(g, pid, civic.as_str(), false))
            .filter_map(|(civic, spec)| {
                let tech = spec.boost.as_ref()?.trigger.strip_prefix("tech:")?;
                (!g.players[pid].techs.contains(&Name::new(tech))
                    && self.culture_road_tech_due(g, pid, tech, civic.as_str()))
                .then(|| {
                    (
                        Self::boost_frac(g, civic.as_str(), false) * g.civic_cost(civic.as_str()),
                        Name::new(tech),
                        *civic,
                    )
                })
            })
            .max_by(|a, b| a.0.total_cmp(&b.0).then_with(|| b.2.cmp(&a.2)))
            .map(|(_, tech, civic)| (tech, civic))
    }

    fn culture_road_finish_turns(g: &Game, pid: usize, civic: &str) -> f64 {
        let player = &g.players[pid];
        let progress = if player.civic.as_deref() == Some(civic) {
            player.civic_progress
        } else {
            0.0
        };
        ((g.civic_cost(civic) - progress).max(0.0) / Self::research_rate(g, pid, false))
            .ceil()
            .max(1.0)
    }

    /// The civic to adopt instead of `pick` on the beeline toward `goal`, or
    /// `None` to keep it.
    pub(super) fn culture_road_pick(
        &self,
        g: &Game,
        pid: usize,
        available: &[Name],
        pick: &Name,
        goal: &str,
        strategy: GrandStrategy,
    ) -> Option<Name> {
        let waits = |civic: &Name| {
            let (eta, trigger) = self.culture_road_inspiration_eta(g, pid, civic.as_str())?;
            (eta > Self::culture_road_finish_turns(g, pid, civic.as_str()))
                .then_some((eta, trigger))
        };
        let (pick_eta, trigger) = waits(pick)?;
        let steps: Vec<Name> = available
            .iter()
            .filter(|civic| *civic != pick && self.civic_leads_to(g, civic, goal))
            .cloned()
            .collect();
        let free: Vec<Name> = steps
            .iter()
            .filter(|civic| waits(civic).is_none())
            .cloned()
            .collect();
        let first = if self.beeline_orders_by_value {
            self.beeline_step(g, pid, strategy, &free, false)
        } else {
            free.into_iter().min_by(|a, b| {
                self.beeline_step_cost(g, pid, a.as_str(), false)
                    .total_cmp(&self.beeline_step_cost(g, pid, b.as_str(), false))
                    .then(a.cmp(b))
            })
        };
        let (choice, why) = match first {
            Some(choice) => (choice, "has nothing to wait for".to_string()),
            None => {
                let (eta, choice) = steps
                    .iter()
                    .filter_map(|civic| waits(civic).map(|(eta, _)| (eta, *civic)))
                    .min_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)))?;
                if eta >= pick_eta {
                    return None;
                }
                (
                    choice,
                    format!("its own inspiration lands sooner, in about {eta:.0}"),
                )
            }
        };
        think!(self.journal(), Research, Decision,
            "Adopting {} before {}", plain(&choice), plain(pick);
            "culture-road-earns-its-inspirations: both lead to {}; {}'s inspiration ({}) lands in \
             about {:.0} turns, after it would finish; {} {}",
            plain(goal), plain(pick), trigger, pick_eta, plain(&choice), why);
        Some(choice)
    }
}

#[cfg(test)]
mod tests;
