//! The religion lane: the Prophet race, what a founder builds, and Faith
//! spending.
//!
//! Getting a religion: how this seat ranks among the contenders
//! (`religious_opening_rank_from`), whether founding is still viable
//! (`religious_opening_viable`), and when the optional secondary race is open
//! (`prophet_race_open_for`, `prophet_race_enterable_for`,
//! `skip_prophet_race_2_for`, `religion_race_closed_for`). Using one:
//! `religious_production`, the conversions counted by
//! `religious_conversion_tally`, and the field corps' posture
//! (`religious_offensive_posture`). Answering someone else's:
//! `religious_defense` and `city_needs_religious_support`. Faith itself:
//! `religious_spending_with_reserve` and `faith_building_spending`.
//!
//! The corps a founder buys is in `religion.rs`, and the soldiers Faith buys
//! are `military_faith_spending` in `advanced.rs`.
//!
//! Moved verbatim out of the `impl AdvancedAi` block in `advanced.rs`
//! (2026-11-06). Methods the rest of the controller calls are `pub(super)`; the
//! code is otherwise unchanged.

use super::*;

impl AdvancedAi {
    /// A Prophet-race contender's rank, for an empire holding at least
    /// `floor` cities ([`RELIGIOUS_OPENING_CITIES`]; only
    /// `astrology-before-the-second-city` reads our own seat below it).
    fn religious_opening_rank_from(g: &Game, pid: usize, floor: usize) -> Option<(u8, f64, f64)> {
        let player = &g.players[pid];
        if !player.alive
            || player.is_minor
            || player.is_barbarian
            || player.religion.is_some()
            || player.prophet_pending
            || g.player_city_ids(pid).len() < floor
        {
            return None;
        }
        let city_ids = g.player_city_ids(pid);
        let has_holy_site = city_ids.iter().any(|cid| {
            g.cities[cid]
                .districts
                .contains_key(crate::name!("holy_site"))
        });
        let holy_site_planned = city_ids.iter().any(|cid| {
            matches!(
                g.cities[cid].queue.first(),
                Some(Item::District { district, .. }) if district == "holy_site"
            )
        });
        let best_site = city_ids
            .iter()
            .flat_map(|cid| g.district_sites(*cid, crate::name!("holy_site")))
            .map(|pos| g.district_site_yields(crate::name!("holy_site"), pos).faith)
            .max_by(f64::total_cmp);
        if !has_holy_site && !holy_site_planned && best_site.is_none() {
            return None;
        }
        // Once an empire has paid toward the race, keep that commitment ahead
        // of an uninvested late entrant. The remaining comparisons select the
        // best available Holy Site and faith economy instead of requiring the
        // unusually rare +3 adjacency that previously left most maps with a
        // single founder.
        let commitment = if has_holy_site {
            4
        } else if holy_site_planned {
            3
        } else if player.techs.contains(&crate::name!("astrology")) {
            2
        } else if player.research.as_deref() == Some("astrology") {
            1
        } else {
            0
        };
        Some((commitment, best_site.unwrap_or(0.0), player.faith))
    }

    pub(super) fn religious_opening_viable(&self, g: &Game, pid: usize) -> bool {
        self.religious_opening_viable_from(g, pid, RELIGIOUS_OPENING_CITIES)
    }

    /// `religious_opening_viable` with this seat ranked at `own_floor` cities;
    /// every rival keeps [`RELIGIOUS_OPENING_CITIES`]. See
    /// `astrology-before-the-second-city`.
    pub(super) fn religious_opening_viable_from(
        &self,
        g: &Game,
        pid: usize,
        own_floor: usize,
    ) -> bool {
        let player = &g.players[pid];
        if player.religion.is_some() {
            return false;
        }
        if player.prophet_pending {
            return true;
        }
        let founded = g.religions_founded();
        let pending = g
            .players
            .iter()
            .filter(|candidate| candidate.prophet_pending)
            .count();
        let claimed = founded + pending;
        if claimed >= g.max_religions()
            || g.turn > if founded > 0 { 180 } else { 120 }
            || Self::religious_opening_rank_from(g, pid, own_floor).is_none()
        {
            return false;
        }

        // Prophet slots are a global race. Let exactly the best uncommitted
        // contenders pursue the slots that remain, while still allowing a
        // newly founded rival religion to trigger a genuine counter-race.
        let open_slots = g.max_religions() - claimed;
        let mut contenders: Vec<_> = g
            .players
            .iter()
            .filter_map(|candidate| {
                let floor = if candidate.id == pid {
                    own_floor
                } else {
                    RELIGIOUS_OPENING_CITIES
                };
                Self::religious_opening_rank_from(g, candidate.id, floor)
                    .map(|rank| (candidate.id, rank))
            })
            .collect();
        contenders.sort_by(|(left_id, left), (right_id, right)| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| right.1.total_cmp(&left.1))
                .then_with(|| right.2.total_cmp(&left.2))
                .then_with(|| left_id.cmp(right_id))
        });
        contenders
            .into_iter()
            .take(open_slots)
            .any(|(contender, _)| contender == pid)
    }

    pub(super) fn religious_conversion_tally(&self, g: &Game, pid: usize) -> (usize, usize) {
        let living_majors: Vec<usize> = g
            .players
            .iter()
            .filter(|player| player.alive && !player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .collect();
        // `civ_follows_religion` is the engine's `following * 2 > cities`
        // test on a native board and the host's `GetReligionInMajorityOfCities`
        // answer on a mirrored one, where a rival's unseen cities count.
        let converted = g.players[pid].religion.as_ref().map_or(0, |religion| {
            living_majors
                .iter()
                .filter(|other| g.civ_follows_religion(**other, religion.as_str()))
                .count()
        });
        (converted, living_majors.len())
    }

    /// Whether the world has closed the religion race with this empire
    /// outside it. See `religion_race_is_closed`.
    ///
    /// Kongo is exempt: Taxis pays it for everyone else's religion, so its
    /// faith economy does not depend on founding one.
    pub(super) fn religion_race_closed_for(&self, g: &Game, pid: usize) -> bool {
        self.religion_race_is_closed
            && g.religions_founded() >= g.max_religions()
            && g.players[pid].religion.is_none()
            && !g.has_ability(pid, "taxis")
    }

    pub(super) fn prophet_race_open_for(&self, g: &Game, pid: usize) -> bool {
        let player = &g.players[pid];
        if player.religion.is_some() {
            return false;
        }
        if player.prophet_pending {
            return true;
        }
        // `immortal-skips-the-prophet-race`: at Immortal and Deity a seat not
        // aiming at Religion sits the race out. See
        // `advanced/immortal_prophet_race.rs`.
        if self.high_tier_skips_the_prophet_race(g) {
            return false;
        }
        // `prophet-builds-its-site`: the host says no Prophet is left.
        if self.prophet_race_exhausted(g, pid) {
            return false;
        }
        let claimed = g.religions_founded()
            + g.players
                .iter()
                .filter(|candidate| candidate.prophet_pending)
                .count();
        claimed < g.max_religions() && g.turn * 2 <= g.max_turns.max(1)
    }

    /// Whether v2 has reached the last call for a Prophet race this empire
    /// has not yet begun. An observed rival only fills a slot when it earns
    /// Prophet points now and can reach the currently offered Prophet inside
    /// the bounded horizon; pending Prophets already consume their slots.
    pub(super) fn skip_prophet_race_2_for(
        &self,
        g: &Game,
        pid: usize,
        target: Option<VictoryTarget>,
    ) -> bool {
        let player = &g.players[pid];
        if !self.skip_the_prophet_race_2
            || target == Some(VictoryTarget::Religion)
            || g.has_ability(pid, "taxis")
            || player.religion.is_some()
            || player.prophet_pending
            || !self.prophet_race_open_for(g, pid)
        {
            return false;
        }

        // Do not cancel a race this empire has already paid to enter.
        if g.great_person_points_per_turn(pid)
            .get("prophet")
            .copied()
            .unwrap_or(0.0)
            > f64::EPSILON
        {
            return false;
        }

        let claimed = g.religions_founded()
            + g.players
                .iter()
                .filter(|candidate| candidate.prophet_pending)
                .count();
        let open_slots = g.max_religions().saturating_sub(claimed);
        if open_slots == 0 {
            return false;
        }

        let rivals_at_last_call = g
            .players
            .iter()
            .filter(|candidate| {
                candidate.id != pid
                    && candidate.alive
                    && !candidate.is_minor
                    && !candidate.is_barbarian
                    && candidate.religion.is_none()
                    && !candidate.prophet_pending
            })
            .filter(|candidate| {
                let rate = g
                    .great_person_points_per_turn(candidate.id)
                    .get("prophet")
                    .copied()
                    .unwrap_or(0.0);
                rate > f64::EPSILON
                    && (g.gp_cost(candidate.id, "prophet")
                        - candidate.gpp.get("prophet").copied().unwrap_or(0.0))
                    .max(0.0)
                        <= rate * PROPHET_RACE_LAST_CALL_TURNS
            })
            .count();
        rivals_at_last_call >= open_slots
    }

    /// Whether the optional secondary Prophet race is compatible with the
    /// seat's explicit lane. Science has a long, dead-end-free
    /// beeline and the 2026-09-01 deployment screen showed the race cutting
    /// its Science wins from 12/16 to 3/16 when it pulled that lane into
    /// Astrology, Holy Sites, and Prophet patronage.
    pub(super) fn prophet_race_enabled_for(&self, target: Option<VictoryTarget>) -> bool {
        self.enter_the_prophet_race_2 && target != Some(VictoryTarget::Science)
    }

    /// The secondary Prophet-race package has to agree on one admission gate:
    /// research, district reservation, and Prophet affinity all use this
    /// rather than reopening the race after v2 withdrew from it.
    pub(super) fn prophet_race_enterable_for(
        &self,
        g: &Game,
        pid: usize,
        target: Option<VictoryTarget>,
    ) -> bool {
        (self.prophet_race_enabled_for(target) || self.faith_veto_due(g, pid))
            && self.prophet_race_open_for(g, pid)
            && self.religious_opening_viable(g, pid)
            && !self.skip_prophet_race_2_for(g, pid, target)
    }

    pub(super) fn religious_production(&self, g: &mut Game, pid: usize) {
        let city_ids = g.player_city_ids(pid);
        let has_holy_site = city_ids.iter().any(|cid| {
            g.cities[cid]
                .districts
                .contains_key(crate::name!("holy_site"))
        });
        if !has_holy_site {
            let holy_site_planned = city_ids.iter().any(|cid| {
                matches!(
                    g.cities[cid].queue.first(),
                    Some(Item::District { district, .. }) if district == "holy_site"
                )
            });
            if holy_site_planned {
                return;
            }
            let mut best: Option<(f64, u32, Pos)> = None;
            for cid in &city_ids {
                if !g.cities[cid].queue.is_empty() {
                    continue;
                }
                for item in g.producible_items(pid, *cid) {
                    let Item::District { district, pos } = item else {
                        continue;
                    };
                    if district != "holy_site" {
                        continue;
                    }
                    let faith = g.district_site_yields(crate::name!("holy_site"), pos).faith;
                    if best
                        .map(|old| {
                            faith > old.0 || (faith == old.0 && (*cid, pos) > (old.1, old.2))
                        })
                        .unwrap_or(true)
                    {
                        best = Some((faith, *cid, pos));
                    }
                }
            }
            if let Some((_, city, pos)) = best {
                let _ = g.apply(
                    pid,
                    &Action::Produce {
                        city,
                        item: Item::District {
                            district: crate::name!("holy_site"),
                            pos,
                        },
                    },
                );
            }
            return;
        }

        let religion_unfounded = g.players[pid].religion.is_none();
        let prophet_slot_open = g.religions_founded()
            + g.players
                .iter()
                .filter(|player| player.prophet_pending)
                .count()
            < g.max_religions();
        if religion_unfounded && !g.players[pid].prophet_pending && prophet_slot_open {
            let shrine_planned = city_ids.iter().any(|cid| {
                matches!(
                    g.cities[cid].queue.first(),
                    Some(Item::Building { building }) if building == "shrine"
                )
            });
            let has_shrine = city_ids.iter().any(|cid| {
                g.cities[cid]
                    .buildings
                    .iter()
                    .any(|building| building == "shrine")
            });
            if !has_shrine && g.religions_founded() == 0 {
                if shrine_planned {
                    return;
                }
                for cid in &city_ids {
                    let item = Item::Building {
                        building: crate::name!("shrine"),
                    };
                    if g.cities[cid].queue.is_empty()
                        && g.cities[cid]
                            .districts
                            .contains_key(crate::name!("holy_site"))
                        && g.can_produce(pid, *cid, &item)
                    {
                        let _ = g.apply(pid, &Action::Produce { city: *cid, item });
                        return;
                    }
                }
                return;
            }

            let prayers = Item::Project {
                project: crate::name!("holy_site_prayers"),
            };
            let prayer_city = {
                let _memo = g.query_memo();
                city_ids
                    .iter()
                    .filter(|cid| {
                        g.cities[cid].queue.is_empty()
                            && g.cities[cid]
                                .districts
                                .contains_key(crate::name!("holy_site"))
                            && g.can_produce(pid, **cid, &prayers)
                    })
                    .max_by(|left, right| {
                        g.city_yields(**left)
                            .production
                            .total_cmp(&g.city_yields(**right).production)
                            .then_with(|| right.cmp(left))
                    })
                    .copied()
            };
            if let Some(city) = prayer_city {
                let _ = g.apply(
                    pid,
                    &Action::Produce {
                        city,
                        item: prayers,
                    },
                );
            }
            return;
        }

        for building in ["shrine", "temple"] {
            for cid in &city_ids {
                let item = Item::Building {
                    building: Name::new(building),
                };
                if g.cities[cid].queue.is_empty()
                    && g.cities[cid]
                        .districts
                        .contains_key(crate::name!("holy_site"))
                    && g.can_produce(pid, *cid, &item)
                {
                    let _ = g.apply(pid, &Action::Produce { city: *cid, item });
                    return;
                }
            }
        }
    }

    /// Home religious defense for civilizations whose grand strategy is NOT
    /// religion. Founders reuse the emergency spending path; everyone else
    /// buys Missionaries of an adopted non-threat majority faith, which the
    /// engine now assigns from the purchase city (stock rule).
    pub(super) fn religious_defense(&mut self, g: &mut Game, pid: usize, threat: &str) {
        // See `counterweight_faithless` (`counterweight-defends-the-diplomat`):
        // a Diplomatic founder whose faith holds none of our cities defends
        // with the counterweight.
        if !self.counterweight_faithless(g, pid) {
            self.religious_spending(g, pid, false);
            return;
        }
        let defenders = self.religious_defense_missionary_count(g, pid, threat);
        let veto = self.religious_veto_engaged(g, pid);
        let shipped = 2 + Self::religious_veto_extra_spreaders(veto.as_ref());
        // See `counterweight_cap` (`counterweight-spends-the-bank`).
        let cap = self.counterweight_cap(g, pid, threat, shipped);
        if defenders >= cap {
            return;
        }
        for cid in g.player_city_ids(pid) {
            let Some(majority) = g.city_religion(&g.cities[&cid]) else {
                continue;
            };
            if majority == threat
                || (self.counterweight_lane(g) && !self.counterfaith_is_safe(g, pid, majority))
            {
                continue;
            }
            if g.apply(
                pid,
                &Action::Buy {
                    city: cid,
                    unit: crate::name!("missionary"),
                    formation: 0,
                    currency: "faith".to_string(),
                },
            )
            .is_ok()
            {
                if defenders >= shipped {
                    think!(self.journal(), Faith, Decision,
                        "{} buys counterweight Missionary {} of {}", g.cities[&cid].name, defenders + 1, cap;
                        "counterweight-spends-the-bank: {} holds our majority, and {} of our cities must leave it before that majority breaks",
                        threat, self.counterweight_need_for(g, pid, threat));
                }
                return;
            }
        }
    }

    pub(super) fn city_needs_religious_support(
        g: &Game,
        pid: usize,
        city: &crate::game::City,
        religion: &str,
    ) -> bool {
        if city.owner != pid {
            return false;
        }
        let own = city.pressure.get(religion).copied().unwrap_or(0.0);
        let rival = city
            .pressure
            .iter()
            .filter(|(faith, _)| faith.as_str() != religion)
            .map(|(_, pressure)| *pressure)
            .fold(0.0_f64, f64::max);
        g.city_religion(city) != Some(religion) || (rival > 0.0 && rival * 2.0 >= own)
    }

    /// Keep a founded religion's small field corps useful after the adaptive
    /// planner changes its primary victory strategy. Previously that switch
    /// made charged Missionaries stop at home and let thousands of Faith sit
    /// idle. A large surplus may start a secondary campaign, while an active
    /// spreader keeps it moving after the initial purchase lowers the bank.
    pub(super) fn religious_offensive_posture(
        &self,
        g: &Game,
        pid: usize,
        strategy: GrandStrategy,
    ) -> bool {
        // `religion-lane-spreads-from-founding`: a Religion-target founder in
        // any plan. See `advanced/religion_lane_spreads.rs`.
        if strategy == GrandStrategy::Religion || self.religion_lane_spreads(g, pid) {
            return true;
        }
        if !g.victory_conditions.religious {
            return false;
        }
        let Some(religion) = g.players[pid].religion.as_deref() else {
            return false;
        };
        let foreign_target = g.cities.values().any(|city| {
            city.owner != pid
                && g.players[city.owner].alive
                && !g.players[city.owner].is_minor
                && !g.players[city.owner].is_barbarian
                && !g.is_at_war(pid, city.owner)
                && g.city_religion(city) != Some(religion)
        });
        if !foreign_target {
            return false;
        }
        let active_campaign = g.units.values().any(|unit| {
            unit.owner == pid
                && unit.religion.as_deref() == Some(religion)
                && unit.charges > 0
                && g.rules.units[unit.kind].religious_spread > 0.0
        });
        active_campaign || g.players[pid].faith >= g.game_speed.scale(2_000.0)
    }

    pub(super) fn religious_spending(&mut self, g: &mut Game, pid: usize, offensive: bool) {
        self.religious_spending_with_reserve(g, pid, offensive, 80.0);
    }

    pub(super) fn religious_spending_with_reserve(
        &mut self,
        g: &mut Game,
        pid: usize,
        offensive: bool,
        ordinary_reserve: f64,
    ) {
        let Some(religion) = g.players[pid].religion.clone() else {
            return;
        };
        // See `founding_pending` (`founder-spreads-only-its-faith`).
        if self.founding_pending(g, pid) {
            think!(self.journal(), Faith, Detail,
                "Buying no religious unit this turn";
                "founder-spreads-only-its-faith: our religion was founded on this turn's board and the host has not shown it yet, so a unit bought now takes its city's old faith (G186 turn 48 bought a Hindu Missionary)");
            return;
        }
        // See `holy_city_guard_buys` (`founder-holds-the-holy-city`): the
        // Holy City's own cover comes before the Apostle save.
        if self.holy_city_guard_buys(g, pid) {
            return;
        }
        if self.prepare_defensive_inquisition(g, pid) {
            return;
        }
        let match_point_defense = self
            .victory_denial(g, pid)
            .is_some_and(|(_, counter)| counter == GrandStrategy::Religion);
        // `religious_veto_defence`: how much of a rival's religious victory
        // is already done. Every defensive lever below scales with it.
        let veto = self.religious_veto_engaged(g, pid);
        // See `founder_corps_count` (`founder-defends-its-cities`): a held
        // spreader of another faith is not one of the corps.
        let count_units = |kind: &str| self.founder_corps_count(g, pid, kind, &religion);
        let missionaries = count_units("missionary");
        let apostles = count_units("apostle");
        let gurus = count_units("guru");
        let inquisitors = count_units("inquisitor");
        let defensive_targets = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| Self::city_needs_religious_support(g, pid, &g.cities[cid], &religion))
            .count();
        let home_under_pressure = defensive_targets > 0;
        let inquisition_launched = g.players[pid]
            .counters
            .get("inquisition")
            .copied()
            .unwrap_or(0)
            > 0;
        let spread_targets = defensive_targets
            + usize::from(offensive)
                * g.cities
                    .values()
                    .filter(|city| {
                        city.owner != pid
                            && !g.is_at_war(pid, city.owner)
                            && g.city_religion(city) != Some(religion.as_str())
                    })
                    .count();
        let veto_spreaders = Self::religious_veto_extra_spreaders(veto.as_ref());
        // A small circulating corps is enough: every Missionary has several
        // spreads, and replacements can be bought as charges are consumed.
        // Scaling gently with live targets preserves a religious push without
        // allowing one faith purchase every turn to fill the map.
        let missionary_cap = if spread_targets == 0 {
            0
        } else if offensive {
            (2 + spread_targets.div_ceil(4)).min(6)
        } else {
            // `religious_defence_scales` reads the shipped constant and, when
            // it is on, answers one spreader per threatened city instead.
            // `founder-defends-its-cities` asks one per rival-held city.
            self.founder_defence_missionary_cap(
                g,
                pid,
                self.defensive_missionary_cap(
                    defensive_targets,
                    (1 + defensive_targets.div_ceil(2)).min(2),
                ),
            ) + veto_spreaders
        };
        let apostle_cap = if offensive { 2 } else { 0 };
        // `guru-heals-the-corps-2`: the defence has damaged units and no heal.
        let guru_defends = self.guru_defends_the_corps(g, pid);
        let guru_cap = usize::from((offensive && apostles > 0) || guru_defends);
        let inquisitor_cap = if home_under_pressure && inquisition_launched {
            2 + Self::religious_veto_extra_inquisitors(veto.as_ref())
        } else {
            0
        };
        // A cap nothing asks for is not a cap. The two defensive orders never
        // named the Guru, so `guru_cap` alone could not buy one; with the gene
        // on they name it last, behind the spread and the Inquisition.
        let inquisitor_corps_short =
            home_under_pressure && inquisition_launched && inquisitors < inquisitor_cap;
        let priorities: &[&str] = if inquisitor_corps_short {
            &["inquisitor", "apostle", "missionary", "guru"]
        } else if !offensive {
            if guru_defends {
                &["missionary", "inquisitor", "guru"]
            } else {
                &["missionary", "inquisitor"]
            }
        } else if apostles < 2 {
            &["apostle", "missionary", "guru"]
        } else if gurus < 1 {
            &["guru", "apostle", "missionary"]
        } else {
            &["missionary", "apostle", "guru"]
        };
        for unit in priorities {
            let cap = match *unit {
                "missionary" => missionary_cap,
                "apostle" => apostle_cap,
                "guru" => guru_cap,
                "inquisitor" => inquisitor_cap,
                _ => 0,
            };
            let current = match *unit {
                "missionary" => missionaries,
                "apostle" => apostles,
                "guru" => gurus,
                "inquisitor" => inquisitors,
                _ => 0,
            };
            if current >= cap {
                continue;
            }
            // The ordinary buffer is useful while safely building toward a
            // victory, but it must not block the last affordable defender at
            // match point or when one of our cities is already losing its
            // religious majority.
            let reserve = if match_point_defense
                || home_under_pressure
                || Self::religious_veto_spends(veto.as_ref())
            {
                0.0
            } else {
                ordinary_reserve
            };
            // See `founder_purchase_order`: the Holy City first under
            // `founder-defends-its-cities`.
            let cities = self.founder_purchase_order(g, pid);
            for cid in cities {
                // Religious units inherit the purchase city's majority.  A
                // converted Holy Site must never make the defender spend its
                // Faith strengthening the runaway rival religion.
                if g.city_religion(&g.cities[&cid]) != Some(religion.as_str())
                    // See `founder_purchase_withheld`.
                    || self.founder_purchase_withheld(g, pid, cid)
                {
                    continue;
                }
                // ⚠ Ask the engine what this costs; do not price it here.
                // The old `spec.cost * 2.0` was the *Standard-speed* rate, and
                // `item_cost` scales every price by `game_speed`. Online — the
                // speed the deployment profile and the live bridge both play —
                // is 50%, so that literal demanded twice the Faith the engine
                // would have taken and the reserve was applied on top of a
                // doubled price. Marathon is 300% and it underquoted by a
                // third, issuing purchases the engine then refused. It also
                // ignored every discount that moves this number: the founder's
                // belief, Theocracy, the Holy Site's own purchase discount, and
                // a Guru's wonder discount. `unit_purchase_cost` is the one
                // that knows all of them, and it returns None when the purchase
                // is illegal here rather than leaving that to a refused order.
                let price = if self.engine_faith_price {
                    let Some(price) = g.unit_purchase_cost(pid, cid, unit, "faith") else {
                        continue;
                    };
                    price
                } else {
                    let Some(spec) = g.rules.units.get(unit) else {
                        continue;
                    };
                    spec.cost * 2.0
                };
                if g.players[pid].faith + f64::EPSILON < price + reserve {
                    continue;
                }
                let bought = g.apply(
                    pid,
                    &Action::Buy {
                        city: cid,
                        unit: Name::new(unit),
                        formation: 0,
                        currency: "faith".to_string(),
                    },
                );
                if bought.is_ok() {
                    return;
                }
            }
        }
    }

    pub(super) fn faith_building_spending(
        &self,
        g: &mut Game,
        pid: usize,
        strategy: GrandStrategy,
    ) {
        let reserve = match strategy {
            GrandStrategy::Religion => 180.0,
            GrandStrategy::Culture if !g.national_park_sites(pid).is_empty() => {
                g.naturalist_purchase_cost(pid)
            }
            GrandStrategy::Culture if g.players[pid].civics.contains(&crate::name!("cold_war")) => {
                700.0
            }
            _ => 80.0,
        };
        let reserve = self.conversion_faith_reserve(g, pid, reserve)
            // See `advanced/culture_band_bank.rs`.
            + self.culture_band_bank(g, pid)
            // See `inquisition_faith_reserve` (`founder-funds-the-inquisition`).
            + self.inquisition_faith_reserve(g, pid)
            // See `counterweight_bank_held` (`counterweight-finishes-one-shrine`).
            + self.counterweight_bank_held(g, pid);
        // Every candidate below must clear `cost + reserve`, and a purchase
        // cost is never negative — so a bank under the reserve cannot buy
        // anything no matter what the menu holds. Skip building the menu at
        // all: `legal_purchase_actions` is the priciest question this
        // function asks (worker clones of the whole Game), and
        // `military_faith_spending` already banks-gates the same way.
        if g.players[pid].faith + f64::EPSILON < reserve {
            return;
        }
        let best = self
            .legal_purchase_actions(g, pid)
            .into_iter()
            .filter_map(|action| match &action {
                Action::BuyBuilding {
                    city,
                    building,
                    currency,
                } if currency == "faith" => {
                    let spec = &g.rules.buildings[building];
                    let cost = g.building_faith_purchase_cost(pid, *city, building)?;
                    if g.players[pid].faith + f64::EPSILON < cost + reserve {
                        return None;
                    }
                    let worship = spec.worship_belief.is_some() as i32;
                    // No defensive term: `building_faith_purchase_cost` refuses
                    // every building with `outer_defense > 0`, because
                    // Civilization VI ships no `PurchaseYield` for a city
                    // defence and sells it for no currency. A weight that can
                    // only ever apply to a candidate this filter has already
                    // dropped is a weight that reads as a policy and is not one.
                    let score = (self.yield_value(spec.yields, strategy) * 25.0) as i32
                        + (spec.housing * 35.0 + spec.amenity * 50.0) as i32
                        + spec.great_work_slots.values().sum::<i32>() * 60
                        + spec.trade_route_capacity * 100
                        + worship * 220
                        - (cost * 0.05) as i32;
                    Some((score, std::cmp::Reverse((*city, *building)), action))
                }
                _ => None,
            })
            .max_by_key(|(score, key, _)| (*score, *key));
        if let Some((_, _, action)) = best {
            let _ = g.apply(pid, &action);
        }
    }
}
