//! `endgame-army-takes-the-queue` (opt-in): once a Domination seat's clock is
//! short, an idle city trains the strongest land unit it can, and a fresh
//! economy build whose return lands after the clock gives its queue to one,
//! as far as the income carries the upkeep. The treasury keeps the army's
//! upkeep before discretionary purchases.
//!
//! **Why (live Deity Gran Colombia, 4p Tiny Pangaea, Online, 2026-11-12).**
//! Every game of the lane that ran past t150 ended on a rival Science victory
//! at t184-210 (`civvis-20261112T004621Z` t184, `T011556Z` t195, `T081824Z`
//! t197, `T075931Z` t210). `T075931Z` reached technology parity (77 against
//! 77 at t210) and out-produced the field (852 at t210), yet ended holding
//! 1,212 military power against Byzantium's 3,261 (579 against 2,614 at
//! t200), with 9-16 land units against the rivals' 27-35.
//! - City production by class, read from the state rows (each city's
//!   per-turn Production while it built an item): t160-179 31% military, 54%
//!   districts, buildings and projects; t180-199 50% and 42%; t200-210 36% and
//!   56%. About 14,300 points went into Neighborhoods, Research Labs, a
//!   Spaceport, Stadium, Zoo, Shopping Mall, Workshop, Factory, district
//!   projects and reactor conversions after t160, about forty Modern Armor at
//!   the 340 they cost, and the building and district bill rose 120 to 180
//!   Gold a turn over the same turns.
//! - The cause is the governor that fills the queues. Under
//!   `lane-delegates-production-2` the Domination seat's cities are filled by
//!   the delegated baseline governor, whose standing army is `mil_per_city`
//!   times the cities; `declaration-needs-production-parity` caps what a war
//!   lends it at 1.0 a city while no home city is threatened. Every one of the
//!   27 production starts journalled from t180 on reads "16 military for 12
//!   cities against a target of 1.0 each", and the floor sits behind fifteen
//!   economy steps of that governor and a chain of idle-queue claims.
//!   `phase_specialization_active` (and `domination-specializes-earlier`)
//!   steers only the strategic scorer, which a delegating lane runs in
//!   Recovery or an appointed war alone.
//! - The rivals' clock was public: Australia launched the Earth Satellite at
//!   t150, Byzantium at t169 (41 turns before its win), the Moon Landing at
//!   t179, the Mars Base at t198 and the Exoplanet Expedition at t204.
//! - Gold: income stayed between -28 and +70 a turn from t150 to t191 with
//!   100-380 banked. At t192 a free policy swap dropped Levee en Masse and
//!   Economic Union (unit bill 108 -> 154) while 400 Gold left the treasury
//!   the same turn; income fell to -48..-111 and the treasury sat at zero
//!   from t193 to t199, when the host disbanded units the war had paid for
//!   (military 1,179 at t190, 579 at t200).
//!
//! **The window.** A Domination seat (the active target) in a game with a
//! turn limit, once either the game clock passes [`ENDGAME_CLOCK_PERCENT`] of
//! its length while we hold no rival's original capital, or a living rival's
//! projected victory is within [`ENDGAME_RIVAL_FINISH_TURNS`]: its latest
//! space launch read at the median turns each launch has left
//! (`SCIENCE_TURNS_AFTER_*`), or its culture finish as the denial reads it
//! (`projected_culture_finish`). The clock's turns left are the nearer of
//! the turn limit and that finish.
//!
//! **The claim** (`claim_endgame_army_queues`, in the idle-queue chain after
//! the breaker, deterrence and idle-route Trader claims and ahead of every
//! growth and economy claim). While the land army, counted as the delegated
//! governor counts it (each unit the fraction of a current front-line unit
//! it fields), is under the enemy-weighted target of
//! [`ENDGAME_ARMY_PER_CITY`] a city, each city, fastest producer first:
//! - an idle queue trains `combined_arms_unit`'s pick (the delegated floor's
//!   own chooser);
//! - a queued wonder, district, building or district project whose return
//!   lands after the clock ([`AdvancedAi::endgame_long_payback`]) and under
//!   [`ENDGAME_PREEMPT_PAID_SHARE`] paid is set aside for that unit (the host
//!   banks its production);
//! - only while the unit's upkeep fits the room: income, less the upkeep the
//!   other queue heads add on completion (`committed_queue_upkeep`), plus the
//!   treasury above the endgame reserve spread over the turns left;
//! - and only where the unit is built within [`ENDGAME_ARRIVAL_SHARE`] of
//!   the turns left, so it reaches the fight.
//! A threatened city (the plan's, or one hit within four turns) and a city
//! with a pillaged district to repair keep their queue for the governor's
//! defence and repair steps. Walls, encampment and aerodrome buildings and
//! districts, income buildings (net Gold above zero), units, repairs and the
//! nuclear and space projects are never set aside; neither is a Spaceport
//! while the seat has joined the space race.
//!
//! **The treasury** (`endgame_purchase_reserve`). Inside the window the
//! delegated governor's Gold pass and `advanced_gold_spending` leave
//! [`ENDGAME_UPKEEP_RESERVE_TURNS`] turns of the unit bill in the treasury,
//! twice the 1.5 turns `purchases-keep-the-upkeep-reserve` keeps. The
//! upgrade passes, which run before both, keep their own floor, so the Gold
//! goes to upgrades and to the army's upkeep before buildings and Builders.
//!
//! The solvency genes still apply: the room is the same income-less-queues
//! test `upkeep-waits-for-the-income` makes, so the claim never trains a unit
//! that gene would hold, and an insolvent treasury claims nothing.
//!
//! With the gene off, or for any other target, nothing changes.

use super::*;

/// Percent of the game's length (the turn limit, or the speed's own if
/// shorter) past which a Domination seat holding no rival's original capital
/// is in its endgame: live t150 of 250. Every Gran Colombia Deity game past
/// t150 ended on a rival Science victory at t184-210.
pub(crate) const ENDGAME_CLOCK_PERCENT: u32 = 60;

/// Turns to a rival's projected victory inside which the endgame opens: a
/// rival's Earth Satellite reads 50 (`SCIENCE_TURNS_AFTER_SATELLITE`).
/// Byzantium won 41 turns after its satellite in `civvis-20261112T075931Z`.
pub(crate) const ENDGAME_RIVAL_FINISH_TURNS: f64 = 50.0;

/// Land bodies a city the endgame army wants before the enemy weighting
/// (`enemy_weighted_army_target`, up to twice that while a rival out-guns
/// us): the target `advanced_production` asks of Conquest and Recovery.
pub(crate) const ENDGAME_ARMY_PER_CITY: usize = 2;

/// Share of the clock's turns left within which a claimed unit must finish:
/// a unit that leaves its city with half the clock left still fights.
pub(crate) const ENDGAME_ARRIVAL_SHARE: f64 = 0.5;

/// A queued long-payback item with at least this share of its cost paid
/// finishes; under it, it is set aside for a unit.
pub(crate) const ENDGAME_PREEMPT_PAID_SHARE: f64 = 0.25;

/// Turns of the unit bill the endgame treasury keeps from discretionary
/// purchases. At t192 of `civvis-20261112T075931Z` the bill was 108-154 Gold
/// a turn and income fell to -48..-111 the next turn.
pub(crate) const ENDGAME_UPKEEP_RESERVE_TURNS: f64 = 3.0;

/// Turns of the threat memory a city keeps its own queue for: the four the
/// production scorer's `threatened` reads.
const ENDGAME_ATTACK_MEMORY: u32 = 4;

/// The endgame's clock, as [`AdvancedAi::endgame_army_clock`] reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EndgameClock {
    /// Turns until the nearer of the turn limit and the nearest rival's
    /// projected victory.
    pub(crate) turns_left: f64,
    /// The rival whose projected victory is nearest, and its turns, when
    /// one is projected.
    pub(crate) rival: Option<(usize, f64)>,
    /// Whether the game clock alone (past [`ENDGAME_CLOCK_PERCENT`], no
    /// rival capital held) opened the window.
    pub(crate) late_clock: bool,
}

impl EndgameClock {
    fn describe(&self, g: &Game) -> String {
        let rival = self.rival.map(|(rival, turns)| {
            format!(
                "{}'s victory projected in {turns:.0} turns",
                g.players[rival].civ
            )
        });
        match (self.late_clock, rival) {
            (true, Some(rival)) => format!(
                "turn {} of {} with no rival capital held, {rival}; {:.0} turns left",
                g.turn,
                Self::length(g),
                self.turns_left
            ),
            (true, None) => format!(
                "turn {} of {} with no rival capital held; {:.0} turns left",
                g.turn,
                Self::length(g),
                self.turns_left
            ),
            (false, Some(rival)) => format!("{rival}; {:.0} turns left", self.turns_left),
            (false, None) => format!("{:.0} turns left", self.turns_left),
        }
    }

    fn length(g: &Game) -> u32 {
        g.max_turns.min(g.game_speed.turn_limit())
    }
}

/// The land army as the delegated governor counts it: each military land
/// unit, fielded or at the head of a queue, the fraction of a current
/// front-line unit it fields (`BasicAi::force_weight`). Rams and Towers, sea
/// and air units are not counted.
#[derive(Clone, Copy, Debug)]
pub(crate) struct EndgameArmy {
    pub(crate) force: f64,
    pub(crate) melee: usize,
    pub(crate) ranged: usize,
    front_line: (f64, f64),
}

impl EndgameArmy {
    pub(crate) fn of(g: &Game, pid: usize, city_ids: &[u32]) -> Self {
        let mut army = Self {
            force: 0.0,
            melee: 0,
            ranged: 0,
            front_line: BasicAi::front_line_strength(g, pid, city_ids),
        };
        for uid in g.player_unit_ids(pid) {
            army.count(g, g.units[&uid].kind);
        }
        for cid in city_ids {
            if let Some(Item::Unit { unit }) = g.cities[cid].queue.first() {
                army.count(g, *unit);
            }
        }
        army
    }

    fn count(&mut self, g: &Game, kind: Name) {
        if matches!(kind.as_str(), "battering_ram" | "siege_tower") {
            return;
        }
        let Some(spec) = g.rules.units.get(&kind) else {
            return;
        };
        if spec.class != "military" || matches!(spec.domain.as_deref(), Some("sea" | "air")) {
            return;
        }
        self.force += BasicAi::force_weight(g, kind.as_str(), self.front_line);
        if spec.is_melee_capable() {
            self.melee += 1;
        }
        if spec.has_ranged_attack() {
            self.ranged += 1;
        }
    }
}

impl AdvancedAi {
    /// `endgame-army-takes-the-queue`: the endgame's clock, when the window is
    /// open for this seat. `None` with the gene off, for a seat not aiming at
    /// Domination, in a game with no turn limit, and before the window.
    pub(crate) fn endgame_army_clock(&self, g: &Game, pid: usize) -> Option<EndgameClock> {
        if !self.endgame_army_takes_the_queue
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || g.max_turns == 0
        {
            return None;
        }
        let length = EndgameClock::length(g);
        let game_left = f64::from(length.saturating_sub(g.turn));
        let late_clock = g.turn.saturating_mul(100) >= length.saturating_mul(ENDGAME_CLOCK_PERCENT)
            && !Self::holds_a_rival_capital(g, pid);
        let rival = self.endgame_rival_finish(g, pid);
        let rival_near = rival.is_some_and(|(_, turns)| turns <= ENDGAME_RIVAL_FINISH_TURNS);
        if !late_clock && !rival_near {
            return None;
        }
        let turns_left = rival
            .map_or(game_left, |(_, turns)| turns.min(game_left))
            .max(1.0);
        Some(EndgameClock {
            turns_left,
            rival,
            late_clock,
        })
    }

    /// Turns to the nearest living rival major's projected victory, and that
    /// rival: its latest space launch at the median turns each launch has
    /// left, or its culture finish as the denial reads it.
    fn endgame_rival_finish(&self, g: &Game, pid: usize) -> Option<(usize, f64)> {
        g.players
            .iter()
            .filter(|rival| {
                rival.id != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && !g.same_team(pid, rival.id)
            })
            .filter_map(|rival| {
                [
                    Self::endgame_science_turns(g, rival.id),
                    self.projected_culture_finish(g, rival.id),
                ]
                .into_iter()
                .flatten()
                .filter(|turns| turns.is_finite())
                .reduce(f64::min)
                .map(|turns| (rival.id, turns.max(0.0)))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
    }

    /// Turns `rival`'s Science victory has left after its latest space
    /// launch, at the medians `science-ladder-reads-the-clock` reads. `None`
    /// before its Earth Satellite.
    pub(crate) fn endgame_science_turns(g: &Game, rival: usize) -> Option<f64> {
        let projects = &g.players[rival].science_projects;
        if projects.contains("exoplanet_expedition") {
            Some(SCIENCE_TURNS_AFTER_EXOPLANET)
        } else if projects.contains("launch_mars_colony") {
            Some(SCIENCE_TURNS_AFTER_MARS)
        } else if projects.contains("launch_moon_landing") {
            Some(SCIENCE_TURNS_AFTER_MOON)
        } else if projects.contains("launch_earth_satellite") {
            Some(SCIENCE_TURNS_AFTER_SATELLITE)
        } else {
            None
        }
    }

    /// The land army the endgame trains toward: [`ENDGAME_ARMY_PER_CITY`] a
    /// city, weighted by the strongest rival's power over ours.
    pub(crate) fn endgame_army_target(&self, g: &Game, pid: usize, cities: usize) -> usize {
        self.enemy_weighted_army_target(g, pid, ENDGAME_ARMY_PER_CITY * cities.max(1))
    }

    /// The treasury the endgame keeps from discretionary purchases:
    /// [`ENDGAME_UPKEEP_RESERVE_TURNS`] turns of the unit bill.
    fn endgame_reserve(g: &Game, pid: usize) -> f64 {
        ENDGAME_UPKEEP_RESERVE_TURNS * g.unit_gold_maintenance(pid).max(0.0)
    }

    /// The Gold a turn of new unit upkeep the endgame can carry: income, less
    /// the upkeep every queue head adds on completion, plus the treasury
    /// above the endgame reserve spread over the clock's turns left.
    pub(crate) fn endgame_upkeep_room(&self, g: &Game, pid: usize, clock: &EndgameClock) -> f64 {
        let player = &g.players[pid];
        let runway =
            (player.gold - Self::endgame_reserve(g, pid)).max(0.0) / clock.turns_left.max(1.0);
        player.gold_per_turn - Self::committed_queue_upkeep(g, pid, None) + runway
    }

    /// `endgame-army-takes-the-queue`: the treasury a discretionary purchase
    /// leaves inside the window, never below [`ENDGAME_UPKEEP_RESERVE_TURNS`]
    /// turns of the unit bill. `stock` comes back unchanged with the gene
    /// off and outside the window.
    pub(crate) fn endgame_purchase_reserve(&self, g: &Game, pid: usize, stock: f64) -> f64 {
        let Some(clock) = self.endgame_army_clock(g, pid) else {
            return stock;
        };
        let reserve = Self::endgame_reserve(g, pid);
        if reserve <= stock {
            return stock;
        }
        think!(self.journal(), Economy, Detail,
            "The endgame treasury keeps the army's upkeep";
            "endgame-army-takes-the-queue: purchases leave {reserve:.0} Gold, {:.0} turns of a \
             {:.0} Gold unit bill, over a reserve of {stock:.0}; {}",
            ENDGAME_UPKEEP_RESERVE_TURNS, g.unit_gold_maintenance(pid), clock.describe(g));
        reserve
    }

    /// Whether `item` in `cid` is an economy build whose return lands after
    /// the endgame's clock: a wonder, a district other than an Encampment or
    /// Aerodrome (or a Spaceport while the seat races), a building other than
    /// Walls, an Encampment or Aerodrome building or one that nets Gold, or a
    /// district project, power conversion or Carbon Recapture. Units,
    /// repairs, products, the nuclear and space projects and the Gold
    /// projects are never one.
    pub(crate) fn endgame_long_payback(&self, g: &Game, item: &Item) -> bool {
        let military_family = |family: &str| matches!(family, "encampment" | "aerodrome");
        match item {
            Item::Wonder { .. } => true,
            Item::District { district, .. } => {
                let family = g.district_family(*district);
                if family.as_str() == "spaceport" {
                    return self.space_race_excluded();
                }
                !military_family(family.as_str())
            }
            Item::Building { building } => {
                let Some(spec) = g.rules.buildings.get(building) else {
                    return false;
                };
                if spec.wonder {
                    return true;
                }
                let military = spec
                    .district
                    .is_some_and(|district| military_family(g.district_family(district).as_str()));
                spec.outer_defense <= 0 && !military && spec.yields.gold - spec.maintenance <= 0.0
            }
            Item::Project { project } => matches!(
                project.as_str(),
                "campus_research_grants"
                    | "holy_site_prayers"
                    | "industrial_zone_logistics"
                    | "theater_square_festival"
                    | "bread_and_circuses"
                    | "train_athletes"
                    | "convert_reactor_to_coal"
                    | "convert_reactor_to_oil"
                    | "convert_reactor_to_uranium"
                    | "recommission_reactor"
                    | "carbon_recapture"
            ),
            _ => false,
        }
    }

    /// Whether `cid` keeps its queue for the governor: the plan's threatened
    /// city, one hit within [`ENDGAME_ATTACK_MEMORY`] turns, or one with a
    /// pillaged district or building to repair.
    fn endgame_city_keeps_its_queue(g: &Game, pid: usize, cid: u32, plan: &StrategicPlan) -> bool {
        let Some(city) = g.cities.get(&cid) else {
            return true;
        };
        plan.threatened_city == Some(cid)
            || (city.last_attacked > 0
                && g.turn.saturating_sub(city.last_attacked) <= ENDGAME_ATTACK_MEMORY)
            || g.producible_items(pid, cid)
                .iter()
                .any(|item| matches!(item, Item::Repair { .. }))
    }

    /// `endgame-army-takes-the-queue`: inside the window, while the land army
    /// is under its endgame target, each city (fastest producer first) that
    /// is idle or holds a fresh long-payback start trains the delegated
    /// floor's unit, as far as the upkeep room carries it and where the unit
    /// finishes within [`ENDGAME_ARRIVAL_SHARE`] of the clock. Exact no-op
    /// with the gene off and outside the window.
    pub(crate) fn claim_endgame_army_queues(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        let Some(clock) = self.endgame_army_clock(g, pid) else {
            return;
        };
        let mut city_ids = g.player_city_ids(pid);
        if city_ids.is_empty() {
            return;
        }
        let target = self.endgame_army_target(g, pid, city_ids.len()) as f64;
        let mut army = EndgameArmy::of(g, pid, &city_ids);
        if army.force >= target {
            return;
        }
        let mut room = self.endgame_upkeep_room(g, pid, &clock);
        city_ids.sort_by(|a, b| {
            g.city_yields(*b)
                .production
                .total_cmp(&g.city_yields(*a).production)
                .then(a.cmp(b))
        });
        for cid in city_ids {
            if army.force >= target {
                break;
            }
            let head = g.cities[&cid].queue.first().cloned();
            let mut freed = 0.0;
            if let Some(item) = head.as_ref() {
                if !self.endgame_long_payback(g, item) {
                    continue;
                }
                let cost = g.item_cost_for_city(pid, cid, item).max(1.0);
                if g.item_invested_production(cid, item) >= ENDGAME_PREEMPT_PAID_SHARE * cost {
                    continue;
                }
                freed = Self::item_added_upkeep(g, item);
            }
            if Self::endgame_city_keeps_its_queue(g, pid, cid, plan) {
                continue;
            }
            let Some(unit) = self
                .base
                .combined_arms_unit(g, pid, cid, army.melee, army.ranged)
            else {
                continue;
            };
            let unit = Name::new(&unit);
            let item = Item::Unit { unit };
            let upkeep = Self::item_added_upkeep(g, &item);
            if room + freed - upkeep < 0.0 {
                think!(self.journal(), Economy, Detail,
                    "The endgame army waits for the income";
                    "endgame-army-takes-the-queue: a {} adds {upkeep:.0} Gold a turn against {:.1} \
                     of room ({:.1} income, {:.0} committed in queues, {:.0} banked over a {:.0} \
                     reserve); land army {:.1} of {target:.0}; {}",
                    Self::plain_item(&item), room + freed, g.players[pid].gold_per_turn,
                    Self::committed_queue_upkeep(g, pid, None), g.players[pid].gold,
                    Self::endgame_reserve(g, pid), army.force, clock.describe(g));
                break;
            }
            let turns = self.production_build_turns(g, pid, cid, &item);
            if turns > ENDGAME_ARRIVAL_SHARE * clock.turns_left {
                continue;
            }
            if g.apply(
                pid,
                &Action::Produce {
                    city: cid,
                    item: item.clone(),
                },
            )
            .is_err()
            {
                continue;
            }
            room += freed - upkeep;
            army.count(g, unit);
            let city = &g.cities[&cid];
            match head.as_ref() {
                Some(old) => {
                    think!(self.journal(), Cities, Decision,
                        "{} sets aside {} for a {}", city.name, Self::plain_item(old),
                        Self::plain_item(&item);
                        "endgame-army-takes-the-queue: its return lands after the clock ({:.0} of \
                         {:.0} paid, banked by the host); {}; land army {:.1} of {target:.0}, \
                         {room:.1} Gold a turn of upkeep room left; {turns:.0} turns to build",
                        g.item_invested_production(cid, old), g.item_cost_for_city(pid, cid, old),
                        clock.describe(g), army.force;
                        city.pos);
                }
                None => {
                    think!(self.journal(), Cities, Decision,
                        "{} trains a {} for the endgame army", city.name, Self::plain_item(&item);
                        "endgame-army-takes-the-queue: {}; land army {:.1} of {target:.0}, \
                         {room:.1} Gold a turn of upkeep room left; {turns:.0} turns to build",
                        clock.describe(g), army.force;
                        city.pos);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
