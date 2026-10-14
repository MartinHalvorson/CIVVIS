use super::*;
use crate::game::expected_damage;

/// The first wall breaker equips an otherwise incomplete campaign. Use the
/// opening conquest reservation scale; ordinary production-time pricing still
/// prefers a faster city and existing/queued weapons close this reservation.
pub(super) const FIRST_WEAPON_RESERVATION: f64 = 400.0;

/// The siege guns a walled assault that cannot breach in time may reserve in
/// total (`siege-positive-damage-budget`).
pub(super) const SHORTFALL_SIEGE_CAP: usize = 3;

/// `siege-train-scales-with-walls`: a target whose walls reach this many hit
/// points at full strength (Renaissance Walls, or Urban Defenses' 400) may
/// reserve up to [`HEAVY_WALL_SIEGE_CAP`] guns instead. Live King
/// civvis-20261003T072557Z stood in Stage before Mistahi-Sipihk's 400 walls
/// from turn 108 to 154 with two Trebuchets and a Bombard, needing 7.9 turns
/// to breach against 5.2 turns of endurance. Its reservation had stopped at
/// three guns.
pub(super) const HEAVY_WALL_HP: i32 = 300;
pub(super) const HEAVY_WALL_SIEGE_CAP: usize = 5;

/// `breaker-supply-scales`: a target whose full walls reach this many hit
/// points (Medieval Walls or better) is supplied with guns in parallel.
pub(super) const SUPPLY_WALL_HP: i32 = 200;
/// The guns such a target is supplied with: one per hundred wall points,
/// at most this many.
pub(super) const SUPPLY_GUNS_MAX: usize = 3;
/// A city may set aside a building it has invested in for the gun only
/// when the gun reaches the front within this many turns.
pub(super) const SUPPLY_DISPLACE_ARRIVAL: f64 = 10.0;
/// `breaker-supply-scales-2`: a stronger gun is preferred only when it
/// arrives within this many turns of the soonest.
pub(super) const SUPPLY_STRENGTH_WINDOW: f64 = 3.0;

/// See `AdvancedAi::breaker_to_the_fastest`: how much sooner, in turns, and
/// by what ratio a busy city must put the gun at the walls to take it from
/// the best idle one. Live King 2026-10-05T024614Z t83: idle Cumana's gun
/// would reach Lisbon in about 20 turns, Bogota's in 11, and the eight-turn,
/// half-again bar sent it to Cumana.
const BREAKER_FASTEST_MARGIN: f64 = 3.0;
const BREAKER_FASTEST_RATIO: f64 = 1.2;

/// `breaker-reads-the-march`: the share of its movement a new siege gun
/// actually closes on the siege it was built for each turn. The reservation
/// priced the road at the gun's full movement; on October 5 the 563 guns
/// born more than 5 tiles from a running siege closed a median 0.80 tiles a
/// turn (two-move guns: 0.4 of their movement), only 233 reached 5 tiles
/// within 20 turns, and they were born a median 13 tiles out against 7 for
/// our nearest city. Read at full movement, a fast city's longer road looked
/// cheaper than it was.
pub(super) const BREAKER_MARCH_FACTOR: f64 = 0.4;

/// `breakers-match-the-walls`: the most siege guns the campaign target's
/// Siege row asks, and the delegated reservation may order, for its walls.
pub(super) const BREAKER_MATCH_MAX: usize = 6;
/// `breakers-match-the-walls`: a gun whose shot does no more than this to the
/// city is not bought for it, however many: live King game 141 ground Munich's
/// walls with guns that could never breach it.
pub(super) const BREAKER_MATCH_MIN_HIT: f64 = 4.0;
/// `breakers-match-the-walls`: the turns the guns are sized to breach and take
/// the city in, 0.8 of the train's endurance as the damage budget reads it,
/// kept within these bounds; [`BREAKER_MATCH_DEFAULT_TURNS`] with no train
/// within the muster's reach.
pub(super) const BREAKER_MATCH_MIN_TURNS: f64 = 6.0;
pub(super) const BREAKER_MATCH_MAX_TURNS: f64 = 15.0;
pub(super) const BREAKER_MATCH_DEFAULT_TURNS: f64 = 12.0;
/// The health a city heals a turn while its ring is open, as the damage
/// budget prices it.
const BREAKER_MATCH_CITY_HEAL: f64 = 20.0;

/// `breakers-match-the-walls`: the fewest guns, at most [`BREAKER_MATCH_MAX`],
/// whose `hit` a shot breaches `walls` and takes `health` within
/// `target_turns` while the city heals [`BREAKER_MATCH_CITY_HEAL`] a turn,
/// with the turns they breach and take it in; `None` when even that many
/// cannot, or the blow is under [`BREAKER_MATCH_MIN_HIT`].
/// `siege-buys-the-gun-resource` sizes the gun it buys the resource for by
/// the same count, at that gun's blow.
pub(super) fn matched_guns(
    hit: f64,
    walls: f64,
    health: f64,
    target_turns: f64,
) -> Option<(usize, f64, f64)> {
    if hit <= BREAKER_MATCH_MIN_HIT {
        return None;
    }
    (1..=BREAKER_MATCH_MAX).find_map(|guns| {
        let fire = guns as f64 * hit;
        if fire <= BREAKER_MATCH_CITY_HEAL {
            return None;
        }
        let breach = walls / fire;
        let take = breach + health / (fire - BREAKER_MATCH_CITY_HEAL) + 1.0;
        (take <= target_turns).then_some((guns, breach, take))
    })
}

/// `breakers-match-the-walls`: how many guns the walls of one city ask at our
/// best gun's blow, and why.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BreakerMatch {
    /// Our best land gun's expected blow against the city, walls or health.
    pub(super) hit: f64,
    /// The city's defensive strength the blow is read against.
    pub(super) defense: f64,
    /// The turns the guns are sized to finish in.
    pub(super) target_turns: f64,
    /// The fewest guns that finish within `target_turns`, with the turns
    /// they breach the walls in and take the city in; `None` when even
    /// [`BREAKER_MATCH_MAX`] cannot, or the blow is under
    /// [`BREAKER_MATCH_MIN_HIT`].
    pub(super) ask: Option<(usize, f64, f64)>,
}

impl AdvancedAi {
    /// Turns a siege gun of `moves` movement takes to cover `distance` tiles
    /// to the siege: at full movement, or under `breaker-reads-the-march` at
    /// [`BREAKER_MARCH_FACTOR`] of it.
    pub(super) fn breaker_march_turns(&self, distance: i32, moves: f64) -> f64 {
        let factor = if self.breaker_reads_the_march {
            BREAKER_MARCH_FACTOR
        } else {
            1.0
        };
        f64::from(distance) / (moves.max(1.0) * factor)
    }

    /// `breakers-match-the-walls`: the guns `cid`'s standing walls and health
    /// ask at our best land gun's blow, read the way the siege damage budget
    /// reads it (`victory_conversion::conversion_siege_budget_within`): each
    /// gun strikes the walls and then the city at full strength, the city
    /// heals [`BREAKER_MATCH_CITY_HEAL`] a turn, and the siege must finish
    /// within 0.8 of the endurance of our land units within
    /// `siege_train::MUSTER_BREACH_FAR`. `None` with the gene off, a city of
    /// ours, no wall standing, or no land gun built or buildable.
    ///
    /// The shipped count is a hundred wall points a gun, at most three
    /// (`objective_board::breaker_guns_wanted`), whatever the gun: of the
    /// 6,060 hopeless siege decisions of October 6-7 (infinite or 20 turns
    /// and more), 69% stood in Stage with no fit gun though guns of ours were
    /// alive in 87% of them, and -d8 read ~70% of the infinite budgets as the
    /// gun tier and count. Live Emperor civvis-20261007T101555Z (game 338)
    /// stood before Ray's 300 walls from turn 165 with three Trebuchets,
    /// 633 strength against a bill of 238, its budget 75 turns then
    /// infinite.
    pub(super) fn breakers_matched_to_walls(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
    ) -> Option<BreakerMatch> {
        if !self.breakers_match_the_walls {
            return None;
        }
        let city = g.cities.get(&cid)?;
        if city.owner == pid || city.wall_hp <= 0 {
            return None;
        }
        let land_gun = |spec: &crate::rules::UnitSpec| {
            spec.class == "military"
                && spec.siege
                && spec.has_ranged_attack()
                && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
        };
        let buildable = g
            .player_city_ids(pid)
            .into_iter()
            .flat_map(|ours| g.producible_items(pid, ours))
            .filter_map(|item| match item {
                Item::Unit { unit } => {
                    let spec = &g.rules.units[&unit];
                    land_gun(spec).then(|| spec.ranged_attack_strength())
                }
                _ => None,
            });
        let fielded = g
            .units
            .values()
            .filter(|unit| unit.owner == pid && land_gun(&g.rules.units[unit.kind]))
            .map(|unit| g.rules.units[unit.kind].ranged_attack_strength());
        let attack = buildable.chain(fielded).reduce(f64::max)?;
        let defense = g.city_strength(cid);
        let hit = expected_damage(attack, defense);
        let train: Vec<u32> = g
            .units
            .values()
            .filter(|unit| {
                let spec = &g.rules.units[unit.kind];
                unit.owner == pid
                    && spec.class == "military"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && !g.is_embarked(unit)
                    && g.wdist(unit.pos, city.pos) <= super::siege_train::MUSTER_BREACH_FAR
            })
            .map(|unit| unit.id)
            .collect();
        let target_turns = self
            .conversion_siege_budget_within(
                g,
                pid,
                cid,
                &train,
                super::siege_train::MUSTER_BREACH_FAR,
            )
            .map(|(_, endurance)| endurance * 0.8)
            .filter(|turns| *turns > 0.0)
            .map_or(BREAKER_MATCH_DEFAULT_TURNS, |turns| {
                turns.clamp(BREAKER_MATCH_MIN_TURNS, BREAKER_MATCH_MAX_TURNS)
            });
        let walls = f64::from(city.wall_hp);
        let health = f64::from(city.hp.max(0));
        let ask = matched_guns(hit, walls, health, target_turns);
        Some(BreakerMatch {
            hit,
            defense,
            target_turns,
            ask,
        })
    }

    /// `breakers-match-the-walls`: the guns the campaign target's Siege row
    /// asks, the matched count when it exceeds the shipped `stock`, and a
    /// Detail line saying why; `stock` when the gene is off, the count asks
    /// no more, or even [`BREAKER_MATCH_MAX`] guns cannot finish in time.
    pub(super) fn breakers_match_the_row(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        stock: usize,
    ) -> usize {
        let Some(matched) = self.breakers_matched_to_walls(g, pid, cid) else {
            return stock;
        };
        let city = &g.cities[&cid];
        let BreakerMatch {
            hit,
            defense,
            target_turns,
            ask,
        } = matched;
        match ask {
            Some((guns, breach, take)) if guns > stock => {
                think!(self.journal(), Military, Detail,
                    "Siege of {}: the walls ask {} guns", city.name, guns;
                    "our best gun hits {:.1} a shot against {:.0}; {} guns breach {} walls in {:.1} turns \
                     and take the city in {:.1}, within {:.1} of the train's endurance",
                    hit, defense, guns, city.wall_hp, breach, take, target_turns;
                    city.pos);
                guns
            }
            Some(_) => stock,
            None => {
                think!(self.journal(), Military, Detail,
                    "Siege of {}: the walls hold the ask at {} guns", city.name, stock;
                    "our best gun hits {:.1} a shot against {:.0}; {} guns cannot breach {} walls \
                     and take the city within {:.1} turns",
                    hit, defense, BREAKER_MATCH_MAX, city.wall_hp, target_turns;
                    city.pos);
                stock
            }
        }
    }

    /// `breakers-match-the-walls`: the guns `cid`'s walls ask, or `None`
    /// when the gene is off or holds the shipped count.
    pub(super) fn breakers_matched_guns(&self, g: &Game, pid: usize, cid: u32) -> Option<usize> {
        self.breakers_matched_to_walls(g, pid, cid)?
            .ask
            .map(|(guns, _, _)| guns)
    }

    /// Whether the wall-breaker reservation reads `owner` as at war: a war
    /// being fought, or, under `breaker-before-the-war`, the Conquest plan's
    /// own target, the war the army is staging for.
    ///
    /// Live King civvis-20261004T070716Z (game 49) aimed its campaign at
    /// Japan's walled Kyoto from turn 15 and held off its war for want of a
    /// staged siege; the first Catapult was ordered at turn 70, after Japan
    /// declared, and the siege read "nothing to open the walls, so the train
    /// holds outside the city's reach" from turn 64 to 77 while the force
    /// stood 83% ready.
    fn breaker_war_with(&self, g: &Game, pid: usize, owner: usize, plan: &StrategicPlan) -> bool {
        g.is_at_war(pid, owner)
            || (self.breaker_before_the_war
                && plan.strategy == GrandStrategy::Conquest
                && plan.target_player == Some(owner))
    }

    /// Remember wall breakers whose production would otherwise be lost when a
    /// later city governor writes over the queue. The target can complete
    /// walls while the gun is under construction, so an active assault keeps
    /// the commitment even before its first wall is observed.
    pub(super) fn invested_wall_breaker_queues(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Vec<(u32, Item)> {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || plan.strategy != GrandStrategy::Conquest
            || !plan
                .target_city
                .and_then(|id| g.cities.get(&id))
                .is_some_and(|target| {
                    Some(target.owner) == plan.target_player && g.is_at_war(pid, target.owner)
                })
        {
            return Vec::new();
        }
        g.player_city_ids(pid)
            .into_iter()
            .filter_map(|cid| {
                let item = g.cities[&cid].queue.first()?;
                let Item::Unit { unit } = item else {
                    return None;
                };
                let spec = g.rules.units.get(unit)?;
                (spec.siege
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && g.item_invested_production(cid, item) > 0.0)
                    .then(|| (cid, item.clone()))
            })
            .collect()
    }

    /// Reclaim a previously chosen wall breaker after all routine queue
    /// writers. A confirmed defender, recent attack, or another siege weapon
    /// remains in charge; a completed peace ends this claim. A nonmilitary
    /// queue also keeps its slot while the governor's treasury-recovery
    /// condition holds: finishing another gun cannot repair its upkeep bill.
    pub(super) fn restore_wall_breaker_queues(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
        claimed: &[(u32, Item)],
        defense_claim: Option<&(u32, Item)>,
    ) {
        if !plan
            .target_city
            .and_then(|id| g.cities.get(&id))
            .is_some_and(|target| {
                Some(target.owner) == plan.target_player && g.is_at_war(pid, target.owner)
            })
        {
            return;
        }
        let recovery_due = g.players[pid].gold_per_turn < -0.5
            && g.players[pid].gold < 100.0 + 25.0 * g.player_city_ids(pid).len() as f64;
        for (cid, item) in claimed {
            let Some(city) = g.cities.get(cid) else {
                continue;
            };
            if city.owner != pid
                || plan.threatened_city == Some(*cid)
                || defense_claim.is_some_and(|(protected, _)| protected == cid)
                || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
                || city.queue.first().is_some_and(|current| {
                    matches!(current, Item::Unit { unit }
                        if g.rules.units.get(unit).is_some_and(|spec| spec.siege))
                })
                || !g.can_produce(pid, *cid, item)
            {
                continue;
            }
            if recovery_due
                && city.queue.first().is_some_and(|current| match current {
                    Item::Unit { unit } | Item::Formation { unit, .. } => g
                        .rules
                        .units
                        .get(unit)
                        .is_some_and(|spec| spec.class != "military"),
                    _ => true,
                })
            {
                if self.journal().wants(crate::reasoning::Level::Decision) {
                    think!(self.journal(), Economy, Decision,
                        "{} keeps its nonmilitary queue while the treasury recovers", city.name;
                        "income {:.1} with {:.0} Gold; an extra wall breaker would add upkeep",
                        g.players[pid].gold_per_turn, g.players[pid].gold);
                }
                continue;
            }
            let city_name = city.name.clone();
            if g.apply(
                pid,
                &Action::Produce {
                    city: *cid,
                    item: item.clone(),
                },
            )
            .is_ok()
                && self.journal().wants(crate::reasoning::Level::Decision)
            {
                think!(self.journal(), Economy, Decision,
                    "{} resumes {}", city_name, Self::plain_item(item);
                    "the active Domination assault's wall breaker was displaced by a later production writer");
            }
        }
    }

    /// The delegated city governor does not call `production_value`, where the
    /// ordinary missing-siege reservation lives. Give a walled Domination
    /// assault one real bombardment unit before delegation fills every idle
    /// queue. An enemy that already knows Masonry can raise walls during the
    /// first few war turns, so a mostly healthy open objective also earns its
    /// first gun while the army marches. A fielded or queued land gun normally
    /// closes this reservation;
    /// a failed positive damage budget can reserve up to three in total. If
    /// only a new, low-production city is idle, a much faster city may give up
    /// an uninvested routine queue instead: a 140-turn gun cannot reinforce a
    /// siege that needs relief now. Likewise, a slow queued gun does not close
    /// the reservation if a second city can deliver one much sooner.
    pub(super) fn reserve_delegated_domination_siege(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<(u32, Item)> {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination) {
            return None;
        }
        let Some(target) = plan
            .target_city
            .and_then(|cid| g.cities.get(&cid))
            .filter(|city| {
                city.owner != pid
                    && !g.players[city.owner].is_minor
                    && self.breaker_war_with(g, pid, city.owner, plan)
                    && (city.wall_hp > 0
                        || (city.hp >= 160
                            && g.players[city.owner]
                                .techs
                                .contains(&crate::name!("masonry"))))
            })
        else {
            return None;
        };
        let objective = target.pos;
        let target_name = target.name.clone();
        let counts = self.counts(g, pid);
        let nearby_force: Vec<_> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|id| g.wdist(g.units[id].pos, objective) <= 5)
            .collect();
        let nearby_taker = nearby_force.iter().any(|id| {
            let spec = &g.rules.units[g.units[id].kind];
            spec.is_melee_capable() && spec.ranged_strength == 0.0 && spec.bombard_strength == 0.0
        });
        let siege_cap =
            if self.siege_train_scales_with_walls && g.city_max_wall_hp(target) >= HEAVY_WALL_HP {
                HEAVY_WALL_SIEGE_CAP
            } else {
                SHORTFALL_SIEGE_CAP
            };
        // `breakers-match-the-walls`: the campaign target's walls ask the guns
        // that breach and take it within the train's endurance; the cap and
        // the parallel supply follow that count.
        let matched = self.breakers_matched_guns(g, pid, target.id);
        let siege_cap = siege_cap.max(matched.unwrap_or(0));
        let breach_shortfall = self.siege_positive_damage_budget
            && counts.siege < siege_cap
            && nearby_taker
            && self
                .conversion_siege_budget(g, pid, target.id, &nearby_force)
                .is_some_and(|(finish, endurance)| finish > endurance * 0.8);
        if self.live_war_economy_requires_recovery(g, pid, &counts) {
            return None;
        }
        // `breaker-supply-scales`: a high-walled target takes one gun per
        // hundred wall points, ordered in parallel. Live King
        // civvis-20261004T083931Z (game 50), dominant at turn 125 (351
        // production, 1,127 power against at most 225), stood before Lisbon,
        // Portugal's capital, with walls 200 then 300 from turn 131 to 146,
        // "holding the capture for a wall-breaker on its way", and stood it
        // down: the reservation had ordered one Bombard, 17 turns out, while
        // Artillery was unlocked (diagnosed by -60).
        let supply_wanted = (g.city_max_wall_hp(target) as usize)
            .div_ceil(100)
            .clamp(1, SUPPLY_GUNS_MAX);
        let supply_short = self.breaker_supply_scales
            && g.city_max_wall_hp(target) >= SUPPLY_WALL_HP
            && counts.siege < supply_wanted;
        // `breaker-supply-scales-2`: the same parallel supply, only for an
        // original capital Domination needs, without setting aside a
        // building already under way, and the stronger gun only when it is
        // nearly as soon. Version 1 measured -9.7 +/- 5.3 pp on the win axis.
        let supply_v2 = self.breaker_supply_scales_2
            && target.is_capital
            && target.original_owner != pid
            && g.city_max_wall_hp(target) >= SUPPLY_WALL_HP
            && counts.siege < supply_wanted;
        let walls_short = matched.is_some_and(|guns| counts.siege < guns);
        let parallel = supply_short || supply_v2 || walls_short;
        let queued_arrival = if counts.land_siege_power > 0.0
            && !breach_shortfall
            && counts.siege == 1
            && !g.player_unit_ids(pid).into_iter().any(|id| {
                let spec = &g.rules.units[g.units[&id].kind];
                spec.siege && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
            }) {
            g.player_city_ids(pid)
                .into_iter()
                .filter_map(|cid| {
                    let item = g.cities[&cid].queue.first()?;
                    let Item::Unit { unit } = item else {
                        return None;
                    };
                    let spec = g.rules.units.get(unit)?;
                    (spec.siege && !matches!(spec.domain.as_deref(), Some("sea" | "air"))).then(
                        || {
                            self.production_build_turns(g, pid, cid, item)
                                + self.breaker_march_turns(
                                    g.wdist(g.cities[&cid].pos, objective),
                                    spec.moves,
                                )
                        },
                    )
                })
                .min_by(f64::total_cmp)
        } else {
            None
        };
        if counts.land_siege_power > 0.0
            && !breach_shortfall
            && !parallel
            && queued_arrival.is_none()
        {
            return None;
        }

        let best = {
            let _memo = g.query_memo();
            let mut candidates: Vec<(bool, f64, u32, Name)> = Vec::new();
            for cid in g.player_city_ids(pid) {
                if plan.threatened_city == Some(cid) {
                    continue;
                }
                // `housing-bound-city-builds-its-granary`: that gene's Granary
                // is neither routine nor displaceable.
                if self.housing_bound_granary_queued(g, cid) {
                    continue;
                }
                // `counterweight-finishes-one-shrine`: the one sanctuary's
                // Shrine is not a routine building to displace.
                if self.counterweight_shrine_held(g, pid, cid) {
                    continue;
                }
                let city = &g.cities[&cid];
                // `breaker-supply-scales`: a building already under way, not
                // a defence, may yield to the gun if the gun arrives soon.
                let displaceable = supply_short
                    && matches!(city.queue.first(), Some(Item::Building { building })
                    if !matches!(
                        building.as_str(),
                        "walls" | "medieval_walls" | "renaissance_walls"
                    ))
                    && !(city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4);
                let fresh_routine = match city.queue.as_slice() {
                    [] => false,
                    [queued]
                        if g.item_invested_production(cid, queued) <= f64::EPSILON
                            && !(city.last_attacked > 0
                                && g.turn.saturating_sub(city.last_attacked) <= 4) =>
                    {
                        match queued {
                            Item::Unit { unit } => matches!(unit.as_str(), "builder" | "trader"),
                            Item::Building { building } => !matches!(
                                building.as_str(),
                                "walls" | "medieval_walls" | "renaissance_walls"
                            ),
                            _ => false,
                        }
                    }
                    _ => continue,
                };
                if !city.queue.is_empty() && !fresh_routine && !displaceable {
                    continue;
                }
                let busy = !city.queue.is_empty() && !fresh_routine;
                for item in g.producible_items(pid, cid) {
                    let Item::Unit { unit } = item else { continue };
                    let spec = &g.rules.units[&unit];
                    if spec.class != "military"
                        || !spec.siege
                        || !spec.has_ranged_attack()
                        || matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    {
                        continue;
                    }
                    let arrival = self.production_build_turns(g, pid, cid, &item)
                        + self.breaker_march_turns(
                            g.wdist(g.cities[&cid].pos, objective),
                            spec.moves,
                        );
                    if busy && arrival > SUPPLY_DISPLACE_ARRIVAL {
                        continue;
                    }
                    candidates.push((fresh_routine || busy, arrival, cid, unit));
                }
            }
            // The soonest gun; under `breaker-supply-scales` the strongest,
            // then the soonest; under `-2` the strongest of those within
            // SUPPLY_STRENGTH_WINDOW turns of the soonest, then the soonest.
            let strength = |unit: &Name| g.rules.units[unit].ranged_attack_strength();
            let pick = |fresh: bool| -> Option<(f64, u32, Name)> {
                let bucket: Vec<&(bool, f64, u32, Name)> =
                    candidates.iter().filter(|c| c.0 == fresh).collect();
                let soonest = bucket.iter().map(|c| c.1).min_by(f64::total_cmp)?;
                bucket
                    .into_iter()
                    .filter(|c| {
                        !supply_v2 || supply_short || c.1 <= soonest + SUPPLY_STRENGTH_WINDOW
                    })
                    .min_by(|a, b| {
                        let by_strength = if supply_short || supply_v2 {
                            strength(&b.3).total_cmp(&strength(&a.3))
                        } else {
                            std::cmp::Ordering::Equal
                        };
                        by_strength
                            .then(a.1.total_cmp(&b.1))
                            .then((a.2, &a.3).cmp(&(b.2, &b.3)))
                    })
                    .map(|c| (c.1, c.2, c.3.clone()))
            };
            let best_idle = pick(false);
            let best_fresh = pick(true);
            match (best_idle, best_fresh) {
                (Some(idle), Some(fresh))
                    if fresh.0 <= 30.0
                        && if self.breaker_to_the_fastest {
                            idle.0 >= fresh.0 + BREAKER_FASTEST_MARGIN
                                && idle.0 >= fresh.0 * BREAKER_FASTEST_RATIO
                        } else {
                            idle.0 >= fresh.0 + 8.0 && idle.0 >= fresh.0 * 1.5
                        } =>
                {
                    Some((fresh.0, fresh.1, fresh.2, Some(idle.0)))
                }
                (None, Some(fresh)) if fresh.0 <= 30.0 => Some((fresh.0, fresh.1, fresh.2, None)),
                (Some(idle), _) => Some((idle.0, idle.1, idle.2, None)),
                _ => None,
            }
        };
        let Some((arrival, city, unit, slow_idle)) = best else {
            return None;
        };
        let faster_than_queued = queued_arrival.is_some_and(|queued| {
            arrival <= 20.0 && queued >= arrival + 8.0 && queued >= arrival * 1.5
        });
        if counts.land_siege_power > 0.0 && !breach_shortfall && !parallel && !faster_than_queued {
            return None;
        }
        let displaced = g.cities[&city].queue.first().cloned();
        let displaced_invested = displaced
            .as_ref()
            .is_some_and(|old| g.item_invested_production(city, old) > f64::EPSILON);
        let item = Item::Unit { unit };
        if g.apply(
            pid,
            &Action::Produce {
                city,
                item: item.clone(),
            },
        )
        .is_err()
        {
            return None;
        }
        if let Some(old) = displaced {
            let alternative = slow_idle.map_or_else(
                || "no idle city could build the gun in time".to_string(),
                |turns| format!("the fastest idle city needed about {turns:.0} turns"),
            );
            think!(self.journal(), Military, Detail,
                "{} gives its fresh {} queue to the siege gun", g.cities[&city].name, Self::plain_item(&old);
                "{}; the gun arrives in about {arrival:.0} turns, while {alternative}",
                if displaced_invested { "its progress waits in the city" } else { "no production was invested in it" };
                objective);
        }
        think!(self.journal(), Military, Decision,
            "{} reserves a {} for the walled assault", g.cities[&city].name, unit;
            "{}; expected arrival at {} in about {arrival:.0} turns",
            if breach_shortfall { "the staged force cannot breach before its health runs out" }
            else if faster_than_queued { "the first queued gun would reach the front too late" }
            else { "the delegated governor has no land siege weapon" },
            target_name;
            objective);
        Some((city, item))
    }

    /// `housing-bound-city-builds-its-granary`: whether `cid`'s queue holds
    /// its Granary (or the civilization's replacement) while its population
    /// is within one of its housing -- the gene's own build, which the
    /// breaker reservation's `fresh_routine` and `displaceable` swaps must
    /// not take, or the two fight over the same queue. Live Emperor G204
    /// (civvis-20261006T065830Z) t110: "Caracas gives its fresh granary queue
    /// to the siege gun", twice, for a 16-turn Trebuchet. Never with the
    /// gene off.
    fn housing_bound_granary_queued(&self, g: &Game, cid: u32) -> bool {
        if !self.housing_bound_city_builds_its_granary {
            return false;
        }
        let city = &g.cities[&cid];
        let granary = crate::name!("granary");
        matches!(
            city.queue.first(),
            Some(Item::Building { building })
                if *building == granary
                    || g.rules
                        .buildings
                        .get(building)
                        .is_some_and(|spec| spec.replaces == Some(granary))
        ) && (city.pop as f64) + 1.0 >= g.city_housing(city)
    }

    /// A roster full of field units can still lack the ability to break walls.
    /// Counts include queued units, so only the first siege order gets this
    /// composition exception to the ordinary army ceiling. An active siege
    /// keeps this requirement when the empire replans for expansion or recovery.
    pub(super) fn missing_domination_siege(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        counts: &EmpireCounts,
        spec: &crate::rules::UnitSpec,
    ) -> bool {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !spec.siege
            || matches!(spec.domain.as_deref(), Some("sea" | "air"))
        {
            return false;
        }
        // A full roster of obsolete Catapults is not a modern siege train.
        // Ten strength is a meaningful step (roughly 50% more damage under
        // the combat curve). Field formations and queued replacements count;
        // the governor's queue-excluded census keeps a reservation from
        // cancelling itself. This still opens only one missing weapon slot.
        if counts.siege != 0 && spec.ranged_attack_strength() < counts.land_siege_power + 10.0 {
            return false;
        }
        if let Some(target) = plan.target_city {
            return g.cities.get(&target).is_some_and(|city| {
                Some(city.owner) == plan.target_player
                    && city.wall_hp > 0
                    && self.breaker_war_with(g, pid, city.owner, plan)
            });
        }

        // A defensive replan can select a different enemy whose cities have
        // not been revealed yet. That does not make the known hostile walls
        // disappear. Preserve the first weapon already ordered for them;
        // without a current target, an empty city receives no new reservation.
        // The governor excludes this city's queue from `counts` when valuing
        // its commitment; a fielded or separately queued weapon still closes
        // the requirement. Emergency defense remains upstream of rescoring.
        let queued_weapon = g.cities.get(&cid).is_some_and(|city| {
            city.owner == pid
                && city.queue.first().is_some_and(|item| match item {
                    Item::Unit { unit } => g.rules.units.get(unit).is_some_and(|queued| {
                        queued.siege && !matches!(queued.domain.as_deref(), Some("sea" | "air"))
                    }),
                    _ => false,
                })
        });
        queued_weapon
            && g.cities
                .values()
                .any(|city| city.wall_hp > 0 && g.is_at_war(pid, city.owner))
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod war_strategy_tests;

#[cfg(test)]
mod modernization_tests;

#[cfg(test)]
mod treasury_tests;

#[cfg(test)]
mod breaker_match_tests;
