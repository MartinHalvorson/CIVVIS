//! City campaign: appraise the neighbours, pick the city — or two, or three
//! — this empire can take AND hold, march a force with units to spare,
//! launch, and pillage with the movement the march does not use.
//!
//! Operator goal (2026-08-24): *"much stronger tactical smarts and planning
//! for taking enemy cities, particularly for weaker enemies. analyze
//! neighboring enemies' military strength (public information) and their
//! progress in science, which often dictates how strong their units are. we
//! can pick a city or two or more that we can likely take and hold and plan
//! out and launch an attack on the city. we should have units to spare as we
//! may lose some to the enemy units or cities (we should try not to though
//! certainly). we should also try to quickly pillage the tiles we can if we
//! have time and that won't slow down our attack too much."*
//!
//! ## What the shipped controller already does, and what it never reads
//!
//! The elective war opens on an EMPIRE-WIDE ratio — `assess`: `my_power >
//! weakest_rival × 1.8 + 20` from turn 55 — aims at a rival by
//! `rival_value` (distance, military power, score) and at a city by
//! `campaign_city_value`, then declares when `my_power > theirs × 1.32 + 12`
//! and three bodies stand on the 3–5 ring (`campaign_staged_for_war`). Three
//! things a human reads before a war are missing from every one of those:
//!
//! 1. **Science.** `military_power` sums the strength of the units a rival
//!    HAS; it says nothing about the units their next fifty turns of research
//!    field, and nothing about ours. Tech count is public (the score
//!    breakdown), and `damage` is `30·e^((att−def)/25)`: ten points of
//!    strength — one unit tier, six to eight techs — is a 1.5× blow each
//!    way. A neighbour ahead in science is a neighbour whose walls and
//!    garrison get stronger while we march; one behind is the weaker enemy
//!    the operator names.
//! 2. **The city, not the empire.** What takes a city is the strength AT it
//!    — its own strength, its walls, the defenders within reach — against
//!    the force we can put on its ring. The 1.32× rule compares empires and
//!    is satisfied by a navy on the other coast.
//! 3. **Holding it.** `campaign_city_value` charges `occupation_risk` for
//!    loyalty pressure but never refuses a city that flips back in eight
//!    turns; `should_defer_city_capture` refuses only the capital-first case.
//!
//! ## The gene `city-campaign`
//!
//! [`AdvancedAi::plan_city_campaign`] appraises every met major with a city
//! within [`CAMPAIGN_REACH`] of ours on public facts alone — military power
//! and tech count ([`AdvancedAi::appraise_neighbour`]) — and keeps the ones
//! we out-muscle by [`CAMPAIGN_POWER_RATIO`] and do not trail by more than
//! [`CAMPAIGN_TECH_DEFICIT_MAX`] techs (or out-muscle by
//! [`CAMPAIGN_OVERWHELMING_RATIO`] regardless). Each of their cities is
//! priced by [`AdvancedAi::campaign_city_requirement`]: the strength at the
//! city — defenders within [`CAMPAIGN_DEFENDER_RADIUS`], the city's own
//! strength, its walls at [`WALL_STRENGTH_PER_100_HP`] — with the tech gap
//! moving every defender by [`TECH_STRENGTH_PER_TECH`] a tech, times
//! [`CAMPAIGN_SUPERIORITY`], which is the spare: the bodies the force can
//! lose to the garrison and the walls and still take the city. A city is
//! holdable when the loyalty its capture projects
//! (`population_loyalty_delta_with_capture`, the engine's own pressure
//! arithmetic) is at least [`CAMPAIGN_HOLD_LOYALTY_FLOOR`] a turn and the
//! capital-first rule does not defer it. The plan is the cheapest holdable
//! city relative to the field army, drawn only once the army can already
//! afford [`CAMPAIGN_PLAN_FRACTION`] of it (Conquest production fills the
//! rest; a plan the army is nowhere near would be the permanent Conquest
//! posture the 2026-08-15 Babylon war was), then up to
//! [`CAMPAIGN_MAX_CITIES`] more of the same rival's cities, each within
//! [`CAMPAIGN_HOP`] of the last and fitting inside
//! [`CAMPAIGN_SEQUEL_FRACTION`] of the army, because the first city costs
//! bodies.
//!
//! The plan is read in three places. `assess` takes Conquest while a plan
//! stands and points the campaign at the plan's rival and its first city
//! still in enemy hands ([`AdvancedAi::campaign_target`],
//! [`AdvancedAi::campaign_objective_city`]). The declaration in
//! `advanced_diplomacy` replaces the empire ratio with the city's own bill
//! ([`AdvancedAi::campaign_launch_ready`]): the bodies on the staging ring
//! must carry [`CampaignPlan::requirement`] strength and
//! [`CampaignPlan::bodies`] bodies, one of them a capturer. And once every
//! planned city is ours the peace desk offers peace
//! ([`AdvancedAi::city_campaign_diplomacy`]), the way the raid closes when
//! it has paid, with [`CAMPAIGN_REPEAT_COOLDOWN`] turns before the next
//! plan. A plan not launched within [`CAMPAIGN_PATIENCE`] standard turns is
//! dropped and re-drawn; while at peace it is refreshed every turn so the
//! target follows the board, keeping its age when the rival is the same.
//!
//! ## The gene `campaign-pillage`
//!
//! [`AdvancedAi::campaign_pillage_step`]: a soldier at war standing on a
//! tile it may pillage spends the movement it was NOT going to use on the
//! pillage — when it cannot move any further this turn (the engine pays a
//! step only from full movement or from at least the step's cost, so a
//! leftover fraction is real and otherwise wasted), when its force is
//! holding, mustering or recovering, or when it stands in the siege ring
//! with its blow declined. It runs AFTER the attack scan — a blow always
//! comes first, and `consume_unit_attack` zeroes movement, so a unit that
//! struck never reaches it — and BEFORE the march, and it refuses whenever
//! the march could still carry an advancing unit, so no pillage costs a
//! tile of advance. The heal half of a pillage is `pillage-to-heal`'s and
//! the raid's prizes are `raid-pillage-prizes`'; this is the campaign's own
//! leftover movement, worth `plunder × (era + 1)` to us and the tile to
//! them.
//!
//! Both genes are off in `AdvancedAi::new()` and `legacy()`, `Kind::OptIn`
//! rows in `genes.rs`, byte-identical when off, and priced apart so the
//! screen says which half pays.

use super::{AdvancedAi, ForceGroup, ForcePosture, StrategicPlan};
use crate::game::{effective_strength, Action, City, Game};
use crate::think;
use crate::Pos;

/// A planned city must lie within this many tiles of one of ours — the
/// same reach the declaration's `close_enough` already asks for.
pub(crate) const CAMPAIGN_REACH: i32 = 18;
/// The next city of a plan lies within this many tiles of the one before
/// it: the army walks on, it does not re-deploy.
pub(crate) const CAMPAIGN_HOP: i32 = 8;
/// Our military power over the neighbour's, both public, for the neighbour
/// to count as weaker.
pub(crate) const CAMPAIGN_POWER_RATIO: f64 = 1.25;
/// At this ratio the neighbour is weaker whatever its science says.
pub(crate) const CAMPAIGN_OVERWHELMING_RATIO: f64 = 2.0;
/// A neighbour ahead of us by more techs than this is not weaker, whatever
/// its army counts today: its next units out-tier ours.
pub(crate) const CAMPAIGN_TECH_DEFICIT_MAX: usize = 3;
/// What a tech of lead is worth on every defender's strength: a unit tier
/// is ten points and six to eight techs apart.
pub(crate) const TECH_STRENGTH_PER_TECH: f64 = 1.5;
/// The tech edge saturates at a tier and a half.
pub(crate) const TECH_STRENGTH_CAP: f64 = 15.0;
/// A hundred points of wall count as this much defending strength: melee
/// blows land at 15% through walls and the city shoots back while they
/// stand.
pub(crate) const WALL_STRENGTH_PER_100_HP: f64 = 10.0;
/// ⭐ THE SPARE. The force at the city carries this much of the strength
/// against it: the bodies it can lose to the garrison and the walls and
/// still take the city.
pub(crate) const CAMPAIGN_SUPERIORITY: f64 = 1.5;
/// One body above what the strength arithmetic asks, so a loss on the
/// march does not send the force back to the ring.
pub(crate) const CAMPAIGN_SPARE_BODIES: usize = 1;
/// Never fewer bodies than the shipped formation floor.
pub(crate) const CAMPAIGN_MIN_BODIES: usize = 3;
/// A captured city whose projected loyalty per turn is at least this holds:
/// from 100 that is twenty-five turns before a flip, time enough for the
/// pressure of the cities we take beside it.
pub(crate) const CAMPAIGN_HOLD_LOYALTY_FLOOR: f64 = -4.0;
/// A plan names at most this many cities.
pub(crate) const CAMPAIGN_MAX_CITIES: usize = 3;
/// The field army must already carry this fraction of the first city's
/// bill for a plan to be drawn. ⚠ At one half the first probe parked the
/// empire in Conquest posture waiting for the other half — on-seats finished
/// with 0.56 fewer cities and a fifth less army (the war-freezes-expansion
/// trap); three quarters keeps the wait short.
pub(crate) const CAMPAIGN_PLAN_FRACTION: f64 = 0.75;
/// A later city of the plan must fit inside this fraction of the army the
/// plan was drawn with — the first city costs bodies.
pub(crate) const CAMPAIGN_SEQUEL_FRACTION: f64 = 0.8;
/// No plan before this standard turn, matching the declaration's own floor.
pub(crate) const CAMPAIGN_MIN_TURN: u32 = 35;
/// A plan not launched within this many standard turns is dropped.
pub(crate) const CAMPAIGN_PATIENCE: u32 = 20;
/// After a campaign closes in peace — or a plan expires unlaunched — no plan
/// for this many standard turns (`campaign_retry_after`).
pub(crate) const CAMPAIGN_REPEAT_COOLDOWN: u32 = 15;
/// Defenders this close to a city are the city's.
pub(crate) const CAMPAIGN_DEFENDER_RADIUS: i32 = 6;
/// A unit this close to the objective city is in its siege ring.
pub(crate) const CAMPAIGN_RING: i32 = 2;
/// A body with no army to average has this strength: a Warrior's.
const DEFAULT_BODY_STRENGTH: f64 = 20.0;

/// The campaign this controller has drawn against one neighbour.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CampaignPlan {
    /// The rival whose cities these are.
    pub(crate) target: usize,
    /// The cities to take, in marching order; a city taken or lost to the
    /// rival is dropped at the start of every turn.
    pub(crate) cities: Vec<u32>,
    /// The friendly strength the first city asks on the staging ring, spare
    /// included.
    pub(crate) requirement: f64,
    /// The bodies the first city asks on the staging ring, spare included.
    pub(crate) bodies: usize,
    /// The turn the plan was drawn.
    pub(crate) planned: u32,
    /// The turn the war on `target` was found open under this plan.
    pub(crate) declared: Option<u32>,
    /// Planned cities captured so far.
    pub(crate) taken: usize,
}

/// A neighbour on public facts: military power and tech count.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NeighbourAppraisal {
    pub(crate) rival: usize,
    /// Our military power over theirs.
    pub(crate) power_ratio: f64,
    /// Our tech count minus theirs.
    pub(crate) tech_lead: i64,
    /// The nearest pair of our city and theirs.
    pub(crate) distance: i32,
}

impl NeighbourAppraisal {
    /// A neighbour we can take a city from: in reach, out-muscled, and not
    /// out-teching us — or out-muscled outright.
    pub(crate) fn weak_enough(&self) -> bool {
        self.distance <= CAMPAIGN_REACH
            && (self.power_ratio >= CAMPAIGN_OVERWHELMING_RATIO
                || (self.power_ratio >= CAMPAIGN_POWER_RATIO
                    && self.tech_lead >= -(CAMPAIGN_TECH_DEFICIT_MAX as i64)))
    }

    /// The strength the tech gap takes off (or adds to) every defender.
    pub(crate) fn tech_strength_edge(&self) -> f64 {
        (self.tech_lead as f64 * TECH_STRENGTH_PER_TECH)
            .clamp(-TECH_STRENGTH_CAP, TECH_STRENGTH_CAP)
    }
}

/// What one city asks of the force that takes it, and whether it holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CityRequirement {
    /// Friendly strength on the ring, spare included.
    pub(crate) strength: f64,
    /// Bodies on the ring, spare included.
    pub(crate) bodies: usize,
    /// The capture projects loyalty at or above the floor and is not
    /// deferred by the capital-first rule.
    pub(crate) holdable: bool,
}

impl AdvancedAi {
    /// Whether a plan stands to read: the gene is on, the rival is alive and
    /// a legal target, and a planned city is still theirs.
    pub(crate) fn city_campaign_stands(&self, g: &Game, pid: usize) -> bool {
        self.city_campaign
            && self.campaign.as_ref().is_some_and(|plan| {
                g.players
                    .get(plan.target)
                    .is_some_and(|player| player.alive)
                    && self.campaign_target_legal(g, pid, plan.target)
                    && plan.cities.iter().any(|cid| {
                        g.cities
                            .get(cid)
                            .is_some_and(|city| city.owner == plan.target)
                    })
            })
    }

    /// The plan's rival, for `assess` to aim the campaign at.
    pub(crate) fn campaign_target(&self, g: &Game, pid: usize) -> Option<usize> {
        self.city_campaign_stands(g, pid)
            .then(|| self.campaign.as_ref().map(|plan| plan.target))
            .flatten()
    }

    /// The plan's first city still in the rival's hands, when `assess` has
    /// aimed the campaign at the plan's rival.
    pub(crate) fn campaign_objective_city(
        &self,
        g: &Game,
        pid: usize,
        target: Option<usize>,
    ) -> Option<u32> {
        if !self.city_campaign_stands(g, pid) {
            return None;
        }
        let plan = self.campaign.as_ref()?;
        if target != Some(plan.target) {
            return None;
        }
        plan.cities.iter().copied().find(|cid| {
            g.cities
                .get(cid)
                .is_some_and(|city| city.owner == plan.target)
        })
    }

    /// Our field army: land soldiers that can take or shell a city, above
    /// the withdraw line.
    pub(crate) fn campaign_field_army(&self, g: &Game, pid: usize) -> Vec<u32> {
        g.player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let unit = &g.units[uid];
                let spec = &g.rules.units[unit.kind];
                spec.class == "military"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && (spec.is_melee_capable() || spec.has_ranged_attack())
                    && unit.hp as f64 > self.base.w.withdraw_hp
            })
            .collect()
    }

    /// The strength of a set of our units, wounds priced.
    pub(crate) fn campaign_strength_of(g: &Game, units: &[u32]) -> f64 {
        units
            .iter()
            .filter_map(|uid| g.units.get(uid))
            .map(|unit| effective_strength(g.unit_strength(unit, true), unit.hp))
            .sum()
    }

    /// A neighbour on public facts: `None` for ourselves, the dead, minors,
    /// barbarians, a major we have not met and are not at war with, one the
    /// campaign may not legally target, and one with no cities.
    pub(crate) fn appraise_neighbour(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> Option<NeighbourAppraisal> {
        let player = g.players.get(rival)?;
        if rival == pid || !player.alive || player.is_minor || player.is_barbarian {
            return None;
        }
        if !g.has_met(pid, rival) && !g.is_at_war(pid, rival) {
            return None;
        }
        if !self.campaign_target_legal(g, pid, rival) {
            return None;
        }
        let mine = g.player_city_ids(pid);
        let theirs = g.player_city_ids(rival);
        let distance = mine
            .iter()
            .flat_map(|a| {
                theirs
                    .iter()
                    .map(move |b| g.wdist(g.cities[a].pos, g.cities[b].pos))
            })
            .min()?;
        let power_ratio = g.military_power(pid) / g.military_power(rival).max(1.0);
        let tech_lead = g.players[pid].techs.len() as i64 - player.techs.len() as i64;
        Some(NeighbourAppraisal {
            rival,
            power_ratio,
            tech_lead,
            distance,
        })
    }

    /// What `city` asks of the force that takes it: the defenders within
    /// [`CAMPAIGN_DEFENDER_RADIUS`], the city's own strength and its walls,
    /// every one of them moved by the tech edge, times the spare — and
    /// whether the capture holds.
    pub(crate) fn campaign_city_requirement(
        &self,
        g: &Game,
        pid: usize,
        city_id: u32,
        appraisal: &NeighbourAppraisal,
        average_body: f64,
    ) -> CityRequirement {
        let city = &g.cities[&city_id];
        let edge = appraisal.tech_strength_edge();
        let defenders: f64 = g
            .units
            .values()
            .filter(|unit| {
                unit.owner == city.owner && g.wdist(unit.pos, city.pos) <= CAMPAIGN_DEFENDER_RADIUS
            })
            .filter(|unit| {
                let spec = &g.rules.units[unit.kind];
                spec.class == "military" && spec.domain.as_deref() != Some("air")
            })
            .map(|unit| (effective_strength(g.unit_strength(unit, true), unit.hp) - edge).max(0.0))
            .sum();
        let walls = city.wall_hp.max(0) as f64 / 100.0 * WALL_STRENGTH_PER_100_HP;
        let at_city = (g.city_strength(city_id) - edge).max(0.0) + walls;
        let strength = (defenders + at_city) * CAMPAIGN_SUPERIORITY;
        let bodies = ((strength / average_body.max(1.0)).ceil() as usize).max(CAMPAIGN_MIN_BODIES)
            + CAMPAIGN_SPARE_BODIES;
        let holdable = !Self::should_defer_city_capture(g, pid, city_id)
            && Self::population_loyalty_delta_with_capture(g, pid, city_id, true)
                >= CAMPAIGN_HOLD_LOYALTY_FLOOR;
        CityRequirement {
            strength,
            bodies,
            holdable,
        }
    }

    /// The nearest of our cities to `pos`.
    fn campaign_core_distance(g: &Game, pid: usize, pos: Pos) -> i32 {
        g.player_city_ids(pid)
            .into_iter()
            .map(|cid| g.wdist(g.cities[&cid].pos, pos))
            .min()
            .unwrap_or(i32::MAX)
    }

    /// ⭐ THE PLAN: the cheapest holdable city of a weaker neighbour relative
    /// to the field army, then up to [`CAMPAIGN_MAX_CITIES`] of that rival's
    /// cities within a hop of the last, each fitting the army with the
    /// first city's cost taken out. `None` before the turn floor, with one
    /// city, with no weaker neighbour, or with no holdable city the army can
    /// already afford three quarters of.
    pub(crate) fn plan_city_campaign(&self, g: &Game, pid: usize) -> Option<CampaignPlan> {
        if g.turn < g.standard_duration(CAMPAIGN_MIN_TURN) || g.player_city_ids(pid).len() < 2 {
            return None;
        }
        let army = self.campaign_field_army(g, pid);
        let army_strength = Self::campaign_strength_of(g, &army);
        let average_body = if army.is_empty() {
            DEFAULT_BODY_STRENGTH
        } else {
            army_strength / army.len() as f64
        };
        let mut best: Option<((f64, i32, u32), NeighbourAppraisal, CityRequirement)> = None;
        for rival in 0..g.players.len() {
            let Some(appraisal) = self.appraise_neighbour(g, pid, rival) else {
                continue;
            };
            if !appraisal.weak_enough() {
                continue;
            }
            for city in g.cities.values().filter(|city| city.owner == rival) {
                let core_distance = Self::campaign_core_distance(g, pid, city.pos);
                if core_distance > CAMPAIGN_REACH {
                    continue;
                }
                let requirement =
                    self.campaign_city_requirement(g, pid, city.id, &appraisal, average_body);
                if !requirement.holdable
                    || army_strength < requirement.strength * CAMPAIGN_PLAN_FRACTION
                {
                    continue;
                }
                let key = (
                    requirement.strength / army_strength.max(1.0),
                    core_distance,
                    city.id,
                );
                if best.as_ref().is_none_or(|(held, _, _)| key < *held) {
                    best = Some((key, appraisal, requirement));
                }
            }
        }
        let ((_, _, first), appraisal, requirement) = best?;
        let mut cities = vec![first];
        let mut last = g.cities[&first].pos;
        let mut remaining: Vec<&City> = g
            .cities
            .values()
            .filter(|city| city.owner == appraisal.rival && city.id != first)
            .collect();
        while cities.len() < CAMPAIGN_MAX_CITIES {
            let next = remaining
                .iter()
                .enumerate()
                .filter(|(_, city)| g.wdist(last, city.pos) <= CAMPAIGN_HOP)
                .filter(|(_, city)| {
                    let bill =
                        self.campaign_city_requirement(g, pid, city.id, &appraisal, average_body);
                    bill.holdable && bill.strength <= army_strength * CAMPAIGN_SEQUEL_FRACTION
                })
                .min_by_key(|(_, city)| (g.wdist(last, city.pos), city.id))
                .map(|(index, _)| index);
            let Some(index) = next else {
                break;
            };
            let city = remaining.remove(index);
            cities.push(city.id);
            last = city.pos;
        }
        Some(CampaignPlan {
            target: appraisal.rival,
            cities,
            requirement: requirement.strength,
            bodies: requirement.bodies,
            planned: g.turn,
            declared: None,
            taken: 0,
        })
    }

    /// One of the `campaign:*` counters the probe rows read.
    fn campaign_count(g: &mut Game, pid: usize, key: &str, by: i64) {
        *g.players[pid].counters.entry(key.to_string()).or_insert(0) += by;
    }

    /// Start of turn: drop the cities the plan has taken or lost, expire a
    /// plan never launched (and hold off the next for the cooldown), and
    /// draw or refresh one while at peace.
    pub(crate) fn maintain_city_campaign(&mut self, g: &mut Game, pid: usize) {
        if !self.city_campaign {
            self.campaign = None;
            return;
        }
        let mut previous = self.campaign.take();
        if let Some(plan) = previous.as_mut() {
            let taken_now = plan
                .cities
                .iter()
                .filter(|cid| g.cities.get(cid).is_some_and(|city| city.owner == pid))
                .count();
            if taken_now > 0 {
                plan.taken += taken_now;
                Self::campaign_count(g, pid, "campaign:taken", taken_now as i64);
            }
            plan.cities.retain(|cid| {
                g.cities
                    .get(cid)
                    .is_some_and(|city| city.owner == plan.target)
            });
            if g.is_at_war(pid, plan.target) && plan.declared.is_none() {
                plan.declared = Some(g.turn);
                Self::campaign_count(g, pid, "campaign:declared", 1);
            }
        }
        let at_war_with = |plan: &CampaignPlan| {
            g.players
                .get(plan.target)
                .is_some_and(|player| player.alive)
                && g.is_at_war(pid, plan.target)
        };
        if let Some(plan) = previous.as_ref().filter(|plan| at_war_with(plan)) {
            // A war under way is a commitment: the plan holds until every
            // city is taken and the peace desk has closed it.
            self.campaign = Some(plan.clone());
            return;
        }
        let cooldown = g
            .turn
            .saturating_add(g.standard_duration(CAMPAIGN_REPEAT_COOLDOWN));
        if previous
            .as_ref()
            .is_some_and(|plan| plan.declared.is_some())
        {
            // The war this plan opened is over: the plan is spent, and the
            // next one waits out the cooldown.
            self.peace_until = self.peace_until.max(cooldown);
            self.campaign_retry_after = self.campaign_retry_after.max(cooldown);
            previous = None;
        }
        let major_war = g.players.iter().any(|player| {
            player.id != pid
                && player.alive
                && !player.is_minor
                && !player.is_barbarian
                && g.is_at_war(pid, player.id)
        });
        if major_war || g.turn < self.peace_until || g.turn < self.campaign_retry_after {
            self.campaign = None;
            return;
        }
        let Some(mut fresh) = self.plan_city_campaign(g, pid) else {
            self.campaign = None;
            return;
        };
        if let Some(plan) = previous.filter(|plan| plan.target == fresh.target) {
            fresh.planned = plan.planned;
        }
        if g.turn.saturating_sub(fresh.planned) >= g.standard_duration(CAMPAIGN_PATIENCE) {
            // Twenty turns without a launch: the target moved, or the army
            // never came. Not again for the cooldown — a plan redrawn the
            // next turn would hold the Conquest posture forever.
            self.campaign_retry_after = self.campaign_retry_after.max(cooldown);
            self.campaign = None;
            return;
        }
        if fresh.planned == g.turn {
            Self::campaign_count(g, pid, "campaign:planned", 1);
        }
        if self.journal().wants(crate::reasoning::Level::Strategy) && fresh.planned == g.turn {
            let names: Vec<String> = fresh
                .cities
                .iter()
                .filter_map(|cid| g.cities.get(cid))
                .map(|city| city.name.clone())
                .collect();
            let appraisal = self.appraise_neighbour(g, pid, fresh.target);
            think!(self.journal(), Military, Strategy,
                   "Planning a campaign against {}", g.players[fresh.target].civ;
                   "{} — power {:.2}× theirs, {:+} techs; the first city asks {:.0} strength in {} bodies with the spare",
                   names.join(", then "),
                   appraisal.map(|a| a.power_ratio).unwrap_or(0.0),
                   appraisal.map(|a| a.tech_lead).unwrap_or(0),
                   fresh.requirement, fresh.bodies);
        }
        self.campaign = Some(fresh);
    }

    /// The declaration's readiness under a plan: `None` when no plan names
    /// `target`, else whether the bodies on the staging ring carry the
    /// first city's bill — strength, bodies and a capturer.
    pub(crate) fn campaign_launch_ready(
        &self,
        g: &Game,
        pid: usize,
        target: usize,
        plan: &StrategicPlan,
    ) -> Option<bool> {
        if !self.city_campaign_stands(g, pid) {
            return None;
        }
        let campaign = self.campaign.as_ref()?;
        if campaign.target != target {
            return None;
        }
        let objective = plan.target_city.and_then(|cid| g.cities.get(&cid))?.pos;
        let staged = self.staged_campaign_units(g, pid, target, objective);
        let strength = Self::campaign_strength_of(g, &staged);
        let has_capturer = staged
            .iter()
            .any(|uid| g.rules.units[g.units[uid].kind].is_melee_capable());
        Some(
            strength + 1e-9 >= campaign.requirement
                && staged.len() >= campaign.bodies
                && has_capturer,
        )
    }

    /// The peace desk: a campaign whose every city is ours offers peace,
    /// once, until it is accepted.
    pub(crate) fn city_campaign_diplomacy(&mut self, g: &mut Game, pid: usize) {
        if !self.city_campaign {
            return;
        }
        let Some(campaign) = self.campaign.clone() else {
            return;
        };
        if !g.is_at_war(pid, campaign.target)
            || campaign.taken == 0
            || campaign.cities.iter().any(|cid| {
                g.cities
                    .get(cid)
                    .is_some_and(|city| city.owner == campaign.target)
            })
        {
            return;
        }
        let peace_pending = g.pending_deals.iter().any(|deal| {
            deal.peace
                && ((deal.from == pid && deal.to == campaign.target)
                    || (deal.from == campaign.target && deal.to == pid))
                && deal.expires >= g.turn
        });
        if peace_pending || self.peace_offers.contains(&campaign.target) {
            return;
        }
        think!(self.journal(), Diplomacy, Decision,
               "Offering peace to {}", g.players[campaign.target].civ;
               "the campaign planned on turn {} has taken its {} cit{}",
               campaign.planned, campaign.taken, if campaign.taken == 1 { "y" } else { "ies" });
        self.peace_offers.insert(campaign.target);
        let _ = g.apply(
            pid,
            &Action::ProposeDeal {
                player: campaign.target,
                give_gold: 0.0,
                request_gold: 0.0,
                open_borders: false,
                friendship: false,
                peace: true,
                alliance: None,
            },
        );
    }

    // ------------------------------------------------------------------
    // campaign-pillage
    // ------------------------------------------------------------------

    /// Whether the unit can still leave its tile this turn.
    fn can_still_march(g: &Game, uid: u32) -> bool {
        let here = g.units[&uid].pos;
        g.approach_reach(uid).keys().any(|pos| *pos != here)
    }

    /// The gene: after the attack scan has declined and before the march, a
    /// soldier at war on a tile it may pillage spends movement it was not
    /// going to use — because it cannot move on, because its force is
    /// holding, mustering or recovering, or because it stands in the siege
    /// ring with its blow declined. `None` whenever the march could still
    /// carry it.
    pub(crate) fn campaign_pillage_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
        group: Option<&ForceGroup>,
    ) -> Option<bool> {
        if !self.campaign_pillage {
            return None;
        }
        let unit = g.units.get(&uid)?;
        let spec = &g.rules.units[unit.kind];
        if spec.class != "military"
            || matches!(spec.domain.as_deref(), Some("sea" | "air"))
            || g.is_embarked(unit)
            || unit.moves_left <= 0.0
            || !g.pillageable_at(pid, unit.pos)
        {
            return None;
        }
        let here = unit.pos;
        let waiting = group.is_some_and(|orders| {
            matches!(
                orders.posture,
                ForcePosture::Hold | ForcePosture::Muster | ForcePosture::Recover
            )
        });
        let in_the_ring = plan
            .target_city
            .and_then(|cid| g.cities.get(&cid))
            .is_some_and(|city| {
                g.is_at_war(pid, city.owner)
                    && g.wdist(here, city.pos) <= CAMPAIGN_RING
                    && g.units[&uid].attacks_left > 0
            });
        if !waiting && !in_the_ring && Self::can_still_march(g, uid) {
            return None;
        }
        let pillaged = g.apply(pid, &Action::Pillage { unit: uid }).is_ok();
        if pillaged {
            Self::campaign_count(g, pid, "campaign:pillaged", 1);
        }
        if pillaged && self.journal().wants(crate::reasoning::Level::Detail) {
            let why = if waiting {
                "the force is waiting here"
            } else if in_the_ring {
                "it stands in the siege ring with no blow to land"
            } else {
                "it could not have moved on this turn"
            };
            think!(self.journal(), Military, Detail,
                   "{} pillages the tile it stands on", crate::reasoning::plain(&g.units[&uid].kind);
                   "{why}, and the movement was going unused"; here);
        }
        Some(pillaged)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::opt_in_off_in_both_controllers;
    use super::super::GrandStrategy;
    use super::*;
    use crate::game::Game;
    use crate::name;

    #[test]
    fn city_campaign_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("city-campaign", |ai| ai.city_campaign);
    }

    #[test]
    fn campaign_pillage_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("campaign-pillage", |ai| ai.campaign_pillage);
    }

    /// A weaker neighbour is one we out-muscle and do not trail in science;
    /// an out-muscled one is weaker whatever it researches.
    #[test]
    fn a_weaker_neighbour_is_read_from_power_and_science() {
        let read = |power_ratio: f64, tech_lead: i64, distance: i32| NeighbourAppraisal {
            rival: 1,
            power_ratio,
            tech_lead,
            distance,
        };
        assert!(read(1.3, 0, 10).weak_enough());
        assert!(read(1.3, -3, 10).weak_enough());
        assert!(
            !read(1.3, -4, 10).weak_enough(),
            "four techs behind is not weaker"
        );
        assert!(
            read(2.0, -9, 10).weak_enough(),
            "twice the army is weaker whatever they research"
        );
        assert!(
            !read(1.2, 5, 10).weak_enough(),
            "a tech lead does not make a peer weaker"
        );
        assert!(!read(1.3, 0, 19).weak_enough(), "out of reach");
        assert_eq!(read(1.0, 4, 1).tech_strength_edge(), 6.0);
        assert_eq!(read(1.0, 40, 1).tech_strength_edge(), TECH_STRENGTH_CAP);
        assert_eq!(read(1.0, -40, 1).tech_strength_edge(), -TECH_STRENGTH_CAP);
    }

    /// A flat board of `majors` empires, every starting unit cleared, the
    /// map explored and everyone met; each capital founded at the position
    /// given, nobody at war, turn 60.
    fn flat_board(seed: u64, capitals: &[Pos]) -> Game {
        let mut game = Game::new_full(capitals.len(), 36, 22, seed, 1_000, 0, false);
        for unit in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(unit);
        }
        game.barb_camps.clear();
        game.barb_naval_camps.clear();
        for tile in game.map.tiles.values_mut() {
            tile.terrain = name!("grassland");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
        }
        for (pid, pos) in capitals.iter().enumerate() {
            game.found_city_for(pid, *pos, None);
        }
        for pid in 0..capitals.len() {
            for other in 0..capitals.len() {
                if pid != other {
                    game.players[pid].met.insert(other);
                }
            }
            game.players[pid]
                .explored
                .extend(game.map.tiles.keys().copied());
        }
        game.at_war.clear();
        game.turn = 60;
        game.current = 0;
        game
    }

    fn fresh(game: &mut Game, uid: u32) {
        let moves = game.unit_max_moves(uid);
        let unit = game.units.get_mut(&uid).unwrap();
        unit.moves_left = moves;
        unit.attacks_left = 1;
        unit.moved = false;
        unit.acted = false;
    }

    fn warriors(game: &mut Game, pid: usize, around: Pos, count: usize) -> Vec<u32> {
        let mut spawned = Vec::new();
        let ring: Vec<Pos> = game
            .wring(around, 2)
            .into_iter()
            .filter(|pos| game.city_at(*pos).is_none() && game.unit_ids_at(*pos).is_empty())
            .collect();
        for pos in ring.into_iter().take(count) {
            let uid = game.spawn_test_unit("warrior", pid, pos);
            fresh(game, uid);
            spawned.push(uid);
        }
        assert_eq!(spawned.len(), count);
        spawned
    }

    fn give_techs(game: &mut Game, pid: usize, count: usize) {
        for tech in [
            name!("pottery"),
            name!("animal_husbandry"),
            name!("mining"),
            name!("sailing"),
            name!("astrology"),
            name!("irrigation"),
            name!("archery"),
            name!("writing"),
            name!("masonry"),
            name!("bronze_working"),
        ]
        .into_iter()
        .take(count)
        {
            game.players[pid].techs.insert(tech);
        }
    }

    /// Three empires: ours with six Warriors and a second city, a weak
    /// neighbour with one Warrior, a strong one with six and eight techs of
    /// lead. The plan names the weak neighbour's capital, asks the spare,
    /// and the gene off draws nothing.
    #[test]
    fn the_plan_names_the_weaker_neighbour_and_asks_the_spare() {
        let mut game = flat_board(80_301, &[(6, 10), (16, 10), (28, 10)]);
        let home = game.cities[&game.player_city_ids(0)[0]].pos;
        game.found_city_for(0, (6, 5), None);
        let army = warriors(&mut game, 0, home, 6);
        let weak = game.cities[&game.player_city_ids(1)[0]].pos;
        warriors(&mut game, 1, weak, 1);
        let strong = game.cities[&game.player_city_ids(2)[0]].pos;
        warriors(&mut game, 2, strong, 6);
        give_techs(&mut game, 2, 8);

        let mut ai = AdvancedAi::new();
        ai.maintain_city_campaign(&mut game, 0);
        assert_eq!(ai.campaign, None, "off, nothing is planned");

        ai.enable_city_campaign();
        let weak_reading = ai.appraise_neighbour(&game, 0, 1).unwrap();
        assert!(weak_reading.weak_enough(), "{weak_reading:?}");
        let strong_reading = ai.appraise_neighbour(&game, 0, 2).unwrap();
        assert!(!strong_reading.weak_enough(), "{strong_reading:?}");
        assert_eq!(strong_reading.tech_lead, -8);

        ai.maintain_city_campaign(&mut game, 0);
        let plan = ai
            .campaign
            .clone()
            .expect("a plan against the weak neighbour");
        assert_eq!(plan.target, 1);
        assert_eq!(plan.cities, vec![game.player_city_ids(1)[0]]);
        assert_eq!(plan.planned, 60);
        assert_eq!(plan.declared, None);
        // The bill: one Warrior (20) plus the city's own strength, times the
        // spare, in bodies of our own average plus one.
        let average = AdvancedAi::campaign_strength_of(&game, &army) / army.len() as f64;
        let hostile = 20.0 + game.city_strength(game.player_city_ids(1)[0]);
        assert!(
            (plan.requirement - hostile * CAMPAIGN_SUPERIORITY).abs() < 1e-9,
            "{plan:?}"
        );
        assert_eq!(
            plan.bodies,
            ((plan.requirement / average).ceil() as usize).max(CAMPAIGN_MIN_BODIES)
                + CAMPAIGN_SPARE_BODIES
        );
        assert!(ai.city_campaign_stands(&game, 0));
        assert_eq!(ai.campaign_target(&game, 0), Some(1));
        assert_eq!(
            ai.campaign_objective_city(&game, 0, Some(1)),
            Some(game.player_city_ids(1)[0])
        );
        assert_eq!(ai.campaign_objective_city(&game, 0, Some(2)), None);

        // The same plan a turn later keeps its age; thirty turns on with no
        // war it is dropped.
        game.turn = 61;
        ai.maintain_city_campaign(&mut game, 0);
        assert_eq!(ai.campaign.as_ref().map(|plan| plan.planned), Some(60));
        assert_eq!(game.players[0].counters.get("campaign:planned"), Some(&1));
        game.turn = 60 + game.standard_duration(CAMPAIGN_PATIENCE);
        ai.maintain_city_campaign(&mut game, 0);
        assert_eq!(ai.campaign, None, "a plan never launched expires");
        assert!(
            ai.campaign_retry_after > game.turn,
            "and holds off the next"
        );
        game.turn += 1;
        ai.maintain_city_campaign(&mut game, 0);
        assert_eq!(ai.campaign, None, "no plan inside the cooldown");
        game.turn = ai.campaign_retry_after;
        ai.maintain_city_campaign(&mut game, 0);
        assert!(ai.campaign.is_some(), "and a fresh one after it");
        assert_eq!(game.players[0].counters.get("campaign:planned"), Some(&2));
    }

    /// A city the capture cannot hold — the rival's population beside it
    /// and none of ours — is not planned; the same city beside a big city
    /// of ours is.
    #[test]
    fn a_city_that_would_flip_back_is_not_planned() {
        let mut game = flat_board(80_302, &[(6, 10), (14, 10)]);
        let home = game.cities[&game.player_city_ids(0)[0]].pos;
        game.found_city_for(0, (6, 4), None);
        let army = warriors(&mut game, 0, home, 6);
        let rival_capital = game.player_city_ids(1)[0];
        // A second rival city three tiles on, populous: its pressure on the
        // capital outweighs ours at eight tiles.
        let second = game.found_city_for(1, (17, 10), None);
        game.cities.get_mut(&second).unwrap().pop = 8;
        let mut ai = AdvancedAi::new();
        ai.enable_city_campaign();
        let appraisal = ai.appraise_neighbour(&game, 0, 1).unwrap();
        assert!(appraisal.weak_enough());
        let average = AdvancedAi::campaign_strength_of(&game, &army) / army.len() as f64;
        let bill = ai.campaign_city_requirement(&game, 0, rival_capital, &appraisal, average);
        assert!(!bill.holdable, "{bill:?}");
        // The populous second city holds itself — a captured city's own
        // citizens are domestic pressure — so the plan names it and leaves
        // the capital that would flip back.
        let second_bill = ai.campaign_city_requirement(&game, 0, second, &appraisal, average);
        assert!(second_bill.holdable, "{second_bill:?}");
        ai.maintain_city_campaign(&mut game, 0);
        let plan = ai.campaign.clone().expect("the city that holds is planned");
        assert_eq!(plan.target, 1);
        assert!(plan.cities.contains(&second), "{plan:?}");
        assert!(
            !plan.cities.contains(&rival_capital),
            "a capital that flips back is not planned: {plan:?}"
        );

        // Our capital grown to twelve and a populous city of ours four tiles
        // from theirs: the capture holds, and the capital joins the plan.
        game.cities
            .get_mut(&game.player_city_ids(0)[0])
            .unwrap()
            .pop = 12;
        let near = game.found_city_for(0, (10, 10), None);
        game.cities.get_mut(&near).unwrap().pop = 10;
        let bill = ai.campaign_city_requirement(&game, 0, rival_capital, &appraisal, average);
        assert!(bill.holdable, "{bill:?}");
        ai.maintain_city_campaign(&mut game, 0);
        let plan = ai.campaign.clone().expect("a holdable capital is planned");
        assert_eq!(plan.target, 1);
        assert!(plan.cities.contains(&rival_capital), "{plan:?}");
        assert!(plan.cities.contains(&second), "{plan:?}");
        assert!(plan.cities.len() <= CAMPAIGN_MAX_CITIES);
    }

    /// Launch waits for the bill on the staging ring: strength, bodies and a
    /// capturer.
    #[test]
    fn the_launch_waits_for_the_bill_on_the_staging_ring() {
        let mut game = flat_board(80_303, &[(6, 10), (16, 10)]);
        let home = game.cities[&game.player_city_ids(0)[0]].pos;
        game.found_city_for(0, (6, 5), None);
        warriors(&mut game, 0, home, 6);
        let objective = game.player_city_ids(1)[0];
        let weak = game.cities[&objective].pos;
        warriors(&mut game, 1, weak, 1);
        let mut ai = AdvancedAi::new();
        ai.enable_city_campaign();
        ai.maintain_city_campaign(&mut game, 0);
        let plan = ai.campaign.clone().expect("planned");
        let strategic = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: Some(objective),
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: 60,
            rush: false,
        };
        assert_eq!(
            ai.campaign_launch_ready(&game, 0, 1, &strategic),
            Some(false),
            "nothing staged"
        );
        assert_eq!(
            ai.campaign_launch_ready(&game, 0, 2, &strategic),
            None,
            "no plan names player 2"
        );
        // Stage the bill on the ring, four tiles out on our side.
        let ring: Vec<Pos> = game
            .wring(weak, 4)
            .into_iter()
            .filter(|pos| {
                game.wdist(*pos, home) < game.wdist(weak, home)
                    && game.map.tiles[pos].owner_city.is_none()
                    && game.unit_ids_at(*pos).is_empty()
            })
            .collect();
        assert!(
            ring.len() >= plan.bodies,
            "{} ring tiles for {} bodies",
            ring.len(),
            plan.bodies
        );
        for pos in ring.iter().take(plan.bodies - 1) {
            let uid = game.spawn_test_unit("warrior", 0, *pos);
            fresh(&mut game, uid);
        }
        assert_eq!(
            ai.campaign_launch_ready(&game, 0, 1, &strategic),
            Some(false),
            "one body short of the bill"
        );
        let uid = game.spawn_test_unit("warrior", 0, ring[plan.bodies - 1]);
        fresh(&mut game, uid);
        assert_eq!(
            ai.campaign_launch_ready(&game, 0, 1, &strategic),
            Some(true)
        );
    }

    /// A warrior at war on an enemy Farm: with a holding force it pillages;
    /// advancing with a march still in it, it does not; with half a move
    /// left it cannot step and pillages; off, never.
    #[test]
    fn leftover_movement_pillages_and_a_march_does_not() {
        let mut game = flat_board(80_304, &[(6, 10), (16, 10)]);
        game.at_war.insert((0, 1));
        let rival_city = game.player_city_ids(1)[0];
        let rival = game.cities[&rival_city].pos;
        let farms: Vec<Pos> = game
            .wring(rival, 1)
            .into_iter()
            .filter(|pos| {
                game.map.tiles[pos].owner_city == Some(rival_city)
                    && game.city_at(*pos).is_none()
                    && game.map.tiles[pos].district.is_none()
            })
            .collect();
        assert!(farms.len() >= 3, "{farms:?}");
        for farm in &farms {
            game.map.tiles.get_mut(farm).unwrap().improvement = Some(name!("farm"));
            assert!(game.pillageable_at(0, *farm));
        }
        let strategic = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: None,
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: 60,
            rush: false,
        };
        let group = |posture: ForcePosture, unit: u32, at: Pos| ForceGroup {
            id: unit,
            domain: super::super::ForceDomain::Land,
            units: vec![unit],
            anchor: at,
            objective: rival,
            focus_target: None,
            posture,
            readiness: 1.0,
            local_strength_ratio: 1.0,
        };

        let warrior = game.spawn_test_unit("warrior", 0, farms[0]);
        fresh(&mut game, warrior);
        let mut stock = AdvancedAi::new();
        let hold = group(ForcePosture::Hold, warrior, farms[0]);
        assert_eq!(
            stock.campaign_pillage_step(&mut game, 0, warrior, &strategic, Some(&hold)),
            None,
            "off, nothing"
        );
        assert!(!game.map.tiles[&farms[0]].pillaged);

        let mut ai = AdvancedAi::new();
        ai.enable_campaign_pillage();
        let advance = group(ForcePosture::Advance, warrior, farms[0]);
        assert_eq!(
            ai.campaign_pillage_step(&mut game, 0, warrior, &strategic, Some(&advance)),
            None,
            "an advancing unit with a march in it keeps marching"
        );
        assert!(!game.map.tiles[&farms[0]].pillaged);
        assert_eq!(
            ai.campaign_pillage_step(&mut game, 0, warrior, &strategic, Some(&hold)),
            Some(true),
            "a holding force pillages where it stands"
        );
        assert!(game.map.tiles[&farms[0]].pillaged);
        assert_eq!(game.units[&warrior].moves_left, 0.0);
        assert_eq!(game.players[0].counters.get("campaign:pillaged"), Some(&1));

        // Half a move left cannot pay a step and is spent on the pillage.
        let stuck = game.spawn_test_unit("warrior", 0, farms[1]);
        fresh(&mut game, stuck);
        game.units.get_mut(&stuck).unwrap().moves_left = 0.5;
        assert!(!AdvancedAi::can_still_march(&game, stuck));
        assert_eq!(
            ai.campaign_pillage_step(&mut game, 0, stuck, &strategic, None),
            Some(true)
        );
        assert!(game.map.tiles[&farms[1]].pillaged);

        // In the siege ring with its blow declined, it pillages too.
        let besieging = game.spawn_test_unit("warrior", 0, farms[2]);
        fresh(&mut game, besieging);
        let siege = StrategicPlan {
            target_city: Some(rival_city),
            ..strategic.clone()
        };
        assert_eq!(
            ai.campaign_pillage_step(&mut game, 0, besieging, &siege, None),
            Some(true)
        );
        assert!(game.map.tiles[&farms[2]].pillaged);
    }
}
