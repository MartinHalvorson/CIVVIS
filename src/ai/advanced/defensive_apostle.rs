//! A founder's defence of its faith: the defensive Inquisition, the Faith
//! reserved for it, and the Apostles and spreaders kept covering source
//! cities.

use super::*;

impl AdvancedAi {
    /// The lanes whose founder defences run: Domination, and Religion with
    /// `religion-seat-defends-its-faith` (the Holy City guard and the
    /// founder's spreaders kept home read it; the defensive Inquisition and
    /// the homeland pass name the gene directly).
    pub(super) fn religion_seat_or_domination(&self, g: &Game) -> bool {
        match self.active_victory_target(g) {
            Some(VictoryTarget::Domination) => true,
            Some(VictoryTarget::Religion) => self.religion_seat_defends_its_faith,
            _ => false,
        }
    }

    /// Build the founder's defensive capability while its Temple still follows
    /// our faith. Waiting for a flipped city leaves only a few turns to save
    /// for an Apostle, launch the Inquisition, and buy its first defender.
    ///
    /// Domination seats only, as written (#3656). With
    /// `inquisition-defends-the-diplomat`, Diplomatic seats too. Measured on
    /// paired live-genome seeds (Emperor, 4 players): Diplomatic seats 3 -> 4
    /// wins of 20, none lost; Culture seats 9 -> 7, both lost to rival
    /// Diplomatic victories -- so Culture, like Science, Religion and a seat
    /// with no target, keeps the old reading. With
    /// `religion-seat-defends-its-faith`, Religion seats too: a Religious
    /// Victory needs our faith to hold our own cities as well as everyone
    /// else's.
    pub(super) fn defensive_inquisition_ready(&self, g: &Game, pid: usize) -> bool {
        let lane = match self.active_victory_target(g) {
            Some(VictoryTarget::Domination) => true,
            Some(VictoryTarget::Diplomacy) => self.inquisition_defends_the_diplomat,
            Some(VictoryTarget::Religion) => self.religion_seat_defends_its_faith,
            _ => false,
        };
        if !lane || !g.victory_conditions.religious {
            return false;
        }
        let Some(faith) = g.players[pid].religion.as_deref() else {
            return false;
        };
        if !g.players[pid]
            .holy_city
            .and_then(|cid| g.cities.get(&cid))
            .is_some_and(|city| city.owner == pid)
        {
            return false;
        }
        let spec = &g.rules.units["apostle"];
        if spec
            .tech
            .as_ref()
            .is_some_and(|tech| !g.players[pid].techs.contains(tech))
            || spec
                .civic
                .as_ref()
                .is_some_and(|civic| !g.players[pid].civics.contains(civic))
        {
            return false;
        }
        let cities = g.player_city_ids(pid);
        let established = cities
            .iter()
            .filter(|cid| g.city_religion(&g.cities[cid]) == Some(faith))
            .count();
        let first_spreader = g.units.values().any(|unit| {
            unit.owner == pid
                && unit.kind == "missionary"
                && unit.religion.as_deref() == Some(faith)
                && unit.charges > 0
        });
        // `inquisition-answers-a-rival-majority`: the wait is for an empire
        // our faith has not reached yet; a city of ours already held by a
        // rival faith is the conversion the Inquisition exists to answer.
        let rival_held = self.inquisition_answers_a_rival_majority
            && cities.iter().any(|cid| {
                g.city_religion(&g.cities[cid])
                    .is_some_and(|religion| religion != faith)
            });
        if established * 2 < cities.len() && !first_spreader && !rival_held {
            return false;
        }
        cities.iter().any(|cid| {
            let city = &g.cities[cid];
            g.city_religion(city) == Some(faith)
                && city.buildings.iter().any(|building| {
                    g.building_is_family(building, crate::name!("temple"))
                        && !city.pillaged_buildings.contains(building)
                })
                && city.districts.iter().any(|(district, position)| {
                    g.district_family(*district).as_str() == "holy_site"
                        && g.map.get(*position).is_some_and(|tile| !tile.pillaged)
                })
        })
    }

    /// Reserve Faith ahead of repeat Missionaries, using the actual purchase
    /// quote when the host offers the unit. An absent affordable menu entry
    /// is not a reason to spend the accumulated Faith on a cheaper unit.
    pub(super) fn prepare_defensive_inquisition(&self, g: &mut Game, pid: usize) -> bool {
        if !self.defensive_inquisition_ready(g, pid) {
            return false;
        }
        let launched = g.players[pid]
            .counters
            .get("inquisition")
            .copied()
            .unwrap_or(0)
            > 0;
        let wanted = if launched { "inquisitor" } else { "apostle" };
        let faith = g.players[pid].religion.clone().unwrap();
        // `founder-defends-its-cities`: with a rival faith in our cities, an
        // Inquisition more than a few turns of Faith away does not hold the
        // ordinary defensive purchases (live Emperor 20261008T101304Z saved
        // 2 to 121 of the Apostle's 200 Faith from turn 49 to 83 and lost).
        let saving_for = if !launched
            && g.units.values().any(|unit| {
                unit.owner == pid
                    && unit.kind == "apostle"
                    && unit.religion.as_deref() == Some(faith.as_str())
                    && unit.charges > 0
            }) {
            "inquisitor"
        } else {
            wanted
        };
        if self.founder_inquisition_unit_out_of_reach(g, pid, saving_for) {
            return false;
        }
        if g.units.values().any(|unit| {
            unit.owner == pid
                && unit.kind == wanted
                && unit.religion.as_deref() == Some(faith.as_str())
                && unit.charges > 0
        }) {
            // Keep funding the first Inquisitor while the Apostle travels.
            // Once the defender exists, ordinary spending resumes.
            // `apostle-transit-keeps-the-cover`: a threatened source still
            // buys its cover Missionary while the Faith left covers the
            // Inquisitor. Exact no-op while the gene is off.
            if !launched && self.apostle_transit_keeps_the_cover {
                self.apostle_transit_cover(g, pid, &faith);
            }
            return !launched;
        }
        for cid in g.player_city_ids(pid) {
            if g.city_religion(&g.cities[&cid]) != Some(faith.as_str())
                // See `founder_purchase_withheld` (`founder-spreads-only-its-faith`).
                || self.founder_purchase_withheld(g, pid, cid)
            {
                continue;
            }
            let Some(price) = g.unit_purchase_cost(pid, cid, wanted, "faith") else {
                continue;
            };
            if g.players[pid].faith + f64::EPSILON < price {
                continue;
            }
            if g.apply(
                pid,
                &Action::Buy {
                    city: cid,
                    unit: Name::new(wanted),
                    formation: 0,
                    currency: "faith".into(),
                },
            )
            .is_ok()
            {
                think!(self.journal(), Faith, Decision,
                    "Buying the first defensive {}", wanted;
                    "prepare an Inquisition before rival conversion removes our source");
                return true;
            }
        }
        // See `inquisition_first_price` (`founder-funds-the-inquisition`): the
        // Inquisition is bought before any cover Missionary.
        if let Some(price) = self.inquisition_first_price(g, pid) {
            think!(self.journal(), Faith, Detail,
                "Saving for the {}", wanted;
                "founder-funds-the-inquisition: {:.0} of {:.0} Faith; a cover Missionary now would spend the Inquisition (G215 spent over 700 Faith on cover Missionaries and Builders and reached its Apostle two turns before Bogota fell)",
                g.players[pid].faith, price);
            return true;
        }
        // Saving must not leave an exposed source without local cover. A
        // charged Missionary across the empire cannot preserve this city's
        // purchase menu before the approaching foreign spreaders convert it.
        // Reuse the home-watch radius and require a short traversable route;
        // this is a locality bound, not a movement-point arrival prediction.
        for cid in g.player_city_ids(pid) {
            let city = &g.cities[&cid];
            if g.city_religion(city) != Some(faith.as_str())
                // See `founder_purchase_withheld` (`founder-spreads-only-its-faith`).
                || self.founder_purchase_withheld(g, pid, cid)
            {
                continue;
            }
            if !Self::source_needs_cover(g, pid, cid, &faith) {
                continue;
            }
            let Some(price) = g.unit_purchase_cost(pid, cid, "missionary", "faith") else {
                continue;
            };
            if g.players[pid].faith + f64::EPSILON < price {
                continue;
            }
            if g.apply(
                pid,
                &Action::Buy {
                    city: cid,
                    unit: crate::name!("missionary"),
                    formation: 0,
                    currency: "faith".into(),
                },
            )
            .is_ok()
            {
                think!(self.journal(), Faith, Decision,
                    "Preserving a threatened faith source";
                    "one nearby Missionary covers the source while the defensive purchase is unaffordable");
                return true;
            }
        }
        true
    }

    /// Whether `cid`, a city on our `faith`, has a rival faith's charged
    /// spreader within [`RELIGIOUS_HOME_WATCH`] and no charged spreader of
    /// ours within that radius and a short route of it.
    fn source_needs_cover(g: &Game, pid: usize, cid: u32, faith: &str) -> bool {
        let city = &g.cities[&cid];
        let threatened = g.units.values().any(|unit| {
            unit.owner != pid
                && unit.charges > 0
                && g.rules.units[unit.kind].religious_spread > 0.0
                && unit.religion.as_deref().is_some_and(|other| other != faith)
                && g.wdist(city.pos, unit.pos) <= RELIGIOUS_HOME_WATCH
        });
        threatened
            && !g.units.values().any(|unit| {
                unit.owner == pid
                    && unit.religion.as_deref() == Some(faith)
                    && unit.charges > 0
                    && g.rules.units[unit.kind].religious_spread > 0.0
                    && g.wdist(city.pos, unit.pos) <= RELIGIOUS_HOME_WATCH
                    && g.route_distance(unit.id, city.pos, 1)
                        .is_some_and(|steps| steps <= RELIGIOUS_HOME_WATCH as usize)
            })
    }

    /// `apostle-transit-keeps-the-cover`: while our first Apostle walks to
    /// launch the Inquisition, `prepare_defensive_inquisition` holds every
    /// Faith purchase for the Inquisitor, the threatened source's cover
    /// Missionary included, and says nothing. Live Emperor Religion seat
    /// civvis-20261031T034930Z bought its Apostle at t68; no Faith line
    /// followed from t69 to t109 while Catholic spreaders worked the Holy
    /// City, which flipped between t80 and t90 with 254 to 769 Faith banked,
    /// and once no city followed us nothing of ours could be bought again
    /// (2,670 Faith unspent at the t218 loss). The cover Missionary is bought
    /// in the first threatened, uncovered source when the Faith left after
    /// it still covers the first Inquisitor (priced as the city's Missionary
    /// scaled by the units' base costs, since the Inquisitor is not for sale
    /// before the launch); otherwise the hold is journalled.
    fn apostle_transit_cover(&self, g: &mut Game, pid: usize, faith: &str) {
        let ratio = match (
            g.rules.units.get("inquisitor"),
            g.rules.units.get("missionary"),
        ) {
            (Some(inquisitor), Some(missionary)) if missionary.cost > 0.0 => {
                inquisitor.cost / missionary.cost
            }
            _ => 1.0,
        };
        let mut held: Option<(String, f64, f64)> = None;
        for cid in g.player_city_ids(pid) {
            if g.city_religion(&g.cities[&cid]) != Some(faith)
                || self.founder_purchase_withheld(g, pid, cid)
                || !Self::source_needs_cover(g, pid, cid, faith)
            {
                continue;
            }
            let Some(price) = g.unit_purchase_cost(pid, cid, "missionary", "faith") else {
                continue;
            };
            let reserve = price * ratio;
            let bank = g.players[pid].faith;
            if bank + f64::EPSILON < price + reserve {
                held.get_or_insert((g.cities[&cid].name.clone(), price, reserve));
                continue;
            }
            if g.apply(
                pid,
                &Action::Buy {
                    city: cid,
                    unit: crate::name!("missionary"),
                    formation: 0,
                    currency: "faith".into(),
                },
            )
            .is_ok()
            {
                let name = g.cities[&cid].name.clone();
                think!(self.journal(), Faith, Decision,
                    "Covering {name} while the Apostle travels";
                    "apostle-transit-keeps-the-cover: a rival spreader is near and {:.0} Faith covers the Missionary ({price:.0}) and the first Inquisitor (~{reserve:.0})",
                    bank);
                return;
            }
        }
        if let Some((name, price, reserve)) = held {
            think!(self.journal(), Faith, Detail,
                "Holding {name}'s cover for the Inquisitor while the Apostle travels";
                "apostle-transit-keeps-the-cover: {:.0} Faith; the Missionary ({price:.0}) would leave less than the first Inquisitor (~{reserve:.0})",
                g.players[pid].faith);
        }
    }
}

#[cfg(test)]
mod tests;
