//! `domination-target-gets-its-breaker`: the Domination lane owns a wall
//! breaker matched to its target's walls before it declares, and keeps one
//! with every siege.
//!
//! **Why (live Deity, Online, Small Continents; 133 runs of 2026-11-09/10).**
//! Of 2,135 siege `stage` decisions 1,588 read "nothing to open the walls, so
//! the train holds outside the city's reach", and a Ram or Tower stood in the
//! breakers suffix of 52. All 1,588 came from Religion (1,012) and Culture
//! (576) seats, where every breaker reservation below is off by design, and
//! 1,294 of them faced Renaissance Walls or Urban Defenses (300-400), which
//! no Ram or Tower opens. The eight Deity Domination games of 2026-11-09 took
//! two cities in all; their logs are pruned. On the Domination lane the
//! shipped reservations still leave a war without a breaker:
//! - `reserve_delegated_domination_siege` reserves land guns only (a Ram or
//!   Tower has no ranged attack, and before Engineering there is no gun), and
//!   before the war only for a Conquest plan's target (`breaker_war_with`),
//!   while the lane plays Expansion until about turn 125.
//! - A Ram or Tower is valued only at war (`support_unit_value`,
//!   `advanced_support_production`, `BasicAi::siege_support_unit`), and the
//!   delegated governor (`lane-delegates-production-2`) never asks the
//!   scorer before then.
//! - `declaration-waits-for-the-breaker` holds the declaration until a breaker
//!   stands on the ring, but nothing builds one in peace and nothing walks a
//!   Ram there (`BasicAi::siege_support_step` moves it only at war), so the
//!   hold runs out its patience and the war opens without one.
//! - `breaker-bought-for-the-siege` buys only while the army within reach
//!   covers the city's bill; it bought twice in the 133 runs.
//!
//! **What the gene does**, for a seat on the Domination lane:
//! - **Target** ([`AdvancedAi::domination_breach_target`]): the plan's target
//!   city while a major holds it and the plan is Conquest or we are at war
//!   with that major; otherwise, from
//!   [`BREACH_TARGET_FROM_STANDARD`] standard turns (turn 35 Online), the
//!   nearest original capital of a met major on our capital's landmass within
//!   [`BREACH_TARGET_REACH`] tiles of a city of ours.
//! - **Breaker** ([`AdvancedAi::domination_breaker_fits`]): a Battering Ram
//!   or Siege Tower while one works on those walls (`city_allows_siege_support`:
//!   a Ram on Ancient Walls or none yet, a Tower on Medieval), else a land
//!   siege gun whose shot does more than `BREAKER_MATCH_MIN_HIT` to the city
//!   (Renaissance Walls, Urban Defenses).
//! - **Claim** ([`AdvancedAi::claim_domination_breaker`]): with no such
//!   breaker alive or queued, the safe city with an idle queue whose breaker
//!   reaches the target soonest lays it down, in the idle-queue claims after
//!   the Settler, Monument and Granary and ahead of `near-rival-deterrence`
//!   (which took every idle queue on the live boards it was replayed on), and
//!   again before the delegated governor so a wartime claim is restored.
//! - **Purchase** ([`AdvancedAi::buy_domination_breaker`]): with none alive,
//!   while the declaration is held for a breaker on that target or our land
//!   units stand within `MUSTER_BREACH_FAR` of it at war, and nothing queued
//!   reaches it within [`BREAKER_BUY_ARRIVAL`] standard turns, Gold above the
//!   purchase upkeep floor buys it in the city of ours nearest the target
//!   within [`BREAKER_BUY_REACH`] tiles; once per [`BREAKER_BUY_COOLDOWN`]
//!   standard turns per target.
//! - **March** ([`AdvancedAi::domination_breaker_step`]): before the war a
//!   Ram or Tower that works on the target walks to the melee unit of ours
//!   nearest the target within `MUSTER_BREACH_FAR` of it, so the
//!   declaration's breaker test finds it on the ring.
//!
//! Off, or on any other lane, nothing changes.

use super::*;
use crate::game::expected_damage;

/// Standard turns before a land capital the plan does not target yet is named
/// the breach target: turn 35 Online. Rival capitals read 100 walls or less
/// on 38 of 40 at turn 50 and 41 of 47 at turn 60 (Deity census).
pub(super) const BREACH_TARGET_FROM_STANDARD: u32 = 52;

/// The farthest an unplanned land capital may stand from a city of ours to be
/// named. The nearest same-land rival capital stands a median 15 tiles away
/// (IQR 13-17), all of them 18 (14-22).
pub(super) const BREACH_TARGET_REACH: i32 = 24;

/// A queued breaker that reaches the target later than this, in standard
/// turns (build plus march), is bought rather than waited for.
pub(super) const BREAKER_BUY_ARRIVAL: u32 = 8;

/// The farthest a buying city may stand from the target.
pub(super) const BREAKER_BUY_REACH: i32 = 12;

/// Standard turns before a second breaker is bought for the same target.
pub(super) const BREAKER_BUY_COOLDOWN: u32 = 8;

/// One breaker a purchase or a claim may lay down: whether it is a Ram or
/// Tower, then its arrival or distance, then the city and unit.
type BreakerChoice = (bool, f64, u32, Name);

impl AdvancedAi {
    /// Whether the gene acts for this seat: on, and the lane is Domination.
    pub(super) fn domination_breaker_active(&self, g: &Game) -> bool {
        self.domination_target_gets_its_breaker
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
    }

    /// The city the lane's breaker is for. See the module.
    pub(super) fn domination_breach_target(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<u32> {
        if !self.domination_breaker_active(g) {
            return None;
        }
        let major = |owner: usize| {
            owner != pid
                && g.players.get(owner).is_some_and(|player| {
                    player.alive
                        && !player.is_minor
                        && !player.is_barbarian
                        && !player.is_free_city
                        && !g.same_team(pid, owner)
                })
        };
        if let Some(city) = plan
            .target_city
            .and_then(|cid| g.cities.get(&cid))
            .filter(|city| {
                major(city.owner)
                    && (plan.strategy == GrandStrategy::Conquest || g.is_at_war(pid, city.owner))
            })
        {
            return Some(city.id);
        }
        if g.turn < g.standard_duration(BREACH_TARGET_FROM_STANDARD) {
            return None;
        }
        let ours: Vec<Pos> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .collect();
        let capitals: Vec<(i32, u32, Pos)> = g
            .cities
            .values()
            .filter(|city| {
                city.is_capital
                    && city.original_owner == city.owner
                    && major(city.owner)
                    && g.has_met(pid, city.owner)
            })
            .filter_map(|city| {
                let reach = ours.iter().map(|pos| g.wdist(*pos, city.pos)).min()?;
                (reach <= BREACH_TARGET_REACH).then_some((reach, city.id, city.pos))
            })
            .collect();
        if capitals.is_empty() {
            return None;
        }
        let landmass = BasicAi::capital_landmass(g, pid);
        capitals
            .into_iter()
            .filter(|(_, _, pos)| landmass.contains(pos))
            .min()
            .map(|(_, cid, _)| cid)
    }

    /// Whether a unit of `kind` opens `cid`'s walls. See the module.
    pub(super) fn domination_breaker_fits(g: &Game, kind: &str, cid: u32) -> bool {
        if matches!(kind, "battering_ram" | "siege_tower") {
            return g.city_allows_siege_support(cid, kind);
        }
        let Some(spec) = g.rules.units.get(kind) else {
            return false;
        };
        spec.class == "military"
            && spec.siege
            && spec.has_ranged_attack()
            && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
            && expected_damage(spec.ranged_attack_strength(), g.city_strength(cid))
                > super::siege_production::BREAKER_MATCH_MIN_HIT
    }

    /// Whether `cid` stands behind walls a breaker is wanted for: walls now,
    /// or an owner who knows Masonry and can raise them in a few turns.
    fn domination_breaker_wanted(g: &Game, cid: u32) -> bool {
        g.cities.get(&cid).is_some_and(|city| {
            g.city_max_wall_hp(city) > 0
                || city.wall_hp > 0
                || g.players[city.owner]
                    .techs
                    .contains(&crate::name!("masonry"))
        })
    }

    /// The kinds a breaker for `cid` may be, Ram and Tower first: a Ram or
    /// Tower while one works there and `pid` can field it, else the land guns
    /// that fit.
    fn domination_breaker_kinds(g: &Game, pid: usize, cid: u32) -> Vec<(bool, Name)> {
        let buildable = |kind: &Name| {
            g.player_city_ids(pid)
                .into_iter()
                .any(|own| g.can_produce(pid, own, &Item::Unit { unit: *kind }))
        };
        let support: Vec<(bool, Name)> = ["battering_ram", "siege_tower"]
            .into_iter()
            .map(Name::new)
            .filter(|kind| Self::domination_breaker_fits(g, kind.as_str(), cid))
            .filter(|kind| {
                buildable(kind)
                    || g.units
                        .values()
                        .any(|unit| unit.owner == pid && unit.kind == *kind)
            })
            .map(|kind| (true, kind))
            .collect();
        if !support.is_empty() {
            return support;
        }
        let mut guns: Vec<Name> = g
            .rules
            .units
            .keys()
            .copied()
            .filter(|kind| {
                !matches!(kind.as_str(), "battering_ram" | "siege_tower")
                    && Self::domination_breaker_fits(g, kind.as_str(), cid)
            })
            .collect();
        guns.sort();
        guns.into_iter().map(|kind| (false, kind)).collect()
    }

    /// Whether a breaker of `kinds` is alive, ours, and the soonest a queued
    /// one reaches `cid` (build plus march), if any is queued.
    fn domination_breaker_in_hand(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        kinds: &[(bool, Name)],
    ) -> (bool, Option<f64>) {
        let wanted = |kind: &Name| kinds.iter().any(|(_, k)| k == kind);
        let objective = g.cities[&cid].pos;
        let held = g
            .units
            .values()
            .any(|unit| unit.owner == pid && wanted(&unit.kind));
        let queued = g
            .player_city_ids(pid)
            .into_iter()
            .flat_map(|own| {
                g.cities[&own]
                    .queue
                    .iter()
                    .filter_map(move |item| match item {
                        Item::Unit { unit } if wanted(unit) => Some((own, item.clone())),
                        _ => None,
                    })
            })
            .map(|(own, item)| {
                let Item::Unit { unit } = &item else {
                    unreachable!()
                };
                self.production_build_turns(g, pid, own, &item)
                    + self.breaker_march_turns(
                        g.wdist(g.cities[&own].pos, objective),
                        g.rules.units[unit].moves,
                    )
            })
            .min_by(f64::total_cmp);
        (held, queued)
    }

    /// Lay the target's breaker down when none is alive or queued. Returns the
    /// claim for `restore_wall_breaker_queues`. See the module.
    pub(super) fn claim_domination_breaker(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<(u32, Item)> {
        let target = self.domination_breach_target(g, pid, plan)?;
        if !Self::domination_breaker_wanted(g, target) {
            return None;
        }
        let kinds = Self::domination_breaker_kinds(g, pid, target);
        if kinds.is_empty() {
            return None;
        }
        let (held, queued) = self.domination_breaker_in_hand(g, pid, target, &kinds);
        if held || queued.is_some() {
            return None;
        }
        let counts = self.counts(g, pid);
        if self.live_war_economy_requires_recovery(g, pid, &counts) {
            return None;
        }
        let objective = g.cities[&target].pos;
        let mut best: Option<BreakerChoice> = None;
        for own in g.player_city_ids(pid) {
            let city = &g.cities[&own];
            if plan.threatened_city == Some(own)
                || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            {
                continue;
            }
            // An idle queue only: the Builder and building claims own their
            // heads, and a breaker that took one would trade it back and forth.
            if !city.queue.is_empty() {
                continue;
            }
            for (support, kind) in &kinds {
                let item = Item::Unit { unit: *kind };
                if !g.can_produce(pid, own, &item) {
                    continue;
                }
                let arrival = self.production_build_turns(g, pid, own, &item)
                    + self.breaker_march_turns(
                        g.wdist(city.pos, objective),
                        g.rules.units[kind].moves,
                    );
                let choice = (*support, arrival, own, *kind);
                let better = best.as_ref().is_none_or(|old| {
                    (!choice.0, choice.1, choice.2, choice.3.as_str())
                        .partial_cmp(&(!old.0, old.1, old.2, old.3.as_str()))
                        .is_some_and(|order| order.is_lt())
                });
                if better {
                    best = Some(choice);
                }
            }
        }
        let (_, arrival, own, kind) = best?;
        // `hopeless-army-yields-the-queue`: no breaker for walls nothing we
        // can field will breach. See `advanced/hopeless_army_yields_the_queue.rs`.
        if self.hopeless_army_active(g) {
            if let Some(walls) = Self::walls_nothing_breaches(g, pid, target) {
                think!(self.journal(), Military, Decision,
                    "{} lays no {} down for the walls of {}", g.cities[&own].name,
                    crate::reasoning::plain(kind.as_str()), g.cities[&target].name;
                    "hopeless-army-yields-the-queue: {}", walls.describe();
                    objective);
                return None;
            }
        }
        let item = Item::Unit { unit: kind };
        if g.apply(
            pid,
            &Action::Produce {
                city: own,
                item: item.clone(),
            },
        )
        .is_err()
        {
            return None;
        }
        let city = &g.cities[&target];
        think!(self.journal(), Military, Decision,
            "{} reserves a {} for the walls of {}", g.cities[&own].name,
            crate::reasoning::plain(kind.as_str()), city.name;
            "domination-target-gets-its-breaker: walls {}/{}, {}, no breaker that opens them is alive or queued; it reaches them in about {arrival:.0} turns",
            city.wall_hp, g.city_max_wall_hp(city),
            if g.is_at_war(pid, city.owner) { "at war" } else { "before the war" };
            objective);
        Some((own, item))
    }

    /// Buy the target's breaker when the war or the declaration waits on it
    /// and nothing queued arrives soon. True when one was bought. See the
    /// module.
    pub(super) fn buy_domination_breaker(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        let Some(target) = self.domination_breach_target(g, pid, plan) else {
            return false;
        };
        let (objective, owner, walls) = {
            let city = &g.cities[&target];
            (city.pos, city.owner, city.wall_hp)
        };
        if walls <= 0 {
            return false;
        }
        let turn = g.turn;
        if self
            .domination_breaker_bought
            .get(&objective)
            .is_some_and(|bought| {
                turn.saturating_sub(*bought) < g.standard_duration(BREAKER_BUY_COOLDOWN)
            })
        {
            return false;
        }
        let kinds = Self::domination_breaker_kinds(g, pid, target);
        if kinds.is_empty() {
            return false;
        }
        let (held, queued) = self.domination_breaker_in_hand(g, pid, target, &kinds);
        if held
            || queued
                .is_some_and(|arrival| arrival <= g.standard_duration(BREAKER_BUY_ARRIVAL) as f64)
        {
            return false;
        }
        let near = |pred: &dyn Fn(&crate::rules::UnitSpec) -> bool| {
            g.units.values().any(|unit| {
                let spec = &g.rules.units[unit.kind];
                unit.owner == pid
                    && spec.class == "military"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && !g.is_embarked(unit)
                    && g.wdist(unit.pos, objective) <= super::siege_train::MUSTER_BREACH_FAR
                    && pred(spec)
            })
        };
        let declaration_held = self
            .declaration_breaker_hold
            .is_some_and(|(pos, _)| pos == objective)
            && turn.saturating_sub(self.declaration_hold_seen) <= 1;
        let at_war = g.is_at_war(pid, owner) && near(&|_| true);
        if !declaration_held && !at_war {
            return false;
        }
        let swingers = near(&|spec| crate::ai::siege_support::eligible_attacker(spec));
        let budget = g.players[pid].gold - self.purchase_upkeep_floor(g, pid, 0.0);
        if budget <= 0.0 {
            return false;
        }
        let mut best: Option<BreakerChoice> = None;
        for own in g.player_city_ids(pid) {
            let distance = g.wdist(g.cities[&own].pos, objective);
            if distance > BREAKER_BUY_REACH {
                continue;
            }
            for (support, kind) in &kinds {
                if *support && !swingers {
                    continue;
                }
                let item = Item::Unit { unit: *kind };
                if !g.can_produce(pid, own, &item) || g.purchase_is_blocked(own, &item) {
                    continue;
                }
                let Some(price) = g.unit_purchase_cost(pid, own, kind.as_str(), "gold") else {
                    continue;
                };
                if price > budget + 1e-9 {
                    continue;
                }
                let choice = (*support, f64::from(distance), own, *kind);
                let better = best.as_ref().is_none_or(|old| {
                    (!choice.0, choice.1, choice.2, choice.3.as_str())
                        .partial_cmp(&(!old.0, old.1, old.2, old.3.as_str()))
                        .is_some_and(|order| order.is_lt())
                });
                if better {
                    best = Some(choice);
                }
            }
        }
        let Some((_, distance, own, kind)) = best else {
            return false;
        };
        let before = g.players[pid].gold;
        if g.apply(
            pid,
            &Action::Buy {
                city: own,
                unit: kind,
                formation: 0,
                currency: "gold".into(),
            },
        )
        .is_err()
        {
            return false;
        }
        self.domination_breaker_bought.insert(objective, turn);
        let city = &g.cities[&target];
        think!(self.journal(), Military, Decision,
            "{} buys a {} for the walls of {}", g.cities[&own].name,
            crate::reasoning::plain(kind.as_str()), city.name;
            "domination-target-gets-its-breaker: walls {}/{}, {}, no breaker alive and none queued within {} turns; {distance:.0} tiles out, {:.0} Gold",
            city.wall_hp, g.city_max_wall_hp(city),
            if declaration_held { "the declaration waits on it" } else { "the army stands at war before them" },
            g.standard_duration(BREAKER_BUY_ARRIVAL),
            (before - g.players[pid].gold).max(0.0);
            objective);
        true
    }

    /// Before the war, walk a Ram or Tower that opens the target's walls to
    /// the melee unit of ours nearest the target within `MUSTER_BREACH_FAR`
    /// of it. `None` leaves the unit to the ordinary movers: the gene off,
    /// another lane, at war with the target's owner, no such melee unit, or
    /// the breaker already beside it.
    pub(super) fn domination_breaker_step(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
    ) -> Option<bool> {
        if !self.domination_target_gets_its_breaker {
            return None;
        }
        let unit = g.units.get(&uid)?;
        if unit.owner != pid
            || !matches!(unit.kind.as_str(), "battering_ram" | "siege_tower")
            || unit.linked_to.is_some()
            || g.is_embarked(unit)
        {
            return None;
        }
        let (here, kind) = (unit.pos, unit.kind);
        let target = self.domination_breach_target(g, pid, plan)?;
        let city = &g.cities[&target];
        if g.is_at_war(pid, city.owner) || !Self::domination_breaker_fits(g, kind.as_str(), target)
        {
            return None;
        }
        let objective = city.pos;
        let escort = g
            .units
            .values()
            .filter(|other| {
                other.owner == pid
                    && other.id != uid
                    && !g.is_embarked(other)
                    && crate::ai::siege_support::eligible_attacker(&g.rules.units[other.kind])
                    && g.wdist(other.pos, objective) <= super::siege_train::MUSTER_BREACH_FAR
            })
            .min_by_key(|other| {
                (
                    g.wdist(other.pos, objective),
                    g.wdist(here, other.pos),
                    other.id,
                )
            })
            .map(|other| other.pos)?;
        if g.wdist(here, escort) == 0 {
            return None;
        }
        let name = city.name.clone();
        let acted = self.base.step_toward(g, pid, uid, escort);
        if acted && self.journal().wants(crate::reasoning::Level::Detail) {
            think!(self.journal(), Military, Detail,
                "The {} walks to the army before {}", crate::reasoning::plain(kind.as_str()), name;
                "domination-target-gets-its-breaker: the war is not open yet and its melee stands within {} tiles of the walls",
                super::siege_train::MUSTER_BREACH_FAR;
                objective);
        }
        Some(acted)
    }
}

#[cfg(test)]
mod tests;
