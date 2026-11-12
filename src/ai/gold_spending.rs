//! Gold spending: what the AI buys with gold, and in what order.
//!
//! `spend_gold` runs the turn's purchases; `buy_gold_military`,
//! `buy_gold_infrastructure`, `buy_gold_plot` and `buy_gold_unit` buy each
//! kind; `buy_worked_backlog_builder` buys a Builder for worked tiles that
//! are waiting on one; and `obsolete_buy` flags a unit too many eras behind
//! the world to be worth buying.
//!
//! Moved verbatim out of the `impl BasicAi` block in `ai.rs` (2026-11-05).
//! Methods the rest of `ai` calls are `pub(super)`, the same reach a private
//! method of `ai` had; the code is otherwise unchanged.

use super::*;

impl BasicAi {
    pub(super) fn buy_gold_unit(
        &self,
        g: &mut Game,
        pid: usize,
        city_ids: &[u32],
        unit: &str,
        reserve: f64,
    ) -> bool {
        let price = match g.rules.units.get(unit) {
            Some(spec) => spec.cost * 4.0,
            None => return false,
        };
        // `gold-unit-buys-read-the-host-quote`: each city's own quote is
        // tested against the reserve below instead of this list price.
        if !self.gold_unit_buys_read_the_host_quote && g.players[pid].gold + 1e-9 < price + reserve
        {
            return false;
        }
        for cid in city_ids {
            let item = Item::Unit {
                unit: Name::new(unit),
            };
            if unit == "trader"
                && self.barbarian_tactics
                && self.solvency_first_trade_slot
                && !self.safe_trade_origin_for_controller(g, pid, *cid)
            {
                continue;
            }
            // `can_produce` answers "can this city BUILD it"; a host purchase
            // refusal is a different set. Both must pass before spending gold.
            if !g.can_produce(pid, *cid, &item) || g.purchase_is_blocked(*cid, &item) {
                continue;
            }
            // `gold-unit-buys-read-the-host-quote`: the reserve holds against
            // what this city charges (live t192: a Builder listed at 200 was
            // quoted 400 and left 20 Gold under a 162 reserve).
            if self.gold_unit_buys_read_the_host_quote {
                let Some(quote) = g.unit_purchase_cost(pid, *cid, unit, "gold") else {
                    continue;
                };
                if g.players[pid].gold + 1e-9 < quote + reserve {
                    continue;
                }
            }
            if g.apply(
                pid,
                &Action::Buy {
                    city: *cid,
                    unit: Name::new(unit),
                    formation: 0,
                    currency: "gold".to_string(),
                },
            )
            .is_ok()
            {
                return true;
            }
        }
        false
    }

    pub(super) fn buy_gold_military(
        &self,
        g: &mut Game,
        pid: usize,
        city_ids: &[u32],
        reserve: f64,
        want_ranged: bool,
    ) -> bool {
        let budget = g.players[pid].gold - reserve;
        if budget <= 0.0 {
            return false;
        }
        let choose = |role: Option<bool>| -> Option<(u32, String)> {
            let mut best: Option<(f64, f64, String, u32)> = None;
            for cid in city_ids {
                for (name, spec) in &g.rules.units {
                    let matches_role = match role {
                        Some(true) => spec.has_ranged_attack(),
                        Some(false) => spec.is_melee_capable(),
                        None => spec.has_ranged_attack() || spec.is_melee_capable(),
                    };
                    if spec.class != "military"
                        || (!self.legacy_defender_purchases && spec.promotion_class == "recon")
                        || spec.domain.as_deref() == Some("sea")
                        || !matches_role
                    {
                        continue;
                    }
                    // `parity-upgrades-before-it-buys-2`: no obsolete body.
                    if self.obsolete_military_buys_barred && Self::obsolete_buy(g, name.as_str()) {
                        continue;
                    }
                    // `parity-upgrades-before-it-buys-3`: obsolete by our own
                    // technology, not by the world era's clock.
                    if self.obsolete_buys_read_our_techs
                        && Self::obsolete_by_our_techs(g, pid, name.as_str())
                    {
                        continue;
                    }
                    // Current controllers budget the actual purchase quote.
                    // The historical anchor retains its original price and
                    // predicate order, together with its old recon eligibility.
                    let price = if self.legacy_defender_purchases {
                        spec.cost * 4.0
                    } else {
                        let Some(price) = g.unit_purchase_cost(pid, *cid, name, "gold") else {
                            continue;
                        };
                        price
                    };
                    if price > budget + 1e-9
                        || !g.can_produce(pid, *cid, &Item::Unit { unit: *name })
                        || g.purchase_is_blocked(*cid, &Item::Unit { unit: *name })
                    {
                        continue;
                    }
                    let power = spec.strength.max(spec.ranged_attack_strength());
                    let replace = match &best {
                        None => true,
                        Some((bp, bc, bn, bid)) => {
                            power > *bp + 1e-9
                                || ((power - *bp).abs() < 1e-9
                                    && (price < *bc - 1e-9
                                        || ((price - *bc).abs() < 1e-9
                                            && (name.as_str(), *cid) < (bn.as_str(), *bid))))
                        }
                    };
                    if replace {
                        best = Some((power, price, name.to_string(), *cid));
                    }
                }
            }
            best.map(|(_, _, unit, city)| (city, unit))
        };
        let (city, unit) = match choose(Some(want_ranged)).or_else(|| choose(None)) {
            Some(choice) => choice,
            None => return false,
        };
        g.apply(
            pid,
            &Action::Buy {
                city,
                unit: Name::new(&unit),
                formation: 0,
                currency: "gold".to_string(),
            },
        )
        .is_ok()
    }

    /// `parity-upgrades-before-it-buys-2`: whether `unit` is
    /// [`OBSOLETE_BUY_ERAS_BEHIND`] or more eras behind the world, the
    /// `obsolete` bar of `advanced/parity_upgrades_before_it_buys.rs`. A
    /// garrison adds no hit points of its own to a city (the city falls when
    /// its own are gone and a melee unit enters), so an Ancient body bought
    /// into a city against an Industrial army is upkeep, not defence: live
    /// 095229Z bought 16 Heavy Chariots from turn 142 to 174, every one into a
    /// city with no military unit on its tile.
    pub(crate) fn obsolete_buy(g: &Game, unit: &str) -> bool {
        g.unit_era(unit) + OBSOLETE_BUY_ERAS_BEHIND <= g.world_era
    }

    /// `parity-upgrades-before-it-buys-3`: whether `unit` is
    /// [`OBSOLETE_BUY_TECH_ERAS_BEHIND`] or more eras behind the newest
    /// technology `pid` knows. The world era is no measure of the army's age:
    /// at Online speed it runs on a clock (live 2026-11-05: era 2 at turn 51,
    /// 3 at 71, 4 at 91, 5 at 111, 6 at 131, whatever anyone had researched),
    /// so [`Self::obsolete_buy`] refuses Ancient defenders from turn 51 and
    /// Renaissance ones from 111. Measured against our own technology, the
    /// Spearman of a turn-60 war is bought and the Heavy Chariot of turn 142
    /// is not.
    pub(crate) fn obsolete_by_our_techs(g: &Game, pid: usize, unit: &str) -> bool {
        let newest = g.players[pid]
            .techs
            .iter()
            .filter_map(|tech| g.rules.techs.get(tech).map(|spec| spec.era))
            .max()
            .unwrap_or(0);
        g.unit_era(unit) + OBSOLETE_BUY_TECH_ERAS_BEHIND <= newest
    }

    pub(super) fn buy_gold_infrastructure(
        &self,
        g: &mut Game,
        pid: usize,
        city_ids: &[u32],
        reserve: f64,
        at_major_war: bool,
    ) -> bool {
        if self.barb {
            return false;
        }
        let budget = g.players[pid].gold - reserve;
        if budget <= 0.0 {
            return false;
        }

        // Prefer buildings with strong immediate value per Gold while still
        // responding to each city's housing, amenity, and defensive needs.
        // Only one purchase is made per turn, keeping the action workload
        // bounded even at Lightning spectator speed.
        let mut best: Option<(f64, f64, f64, String, u32)> = None;
        for cid in city_ids {
            let city = &g.cities[cid];
            let housing_need = (city.pop as f64 + 2.0 - g.city_housing(city)).max(0.0);
            let amenity_need = (-g.city_amenity_surplus(city)).max(0) as f64;
            for (building, spec) in &g.rules.buildings {
                if spec.wonder
                    || !g.can_produce(
                        pid,
                        *cid,
                        &Item::Building {
                            building: *building,
                        },
                    )
                    || g.purchase_is_blocked(
                        *cid,
                        &Item::Building {
                            building: *building,
                        },
                    )
                {
                    continue;
                }
                let Some(price) = g.building_purchase_cost(pid, *cid, building, "gold") else {
                    continue;
                };
                if price > budget + 1e-9 {
                    continue;
                }

                let great_people = spec.great_person_points.values().sum::<f64>();
                let work_slots = spec.great_work_slots.values().sum::<i32>().max(0) as f64;
                let mut value = spec.yields.food * 34.0
                    + spec.yields.production * 48.0
                    + spec.yields.gold * 26.0
                    + spec.yields.science * 44.0
                    + spec.yields.culture * 42.0
                    + spec.yields.faith * 24.0
                    + spec.housing * (16.0 + 24.0 * housing_need)
                    + spec.amenity * (28.0 + 28.0 * amenity_need)
                    + great_people * 24.0
                    + work_slots * 30.0
                    + spec.citizen_slots.max(0) as f64 * 8.0
                    + spec.trade_route_capacity.max(0) as f64 * 90.0
                    + spec.growth_pct.max(0.0) * 2.0
                    + spec.builder_charges.max(0) as f64 * 24.0
                    + spec.unit_levels.max(0) as f64 * 18.0
                    - spec.maintenance.max(0.0) * 10.0;
                if building == "monument" {
                    value += 90.0;
                }
                if building == "granary" && housing_need > 0.0 {
                    value += 120.0;
                }
                if spec.outer_defense > 0 {
                    if at_major_war {
                        value += spec.outer_defense as f64;
                    } else {
                        value -= 80.0;
                    }
                }
                if value <= 0.0 {
                    continue;
                }
                let efficiency = value / price.max(1.0);
                let replace = match &best {
                    None => true,
                    Some((old_efficiency, old_value, old_price, old_building, old_cid)) => {
                        efficiency > *old_efficiency + 1e-9
                            || ((efficiency - *old_efficiency).abs() < 1e-9
                                && (value > *old_value + 1e-9
                                    || ((value - *old_value).abs() < 1e-9
                                        && (price < *old_price - 1e-9
                                            || ((price - *old_price).abs() < 1e-9
                                                && (building.as_str(), *cid)
                                                    < (old_building.as_str(), *old_cid))))))
                    }
                };
                if replace {
                    best = Some((efficiency, value, price, building.to_string(), *cid));
                }
            }
        }
        let Some((_, _, _, building, city)) = best else {
            return false;
        };
        g.apply(
            pid,
            &Action::BuyBuilding {
                city,
                building: Name::new(&building),
                currency: "gold".to_string(),
            },
        )
        .is_ok()
    }

    /// Annex a genuinely useful plot instead of treating every affordable
    /// border hex as equivalent. Resources, Natural Wonders, and strong raw
    /// yields can justify the immediate tempo spend; a reserve still protects
    /// unit upgrades and emergency purchases.
    pub(super) fn buy_gold_plot(&self, g: &mut Game, pid: usize, reserve: f64) -> bool {
        let bank = g.players[pid].gold;
        // A plot must leave the reserve behind (the test at the head of the
        // loop below), and no plot is free: under the reserve there is none
        // to find, so the menu is not listed.
        if bank + f64::EPSILON < reserve {
            return false;
        }
        let mut best: Option<PlotPurchaseCandidate> = None;
        // Only the plots of the menu: this used to list every unit, building
        // and district as well and drop them one by one. See
        // `Game::legal_plot_purchases`.
        for action in g.legal_plot_purchases(pid) {
            let Action::BuyPlot { city, pos, cost } = action else {
                continue;
            };
            if bank + f64::EPSILON < reserve + cost {
                continue;
            }
            let tile = &g.map.tiles[&pos];
            let resource = tile
                .resource
                .as_ref()
                .and_then(|name| g.rules.resources.get(name))
                .filter(|spec| {
                    spec.tech
                        .as_ref()
                        .is_none_or(|tech| g.players[pid].techs.contains(tech))
                        && spec
                            .civic
                            .as_ref()
                            .is_none_or(|civic| g.players[pid].civics.contains(civic))
                });
            let mut visible_tile = tile.clone();
            if tile.resource.is_some() && resource.is_none() {
                visible_tile.resource = None;
            }
            let yields = g.rules.tile_yields(&visible_tile);
            let resource = resource
                .map(|spec| match spec.class.as_str() {
                    "luxury" => 220.0,
                    "strategic" => 190.0,
                    "bonus" => 55.0,
                    _ => 0.0,
                })
                .unwrap_or(0.0);
            let wonder =
                tile.feature
                    .as_ref()
                    .and_then(|name| g.rules.features.get(name))
                    .is_some_and(|feature| feature.natural_wonder) as u8 as f64
                    * 280.0;
            let value = yields.food * 28.0
                + yields.production * 42.0
                + yields.gold * 22.0
                + yields.science * 40.0
                + yields.culture * 38.0
                + yields.faith * 26.0
                + resource
                + wonder;
            let score = value - cost * 0.75;
            if value + f64::EPSILON < cost * 1.35 || score < 35.0 {
                continue;
            }
            let candidate = (score, std::cmp::Reverse((city, pos)), action);
            if best.as_ref().is_none_or(|old| {
                candidate.0 > old.0 + 1e-9
                    || (candidate.0 - old.0).abs() < 1e-9 && candidate.1 > old.1
            }) {
                best = Some(candidate);
            }
        }
        best.is_some_and(|(_, _, action)| g.apply(pid, &action).is_ok())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn spend_gold(
        &self,
        g: &mut Game,
        pid: usize,
        city_ids: &[u32],
        settlers: usize,
        builders: usize,
        traders: usize,
        military: usize,
        melee: usize,
        ranged: usize,
    ) -> bool {
        if city_ids.is_empty() {
            return false;
        }
        let n_cities = city_ids.len();
        let at_major_war = g
            .players
            .iter()
            .any(|p| p.id != pid && p.alive && !p.is_barbarian && g.is_at_war(pid, p.id));
        let reserve = if at_major_war {
            40.0 + 10.0 * n_cities as f64
        } else {
            100.0 + 25.0 * n_cities as f64
        }
        // See `reserve_floor`: 0.0 unless the deployed controller is holding
        // an emergency defender's price back for a threatened city.
        .max(self.reserve_floor);
        let want_ranged = melee > ranged;

        // Gold defense is city-local: buying the strongest unit in a safe
        // capital does not save a frontier city whose camp is already active.
        // Cover the local body gap first, then return to the ordinary war and
        // economy order. A purchase is still reserve-gated by the helper.
        let barbarian_cities: Vec<u32> = city_ids
            .iter()
            .copied()
            .filter(|cid| self.barbarian_tactics && self.barbarian_defense_gap(g, pid, *cid) > 0)
            .collect();
        if !self.minor
            && !self.barb
            && !barbarian_cities.is_empty()
            && self.buy_gold_military(g, pid, &barbarian_cities, reserve, false)
        {
            return true;
        }

        // A threatened empire converts cash into defenders before pursuing
        // infrastructure. Two units per city is enough to react without
        // draining the treasury into an endless standing army.
        let normal_military = (self.w.mil_per_city * n_cities as f64).ceil() as usize;
        let wartime_military = normal_military.max(2 * n_cities);
        if at_major_war
            && military < wartime_military
            && self.buy_gold_military(g, pid, city_ids, reserve, want_ranged)
        {
            return true;
        }

        let desired_builders = (self.w.builder_per_city * n_cities as f64).ceil() as usize;
        if builders < desired_builders
            && Self::has_builder_work(g, pid)
            && self.buy_gold_unit(g, pid, city_ids, "builder", reserve)
        {
            return true;
        }

        if !self.minor
            && self.should_add_trader_for_controller(g, pid, traders)
            && self.buy_gold_unit(g, pid, city_ids, "trader", reserve)
        {
            return true;
        }

        if !self.minor
            && self.walking_settlers(g, pid, settlers) == 0
            && (n_cities as f64) < self.w.city_target
            && (g.turn as f64) < self.w.settler_stop_turn
            && self.has_practical_settle_site(g, pid)
            && self.buy_gold_unit(g, pid, city_ids, "settler", reserve)
        {
            return true;
        }

        if self.buy_gold_infrastructure(g, pid, city_ids, reserve, at_major_war) {
            return true;
        }

        // Plots are a surplus investment after concrete unit and building
        // gaps are filled. Keep another 200 Gold above the ordinary reserve
        // so border appetite cannot crowd out next turn's Builder or upgrade.
        if self.buy_gold_plot(g, pid, reserve + 200.0) {
            return true;
        }

        // At peace, retain a larger reserve but turn a deep surplus into a
        // modest deterrent instead of hoarding gold indefinitely.
        g.players[pid].gold >= reserve + 600.0
            && military < 2 * n_cities
            && self.buy_gold_military(g, pid, city_ids, reserve, want_ranged)
    }

    /// See `builders_cover_the_worked_backlog`: buy a Builder with Gold in the
    /// city that sells one cheapest while the backlog runs away from the
    /// charges (`WORKED_BACKLOG_RUNAWAY_RATIO`) and the treasury keeps the
    /// reserve `spend_gold` holds. One Builder at most per city.
    pub(crate) fn buy_worked_backlog_builder(
        &self,
        g: &mut Game,
        pid: usize,
        city_ids: &[u32],
        builders: usize,
    ) -> bool {
        let n_cities = city_ids.len();
        if !self.builders_cover_the_worked_backlog
            || self.minor
            || self.barb
            || n_cities < 2
            || builders >= n_cities
        {
            return false;
        }
        let backlog = Self::unimproved_worked_tiles(g, pid);
        let cover = Self::builder_charge_cover(g, pid);
        if backlog < Self::WORKED_BACKLOG_MIN_TILES
            || (backlog as f64) < f64::from(cover) * Self::WORKED_BACKLOG_RUNAWAY_RATIO
        {
            return false;
        }
        let at_major_war = g
            .players
            .iter()
            .any(|p| p.id != pid && p.alive && !p.is_barbarian && g.is_at_war(pid, p.id));
        // `spend_gold`'s reserve, unchanged.
        let reserve = if at_major_war {
            40.0 + 10.0 * n_cities as f64
        } else {
            100.0 + 25.0 * n_cities as f64
        }
        .max(self.reserve_floor);
        let builder = Item::Unit {
            unit: crate::name!("builder"),
        };
        let Some((price, city)) = city_ids
            .iter()
            .copied()
            .filter(|cid| {
                g.can_produce(pid, *cid, &builder) && !g.purchase_is_blocked(*cid, &builder)
            })
            .filter_map(|cid| {
                g.unit_purchase_cost(pid, cid, "builder", "gold")
                    .map(|price| (price, cid))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        else {
            return false;
        };
        if g.players[pid].gold + 1e-9 < price + reserve {
            return false;
        }
        let bought = g
            .apply(
                pid,
                &Action::Buy {
                    city,
                    unit: crate::name!("builder"),
                    formation: 0,
                    currency: "gold".to_string(),
                },
            )
            .is_ok();
        if bought {
            think!(self.journal, Economy, Decision,
                   "Buying a Builder in {} for the worked backlog", g.cities[&city].name;
                   "{backlog} worked tiles stand unimproved against {cover} Builder charges; \
                    {price:.0} Gold, {:.0} left above a reserve of {reserve:.0}",
                   g.players[pid].gold - reserve);
        }
        bought
    }
}
