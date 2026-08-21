//! Great People never pile up: the `great-person-housing` gene.
//!
//! ★★★★ A GREAT PERSON THE EMPIRE HAS EARNED AND CANNOT USE IS A RACE LOST
//! FOR NOTHING. Great People are a global race — the named person every seat
//! is earning toward retires the moment one seat claims them — and a seat
//! whose points sit at the price while the claim is refused hands the person
//! to a rival, then starts the next, dearer person with the same blocker in
//! place. In this rules model a claim is immediate and its blockers are
//! concrete (`Game::validate_great_person_activation`): a Writer wants two
//! open Writing slots, an Artist three Art slots, a Musician two Music
//! slots, a Scientist a Campus, an Engineer an Industrial Zone or a wonder
//! under construction, a General a promotable land unit. Nothing in the
//! deployed agent answered those blockers on a native board: the cultural
//! chain (`amphitheater` → `art_museum` / `broadcast_center`) is vetoed at
//! −10,000 by `production_value` for every lane but Culture, so the Writer
//! points a Theater Square or a wonder yields to a Science seat can never be
//! spent, and the physical-person path (`prioritize_live_great_person_
//! activation`) reads only the host's `live_great_person_activation_needs`,
//! which a native game never fills.
//!
//! The gene is a ladder, run once a turn before strategic production fills
//! the queues:
//!
//! 1. **Build space ahead of the person.** A class whose points will reach
//!    the price within [`GREAT_PERSON_HOUSING_LEAD_TURNS`] and whose claim is
//!    blocked reserves one city for the cheapest thing that lifts the block —
//!    the typed slot building for a Writer, Artist or Musician (the
//!    prerequisite Amphitheater when the museum is not yet buildable), the
//!    district for a Scientist, Merchant, Admiral or Prophet, the cheapest
//!    available wonder for a wonder Engineer, a land soldier for a General.
//!    Districts and wonders are reserved only once the person is **due**: a
//!    Theater Square for points that are still fifteen turns out is a bet,
//!    an Amphitheater in a city that has the square is not. The reservation
//!    takes an idle city or one on a repeatable project; once the person is
//!    due it may also pause an ordinary building (progress is kept per item,
//!    so nothing is lost), never a unit, district, wonder, or the plan's
//!    threatened city.
//! 2. **Sell to make room.** A cultural person who is due while no slot
//!    building can be started anywhere sells duplicate works of the kind the
//!    person makes through the Quick Deals market — which exposes only
//!    genuine duplicates, never a last copy — and recruits the same turn. A
//!    sold work is replaced by the person's new works at the same value, the
//!    Gold is banked, and the race is won rather than forfeited. Works are
//!    never sold to the plan's target or to a rival already past 60% of a
//!    victory: a Great Work is Tourism for whoever houses it.
//!
//! Both rungs are measured as one gene; the screen decides whether the
//! bundle ships ON. Off everywhere by default until then.

use super::{AdvancedAi, StrategicPlan};
use crate::ai::BasicAi;
use crate::game::{Action, Game, Item};
use crate::name::Name;
use crate::reasoning::plain;
use crate::think;

/// How many turns ahead of a class reaching its price the gene starts the
/// building that lifts the claim's blocker. An Amphitheater costs 150
/// production; a mid-game city at ten a turn finishes one in fifteen.
pub(super) const GREAT_PERSON_HOUSING_LEAD_TURNS: f64 = 15.0;

/// A rival this far along any victory is never sold a Great Work.
const GREAT_WORK_SALE_RIVAL_PROGRESS: i32 = 60;

/// The classes the gene watches, in the order ties are broken.
const WATCHED_CLASSES: [&str; 9] = [
    "writer",
    "artist",
    "musician",
    "scientist",
    "engineer",
    "merchant",
    "admiral",
    "general",
    "prophet",
];

/// What lifts a blocked claim, read from the offered person's own effects
/// the way `Game::validate_great_person_activation` reads them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum GreatPersonRemedy {
    /// Open typed Great Work slots of this kind, this many.
    Slots(&'static str, usize),
    /// A district of this family anywhere in the empire.
    District(&'static str),
    /// A wonder under construction for the Engineer's charges to land on.
    Wonder,
    /// A military land unit for a General to promote or lead.
    LandUnit,
    /// A military sea unit for an Admiral to lead.
    SeaUnit,
}

/// One class this empire has earned, or is about to earn, and cannot claim.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct StuckGreatPerson {
    pub(super) kind: String,
    pub(super) remedy: GreatPersonRemedy,
    /// Points already at the price: the person is on the market now.
    pub(super) due: bool,
    /// Turns until the points reach the price at today's rate (zero when due).
    pub(super) turns_to_due: f64,
}

impl StuckGreatPerson {
    /// The Great Work kind and count a cultural person needs housed.
    pub(super) fn work(&self) -> Option<(&'static str, usize)> {
        match self.remedy {
            GreatPersonRemedy::Slots(work, count) => Some((work, count)),
            _ => None,
        }
    }
}

impl AdvancedAi {
    /// Every watched class whose claim is blocked while its points are at, or
    /// within the lead of, the price. A class the host has run out of, or
    /// is not offering, is not stuck: there is nobody to claim.
    pub(super) fn stuck_great_people(&self, g: &Game, pid: usize) -> Vec<StuckGreatPerson> {
        let rates = g.great_person_points_per_turn(pid);
        let mut stuck = Vec::new();
        for kind in WATCHED_CLASSES {
            let Some((_, spec)) = g.current_great_person(kind) else {
                continue;
            };
            if !g.great_person_class_offered_now(pid, kind)
                || !g.great_person_class_earnable(pid, kind)
                || g.can_activate_current_great_person(pid, kind)
            {
                continue;
            }
            let points = g.players[pid].gpp.get(kind).copied().unwrap_or(0.0);
            let cost = g.gp_cost(pid, kind);
            let due = points + f64::EPSILON >= cost;
            let rate = rates.get(kind).copied().unwrap_or(0.0);
            let turns_to_due = if due {
                0.0
            } else if rate > f64::EPSILON {
                (cost - points) / rate
            } else {
                f64::INFINITY
            };
            if turns_to_due > GREAT_PERSON_HOUSING_LEAD_TURNS {
                continue;
            }
            let Some(remedy) = Self::great_person_remedy(kind, &spec.effects) else {
                continue;
            };
            stuck.push(StuckGreatPerson {
                kind: kind.to_string(),
                remedy,
                due,
                turns_to_due,
            });
        }
        stuck
    }

    /// The remedy for a class, from the offered person's effects: the same
    /// reading `Game::validate_great_person_activation` makes of them.
    pub(super) fn great_person_remedy(
        kind: &str,
        effects: &std::collections::BTreeMap<String, f64>,
    ) -> Option<GreatPersonRemedy> {
        let work = [
            ("great_work_writing", "writing"),
            ("great_work_art", "art"),
            ("great_work_music", "music"),
        ]
        .into_iter()
        .find_map(|(effect, work)| {
            let count = effects.get(effect).copied().unwrap_or(0.0).round() as usize;
            (count > 0).then_some(GreatPersonRemedy::Slots(work, count))
        });
        if work.is_some() {
            return work;
        }
        let trade = [
            "free_trader",
            "destination_foreign_trade_gold",
            "free_quadrireme",
            "free_lighthouse",
            "free_shipyard",
        ]
        .iter()
        .any(|effect| effects.contains_key(*effect));
        match kind {
            "scientist" => Some(GreatPersonRemedy::District("campus")),
            "engineer" if effects.contains_key("wonder_production") => {
                Some(GreatPersonRemedy::Wonder)
            }
            "engineer" => Some(GreatPersonRemedy::District("industrial_zone")),
            "merchant" if trade => Some(GreatPersonRemedy::District("commercial_hub")),
            "admiral" if trade => Some(GreatPersonRemedy::District("harbor")),
            "admiral" if effects.contains_key("naval_unit_formation") => {
                Some(GreatPersonRemedy::SeaUnit)
            }
            "general"
                if effects.contains_key("land_unit_formation")
                    || effects.contains_key("land_unit_promotion_level") =>
            {
                Some(GreatPersonRemedy::LandUnit)
            }
            "prophet" => Some(GreatPersonRemedy::District("holy_site")),
            _ => None,
        }
    }

    /// Whether an item already in some queue is the remedy this class waits
    /// on, so the gene reserves one city at a time per class.
    fn great_person_remedy_queued(g: &Game, pid: usize, stuck: &StuckGreatPerson) -> bool {
        g.player_city_ids(pid).into_iter().any(|city| {
            match (g.cities[&city].queue.first(), stuck.remedy) {
                (Some(Item::Building { building }), GreatPersonRemedy::Slots(work, _)) => {
                    let spec = &g.rules.buildings[building];
                    spec.great_work_slots.get(work).copied().unwrap_or(0) > 0
                        || spec.great_work_slots.get("any").copied().unwrap_or(0) > 0
                        || Self::great_work_slot_prerequisites(work)
                            .iter()
                            .any(|family| g.building_is_family(building, Name::new(family)))
                }
                (Some(Item::District { district, .. }), _) => {
                    Self::great_person_district_family(stuck)
                        .is_some_and(|family| g.district_family(*district) == family)
                }
                (Some(Item::Wonder { .. }), GreatPersonRemedy::Wonder) => true,
                (Some(Item::Unit { unit }), GreatPersonRemedy::LandUnit) => {
                    g.rules.units[unit].class == "military"
                        && g.rules.units[unit].domain.as_deref() != Some("sea")
                }
                (Some(Item::Unit { unit }), GreatPersonRemedy::SeaUnit) => {
                    g.rules.units[unit].class == "military"
                        && g.rules.units[unit].domain.as_deref() == Some("sea")
                }
                _ => false,
            }
        })
    }

    /// The buildings a work kind's slot building stands on, nearest first:
    /// a museum needs an Amphitheater, a Broadcast Center needs a museum.
    fn great_work_slot_prerequisites(work: &str) -> &'static [&'static str] {
        match work {
            "art" => &["amphitheater"],
            "music" => &["art_museum", "archaeological_museum", "amphitheater"],
            _ => &[],
        }
    }

    /// The district family whose absence blocks this class, if a district is
    /// the remedy at all.
    fn great_person_district_family(stuck: &StuckGreatPerson) -> Option<&'static str> {
        match stuck.remedy {
            GreatPersonRemedy::Slots(..) => Some("theater_square"),
            GreatPersonRemedy::District(family) => Some(family),
            _ => None,
        }
    }

    /// What this city could start that lifts the class's blocker: the typed
    /// slot building first (more slots of the wanted kind, then cheaper),
    /// then the prerequisite Amphitheater, then — only once the person is due
    /// — the district, wonder, or soldier.
    pub(super) fn great_person_housing_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        stuck: &StuckGreatPerson,
    ) -> Option<Item> {
        if let Some((work, _)) = stuck.work() {
            let mut best: Option<(i32, f64, String, Item)> = None;
            for item in g.producible_items(pid, cid) {
                let Item::Building { building } = &item else {
                    continue;
                };
                let spec = &g.rules.buildings[building];
                let slots = spec.great_work_slots.get(work).copied().unwrap_or(0)
                    + spec.great_work_slots.get("any").copied().unwrap_or(0);
                if slots <= 0 {
                    continue;
                }
                let cost = g.item_cost_for_city(pid, cid, &item);
                let key = format!("{item:?}");
                let better = best
                    .as_ref()
                    .is_none_or(|(old_slots, old_cost, old_key, _)| {
                        slots > *old_slots
                            || (slots == *old_slots
                                && (cost + f64::EPSILON < *old_cost
                                    || ((cost - *old_cost).abs() <= f64::EPSILON
                                        && key < *old_key)))
                    });
                if better {
                    best = Some((slots, cost, key, item));
                }
            }
            if let Some((_, _, _, item)) = best {
                return Some(item);
            }
            // The museum stands on an Amphitheater, the Broadcast Center on
            // a museum: start the nearest missing step of the chain.
            if let Some(item) = Self::great_work_slot_prerequisites(work)
                .iter()
                .find_map(|family| BasicAi::civ_building(g, pid, cid, family))
            {
                return Some(item);
            }
        }
        if !stuck.due {
            return None;
        }
        match stuck.remedy {
            GreatPersonRemedy::Wonder => {
                let wonder_queued = g
                    .player_city_ids(pid)
                    .into_iter()
                    .any(|city| matches!(g.cities[&city].queue.first(), Some(Item::Wonder { .. })));
                if wonder_queued {
                    None
                } else {
                    BasicAi::cheapest_available_wonder(g, pid, cid)
                }
            }
            GreatPersonRemedy::Slots(..) | GreatPersonRemedy::District(_) => {
                let family = Self::great_person_district_family(stuck)?;
                if BasicAi::empire_district_family_ready_or_queued(g, pid, family) {
                    return None;
                }
                BasicAi::live_great_person_district_item(g, pid, cid, family)
            }
            GreatPersonRemedy::LandUnit => {
                self.base
                    .best_military(g, pid, cid, None)
                    .map(|unit| Item::Unit {
                        unit: Name::new(&unit),
                    })
            }
            GreatPersonRemedy::SeaUnit => self
                .base
                .best_naval_unit(g, pid, cid)
                .map(|unit| Item::Unit { unit }),
        }
    }

    /// Sell duplicate works of the kind a due cultural person makes until the
    /// person can be housed, never to a rival who is winning, then recruit.
    /// Returns how many works were sold.
    fn great_person_housing_sale(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
        stuck: &StuckGreatPerson,
    ) -> usize {
        let Some((work, count)) = stuck.work() else {
            return 0;
        };
        let mut sold = 0;
        for _ in 0..count {
            if g.can_house_great_works(pid, work, count) {
                break;
            }
            let deal = g
                .quick_deals(pid)
                .into_iter()
                .filter(|deal| {
                    deal.category == "great_work"
                        && deal.direction == "sell"
                        && deal.item == work
                        && deal.my_value >= 2.0
                        && deal.partner_value >= 2.0
                        && Some(deal.partner) != plan.target_player
                        && self.rival_victory_pressure(g, deal.partner).progress
                            < GREAT_WORK_SALE_RIVAL_PROGRESS
                })
                .max_by(|left, right| {
                    left.my_value
                        .partial_cmp(&right.my_value)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| right.partner.cmp(&left.partner))
                });
            let Some(deal) = deal else {
                break;
            };
            let partner = deal.partner;
            if g.apply(
                pid,
                &Action::Trade {
                    player: partner,
                    offer: Box::new(deal.offer),
                    request: Box::new(deal.request),
                },
            )
            .is_err()
            {
                break;
            }
            sold += 1;
            think!(self.journal(), Economy, Decision,
                   "Selling a {} Great Work to {} to house the {}", work, g.players[partner].civ,
                   plain(&stuck.kind);
                   "the {} is earned and no slot building can start", plain(&stuck.kind));
        }
        if sold > 0 && g.can_activate_current_great_person(pid, &stuck.kind) {
            let _ = g.apply(
                pid,
                &Action::RecruitGreatPerson {
                    kind: stuck.kind.clone(),
                },
            );
        }
        sold
    }

    /// The gene's turn: reserve one city for the first stuck class that any
    /// city can answer, and sell works for a due cultural person no city can
    /// answer. Returns whether a queue or the inventory changed.
    pub(super) fn great_person_housing(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        if !self.great_person_housing || self.base.minor || self.base.barb {
            return false;
        }
        let stuck = self.stuck_great_people(g, pid);
        if stuck.is_empty() {
            return false;
        }
        let city_ids = g.player_city_ids(pid);
        let mut changed = false;
        let mut reserved = false;
        for person in &stuck {
            if Self::great_person_remedy_queued(g, pid, person) {
                continue;
            }
            let mut choice: Option<(f64, String, u32, Item)> = None;
            if !reserved {
                for &cid in &city_ids {
                    let city = &g.cities[&cid];
                    if plan.threatened_city == Some(cid)
                        || (city.last_attacked > 0
                            && g.turn.saturating_sub(city.last_attacked) <= 4)
                    {
                        continue;
                    }
                    let redirectable = match city.queue.first() {
                        None => true,
                        Some(Item::Project { project }) => g.rules.projects[project].repeatable,
                        Some(Item::Building { building }) => {
                            person.due && !g.rules.buildings[building].wonder
                        }
                        _ => false,
                    };
                    if !redirectable {
                        continue;
                    }
                    let Some(item) = self.great_person_housing_item(g, pid, cid, person) else {
                        continue;
                    };
                    // Progress saved for a paused item is keyed privately by
                    // the engine; the estimate counts only an idle city's
                    // carried overflow, as the live helper does.
                    let carried = if city.queue.is_empty() {
                        city.production
                    } else {
                        0.0
                    };
                    let remaining = (g.item_cost_for_city(pid, cid, &item) - carried).max(0.0);
                    let turns = remaining / g.city_yields(cid).production.max(0.1);
                    let key = format!("{item:?}");
                    let better = choice
                        .as_ref()
                        .is_none_or(|(old_turns, old_key, old_city, _)| {
                            turns + f64::EPSILON < *old_turns
                                || ((turns - *old_turns).abs() <= f64::EPSILON
                                    && (key.as_str(), cid) < (old_key.as_str(), *old_city))
                        });
                    if better {
                        choice = Some((turns, key, cid, item));
                    }
                }
            }
            if let Some((turns, _, cid, item)) = choice {
                let prior = g.cities[&cid].queue.first().cloned();
                let city_name = g.cities[&cid].name.clone();
                if g.apply(
                    pid,
                    &Action::Produce {
                        city: cid,
                        item: item.clone(),
                    },
                )
                .is_ok()
                {
                    changed = true;
                    reserved = true;
                    let when = if person.due {
                        "is earned and waiting".to_string()
                    } else {
                        format!("arrives in {:.0} turns", person.turns_to_due)
                    };
                    match prior {
                        Some(prior) => think!(self.journal(), Economy, Decision,
                            "{} pauses {} for {} to house the {}", city_name,
                            Self::plain_item(&prior), Self::plain_item(&item), plain(&person.kind);
                            "the {} {} and cannot be claimed; {:.0} turns to build",
                            plain(&person.kind), when, turns),
                        None => think!(self.journal(), Economy, Decision,
                            "{} starts {} to house the {}", city_name, Self::plain_item(&item),
                            plain(&person.kind);
                            "the {} {} and cannot be claimed; {:.0} turns to build",
                            plain(&person.kind), when, turns),
                    }
                    continue;
                }
            }
            // No city can start a remedy: a due cultural person sells its way
            // to a slot.
            if person.due
                && person.work().is_some()
                && self.great_person_housing_sale(g, pid, plan, person) > 0
            {
                changed = true;
            }
        }
        changed
    }
}
