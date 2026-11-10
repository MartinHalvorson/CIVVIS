//! The turn's assessment: how `AdvancedAi` reads the board before it acts.
//!
//! `assess` builds the turn's `StrategicPlan`. The helpers beside it read
//! the war (who declared on us, whether a war we did not choose holds our
//! victory lane, which rivals are in campaign reach, which domination
//! objective has gone stale, what lies within declaration range or the
//! first capture march, and whether city-states count as targets) and the
//! land (fog land estimate, settle sites for room, map settlement room).
//!
//! Moved verbatim out of the one `impl AdvancedAi` block in `advanced.rs`
//! (2026-11-05). Methods the rest of the controller calls are `pub(super)`;
//! the code is otherwise unchanged.

use super::*;

impl AdvancedAi {
    /// Whether every live major war against `pid` was opened by the other
    /// side.
    ///
    /// A war we declared is one we chose and must pay for. A war declared on
    /// us is a rival's move, and `WarRecord::declarer` is the engine's own
    /// record of which is which — the same field the warmonger and grievance
    /// accounting reads, so this cannot disagree with the rest of the game
    /// about who started it. False when we are not at war with a major at all,
    /// so the caller can use it as the whole condition.
    pub(super) fn every_major_war_was_declared_on_us(&self, g: &Game, pid: usize) -> bool {
        let mut any = false;
        for other in g.players.iter().filter(|other| {
            other.id != pid
                && other.alive
                && !other.is_minor
                && !other.is_barbarian
                && g.is_at_war(pid, other.id)
        }) {
            any = true;
            let key = (pid.min(other.id), pid.max(other.id));
            match g.wars.get(&key) {
                // No record is no evidence, and this gene only stands down on
                // evidence: an unrecorded front counts as ours.
                None => return false,
                Some(war) if war.declarer == pid => return false,
                Some(_) => {}
            }
        }
        any
    }

    /// Whether a war nobody here chose should keep the empire's own victory
    /// lane instead of taking the grand strategy from it.
    ///
    /// See [`Self::unchosen_war_keeps_the_lane`]. Every condition is a gate,
    /// and the caller sits below the Recovery branches, so by the time this is
    /// asked no city of ours is threatened and we are not losing ground.
    /// Whether an elective war should yield the grand strategy to the lane the
    /// empire is already racing.
    ///
    /// See [`Self::elective_war_yields_to_a_lane`]. Deliberately the same
    /// `LIVE_LANE_FLOOR` test as [`Self::unchosen_war_holds_the_lane`], and
    /// deliberately no other condition: the difference between the two genes
    /// is which branch asks, not what is asked.
    pub(super) fn elective_war_yields(&self, g: &Game, pid: usize) -> bool {
        self.elective_war_yields_to_a_lane && self.victory_focus(g, pid).progress >= LIVE_LANE_FLOOR
    }

    pub(super) fn unchosen_war_holds_the_lane(&self, g: &Game, pid: usize) -> bool {
        self.unchosen_war_keeps_the_lane
            && self.every_major_war_was_declared_on_us(g, pid)
            && self.victory_focus(g, pid).progress >= LIVE_LANE_FLOOR
    }

    /// Whether `rival` has a city within the campaign planner's own reach of
    /// one of ours.
    ///
    /// See [`Self::elective_war_in_reach`]. `CAMPAIGN_REACH` is
    /// `city_campaign`'s constant and its comment calls it "the same reach the
    /// declaration's `close_enough` already asks for"; this asks the same
    /// question of the same distance, so the branch that declares and the
    /// planner that has to prosecute cannot disagree about who is a
    /// neighbour.
    pub(super) fn rival_is_in_campaign_reach(&self, g: &Game, pid: usize, rival: usize) -> bool {
        let ours = g.player_city_ids(pid);
        if ours.is_empty() {
            return false;
        }
        let theirs = g.player_city_ids(rival);
        theirs.iter().any(|their| {
            let Some(their) = g.cities.get(their) else {
                return false;
            };
            ours.iter().any(|our| {
                g.cities.get(our).is_some_and(|our| {
                    g.wdist(our.pos, their.pos)
                        <= crate::ai::advanced::city_campaign::CAMPAIGN_REACH
                })
            })
        })
    }

    /// Break a stale march to an untouched city only when a different city
    /// of the same rival is already within the field army's short march.
    /// An army on the original siege ring, or damage to its defenses, keeps
    /// the existing commitment. In particular, an unwalled distant objective
    /// must not strand the army while a nearby open city builds its first wall.
    pub(super) fn stale_domination_objective_city(
        &self,
        g: &Game,
        pid: usize,
        prior_id: u32,
        strategy: GrandStrategy,
    ) -> Option<u32> {
        if !self.siege_commitment
            || strategy != GrandStrategy::Conquest
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
        {
            return None;
        }
        let prior = g.cities.get(&prior_id)?;
        let prior_wall_max = g.city_max_wall_hp(prior);
        if !g.is_at_war(pid, prior.owner)
            || g.players[prior.owner].is_minor
            || g.players[prior.owner].is_barbarian
            || prior.hp < CITY_MAX_HP
            || (prior_wall_max > 0 && prior.wall_hp < prior_wall_max)
        {
            return None;
        }
        let army = self.campaign_field_army(g, pid);
        let nearest = |pos| army.iter().map(|uid| g.wdist(g.units[uid].pos, pos)).min();
        let prior_distance = nearest(prior.pos)?;
        if prior_distance <= SIEGE_COMMITMENT_REACH {
            return None;
        }
        let prior_value = self.campaign_city_value(g, pid, prior, strategy);
        // `stale-swap-reads-the-march`: when the army's BODY (its median
        // unit) stands `2 * DOMINATION_FIRST_CAPTURE_MARCH` or more from the
        // old objective, the march itself is the cost the value margin never
        // priced, and a capital's bonus no longer holds it. Live King
        // civvis-20261005T124739Z (game 130) aimed at Delhi, India's walled
        // capital, from turn 144: army median 15-20 tiles, 0 of 10-12 units
        // staged, while walled Jabalpur stood 4 tiles from our city with our
        // nearest unit 3 away -- every distance gate below passed and the
        // 180-point capital bonus failed the margin. Only 2 of the 43 live
        // runs of October 5 ever took this swap.
        let march_stale = self.stale_swap_reads_the_march && {
            let mut distances: Vec<i32> = army
                .iter()
                .map(|uid| g.wdist(g.units[uid].pos, prior.pos))
                .collect();
            distances.sort_unstable();
            distances
                .get(distances.len() / 2)
                .is_some_and(|median| *median >= 2 * DOMINATION_FIRST_CAPTURE_MARCH)
        };
        let visible = g.player_vision_frame(pid);
        g.cities
            .values()
            .filter(|city| {
                city.owner == prior.owner
                    && city.id != prior_id
                    && if prior_wall_max > 0 {
                        city.wall_hp > 0
                    } else {
                        city.wall_hp <= 0
                    }
                    && g.sees(&visible, city.pos)
                    && !self.capture_stood_down_holds(g, city.id)
                    && !Self::should_defer_city_capture(g, pid, city.id)
            })
            .filter_map(|city| {
                let distance = nearest(city.pos)?;
                let value = self.campaign_city_value(g, pid, city, strategy);
                (distance <= DOMINATION_FIRST_CAPTURE_MARCH
                    && prior_distance - distance >= STALE_DOMINATION_MARCH_GAIN
                    && (march_stale || value + STALE_DOMINATION_VALUE_GAIN < prior_value))
                    .then_some((value, distance, city.id))
            })
            .min_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then(left.1.cmp(&right.1))
                    .then(left.2.cmp(&right.2))
            })
            .map(|(_, _, city)| city)
    }

    pub(super) fn assess(&self, g: &Game, pid: usize) -> StrategicPlan {
        // The outlook reads one immutable snapshot. Reuse city and empire
        // derivations across its rival and settlement comparisons.
        let _memo = g.query_memo();
        let cities = g.player_city_ids(pid);
        let my_power = g.military_power(pid);
        let major_rivals: Vec<usize> = g
            .players
            .iter()
            .filter(|p| {
                p.id != pid
                    && p.alive
                    && !p.is_minor
                    && !p.is_barbarian
                    // A mirror retains empty seats for civilizations the host has
                    // not introduced. Their generated zero power must not make a
                    // real, met rival look weak enough to attack. This is part of
                    // live fog observation, not the frozen advanced_v1 anchor.
                    && (!self.battlefront_observation
                        || g.has_met(pid, p.id)
                        || g.is_at_war(pid, p.id))
            })
            .map(|p| p.id)
            .collect();
        // City-states follow their Suzerain into wars and can also be attacked
        // directly. Once hostilities exist they are real campaign actors, not
        // an uncoordinated side task for whichever unit happens to be nearby.
        // A public city count can prove a major still owns cities whose
        // locations this seat has not seen. Keep that war ahead of a joined
        // minor without manufacturing a city objective. An empty seat with
        // neither located cities nor this positive evidence cannot pin it.
        let wartime_rivals: Vec<usize> = g
            .players
            .iter()
            .filter(|p| {
                p.id != pid
                    && p.alive
                    && !p.is_barbarian
                    && (!g.player_city_ids(p.id).is_empty()
                        || (!p.is_minor && g.unseen_city_owners.contains(&p.id)))
                    && g.is_at_war(pid, p.id)
            })
            .map(|p| p.id)
            .collect();
        let wartime_majors: Vec<usize> = wartime_rivals
            .iter()
            .copied()
            .filter(|rival| !g.players[*rival].is_minor)
            .collect();
        let at_war = !wartime_rivals.is_empty();
        // See `opportunistic_war`: a raid is a bounded war the empire chose
        // for a prize, priced against that one neighbour. While it is the
        // only major war it neither pins the plan on Conquest nor drops it
        // into the power-gap Recovery a strong third party would trigger;
        // a threatened city still does.
        let raid_only_war = self.raid_only_war(g, pid);
        let strongest_rival = major_rivals
            .iter()
            .map(|o| g.military_power(*o))
            .fold(0.0_f64, f64::max);
        // ⚠ `recovery_reads_the_war`: both family versions read the strongest
        // major we are fighting. Do not sum armies or discard a remote current
        // opponent: dedicated screens rejected both alternatives. Summing
        // cost 2.93 end techs and 36 science/turn versus off across 48 games;
        // filtering by campaign reach trailed v1 by 8.2 win points and 1.60
        // score-share points in its 24-game directional block. Off, preserve
        // the historical board-wide maximum. V2 differs in Recovery's economic
        // weights, not in whom the war reading sees or when the posture starts.
        let recovery_opponent_power =
            if self.recovery_reads_the_war || self.recovery_reads_the_war_2 {
                wartime_majors
                    .iter()
                    .map(|o| g.military_power(*o))
                    .fold(0.0_f64, f64::max)
            } else {
                strongest_rival
            };
        // ⚠ `elective_war_in_reach`: the weakest rival WE CAN REACH, by the
        // reach the campaign planner already requires. Off, this is the
        // weakest empire anywhere on the board, which is what the branch
        // below has always compared itself against. Infinity when nobody is
        // in reach, so the branch cannot fire — the correct reading of "there
        // is nobody nearby to take anything from".
        let weakest_rival = major_rivals
            .iter()
            // `domination-targets-land-first`: on a Domination seat the bar's
            // weakest rival is one we can march to. See
            // `advanced/domination_land_first.rs`.
            .filter(|other| self.domination_land_rank(g, pid, **other) == 0)
            .filter(|other| {
                !self.elective_war_in_reach || self.rival_is_in_campaign_reach(g, pid, **other)
            })
            .map(|o| g.military_power(*o))
            .fold(f64::INFINITY, f64::min);

        let threatened_city = self.threatened_city(g, pid);
        let land = g
            .map
            .tiles
            .values()
            .filter(|t| g.rules.is_passable(t) && !g.rules.is_water(t))
            .count();
        // ★★★ `wide_map_capacity` REPAIRED 2026-08-19 after the 4,000-pair
        // native screen priced it at −3.4 pp wins with +2.3 pp score share
        // (z −4.5 / +16.5, seeds 40000000..) and the anchors at +3.45 cities
        // with no wins: a bigger empire that does not convert. The mechanism
        // is not subtle — two thirds of native games end by RELIGIOUS
        // conversion, a religious victory is won by converting cities, and
        // every extra unconverted city this flag buys is another follower
        // for the rival's clock.
        // (Second cut, same day: the first gate stood the target down once a
        // third of our cities flew a foreign faith. Measured on 1,200
        // verification pairs (seeds 43000000..) that signal arrives ~t120,
        // AFTER the capacity is consumed — the paired cities delta did not
        // move (+2.68 → +2.78). The regime split of the 4,000 screened pairs
        // names the early fact instead: +9.0 pp in games that do NOT end by
        // conversion (and +19.2, z +18.5, in the domination,score screen)
        // against −7.5 pp in games that do. So: the wide target stands while
        // a religious victory is disabled, or until the first RIVAL religion
        // is founded (~t40–70, before the settling window closes); from that
        // moment the conversion race is live and the narrow target stands.
        // A war-lane game keeps the full +19 by construction.)
        let conversion_race_live = g.victory_conditions.religious
            && g.players.iter().any(|player| {
                player.id != pid
                    && player.alive
                    && !player.is_minor
                    && !player.is_barbarian
                    && player.religion.is_some()
            });
        let active_victory_target = self.active_victory_target(g);
        let specialization_active = self.phase_specialization_active(g);
        let science_targeted = active_victory_target == Some(VictoryTarget::Science);
        let map_capacity = if self.wide_map_capacity && !conversion_race_live {
            // Live-bridge pricing: one city per 45 passable tiles, ceiling
            // twelve. A fresh live mirror knows the world's dimensions but
            // deliberately carries only revealed terrain, so its counted
            // passable land is smallest precisely when expansion must begin.
            // Treat fog as unknown capacity, not proof of a three-city map.
            // Practical-site checks still stop production when sites run out.
            //
            // The floor answers the opening; the fogged MIDDLE game is what
            // `fog_land_capacity` answers, by reading the land share of what
            // is known across the whole board. See that flag.
            let land = if self.fog_land_capacity {
                Self::fog_land_estimate(g, land)
            } else {
                land
            };
            (2 + land / 45)
                .clamp(3, 12)
                .max(PRODUCTION_CITY_TARGET_FLOOR)
        } else {
            (2 + land / 55).clamp(3, 9)
        };
        // Expansion must compound before it pays back. Add roughly one city
        // per era instead of continuously raising the target and starving a
        // young empire of districts, buildings, and population growth. Scale
        // the cadence with game speed; the old fixed turn-175 cutoff expired
        // before the five-city target even became active on Standard speed.
        // See `era_paced_expansion`: an era of the Settler game is forty
        // Online turns, not sixty.
        let city_cadence = g
            .standard_duration(if self.era_paced_expansion { 60 } else { 90 })
            .max(1) as usize;
        // Historical controls keep the old six-city planning ceiling. The
        // production delegation flag is also the compatibility boundary that
        // lets the promoted controller consume the full land-aware capacity.
        let city_ceiling = if self.plan_city_target {
            map_capacity
        } else {
            map_capacity.min(6)
        };
        // The deployed wide treatment formerly changed only the late ceiling,
        // leaving the current live opening bound at three cities. Start that
        // plan at the measured six-city floor; ordinary Advanced and the
        // frozen anchor retain three.
        let city_floor = if self.wide_map_capacity && !conversion_race_live {
            self.city_target_floor.max(PRODUCTION_CITY_TARGET_FLOOR)
        } else {
            self.city_target_floor
        };
        // ★★★★ THE LAND GRAB WANTS THE LAND, NOT A RUNG. See `land_grab`: the
        // seat leads the count at t50 and loses it by t150 because the target
        // climbs one rung per era while the rivals settle to the ground. With
        // it on, the target is the fog-estimated capacity itself — priced
        // denser than the wide-map ceiling — from turn one; the pipeline,
        // the practical-site search and the payback deadline pace the count.
        let ordinary_city_target = (city_floor + g.turn as usize / city_cadence).min(city_ceiling);
        let desired_cities = if self.rapid_city_expansion_2 {
            // `land-grab-outlasts-the-opening` keeps the land grab's horizon
            // here; see `advanced/land_grab_outlasts.rs`.
            let rapid = rapid_city_expansion::city_target(g, ordinary_city_target, city_ceiling);
            self.rapid_city_target(g, rapid, land, science_targeted)
        } else if self.land_grab {
            self.land_grab_city_target(g, land, science_targeted)
        } else {
            ordinary_city_target
        };
        // The shared production contract is a horizon, not a promise to make
        // an impossible Settler. It deliberately sits after every expansion
        // variant so a religious conversion race, a science phase, or an
        // experimental arm cannot silently return either verification or the
        // standard tournament to a six- or eight-city plan.
        // `city_target_meets_the_map` below remains the hard practical-site
        // gate.
        let desired_cities = if self.shared_city_target {
            desired_cities
                .clamp(SHARED_CITY_TARGET_FLOOR, SHARED_CITY_TARGET_CEILING)
                .max(cities.len())
        } else {
            desired_cities
        };
        // `expansion-scales-with-difficulty`: the 4-6 band is a King-level
        // reading, and each rung above Prince hands the rivals a percentage of
        // every yield and free Settlers. Raise the horizon by the rung, never
        // lower it. `city_target_meets_the_map`'s practical-site room still
        // cuts it back down below; the Science contract is raised to meet it
        // instead (see the cap), because a `min` applied after this line
        // would swallow the widening on the very lane the ladder plays. See
        // `advanced/expansion_scales_with_difficulty.rs`.
        let wide_city_target = self.expansion_wide_city_target(g);
        let desired_cities =
            wide_city_target.map_or(desired_cities, |wide| desired_cities.max(wide));
        let mut expansion_origins: Vec<Pos> = cities.iter().map(|cid| g.cities[cid].pos).collect();
        if expansion_origins.is_empty() {
            expansion_origins.extend(
                g.player_unit_ids(pid)
                    .into_iter()
                    .filter(|uid| g.units[uid].kind == "settler")
                    .map(|uid| g.units[&uid].pos),
            );
        }
        let has_site = expansion_origins.iter().any(|pos| {
            self.settle_site_exists(g, pid, *pos, 10)
                || (g.players[pid].techs.contains(&crate::name!("shipbuilding"))
                    && self.settle_site_exists(g, pid, *pos, g.map.width + g.map.height))
        });
        // See `city_target_meets_the_map`: the target above is a land census,
        // and land is not the same thing as somewhere to put a city.
        let desired_cities = if self.city_target_meets_the_map {
            let room = self.map_settlement_room(g, pid, &expansion_origins, desired_cities);
            // ⚠ THE CAP MUST NEVER CLOSE THE OPENING. `room` and `has_site`
            // read the same scanner at the same radius, so a site the shipped
            // expansion arms can see is always at least one seat here. Saying
            // so out loud makes the failure mode that would matter -- a young
            // empire capped to the cities it already holds while good land
            // sits beside it -- unrepresentable rather than merely unlikely.
            let room = room.max(usize::from(has_site));
            desired_cities.min(cities.len() + room)
        } else {
            desired_cities
        };
        // `land_grab` is a useful generic deployment policy, but its sixteen
        // city horizon is incompatible with an explicit Science contract. A
        // Science seat still settles enough productive cities to scale its
        // Campuses, then stops treating open land as the main objective. Never
        // lower the target beneath an empire that already exceeded the cap.
        // `science-expansion-phase` is the standing counter-hypothesis: while
        // a founded city can still mature into the super-linear pop term the
        // contract is deferred — the widened land-grab ceiling above sets the
        // window's target — and the cap resumes when the window closes.
        let desired_cities = if science_targeted
            && specialization_active
            && !(self.science_expansion_phase
                && g.turn < g.standard_duration(SCIENCE_EXPANSION_UNTIL_TURN))
        {
            let cap = if self.shared_city_target {
                SHARED_SCIENCE_CITY_TARGET
            } else {
                SCIENCE_CITY_TARGET_CAP
            };
            // `expansion-scales-with-difficulty`: the rung's horizon is the
            // floor of this cap, or the gene is inert on a Science seat —
            // `SCIENCE_CITY_TARGET_CAP` (6) is below every rung target the
            // gene sets (7 at Prince). `None` with the gene off, so the cap
            // reads its shipped constant.
            let cap = wide_city_target.map_or(cap, |wide| cap.max(wide));
            desired_cities.min(cap).max(cities.len())
        } else {
            desired_cities
        };
        // `rival-pace`: a calibrated rival chair settles to the live field's
        // curve, last so that no widening above can lift it back. See
        // `advanced/rival_pace.rs`.
        let desired_cities = match self.rival_city_pace {
            Some(extra) => rival_pace::capped(g, desired_cities, cities.len(), extra),
            None => desired_cities,
        };
        // `gene_screen --seat-pace`: a measured seat settles to the live
        // seat's own curve. See `advanced/rival_pace.rs`.
        let desired_cities = if self.seat_city_pace {
            rival_pace::seat_capped(g, desired_cities, cities.len())
        } else {
            desired_cities
        };
        let military_civ = matches!(
            g.players[pid].civ.as_str(),
            "Sumeria" | "Aztec" | "Nubia" | "Scythia" | "Byzantium"
        );
        let basil_tagma_timing = g.has_ability(pid, "taxis")
            && (g.players[pid].religion.is_some()
                || g.players[pid]
                    .civics
                    .contains(&crate::name!("divine_right")));
        // The ancient rush is a *window*, not a preference, so it is decided
        // beside the other timing arm rather than among the victory lanes.
        let rush_victim = self.early_rush_victim(g, pid);
        let victory = self.victory_focus(g, pid);
        // Target selection needs the same public culture-race totals as
        // victory denial. Build them once for the assessment instead of
        // repeating a whole-world tourism scan for every sort comparison.
        let rival_culture_pressures = self.rival_culture_pressures(g);
        let actionable_denial =
            self.actionable_victory_denial_with_culture_pressures(g, pid, &rival_culture_pressures);
        let domination_finish = actionable_denial
            .and_then(|(rival, _)| {
                self.domination_finishing_capital_for(g, pid, rival)
                    .map(|city| (rival, city))
            })
            // See `domination_finish_at_war`.
            .or_else(|| self.domination_finish_at_war(g, pid))
            // See `free_city_finish`: the last capital held by a Free City.
            .or_else(|| self.free_city_finish(g, pid));
        let emergency_objective = g.emergency_objective(pid).cloned();
        // Each arm carries the reason it fired. The strings are static and
        // cost nothing to build; they exist so the spectator's reasoning log
        // can say which of these tests the empire's whole plan turned on
        // instead of only naming the strategy that came out.
        // ⚠ Recovery is documented one screen down as "a temporary battlefield
        // posture", and this branch is the only thing that makes it permanent.
        // A threatened city IS transient — the attacker leaves or the city
        // falls. A power gap against a runaway leader is not: Recovery does not
        // build an army, so `my_power * 1.25 < strongest_rival` stays true
        // precisely because Recovery is what we chose, and it re-fires every
        // assessment for the rest of the game.
        //
        // Measured on `civvis-20260802T205959Z` (Sweden, Settler, small): the
        // journal names this exact arm — "at war and losing ground at home" —
        // **160 times**, matching all 160 Recovery turns. The empire held that
        // posture from t65 to t229, 72% of the game, peaking at 4 cities on t61
        // and holding 1 by t85. It finished with ONE warrior, military 34
        // against the Mapuche's 1354, score 205 against 1324.
        //
        // So the power-gap half is bounded and the threatened-city half is not.
        // Local defence no longer depends on this posture in any case: #930's
        // `besieged_city_item` builds walls or a defender for a city with a
        // raiding party at its gates before the ordinary build order, whatever
        // the grand strategy says.
        //
        // ⚠ The bound is measured against the WAR's age, not the plan's. Keyed
        // on `major_war_since`, it releases Recovery until the war ends rather
        // than re-arming after one assessment. Replayed against the real
        // `civvis-20260802T205959Z`
        // mirror that produced a 1-in-25 duty cycle — recovery 86% -> 81%, with
        // only 5 of 93 assessed turns changed, at t87, t153, t176, t201 and
        // t219, each ~25 turns apart. `major_war_since` is monotone for as long
        // as the war lasts, so a posture that has already been given its turns
        // stays released until the war ends.
        let recovery_is_stale = self.bounded_recovery
            && self.major_war_since.is_some_and(|started| {
                g.turn.saturating_sub(started) >= g.standard_duration(RECOVERY_POSTURE_LIMIT).max(1)
            });
        // A stale Recovery normally gives the planner permission to pursue a
        // different win condition. It must not turn a collapsing major war
        // into an offensive denial the army cannot possibly execute: recent
        // live losses repeatedly held 14--41% of the leader's power at this
        // point. Preserve ordinary terminal counters; this is only the live
        // bridge's already-stale, already-at-war, less-than-half-power case.
        let impossible_denial = self.battlefront_observation
            && recovery_is_stale
            && !wartime_majors.is_empty()
            && actionable_denial.is_some_and(|(rival, counter)| {
                counter == GrandStrategy::Conquest && my_power * 2.0 < g.military_power(rival)
            });
        // An explicit Science contract must not let a power-gap comparison
        // turn a defensive answer into a permanent empire-wide Recovery
        // posture. The comparison is useful for an adaptive seat, but a
        // targeted seat can remain below a wartime rival for the whole race;
        // only a city-level threat should interrupt its research lane.
        // `recovery-needs-the-deficit`: a threatened city puts a war into
        // Recovery only while the army is short of
        // `RECOVERY_THREAT_POWER_RATIO` times the strongest opponent; above
        // it the Board's Defend rows answer the threat and the Conquest
        // plan's finishing front, capture opportunities and stale swaps stay
        // on. Live King civvis-20261005T141932Z (game 135) flipped to
        // Recovery at turn 224 at 1848 power against 645, one capital from
        // Domination, and the finishing front fell away. Over October 4-5,
        // Recovery turns at twice the strongest rival's power or more lost a
        // city within 5 turns 4 times in 82, against 110 in 807 under parity.
        let threat_recovery = threatened_city.is_some()
            && !(self.recovery_needs_the_deficit
                && my_power >= RECOVERY_THREAT_POWER_RATIO * recovery_opponent_power.max(1.0));
        // A final war has no later to recover for and no race left to deny:
        // every civilization in it attacks. See `last_civ_standing`.
        let (strategy, because) = if last_civ_standing::fights_the_final_war(g, pid) {
            (
                GrandStrategy::Conquest,
                "only the last civilization standing wins",
            )
        } else if at_war
            && (threat_recovery
                || (my_power * 1.25 < recovery_opponent_power
                    && !recovery_is_stale
                    && !raid_only_war
                    && !science_targeted))
        {
            (GrandStrategy::Recovery, "at war and losing ground at home")
        } else if impossible_denial {
            (
                GrandStrategy::Recovery,
                "holding the line before an impossible counter-campaign",
            )
        } else if emergency_objective.is_some() {
            (
                GrandStrategy::Conquest,
                "an emergency objective is standing",
            )
        } else if basil_tagma_timing {
            (GrandStrategy::Conquest, "Tagma timing is live")
        } else if rush_victim.is_some() {
            (
                GrandStrategy::Conquest,
                "a neighbour is inside the ancient window and cannot wall in time",
            )
        } else if domination_finish.is_some() {
            (
                GrandStrategy::Conquest,
                "the rival's original capital completes Domination before its victory clock",
            )
        } else if let Some((_, counter)) = actionable_denial.filter(|(rival, counter)| {
            (self.denial_outranks_expansion || self.domination_counter_target(g, pid, *rival))
                // Native Pericles 141453Z kept Culture as its target but
                // answered a launch clock with its own Spaceport. Racing for
                // Science does not deny that clock or serve the Culture goal.
                // Keep the threat and direct military counters; only this
                // empire-wide change to a competing victory yields the lane.
                && !(active_victory_target == Some(VictoryTarget::Culture)
                    && *counter == GrandStrategy::Science)
        }) {
            // `denial-outranks-expansion`: the same answer the branch below
            // gives, reached before the lane can say "keep expanding". A
            // rival at match point does not wait for our sixth city.
            (
                counter,
                "countering a rival close to winning, ahead of the lane",
            )
        } else if let Some(target) = active_victory_target {
            if target == VictoryTarget::Religion
                && g.players[pid].religion.is_none()
                // `religion-race-keeps-expanding`: with cities still to found
                // and a site in reach, the Expansion branch below answers
                // first. See `advanced/religion_race_expands.rs`.
                && !self.religion_race_expands_first(
                    !specialization_active && cities.len() < desired_cities && has_site,
                )
            {
                (
                    GrandStrategy::Religion,
                    "the religion lane still needs a religion",
                )
            } else if !specialization_active
                && cities.len() < desired_cities
                && has_site
                // `domination-lane-hands-over`: a conquest lane with the opening
                // band's cities in hand goes to war; it does not wait for a city
                // target the map cannot meet.
                && !(self.domination_lane_hands_over
                    && target == VictoryTarget::Domination
                    && cities.len() >= DOMINATION_HANDOVER_CITIES
                    // ⚠ And only with an army that can win the war it opens. Game 6
                    // (2026-09-10) handed over at t100 with power 198 against the
                    // strongest rival's 384: Recovery by t116, one city by t170 —
                    // a t270 victory-clock loss turned into a t170 suicide. The bar
                    // is the same one the elective-war branch below has always
                    // used against the weakest living rival.
                    && my_power > weakest_rival * 1.80 + 20.0)
            {
                (
                    GrandStrategy::Expansion,
                    "the first half is reserved for expansion and defense",
                )
            } else {
                (target.strategy(), "following the assigned victory lane")
            }
        } else if let Some((_, counter)) = actionable_denial {
            (counter, "countering a rival close to winning")
        } else if at_war && !raid_only_war {
            // ⚠ `unchosen_war_keeps_the_lane`: a war a rival opened on us,
            // which the Recovery branches above have already found is not
            // hurting us, hands the plan to the lane the empire is racing
            // instead of to the war. It NAMES that lane here rather than
            // falling through, because the ladder below lands on Expansion —
            // an empire walking Settlers into somebody else's war.
            if self.unchosen_war_holds_the_lane(g, pid) {
                (
                    self.victory_focus(g, pid).strategy,
                    "a war we did not declare, and a lane that is live",
                )
            } else {
                (GrandStrategy::Conquest, "already at war")
            }
        } else if !self.no_elective_war
            // ⚠ `elective_war_yields_to_a_lane`: a war we CHOOSE does not take
            // the plan from a lane we are already racing. See that flag.
            && !self.elective_war_yields(g, pid)
            && cities.len() >= 2
            && self.city_campaign_stands(g, pid)
        {
            // `city_campaign`: a weaker neighbour appraised on public power
            // and science, and a holdable city priced with the spare. See
            // `advanced/city_campaign.rs`.
            (
                GrandStrategy::Conquest,
                "a city campaign is planned against a weaker neighbour",
            )
        } else if
        // ★★★★ Not on the live seat: see `no_elective_war` — eight games,
        // no city ever taken, sixteen lost.
        !self.no_elective_war
            && !self.elective_war_yields(g, pid)
            && ((g.turn >= 55 && cities.len() >= 2 && my_power > weakest_rival * 1.80 + 20.0)
                || (military_civ
                    && g.turn >= 35
                    && cities.len() >= 2
                    && my_power >= strongest_rival * 1.10))
        {
            (
                GrandStrategy::Conquest,
                "strong enough to take what a neighbour has",
            )
        } else if self.expansion_before_prophet
            && cities.len() < EXPANSION_BEFORE_PROPHET_CITIES
            && has_site
            && self.adaptive_expansion_window_open(g)
            && self.religious_opening_viable(g, pid)
        {
            // ★★★★ The Prophet branch below assumes "the rest of the empire"
            // keeps settling while one city races; with one or two cities the
            // capital is the whole empire, and on the live Settler seat the
            // race was never close. See `expansion_before_prophet`. Only the
            // Religion outcome is displaced: every other branch runs as before.
            (
                GrandStrategy::Expansion,
                "the third city comes before the Prophet",
            )
        } else if self.religious_opening_viable(g, pid) {
            // A Prophet is a finite global race, not an economic goal that can
            // wait until the generic city target is complete. Religious
            // production only occupies one city, so the baseline governor can
            // continue settlers and development in the rest of the empire.
            // Keep that commitment independent of the generic progress race:
            // improving a contender's science readiness must not make it
            // abandon a nearly earned, globally limited Prophet.
            (
                GrandStrategy::Religion,
                "a Prophet is a finite race worth entering now",
            )
        } else if self.victory_planning
            && !specialization_active
            && cities.len() < desired_cities
            && has_site
            && self.adaptive_expansion_window_open(g)
        {
            (
                GrandStrategy::Expansion,
                "the first half is reserved for expansion and defense",
            )
        } else if victory.progress >= 65 {
            (victory.strategy, "already well down its best victory lane")
        } else if !self.victory_planning
            && cities.len() < desired_cities
            && has_site
            && self.adaptive_expansion_window_open(g)
        {
            (
                GrandStrategy::Expansion,
                "short of cities with land still open",
            )
        } else {
            (victory.strategy, "its best available victory lane")
        };
        think!(self.journal(), Strategy, Strategy,
               "Grand strategy: {}", strategy.as_str();
               "{because} — {} cities of {desired_cities} wanted, power {my_power:.0} \
                against the strongest rival's {strongest_rival:.0}; \
                best lane {} at {}% progress",
               cities.len(), victory.strategy.as_str(), victory.progress);
        // Name the rival a counter answers: "countering a rival close to
        // winning" alone left live King civvis-20261005T143823Z (game 136)
        // unreadable -- 40 turns of the counter from turn 132 while the army
        // stayed on Canada, and the Netherlands won on Culture at 179.
        if let Some((rival, _)) = actionable_denial.filter(|_| because.starts_with("countering")) {
            let culture = self.rival_culture_pressures(g);
            let pressure =
                self.rival_victory_pressure_with_culture(g, rival, culture.get(&rival).copied());
            // A culture race also says how many turns it has left
            // (`projected_culture_finish`): 073241Z read "its culture race
            // reads 96%" for 16 turns before Egypt won at 190, and the
            // percentage alone never said whether the army had time.
            let finish = denial_nearest_finish::countering_finish_note(
                pressure.strategy,
                (pressure.strategy == GrandStrategy::Culture)
                    .then(|| self.projected_culture_finish(g, rival))
                    .flatten(),
            );
            think!(self.journal(), Strategy, Strategy,
                   "Countering {}", g.players[rival].civ;
                   "its {} race reads {}%{finish}", pressure.strategy.as_str(), pressure.progress);
        }
        if let Some(city) = threatened_city.and_then(|id| g.cities.get(&id)) {
            think!(self.journal(), Strategy, Strategy,
                   "{} is under threat", city.name;
                   "the plan is written around defending it"; city.pos);
        }

        // Finish wars already in progress before selecting the next major
        // rival. In particular, this gives hostile city-states an explicit
        // city objective that the force-group planner can actually consume.
        let forced_target = self.forced_target_player.filter(|target| {
            g.players
                .get(*target)
                .map(|player| player.alive)
                .unwrap_or(false)
                && self.campaign_target_legal(g, pid, *target)
        });
        // `faith-war-is-no-front`: a running faith war's rival is not a front
        // the campaign target is chosen among. See
        // `advanced/faith_war_is_no_front.rs`.
        let wartime_rivals = self.without_faith_war(g, pid, wartime_rivals);
        let wartime_majors = self.without_faith_war(g, pid, wartime_majors);
        // `city-state-war-is-no-front`: see
        // `advanced/city_state_war_is_no_front.rs`.
        let wartime_rivals = self.without_city_state_war(g, wartime_rivals);
        let finishing_front = domination_finish.filter(|(rival, _)| {
            strategy == GrandStrategy::Conquest
                && rush_victim.is_none()
                && forced_target.is_none()
                && (!self.one_war_at_a_time
                    || wartime_rivals.is_empty()
                    || wartime_rivals.contains(rival)
                    // `flipped-capital-finishes`: the barbarian-flagged Free
                    // Cities seat is never in `wartime_rivals`, yet always at
                    // war with us.
                    || (self.flipped_capital_finishes && g.players[*rival].is_free_city))
        });
        let domination_capital = self.domination_capital_target(g, pid);
        // A Domination plan that names a distant, weak empire cannot start
        // its war while a nearer rival has a city inside the declaration
        // range. Keep the army on an actionable frontier until it can open
        // that front. Forced targets, urgent denial, and an active war retain
        // their precedence below.
        let domination_frontier_target = if strategy == GrandStrategy::Conquest
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && wartime_rivals.is_empty()
        {
            major_rivals
                .iter()
                .copied()
                .filter(|rival| self.campaign_target_legal(g, pid, *rival))
                .filter(|rival| self.war_policy_target_feasible(g, pid, *rival))
                .filter(|rival| {
                    g.player_city_ids(*rival)
                        .iter()
                        .any(|city| Self::city_within_declaration_range(g, pid, g.cities[city].pos))
                })
                .min_by(|left, right| {
                    // `domination-targets-land-first`: a rival we can march
                    // to before one across water. See
                    // `advanced/domination_land_first.rs`.
                    self.domination_land_rank(g, pid, *left)
                        .cmp(&self.domination_land_rank(g, pid, *right))
                        .then_with(|| {
                            self.campaign_target_value_with_culture(
                                g,
                                pid,
                                *left,
                                rival_culture_pressures.get(left).copied(),
                            )
                            .total_cmp(
                                &self.campaign_target_value_with_culture(
                                    g,
                                    pid,
                                    *right,
                                    rival_culture_pressures.get(right).copied(),
                                ),
                            )
                        })
                        .then(left.cmp(right))
                })
        } else {
            None
        };
        // `science-leader-is-the-target`: `None` with the gene off or at war.
        let science_leader = self.science_leader_at_peace(g, pid);
        let target_player = if let Some(emergency) = &emergency_objective {
            Some(emergency.target)
        } else if let Some((rival, _)) = finishing_front {
            Some(rival)
        } else if let Some(prey) = (wartime_rivals.is_empty()
            && rush_victim.is_none()
            && self.forced_target_player.is_none()
            && self.capital_prey_opens_a_front_2)
            .then(|| self.capital_prey_beside_the_front(g, pid, None).0)
            .flatten()
        {
            // `capital-prey-opens-a-front-2`: at peace too, a collapsed
            // rival's original capital in reach is the campaign's first front.
            Some(prey)
        } else if wartime_rivals.is_empty() {
            // The rush already chose, on nearness and weakness, and the
            // generic value sort would happily re-aim the column at a richer
            // rival two weeks' march away.
            rush_victim.map(|(target, _)| target).or_else(|| {
                forced_target.or_else(|| {
                    // See `diplomatic_contender_at_peace`: ahead of the
                    // denial counter's own pick.
                    self.diplomatic_contender_at_peace(g, pid)
                        .or_else(|| {
                            actionable_denial
                                .filter(|(rival, _)| self.campaign_target_legal(g, pid, *rival))
                                // `counter-out-of-reach-takes-the-weak`: see
                                // `counter_out_of_reach`.
                                .filter(|(rival, _)| !self.counter_out_of_reach(g, pid, *rival))
                                .map(|(rival, _)| rival)
                        })
                        // See `road_blocker_front` (`blocker-becomes-the-target`).
                        .or_else(|| self.road_blocker_front(g, pid))
                        // See `science_leader_at_peace`
                        // (`science-leader-is-the-target`): behind the
                        // counter, ahead of every elective pick below.
                        .or(science_leader)
                        // `war-target-prices-the-walls`: behind every counter,
                        // contender and the Science leader, the elective
                        // capital pick below yields to a capital our fielded
                        // guns take sooner. See `advanced/war_target_walls.rs`.
                        .or_else(|| {
                            if !self.war_target_prices_the_walls {
                                return None;
                            }
                            let default = self
                                .domination_followup_target(g, pid, None)
                                .or(domination_frontier_target)
                                .or_else(|| domination_capital.map(|(owner, _)| owner));
                            self.wall_priced_war_target(g, pid, default)
                        })
                        // A secured capital advances the Domination campaign
                        // to another capital owner, even when its frontier
                        // must be taken first. Otherwise prefer an eligible
                        // capital before optional economic conquests.
                        .or_else(|| self.domination_followup_target(g, pid, None))
                        .or(domination_frontier_target)
                        .or_else(|| domination_capital.map(|(owner, _)| owner))
                        // `city_campaign`: the plan's rival before the generic
                        // value sort. See `advanced/city_campaign.rs`.
                        // `war-policy-via-board`: a rival whose nearest
                        // city's Siege bill is over the whole roster is no
                        // target (every rival passes with the gene off). See
                        // `advanced/war_policy.rs`.
                        .or_else(|| {
                            self.campaign_target(g, pid)
                                .filter(|rival| self.war_policy_target_feasible(g, pid, *rival))
                        })
                        .or_else(|| {
                            let mut candidates: Vec<_> = major_rivals
                                .iter()
                                .copied()
                                .filter(|rival| self.campaign_target_legal(g, pid, *rival))
                                .filter(|rival| self.war_policy_target_feasible(g, pid, *rival))
                                .collect();
                            if strategy == GrandStrategy::Conquest
                                && self.conquest_campaign_considers_city_states(g)
                            {
                                candidates.extend(
                                    g.players
                                        .iter()
                                        .filter(|player| player.is_minor)
                                        .filter(|player| {
                                            self.campaign_target_legal(g, pid, player.id)
                                                && self
                                                    .war_policy_target_feasible(g, pid, player.id)
                                        })
                                        .map(|player| player.id),
                                );
                            }
                            candidates
                                .into_iter()
                                .map(|rival| {
                                    (
                                        rival,
                                        self.campaign_target_value_with_culture(
                                            g,
                                            pid,
                                            rival,
                                            rival_culture_pressures.get(&rival).copied(),
                                        ),
                                    )
                                })
                                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap().then(a.0.cmp(&b.0)))
                                .map(|(rival, _)| rival)
                        })
                })
            })
        } else {
            // A suzerained city-state joins its major's war, but it is not a
            // reason to abandon the weaker major city the campaign was staged
            // to capture. Only make a city-state the primary front when there
            // is no living major war left to finish.
            let active_fronts = if wartime_majors.is_empty() {
                &wartime_rivals
            } else {
                &wartime_majors
            };
            // `one_war_at_a_time`: the front already chosen stays the plan's
            // target while it is at war with us, so the army and the peace
            // desk agree on which war this is. See `advanced/one_war.rs`.
            // A second front the Domination plan must open (the next
            // capital, an urgent clock) takes the plan's target first.
            // See `domination_finish_front`: the war that ends the game first,
            // under any grand strategy.
            self.domination_finish_front(g, pid)
                .map(|(owner, _)| owner)
                .filter(|owner| active_fronts.contains(owner))
                // See `diplomatic_contender_to_eliminate`: ahead of a second
                // front and of the capital hop.
                .or_else(|| {
                    self.diplomatic_contender_to_eliminate(g, pid)
                        .filter(|rival| active_fronts.contains(rival))
                })
                // `denial-target-leads-the-fronts`: the countered rival we are
                // already fighting, ahead of the second front and the running
                // front. See `advanced/denial_target_front.rs`.
                .or_else(|| self.denial_target_front(actionable_denial, active_fronts))
                .or_else(|| {
                    self.one_war_second_front(g, pid)
                        // See `second_front_waits_for_the_front` and
                        // `second_front_waits_for_its_war`.
                        .filter(|rival| {
                            !self.second_front_waits_for_the_front(g, pid, *rival)
                                && !self.second_front_waits_for_its_war(g, pid, *rival)
                        })
                })
                .or_else(|| {
                    self.one_war_front()
                        .filter(|front| active_fronts.contains(front))
                })
                // `city_campaign`: the war the plan opened stays the front
                // while it is being fought. See `advanced/city_campaign.rs`.
                .or_else(|| {
                    self.campaign_target(g, pid)
                        .filter(|front| active_fronts.contains(front))
                })
                .or_else(|| {
                    active_fronts
                        .iter()
                        .copied()
                        .map(|rival| {
                            (
                                rival,
                                self.rival_value_with_culture(
                                    g,
                                    pid,
                                    rival,
                                    rival_culture_pressures.get(&rival).copied(),
                                ),
                            )
                        })
                        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap().then(a.0.cmp(&b.0)))
                        .map(|(rival, _)| rival)
                })
        };
        // A Domination army needs a first foothold it can reach in this war,
        // even when an urgent religious counter selected a founder's distant
        // Holy Site. The direct infrastructure target remains first for
        // Science and Culture, and emergency defence and a rush outrank this
        // choice below.
        let short_domination_city =
            if matches!(strategy, GrandStrategy::Conquest | GrandStrategy::Expansion)
                && self.active_victory_target(g) == Some(VictoryTarget::Domination)
                && wartime_rivals.is_empty()
            {
                target_player
                    .filter(|target| !g.players[*target].is_minor)
                    .and_then(|target| {
                        let short_capital = self
                            .domination_capital_target_for(g, pid, Some(target))
                            .map(|(_, city)| city)
                            .filter(|city| {
                                Self::city_within_first_capture_march(g, pid, g.cities[city].pos)
                                    || Self::finishing_capital_in_reach(g, pid, target, *city)
                            });
                        // A capital at the edge of the first march is not the
                        // shortest first capture when it has walls and an open
                        // border town lies between us and it. Taking that town
                        // moves the siege base forward without abandoning the
                        // capital as the campaign's follow-up objective.
                        let open_foothold = short_capital.and_then(|capital| {
                            let capital_pos = g.cities[&capital].pos;
                            let capital_distance = g
                                .player_city_ids(pid)
                                .iter()
                                .map(|ours| g.wdist(g.cities[ours].pos, capital_pos))
                                .min()
                                .unwrap_or(i32::MAX);
                            if g.cities[&capital].wall_hp <= 0 {
                                return None;
                            }
                            g.cities
                                .values()
                                .filter(|city| {
                                    city.owner == target
                                        && city.id != capital
                                        && city.wall_hp <= 0
                                        && g.wdist(city.pos, capital_pos)
                                            <= city_campaign::CAMPAIGN_HOP
                                })
                                .filter(|city| {
                                    g.player_city_ids(pid).iter().any(|ours| {
                                        let distance = g.wdist(g.cities[ours].pos, city.pos);
                                        distance <= DOMINATION_FIRST_CAPTURE_MARCH
                                            && distance.saturating_add(2) <= capital_distance
                                    })
                                })
                                .min_by(|left, right| {
                                    self.campaign_city_value(g, pid, left, GrandStrategy::Conquest)
                                        .total_cmp(&self.campaign_city_value(
                                            g,
                                            pid,
                                            right,
                                            GrandStrategy::Conquest,
                                        ))
                                        .then(left.id.cmp(&right.id))
                                })
                                .map(|city| city.id)
                        });
                        open_foothold.or(short_capital).or_else(|| {
                            g.cities
                                .values()
                                .filter(|city| {
                                    city.owner == target
                                        && Self::city_within_first_capture_march(g, pid, city.pos)
                                })
                                .min_by(|left, right| {
                                    self.campaign_city_value(g, pid, left, GrandStrategy::Conquest)
                                        .total_cmp(&self.campaign_city_value(
                                            g,
                                            pid,
                                            right,
                                            GrandStrategy::Conquest,
                                        ))
                                        .then(left.id.cmp(&right.id))
                                })
                                .map(|city| city.id)
                        })
                    })
            } else {
                None
            };
        // See `diplomatic_contender_to_eliminate`.
        if let Some(rival) = target_player
            .filter(|rival| self.diplomatic_contender_to_eliminate(g, pid) == Some(*rival))
        {
            think!(self.journal(), Strategy, Strategy,
                   "Eliminating {}", g.players[rival].civ;
                   "{} Diplomatic Victory points that only its elimination takes off the board; {:.0} power against their {:.0}",
                   g.players[rival].dvp, g.military_power(pid), g.military_power(rival));
        }
        // See `science_leader_at_peace`.
        if let Some(rival) = science_leader.filter(|rival| target_player == Some(*rival)) {
            let (_, science, runner_up) =
                Self::science_leader_by(g, pid, 1.0).unwrap_or((rival, 0.0, 0.0));
            think!(self.journal(), Strategy, Strategy,
                   "Science leader is the target: {}", g.players[rival].civ;
                   "{science:.0} Science a turn against the next major's {runner_up:.0}; {:.0} power against their steady {:.0}",
                   g.military_power(pid), self.steady_rival_power(g, rival));
        }
        let suppression_target = actionable_denial
            .filter(|(rival, counter)| {
                *counter == GrandStrategy::Conquest && target_player == Some(*rival)
            })
            .and_then(|(rival, _)| {
                let pressure = self.rival_victory_pressure_with_culture(
                    g,
                    rival,
                    rival_culture_pressures.get(&rival).copied(),
                );
                self.victory_suppression_city(g, pid, rival, pressure)
                    .map(|city| (city, pressure.strategy))
                    .or_else(|| {
                        // A launch chain another lane outreads still has a
                        // Spaceport to take.
                        let launches = VictoryFocus {
                            strategy: GrandStrategy::Science,
                            progress: Self::science_launch_progress(g, rival),
                        };
                        (pressure.strategy != GrandStrategy::Science)
                            .then(|| self.victory_suppression_city(g, pid, rival, launches))
                            .flatten()
                            .map(|city| (city, GrandStrategy::Science))
                    })
            });
        let suppression_target_city = suppression_target.map(|(city, _)| city);
        let ranked_target_city = emergency_objective
            .as_ref()
            .map(|emergency| emergency.city)
            // The rush aims at the capital and nothing else. A capital is the
            // one city whose loss can end a small neighbour outright, it is
            // where `city_strength`'s palace +3 is paid for by having the
            // whole empire's defence in one place, and the generic
            // `campaign_city_value` sort would otherwise send the column at
            // whichever border town scored best.
            .or_else(|| {
                rush_victim
                    .filter(|(target, _)| target_player == Some(*target))
                    .map(|(_, capital)| capital)
            })
            // The capital that ends the game, against an owner we are already
            // fighting, is the objective. See `domination_finish_at_war`.
            .or_else(|| {
                domination_finish
                    .filter(|(rival, _)| target_player == Some(*rival) && g.is_at_war(pid, *rival))
                    .map(|(_, capital)| capital)
            })
            // See `elimination_objective_city`: elimination takes every city,
            // so the nearest comes first, not the capital.
            .or_else(|| {
                self.diplomatic_contender_to_eliminate(g, pid)
                    .filter(|rival| target_player == Some(*rival))
                    .and_then(|rival| self.elimination_objective_city(g, pid, rival))
            })
            // `racer-blocker-declares`: the blocker's city on the land road
            // to the racer, not its far capital. `None` with the gene off.
            // See `advanced/silent_counter.rs`.
            .or_else(|| {
                self.racer_blocker_objective(g, pid)
                    .filter(|(blocker, _)| target_player == Some(*blocker))
                    .map(|(_, city)| city)
            })
            .or_else(|| {
                if matches!(suppression_target, Some((_, GrandStrategy::Religion))) {
                    suppression_target_city
                        .filter(|city| {
                            Self::city_within_first_capture_march(g, pid, g.cities[city].pos)
                        })
                        .or(short_domination_city)
                        .or(suppression_target_city)
                } else {
                    suppression_target_city.or(short_domination_city)
                }
            })
            // The nearest usable objective of the selected frontier is the
            // first capture. Prefer its capital when it is a short march.
            // Otherwise take a nearby border city before crossing the whole
            // declaration range to the capital; the first capture supplies
            // a forward base for the next siege.
            .or_else(|| {
                domination_frontier_target
                    .filter(|frontier| Some(*frontier) == target_player)
                    .and_then(|frontier| {
                        let capital = self
                            .domination_capital_target_for(g, pid, Some(frontier))
                            .filter(|(_, city)| {
                                Self::city_within_declaration_range(g, pid, g.cities[city].pos)
                            })
                            .map(|(_, city)| city);
                        // `strategic-deposit-prey`: a city holding the strategic
                        // deposit the army lacks is worth the longer march; the
                        // campaign value still prices its distance.
                        let short_march = |city: &crate::game::City| {
                            Self::city_within_first_capture_march(g, pid, city.pos)
                                || self.strategic_deposit_prey_value(
                                    g,
                                    pid,
                                    city,
                                    GrandStrategy::Conquest,
                                ) > 0.0
                        };
                        let best = |nearby_only: bool| {
                            g.cities
                                .values()
                                .filter(|city| {
                                    city.owner == frontier
                                        && Self::city_within_declaration_range(g, pid, city.pos)
                                        && (!nearby_only || short_march(city))
                                })
                                .min_by(|left, right| {
                                    self.campaign_city_value(g, pid, left, GrandStrategy::Conquest)
                                        .total_cmp(&self.campaign_city_value(
                                            g,
                                            pid,
                                            right,
                                            GrandStrategy::Conquest,
                                        ))
                                        .then(left.id.cmp(&right.id))
                                })
                                .map(|city| city.id)
                        };
                        capital
                            .filter(|city| short_march(&g.cities[city]))
                            .or_else(|| best(true))
                            .or(capital)
                            .or_else(|| best(false))
                    })
            })
            .or_else(|| self.conversion_campaign_target(g, pid, target_player))
            // `city_campaign`: the plan's first city still in the rival's
            // hands. See `advanced/city_campaign.rs`.
            .or_else(|| self.campaign_objective_city(g, pid, target_player))
            .or_else(|| {
                let capital = if self.domination_capital_focus {
                    target_player
                        .and_then(|target| self.domination_capital_target_for(g, pid, Some(target)))
                } else {
                    domination_capital
                };
                capital
                    .filter(|(target, _)| target_player == Some(*target))
                    .map(|(_, capital)| capital)
            })
            .or_else(|| {
                target_player.and_then(|target| {
                    g.cities
                        .values()
                        .filter(|c| c.owner == target)
                        .min_by(|left, right| {
                            self.campaign_city_value(g, pid, left, strategy)
                                .total_cmp(&self.campaign_city_value(g, pid, right, strategy))
                                .then_with(|| left.id.cmp(&right.id))
                        })
                        .map(|c| c.id)
                })
            });

        // A campaign objective is a commitment, not a continuously refreshed
        // preference. The city scorer below rightly ranks a fresh objective
        // when a war opens and after a capture, but its distance and fog terms
        // also change while an army is marching. Re-ranking that city every
        // five turns made a superior force walk Batumi -> Tbilisi -> Kutaisi
        // without firing enough shots to take any of them. Keep the existing
        // foreign objective through an active major war while it is still
        // enemy-owned. A home emergency deliberately remains a hard override;
        // a city-state is excluded because its diplomacy has its own reclaim
        // path rather than a major-war capture campaign.
        let committed_target_city = if self.siege_commitment && emergency_objective.is_none() {
            target_player
                .filter(|target| {
                    g.is_at_war(pid, *target)
                        && g.players
                            .get(*target)
                            .is_some_and(|player| !player.is_minor && !player.is_barbarian)
                })
                .and_then(|target| {
                    self.plan
                        .as_ref()
                        .filter(|prior| prior.target_player == Some(target))
                        .and_then(|prior| prior.target_city)
                        .filter(|city| g.cities.get(city).is_some_and(|city| city.owner == target))
                        // A failed capture's explicit stand-down outranks a
                        // cached siege objective until the cooldown expires.
                        .filter(|city| !self.capture_stood_down_holds(g, *city))
                })
        } else {
            None
        };
        // A live column should not march away from a city it has nearly
        // opened. The converse matters too: on the King Babylon front the
        // army had stripped an unwalled Karkar to 172 HP with a man-at-arms
        // two tiles away, yet its old order still pointed to fully walled
        // Malgium. On the King Japan front Kyoto finished Walls while Nagoya
        // was still open with a healthy chariot three tiles away. An open
        // foothold can close before it takes damage. Permit one narrow
        // change of objective while the old siege has not breached anything
        // and a land taker can reach a visibly weaker, unwalled city. Once
        // selected, the ordinary commitment keeps that foothold as its goal.
        let capture_opportunity_city = committed_target_city
            .and_then(|prior_id| self.capture_opportunity_city(g, pid, prior_id, strategy));
        let stale_walled_city = committed_target_city
            .filter(|_| capture_opportunity_city.is_none())
            .and_then(|prior_id| self.stale_domination_objective_city(g, pid, prior_id, strategy));
        // `siege-moves-under-the-wing`: a walled objective our land guns
        // cannot breach, beyond the Bombers' reach, yields to a city of the
        // same rival the wing can hit. `None` with the gene off. See
        // `advanced/siege_under_the_wing.rs`.
        let wing_city = committed_target_city
            .or(ranked_target_city)
            .filter(|_| capture_opportunity_city.is_none() && stale_walled_city.is_none())
            .and_then(|prior_id| {
                self.siege_under_the_wing_city(g, pid, prior_id, strategy)
                    .map(|city| (prior_id, city))
            });
        // `capture-go-or-stand-down`: a city the ledger stood down is not
        // ranked again until the stand-down expires; the next-best city of the
        // same rival takes its place. A home emergency is never stood down.
        let ranked_target_city = if self.capture_stand_down_on() && emergency_objective.is_none() {
            ranked_target_city
                .filter(|city| !self.capture_stood_down_holds(g, *city))
                .or_else(|| {
                    target_player.and_then(|target| {
                        g.cities
                            .values()
                            .filter(|c| {
                                c.owner == target && !self.capture_stood_down_holds(g, c.id)
                            })
                            .min_by(|left, right| {
                                self.campaign_city_value(g, pid, left, strategy)
                                    .total_cmp(&self.campaign_city_value(g, pid, right, strategy))
                                    .then_with(|| left.id.cmp(&right.id))
                            })
                            .map(|c| c.id)
                    })
                })
        } else {
            ranked_target_city
        };
        // See `domination_finish_front`: the city that completes Domination
        // outranks a committed objective the siege has not yet damaged (a
        // damaged one still finishes) and every swap. Game 135 stayed
        // "committed" to Ratumacos, Divodurum and Córdoba from turn 193 to 211
        // beside Madrid, the last capital it needed.
        let finish_or_racer = self
            .domination_finish_front(g, pid)
            // `launched-racer-yields-the-capital`: the launched racer's
            // finishing capital is the objective at peace too. `None` with
            // the gene off. See `advanced/launched_racer.rs`.
            .or_else(|| self.launched_racer_finish(g, pid))
            .filter(|(owner, _)| target_player == Some(*owner))
            .map(|(_, capital)| capital);
        // `the-last-two-capitals-come-first`: with two or fewer capitals
        // left, the target's own is the objective. `None` with the gene off.
        // See `advanced/last_two_capitals.rs`.
        let last_two_capital = if finish_or_racer.is_none() {
            self.last_two_capitals_objective(g, pid, target_player)
        } else {
            None
        };
        let finishing_capital = finish_or_racer.or(last_two_capital).filter(|capital| {
            committed_target_city.is_none_or(|committed| {
                committed == *capital
                    || g.cities
                        .get(&committed)
                        .is_none_or(|city| city.hp >= CITY_MAX_HP)
            })
        });
        let target_city = finishing_capital
            .or(capture_opportunity_city)
            .or(stale_walled_city)
            .or(wing_city.map(|(_, city)| city))
            .or(committed_target_city)
            .or(ranked_target_city);
        // A final war aims at the nearest rival city, whichever race the
        // ranking above would still be countering. See `last_civ_standing`.
        let (target_player, target_city) = match self.final_war_objective(g, pid) {
            Some((rival, city)) => (Some(rival), Some(city)),
            None => (target_player, target_city),
        };

        if let Some(capital) =
            finishing_capital.filter(|capital| committed_target_city != Some(*capital))
        {
            let city = &g.cities[&capital];
            if last_two_capital == Some(capital) {
                think!(self.journal(), Strategy, Strategy,
                       "Campaign holds on {}", city.name;
                       "the-last-two-capitals-come-first: one of the last {} capitals Domination needs; no committed siege, swap or second front outranks it",
                       last_two_capitals::LAST_CAPITALS;
                       city.pos);
            } else {
                think!(self.journal(), Strategy, Strategy,
                       "Campaign holds on {}", city.name;
                       "its capture completes Domination; no committed siege, swap or second front outranks it";
                       city.pos);
            }
        }
        if let Some(committed_city) = committed_target_city.filter(|city| {
            finishing_capital.is_none()
                && capture_opportunity_city.is_none()
                && stale_walled_city.is_none()
                && Some(*city) != ranked_target_city
        }) {
            let city = &g.cities[&committed_city];
            think!(self.journal(), Strategy, Strategy,
                   "Campaign remains aimed at {}", city.name;
                   "the major-war army stays committed until it captures the city or its war target changes";
                   city.pos);
        }
        if let Some(opportunity) = capture_opportunity_city {
            if let (true, Some(prior)) = (self.open_city_draws_the_front, committed_target_city) {
                // See `advanced/open_city_front.rs`.
                self.journal_open_city_turn(g, pid, prior, opportunity);
            } else {
                let city = &g.cities[&opportunity];
                think!(self.journal(), Strategy, Strategy,
                       "Campaign seizes open foothold {}", city.name;
                       "an unbreached prior objective yields to a weaker unwalled city with a land taker nearby";
                       city.pos);
            }
        }
        if let Some(nearer) = stale_walled_city {
            let city = &g.cities[&nearer];
            think!(self.journal(), Strategy, Strategy,
                   "Campaign abandons a distant untouched city for {}", city.name;
                   "a land force is within a short march of this cheaper objective, while none reached the old siege";
                   city.pos);
        }
        if let Some((prior, under)) = wing_city.filter(|_| finishing_capital.is_none()) {
            let (prior, city) = (&g.cities[&prior], &g.cities[&under]);
            think!(self.journal(), Strategy, Strategy,
                   "Campaign moves under the wing to {}", city.name;
                   "siege-moves-under-the-wing: {}'s {} walls are beyond our land guns and out of the Bombers' reach; {} is within it",
                   prior.name, prior.wall_hp, city.name;
                   city.pos);
        }

        if self.journal().wants(crate::reasoning::Level::Strategy) {
            match target_player.and_then(|id| g.players.get(id)) {
                Some(rival) => {
                    let objective = target_city
                        .and_then(|id| g.cities.get(&id))
                        .map(|city| format!("first objective {}", city.name))
                        .unwrap_or_else(|| "no city objective yet".to_string());
                    let at_war_now = if g.is_at_war(pid, rival.id) {
                        "already at war"
                    } else if wartime_rivals.is_empty() {
                        "not yet at war"
                    } else {
                        "a second front beside the wars already running"
                    };
                    think!(self.journal(), Strategy, Strategy,
                           "Campaign aimed at {}", rival.civ;
                           "{at_war_now}; {objective}");
                }
                None => {
                    think!(self.journal(), Strategy, Strategy, "No campaign target";
                           "no rival is both reachable and worth opening against");
                }
            }
        }

        StrategicPlan {
            strategy,
            target_player,
            target_city,
            threatened_city,
            desired_cities,
            assessed_turn: g.turn,
            // Only a plan that actually aims at the victim is a rush. If
            // something later in target selection re-aimed the campaign, the
            // production bonus must not follow it.
            rush: rush_victim.is_some_and(|(victim, _)| target_player == Some(victim)),
        }
    }

    /// Whether a settler built *here, now* would still pay for itself.
    ///
    /// `expansion_window_open` reserves a flat `standard_duration(50)` at the
    /// end of every game for every city, whatever that city can actually do.
    /// A city making 20 production a turn builds a settler in four turns; one
    /// making three takes twenty-seven. A single reserve is wrong for both, and
    /// `expansion_funnel_blocker_census` measures the cost of being wrong in
    /// the strict direction: on the recorded 6p/74x46 profile, the
    /// shut window is the **sole** blocker on 310 of 993 city-turns spent short
    /// of the empire's own city target — 31.2% of them.
    ///
    /// This asks the question the reserve was standing in for. Time enough to
    /// build the settler at this city's real production rate, walk it out, and
    /// then hold the ground long enough to return more than it cost.
    /// The whole board's passable land, extrapolated from the share of the
    /// KNOWN plots that are passable land. Never below `counted_land`, and
    /// equal to it on a board with no unknown terrain. See `fog_land_capacity`.
    pub(crate) fn fog_land_estimate(g: &Game, counted_land: usize) -> usize {
        let mut known = 0usize;
        let mut known_land = 0usize;
        for tile in g.map.tiles.values() {
            if g.rules.is_unknown(tile) {
                continue;
            }
            known += 1;
            if g.rules.is_passable(tile) && !g.rules.is_water(tile) {
                known_land += 1;
            }
        }
        if known == 0 {
            return counted_land;
        }
        let total = g.map.tiles.len();
        (known_land.saturating_mul(total) / known).max(counted_land)
    }

    /// The ordinary war-opening gate uses this distance from a home city to
    /// its first objective. Target selection must use the same gate or it can
    /// keep naming a rival that diplomacy cannot attack.
    pub(super) fn city_within_declaration_range(g: &Game, pid: usize, objective: Pos) -> bool {
        g.player_city_ids(pid)
            .iter()
            .any(|city| g.wdist(g.cities[city].pos, objective) <= 18)
    }

    /// Whether `city` is the capital whose capture completes Domination,
    /// within [`FINISHING_CAPITAL_MARCH`] of a city of ours, while we hold
    /// [`FINISHING_CAPITAL_POWER`] times `owner`'s power. A border town first
    /// is a forward base for a long war; for the last capital, against an
    /// owner already outgunned, it is only turns lost. Live King
    /// civvis-20261004T083931Z (game 50) held two of three capitals at turn
    /// 201 and declared on Hungary at 218, at 3,889 power against 1,563, with
    /// Eger, four tiles from Porto, as its first objective; Buda, the last
    /// capital, stood nine tiles out.
    pub(super) fn finishing_capital_in_reach(
        g: &Game,
        pid: usize,
        owner: usize,
        city: u32,
    ) -> bool {
        let Some(capital) = g.cities.get(&city) else {
            return false;
        };
        Self::capture_completes_domination(g, pid, city)
            && g.military_power(pid) >= FINISHING_CAPITAL_POWER * g.military_power(owner).max(1.0)
            && g.player_city_ids(pid)
                .iter()
                .any(|ours| g.wdist(g.cities[ours].pos, capital.pos) <= FINISHING_CAPITAL_MARCH)
    }

    fn city_within_first_capture_march(g: &Game, pid: usize, objective: Pos) -> bool {
        g.player_city_ids(pid)
            .iter()
            .any(|city| g.wdist(g.cities[city].pos, objective) <= DOMINATION_FIRST_CAPTURE_MARCH)
    }

    /// Lower is a better operational objective. Unlike a nearest-city rule,
    /// this combines approach geometry, live defenses, staged forces,
    /// occupation pressure, development, and victory-denial value. It is the
    /// campaign analogue of a chess engine's move ordering: forces search the
    /// most forcing and profitable front first rather than the first legal one.
    /// Whether the Conquest campaign's fallback ranking may name a city-state.
    /// Always, except for an assigned Domination seat with
    /// `domination-ignores-city-states` on: its victory counts foreign
    /// original capitals, and a city-state war it opens becomes the plan's
    /// front (`wartime_rivals`) until it ends, so the army that should be
    /// marching on a capital besieges a city worth nothing to the lane.
    pub(super) fn conquest_campaign_considers_city_states(&self, g: &Game) -> bool {
        !(self.domination_ignores_city_states
            && self.active_victory_target(g) == Some(VictoryTarget::Domination))
    }

    /// The map-room census is allowed to count the scanner's emergency-grade
    /// sites for a shared production profile. The ordinary Settler picker
    /// still ranks normal sites first and reaches these only after those sites
    /// have gone, so this raises the long-term plan without making a marginal
    /// site outrank a good nearby one.
    #[allow(clippy::too_many_arguments)]
    fn settle_sites_for_room(
        &self,
        g: &Game,
        pid: usize,
        from: Pos,
        radius: i32,
        prefilter_limit: Option<usize>,
        include_emergency: bool,
        score_cache: &mut BTreeMap<Pos, f64>,
        shared: &mut SettlementScanShared,
    ) -> Vec<(Pos, f64)> {
        self.settle_sites_scanning_shared(
            g,
            pid,
            from,
            radius,
            prefilter_limit,
            Some(score_cache),
            false,
            include_emergency,
            Some(shared),
        )
    }

    /// How many more cities the map can actually seat, counted rather than
    /// assumed. See `city_target_meets_the_map`.
    ///
    /// Sites are taken best-first and a site is only counted if it stands at
    /// least [`SETTLEMENT_SPACING`] from every site already counted — the
    /// same spacing the scanner's own `city_exclusion` disk imposes around a
    /// founded city, so two candidate plots three tiles apart count once, not
    /// twice. The walk stops as soon as it has counted `wanted` of them: the
    /// only question asked here is whether the target is reachable, and the
    /// scan is the expensive part of `assess`.
    pub(super) fn map_settlement_room(
        &self,
        g: &Game,
        pid: usize,
        origins: &[Pos],
        wanted: usize,
    ) -> usize {
        if wanted == 0 || origins.is_empty() {
            return 0;
        }
        let radius = if g.players[pid].techs.contains(&crate::name!("shipbuilding")) {
            g.map.width + g.map.height
        } else {
            10
        };
        let mut sites: Vec<(Pos, f64)> = Vec::new();
        // Every origin scans the same board for the same seat: the exclusion
        // disks, the prefilter scores and the site values are shared across
        // the origins, and only the disk and the distance term differ.
        let mut score_cache = BTreeMap::new();
        let mut shared = SettlementScanShared::default();
        for origin in origins {
            sites.extend(self.settle_sites_for_room(
                g,
                pid,
                *origin,
                radius,
                Some(SETTLEMENT_GLOBAL_PREFILTER_LIMIT),
                self.shared_city_target,
                &mut score_cache,
                &mut shared,
            ));
        }
        sites.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });
        let mut seated: Vec<Pos> = Vec::new();
        for (pos, _) in sites {
            if seated
                .iter()
                .all(|taken| g.wdist(*taken, pos) >= SETTLEMENT_SPACING)
            {
                seated.push(pos);
                if seated.len() >= wanted {
                    break;
                }
            }
        }
        seated.len()
    }
}
