//! `hopeless-army-yields-the-queue`: a Domination seat stops training
//! soldiers for an offensive whose walls nothing it can field will breach,
//! and stops adding deterrents a rival twice its power will not notice.
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
//! another player whose walls stand, where no Battering Ram or Siege Tower
//! of ours (fielded or buildable) works on those walls
//! (`Game::city_allows_siege_support`) and our best land gun, fielded or
//! buildable, cannot breach and take it even with
//! `siege_production::BREAKER_MATCH_MAX` guns within
//! `siege_production::BREAKER_MATCH_MAX_TURNS` turns at the city's
//! strength (`siege_production::matched_guns`, the `breakers-match-the-walls`
//! arithmetic at its most generous budget), or we have no land gun at all.
//! A buildable gun or Ram that would open the walls makes the siege viable,
//! so it is never withheld. The offensive read is the plan's target city
//! while we are at war with its owner or the plan is Conquest against it.
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
//! - `near-rival-deterrence`, while the neighbour it reads fields
//!   [`DETERRENCE_OUT_OF_REACH`] times our power or more, trains only a
//!   ranged garrison, and only in a city with no ranged land unit of ours on
//!   or beside it; and it holds a queue against the economy's claims only
//!   for such a garrison.
//!
//! **What still trains**: a threatened, under-fire or freshly attacked city
//! (`plan.threatened_city`, `last_attacked` within four turns, a barbarian
//! alarm) keeps the scorer's full army target, and the defence writers
//! (`redirect_unsafe_city_queue_for_defense`, the confirmed-defence queue,
//! the surprise and capital garrison purchases, the barbarian floor) are
//! untouched. Guns already queued keep their progress.
//!
//! Off, or on any other lane, nothing changes. Grep the journal for
//! `hopeless-army-yields-the-queue:`.

use super::siege_production::{matched_guns, BREAKER_MATCH_MAX, BREAKER_MATCH_MAX_TURNS};
use super::*;
use crate::game::expected_damage;

/// The neighbour's power against ours from which a deterrent cannot deter:
/// half the rival's army is past what a city floor's queue can answer.
pub(crate) const DETERRENCE_OUT_OF_REACH: f64 = 2.0;

/// How far from its city a ranged land unit of ours stands as its garrison:
/// the city tile, or beside it when a melee unit holds the tile.
pub(crate) const GARRISON_REACH: i32 = 1;

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
}

impl HopelessWalls {
    /// The reading as a journal detail.
    pub(crate) fn describe(&self) -> String {
        match self.hit {
            Some(hit) => format!(
                "walls {}/{}: our best gun hits {hit:.1} a shot against {:.0}, and \
                 {BREAKER_MATCH_MAX} cannot breach and take the city within \
                 {BREAKER_MATCH_MAX_TURNS:.0} turns; no Ram or Tower of ours works on them",
                self.walls, self.max_walls, self.defense
            ),
            None => format!(
                "walls {}/{}: no land gun fielded or buildable, and no Ram or Tower of ours \
                 works on them",
                self.walls, self.max_walls
            ),
        }
    }
}

/// A land siege gun: the units `breakers-match-the-walls` reads.
fn land_gun(spec: &crate::rules::UnitSpec) -> bool {
    spec.class == "military"
        && spec.siege
        && spec.has_ranged_attack()
        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
}

/// A land ranged unit that is not a siege gun: a city's garrison.
fn garrison_shooter(spec: &crate::rules::UnitSpec) -> bool {
    spec.class == "military"
        && !spec.siege
        && spec.has_ranged_attack()
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

    /// See the module: `cid`'s standing walls, when nothing `pid` fields or
    /// can build opens them. `None` for a city of ours, a city with no wall
    /// standing, or walls a Ram, Tower or matched gun count would open.
    /// Ungated; callers check [`Self::hopeless_army_active`].
    pub(crate) fn walls_nothing_breaches(g: &Game, pid: usize, cid: u32) -> Option<HopelessWalls> {
        let city = g.cities.get(&cid)?;
        if city.owner == pid || city.wall_hp <= 0 {
            return None;
        }
        for kind in ["battering_ram", "siege_tower"] {
            let name = Name::new(kind);
            if g.rules.units.contains_key(&name)
                && g.city_allows_siege_support(cid, kind)
                && (g
                    .units
                    .values()
                    .any(|unit| unit.owner == pid && unit.kind == name)
                    || Self::hopeless_army_buildable(g, pid, name))
            {
                return None;
            }
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
        let defense = g.city_strength(cid);
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

    /// Whether a deterrent against a neighbour of `theirs` power is out of
    /// reach: the gene acts and the neighbour fields
    /// [`DETERRENCE_OUT_OF_REACH`] times our power or more.
    pub(super) fn deterrence_out_of_reach(&self, g: &Game, pid: usize, theirs: f64) -> bool {
        self.hopeless_army_active(g)
            && theirs > 0.0
            && theirs >= DETERRENCE_OUT_OF_REACH * g.military_power(pid)
    }

    /// Whether a ranged land unit of ours (not a siege gun) stands on `cid`
    /// or within [`GARRISON_REACH`] of it.
    pub(super) fn city_has_ranged_garrison(g: &Game, pid: usize, cid: u32) -> bool {
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        g.units.values().any(|unit| {
            unit.owner == pid
                && garrison_shooter(&g.rules.units[unit.kind])
                && !g.is_embarked(unit)
                && g.wdist(unit.pos, city.pos) <= GARRISON_REACH
        })
    }

    /// The strongest ranged land unit (not a siege gun) `cid` can train: the
    /// garrison an out-of-reach deterrence still asks for.
    pub(super) fn garrison_shooter_for(g: &Game, pid: usize, cid: u32) -> Option<Name> {
        g.rules
            .units
            .iter()
            .filter(|(kind, spec)| {
                garrison_shooter(spec) && g.can_produce(pid, cid, &Item::Unit { unit: **kind })
            })
            .max_by(|a, b| {
                a.1.ranged_attack_strength()
                    .total_cmp(&b.1.ranged_attack_strength())
                    .then(b.0.cmp(a.0))
            })
            .map(|(kind, _)| *kind)
    }

    /// Whether `item` is the ranged garrison of a city of ours that has none
    /// yet: the one deterrent an out-of-reach deterrence still holds.
    pub(super) fn deterrence_garrison_head(g: &Game, pid: usize, item: &Item) -> bool {
        let Item::Unit { unit } = item else {
            return false;
        };
        g.rules.units.get(unit).is_some_and(garrison_shooter)
            && g.player_city_ids(pid).into_iter().any(|cid| {
                g.cities[&cid].queue.first() == Some(item)
                    && !Self::city_has_ranged_garrison(g, pid, cid)
            })
    }
}

#[cfg(test)]
mod tests;
