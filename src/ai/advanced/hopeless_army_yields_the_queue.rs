//! `hopeless-army-yields-the-queue`: a Domination seat stops training
//! guns and offensive bodies for an offensive whose walls nothing it can
//! field will breach.
//!
//! **Why (live Deity Gran Colombia, 4-player Pangaea, Online; the 17
//! Domination games of 2026-11-12 00:01Z-06:48Z, turns 40-120).** The seat
//! ends turn 100 about 20 techs behind (22 against 46 in
//! civvis-20261112T064834Z, Science 39 against ~190) while it pays upkeep
//! on a field army of Ancient and Classical units. Over those games it
//! ordered 349 land soldiers and siege guns:
//! - **At peace, 168** (128 soldiers, 40 guns). 89 of them were
//!   `near-rival-deterrence` (29 claimed directly, 60 more the units the
//!   claim held against `campus-claims-the-free-slot`), 19 walled-assault
//!   guns reserved before a war (`breaker-before-the-war`) and 13
//!   `domination-target-gets-its-breaker` guns. 58 of the 191 deterrent
//!   decisions were taken against a rival with twice our power or more.
//! - **At war, on the 396 of 781 war turns the siege reading said the walls
//!   could not be opened ("nothing to open the walls", "cannot breach",
//!   infinite turns, 0% ready), 181** (98 soldiers, 83 guns). 62 answered a
//!   city under fire and stay; the rest were 41 walled-assault reservations,
//!   about 22 resumed guns, 21 value-chooser switches, 12 deterrents and 5
//!   breaker claims. Those wars landed 20 hits on a city in 396 turns, 17 of
//!   them in one game. In 064834Z the war with Ethiopia ran 57 turns with no
//!   hit on Gondar (walls 300, our best gun a Catapult striking 7.4 a shot
//!   against 70) while seven Catapults were reserved for it.
//!
//! **The reading** ([`AdvancedAi::walls_nothing_breaches`]): a city of
//! another player whose walls stand and that neither path takes:
//! - **guns**: our best land gun, fielded or buildable, cannot breach and
//!   take it even with `siege_production::BREAKER_MATCH_MAX` guns within
//!   `siege_production::BREAKER_MATCH_MAX_TURNS` turns at the city's
//!   strength (`siege_production::matched_guns`, the
//!   `breakers-match-the-walls` arithmetic at its most generous budget), or
//!   we have no land gun at all. Guns that take the city to 0 health need no
//!   strong taker: the engine lets any melee unit walk into a city at 0
//!   health without a blow (`Game::do_attack`);
//! - **Ram or Tower**: no Battering Ram or Siege Tower of ours (fielded or
//!   buildable) works on those walls (`Game::city_allows_siege_support`), or
//!   one does but the city's return blow removes our best land melee taker,
//!   fielded or buildable (non-siege, non-ranged, cavalry included), at full
//!   health: its strength stands `outclassed_siege::ONE_BLOW_GAP` or more
//!   over the taker's, the gap at which a blow's upper roll removes a
//!   full-health unit. A taker that dies attacking leaves the city at 1
//!   health and never captures (`Game::do_attack`). Live Deity
//!   civvis-20261112T043248Z t85-103 stood before Smolensk's Ancient Walls
//!   with a Ram in hand at a city strength of 67 against our best melee of
//!   35-36 and took nothing in 40 turns of war.
//!
//! A buildable gun count, Ram or taker that would make the siege viable is
//! never withheld. The offensive read is the plan's target city while we are
//! at war with its owner or the plan is Conquest against it.
//!
//! **What yields**, on the Domination lane only:
//! - the delegated walled-assault reservation
//!   (`reserve_delegated_domination_siege`) orders no gun, and journals the
//!   reading once a turn;
//! - `restore_wall_breaker_queues` does not take a queue back for a gun;
//! - `domination-target-gets-its-breaker` claims no breaker for that target;
//! - the strategic scorer opens no missing-siege slot
//!   (`missing_domination_siege`) and prices its land army at the per-city
//!   garrison (`city_count`) instead of the offensive's target, so the force
//!   gap closes and the economy's items are priced as at peace
//!   ([`AdvancedAi::hopeless_army_target`]). The land-grab, barbarian and
//!   rush floors still apply after it.
//!
//! **What still trains**: a threatened, under-fire or freshly attacked city
//! (`plan.threatened_city`, `last_attacked` within four turns, a barbarian
//! alarm) keeps the scorer's full army target, and the defence writers
//! (`redirect_unsafe_city_queue_for_defense`, the confirmed-defence queue,
//! the surprise and capital garrison purchases, the barbarian floor) are
//! untouched. Guns already queued keep their progress, and nothing here
//! disbands or holds back a unit already standing.
//!
//! **The near-rival deterrence cap** that shipped with this gene in #5169
//! (a ranged garrison only against a neighbour of twice our power) is its
//! own opt-in gene since 11-12, `deterrence-stops-at-a-garrison`; see
//! `advanced/deterrence_stops_at_a_garrison.rs` for why it was split out.
//!
//! Off, or on any other lane, nothing changes. Grep the journal for
//! `hopeless-army-yields-the-queue:`.

use super::outclassed_siege::ONE_BLOW_GAP;
use super::siege_production::{matched_guns, BREAKER_MATCH_MAX, BREAKER_MATCH_MAX_TURNS};
use super::*;
use crate::game::expected_damage;

/// Why a city's walls cannot be opened by anything we field or can build.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct HopelessWalls {
    /// The walled city.
    pub(crate) city: u32,
    /// Its standing and full wall hit points.
    pub(crate) walls: i32,
    pub(crate) max_walls: i32,
    /// The city's defensive strength the blow is read against.
    pub(crate) defense: f64,
    /// Our best land gun's expected blow, fielded or buildable; `None` with
    /// no land gun at all.
    pub(crate) hit: Option<f64>,
    /// Whether a Ram or Tower of ours (fielded or buildable) works on the
    /// walls; it opens them only for a taker that survives its blow.
    pub(crate) breach_support: bool,
    /// Our best land melee taker's strength, fielded or buildable; `None`
    /// with no melee unit at all.
    pub(crate) taker: Option<f64>,
}

impl HopelessWalls {
    /// The reading as a journal detail.
    pub(crate) fn describe(&self) -> String {
        let guns = match self.hit {
            Some(hit) => format!(
                "our best gun hits {hit:.1} a shot against {:.0}, and {BREAKER_MATCH_MAX} \
                 cannot breach and take the city within {BREAKER_MATCH_MAX_TURNS:.0} turns",
                self.defense
            ),
            None => "no land gun fielded or buildable".to_string(),
        };
        let support = if !self.breach_support {
            "no Ram or Tower of ours works on them".to_string()
        } else {
            match self.taker {
                Some(taker) => format!(
                    "a Ram or Tower works on them, but the city's strength {:.0} stands \
                     {ONE_BLOW_GAP:.0} or more over our best melee taker's {taker:.0}, so its \
                     return blow removes the taker",
                    self.defense
                ),
                None => "a Ram or Tower works on them, but we field and can build no melee \
                         taker"
                    .to_string(),
            }
        };
        format!("walls {}/{}: {guns}; {support}", self.walls, self.max_walls)
    }
}

/// A land siege gun: the units `breakers-match-the-walls` reads.
fn land_gun(spec: &crate::rules::UnitSpec) -> bool {
    spec.class == "military"
        && spec.siege
        && spec.has_ranged_attack()
        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
}

/// A land melee unit that can take a city: not a siege gun or a shooter,
/// cavalry included.
fn melee_taker(spec: &crate::rules::UnitSpec) -> bool {
    spec.class == "military"
        && !spec.siege
        && spec.is_melee_capable()
        && !spec.has_ranged_attack()
        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
}

impl AdvancedAi {
    /// The gene is on and the seat plays the Domination lane.
    pub(super) fn hopeless_army_active(&self, g: &Game) -> bool {
        self.hopeless_army_yields_the_queue
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
    }

    /// Whether `pid` can lay `kind` down in any of its cities. A unit whose
    /// tech is unknown is skipped before the per-city menu is asked.
    fn hopeless_army_buildable(g: &Game, pid: usize, kind: Name) -> bool {
        let Some(spec) = g.rules.units.get(&kind) else {
            return false;
        };
        spec.buildable
            && spec
                .tech
                .as_ref()
                .is_none_or(|tech| g.players[pid].techs.contains(tech))
            && g.player_city_ids(pid)
                .into_iter()
                .any(|cid| g.can_produce(pid, cid, &Item::Unit { unit: kind }))
    }

    /// Our best land melee taker's attack strength: the strongest land
    /// melee unit (not a siege gun, not a shooter, cavalry included) we field
    /// unembarked (`Game::unit_strength`) or can build (its listed strength).
    /// `None` with neither.
    pub(crate) fn best_melee_taker(g: &Game, pid: usize) -> Option<f64> {
        let fielded = g
            .units
            .values()
            .filter(|unit| {
                unit.owner == pid && melee_taker(&g.rules.units[unit.kind]) && !g.is_embarked(unit)
            })
            .map(|unit| g.unit_strength(unit, false));
        let buildable = g
            .rules
            .units
            .iter()
            .filter(|(kind, spec)| {
                melee_taker(spec) && Self::hopeless_army_buildable(g, pid, **kind)
            })
            .map(|(_, spec)| spec.strength);
        fielded.chain(buildable).reduce(f64::max)
    }

    /// See the module: `cid`'s standing walls, when nothing `pid` fields or
    /// can build takes it. `None` for a city of ours, a city with no wall
    /// standing, walls a Ram or Tower opens for a taker that survives the
    /// city's blow, or a matched gun count that takes it.
    /// Ungated; callers check [`Self::hopeless_army_active`].
    pub(crate) fn walls_nothing_breaches(g: &Game, pid: usize, cid: u32) -> Option<HopelessWalls> {
        let city = g.cities.get(&cid)?;
        if city.owner == pid || city.wall_hp <= 0 {
            return None;
        }
        let defense = g.city_strength(cid);
        let breach_support = ["battering_ram", "siege_tower"].into_iter().any(|kind| {
            let name = Name::new(kind);
            g.rules.units.contains_key(&name)
                && g.city_allows_siege_support(cid, kind)
                && (g
                    .units
                    .values()
                    .any(|unit| unit.owner == pid && unit.kind == name)
                    || Self::hopeless_army_buildable(g, pid, name))
        });
        let taker = if breach_support {
            Self::best_melee_taker(g, pid)
        } else {
            None
        };
        if taker.is_some_and(|taker| defense < taker + ONE_BLOW_GAP) {
            return None;
        }
        let fielded = g
            .units
            .values()
            .filter(|unit| unit.owner == pid && land_gun(&g.rules.units[unit.kind]))
            .map(|unit| g.rules.units[unit.kind].ranged_attack_strength());
        let buildable = g
            .rules
            .units
            .iter()
            .filter(|(kind, spec)| land_gun(spec) && Self::hopeless_army_buildable(g, pid, **kind))
            .map(|(_, spec)| spec.ranged_attack_strength());
        let attack = fielded.chain(buildable).reduce(f64::max);
        let hit = attack.map(|attack| expected_damage(attack, defense));
        if let Some(hit) = hit {
            let walls = f64::from(city.wall_hp);
            let health = f64::from(city.hp.max(0));
            if matched_guns(hit, walls, health, BREAKER_MATCH_MAX_TURNS).is_some() {
                return None;
            }
        }
        Some(HopelessWalls {
            city: cid,
            walls: city.wall_hp,
            max_walls: g.city_max_wall_hp(city),
            defense,
            hit,
            breach_support,
            taker,
        })
    }

    /// The plan's offensive, read hopeless: the gene acts for this seat, the
    /// plan's target city is held by another player we are at war with (or
    /// the plan is Conquest against it), and its walls are
    /// [`Self::walls_nothing_breaches`]. `None` otherwise.
    pub(super) fn hopeless_offensive(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<HopelessWalls> {
        if !self.hopeless_army_active(g) {
            return None;
        }
        let target = plan.target_city.and_then(|cid| g.cities.get(&cid))?;
        if target.owner == pid
            || !(g.is_at_war(pid, target.owner)
                || (plan.strategy == GrandStrategy::Conquest
                    && plan.target_player == Some(target.owner)))
        {
            return None;
        }
        Self::walls_nothing_breaches(g, pid, target.id)
    }

    /// The strategic scorer's land army target under the gene: the per-city
    /// garrison (`city_count`) while the plan's offensive is hopeless and
    /// neither this city (`threatened`) nor any home city the plan names is
    /// under threat; `desired` otherwise.
    pub(super) fn hopeless_army_target(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        threatened: bool,
        city_count: usize,
        desired: usize,
    ) -> usize {
        if threatened
            || plan.threatened_city.is_some()
            || desired <= city_count
            || self.hopeless_offensive(g, pid, plan).is_none()
        {
            return desired;
        }
        city_count
    }
}

#[cfg(test)]
mod tests;
