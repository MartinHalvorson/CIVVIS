//! `religion-seat-fields-apostles`: a Religion-target founder on the offensive
//! buys Apostles first and saves its Faith for them, and
//! `religion-seat-takes-the-faith-beliefs`: it founds on the founder beliefs
//! that pay Faith.
//!
//! **Why (live King, 2026-11-01).** civvis-20261101T071051Z, a Religion seat,
//! held every city of its own and flipped no rival majority from about 49
//! verified spreads. It lost 17 Missionaries, had no Apostle until t200 and
//! fought no theological battle. Faith ran at 135 a turn. The host's purchase
//! list shows only what the bank can pay for: London at t150 listed Missionary
//! 110, Inquisitor 90 and Guru 120 with no Apostle at 189 Faith, then Apostle
//! 240 at 258 Faith at t180. So `unit_purchase_cost` answers `None` until the
//! price is banked, "Apostle first" falls through to a Missionary, and that
//! purchase spends the bank the Apostle was waiting for.
//!
//! **Why an Apostle.** A Missionary carries 200 pressure a charge and removes
//! 10% of rival pressure; an Apostle carries 220 and removes 25%, and it is the
//! only unit that can clear the rival spreaders that kill Missionaries. Its
//! promotions multiply a charge (Translator +200% in foreign lands).
//!
//! **What the corps gene does.** With the gene on, a Religion-target founder
//! on the offensive:
//! - raises the Apostle cap from 2 by one per six major-power cities not yet
//!   following its faith, up to [`APOSTLE_CAP_CEILING`];
//! - buys an Apostle first for the first two, then alternately with
//!   Missionaries;
//! - while an Apostle comes next and a city of ours could field one, holds the
//!   Faith until the Apostle is payable. Worship buildings wait on the same
//!   reserve.
//! When this pass saves for an Apostle, the later routine city governor's
//! Missionary purchase respects the same bank.
//!
//! A city of ours losing its faith, a rival's match point and the Inquisition
//! all still spend at once. Before a Temple stands nothing is held, so the
//! early Missionaries that convert our own cities are unchanged.
//!
//! **What the belief gene does.** Live Deity founders take Pilgrimage in 102
//! of 117 games and Lay Ministry in 114, while the shipped list tries Tithe
//! first for every lane. A Religion-target seat tries Pilgrimage (Faith per
//! following city, so the corps pays for itself as it converts), then Lay
//! Ministry, then the shipped order.
//!
//! **What the Holy Site gene does.** `religious_production` builds one Holy
//! Site and then only Shrines and Temples, so a founder at 7 cities earns 24
//! Faith a turn at t100 in the Deity Khmer sim, against 45 for its rivals
//! there and 102 live. Religious units can be bought only in a Holy Site city.
//! With `religion-seat-builds-holy-sites` a founder of three or more cities
//! claims one in its best idle city of two or more citizens, one a turn.
//! If a founder has lost every completed Holy Site, recovery instead restores
//! one legal site before the city-count floor, ahead of ordinary work. Units,
//! Walls, repairs and threatened cities retain their queues.
//! Live Khmer Deity civvis-20261109T022744Z, the first Religion seat at that
//! rung, founded at t32 but at t75 held one Holy Site in five cities, a Shrine
//! and no Prasat 14 turns after Theology, and earned 8 Faith a turn: an idle
//! queue never reached `religious_production`. So the gene also takes the
//! head of a Holy Site city's queue for its Shrine and then its Temple, or the
//! civilization's replacement, ahead of Districts and ordinary buildings, and
//! buys either with Faith when the bank covers it.
//!
//! **What the campaign gene does.** `spreader-steps-out-of-the-apostle` counts
//! a rival as a threat when its mean blow would out-trade ours. On a Religion
//! seat that pins the corps: civvis-20261109T022744Z (first Deity Khmer Religion
//! seat) ended t102-136 with its Missionaries held outside a rival Apostle's
//! reach on most turns, and Immortal 063431Z spread 5 times all game for the
//! same reason. `stalled-spreader-braves-the-march` already reads a rival as a
//! threat only when its blow would kill, but only after three held turns. With
//! `religion-seat-presses-the-campaign` the corps of a Religion-target founder
//! reads threats that way from the start, and an Apostle beside a rival
//! religious unit takes a fight whose mean exchange wins before any step away.
//! A kill is still avoided, and a wounded unit's next blow is lethal again, so
//! it still retreats.
//!
//! **What the race gene does.** Live Khmer Deity shows the founding lost by
//! a few turns. A Prophet costs 30 points at Online and a Holy Site with Divine
//! Spark earns 2 a turn; Holy Site Prayers cost 21 Production and pay 10 points.
//! civvis-20261109T022744Z finished its Holy Site at t15, then built a Settler
//! and an Archer and founded at t34, as the Prophets ran out at t32.
//! civvis-20261109T031713Z finished it at t17, had 15 points at t25 when the
//! class closed, and never founded; 034941Z started it at t20, finished it at
//! t31 and never founded either (the class closed at t39). In the 98 Deity
//! games on record the class closes at a median t39, as early as t26.
//! `religion-seat-founds-first` claims only an idle queue, and Prayers ran only
//! in an idle queue on a Religion plan. With `religion-seat-races-the-prophet`
//! a Religion-target seat that has not founded takes any unthreatened queue for
//! its Holy Site, and its best Holy Site city runs Prayers at the head of its
//! queue until the Prophet comes.
//!
//! Off, or for a seat aiming elsewhere, nothing changes.

use super::*;

/// Bound on concurrent Apostles; the estimated remaining workload raises the
/// target up to this ceiling. Charges alone do not guarantee conversions.
const APOSTLE_CAP_CEILING: usize = 6;

/// One more Apostle for this many major-power cities not yet following us.
const CITIES_PER_APOSTLE: usize = 6;

/// An Apostle's price against a Missionary's (`data/units.json`, 200 and 100),
/// for a board where the host has not quoted an Apostle yet.
const APOSTLE_PRICE_RATIO: f64 = 2.0;

/// A Holy Site claim waits for this many cities, so the opening's Settlers
/// keep their queues.
const HOLY_SITE_EMPIRE_FLOOR: usize = 3;

/// The smallest city that takes a Holy Site claim.
const HOLY_SITE_POP_FLOOR: i32 = 2;

impl AdvancedAi {
    /// Whether the corps gene acts for this seat.
    fn religion_seat_corps_active(&self, g: &Game, pid: usize, offensive: bool) -> bool {
        self.religion_seat_fields_apostles
            && offensive
            && self.active_victory_target(g) == Some(VictoryTarget::Religion)
            && g.players[pid].religion.is_some()
    }

    /// Estimate the major-power cities that may still need this seat's faith.
    /// Public totals include cities whose locations and religions are unknown;
    /// they size the corps without inventing targets or religious pressure.
    fn unconverted_major_cities(g: &Game, pid: usize) -> usize {
        let Some(religion) = g.players[pid].religion.as_deref() else {
            return 0;
        };
        g.players
            .iter()
            .filter(|owner| {
                owner.id != pid && owner.alive && !owner.is_minor && !owner.is_barbarian
            })
            .map(|owner| {
                let known = g.player_city_ids(owner.id);
                let known_left = known
                    .iter()
                    .filter(|cid| g.city_religion(&g.cities[cid]) != Some(religion))
                    .count();
                // Keep the existing contribution of known holdouts. A native
                // verdict that the major already follows us is no reason to
                // assume its unseen cities need conversion as well.
                let already_follows = g
                    .observed_majority_religion
                    .get(&owner.id)
                    .is_some_and(|faith| faith == religion);
                let unseen = if already_follows {
                    0
                } else {
                    g.observed_public_empire_stats
                        .get(&owner.id)
                        .and_then(|stats| stats.city_count)
                        .map_or(0, |count| count.saturating_sub(known.len()))
                };
                known_left.saturating_add(unseen)
            })
            .sum()
    }

    /// The Apostle cap. `shipped` with the gene off or on another target.
    pub(super) fn religion_seat_apostle_cap(
        &self,
        g: &Game,
        pid: usize,
        offensive: bool,
        shipped: usize,
    ) -> usize {
        if !self.religion_seat_corps_active(g, pid, offensive) {
            return shipped;
        }
        let workload = Self::unconverted_major_cities(g, pid);
        let cap = (shipped + workload.div_ceil(CITIES_PER_APOSTLE))
            .min(APOSTLE_CAP_CEILING)
            .max(shipped);
        think!(self.journal(), Faith, Detail, "Sizing the Apostle corps to {cap}";
               "religion-seat-fields-apostles: up to {workload} major-power cities may still need conversion; public city totals size the corps without revealing their locations or religions");
        cap
    }

    /// Whether the next religious unit is an Apostle: always for the first
    /// two, then one for one with Missionaries.
    pub(super) fn religion_seat_leads_with_apostles(
        &self,
        g: &Game,
        pid: usize,
        offensive: bool,
        apostles: usize,
        missionaries: usize,
        apostle_cap: usize,
    ) -> bool {
        self.religion_seat_corps_active(g, pid, offensive)
            && apostles < apostle_cap
            && (apostles < 2 || apostles <= missionaries)
    }

    /// A city of ours that could field an Apostle once the Faith is banked:
    /// our faith holds it, and it has a Holy Site with a Temple or the
    /// civilization's replacement for one (the Khmer Prasat).
    fn city_can_field_an_apostle(g: &Game, city: &crate::game::City, religion: &str) -> bool {
        g.city_religion(city) == Some(religion)
            && g.city_has_district_family(city, crate::name!("holy_site"))
            && Self::city_holds_family(g, city, "temple")
    }

    /// The cheapest Apostle price this seat can see: the engine's quote where
    /// it has one, else a Missionary's quote at the data's ratio. `None` when
    /// no city of ours can field one.
    fn religion_seat_apostle_price(&self, g: &Game, pid: usize, religion: &str) -> Option<f64> {
        let cities: Vec<u32> = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| Self::city_can_field_an_apostle(g, &g.cities[cid], religion))
            .collect();
        if cities.is_empty() {
            return None;
        }
        let quote = |unit: &str, scale: f64| {
            cities
                .iter()
                .filter_map(|cid| g.unit_purchase_cost(pid, *cid, unit, "faith"))
                .map(|price| price * scale)
                .min_by(f64::total_cmp)
        };
        quote("apostle", 1.0).or_else(|| quote("missionary", APOSTLE_PRICE_RATIO))
    }

    /// Whether this turn's Faith waits for an Apostle: one is next, a city of
    /// ours can field it, and the bank cannot pay for it above `reserve`.
    pub(super) fn religion_seat_saving_for_an_apostle(
        &self,
        g: &Game,
        pid: usize,
        religion: &str,
        reserve: f64,
    ) -> bool {
        let Some(price) = self.religion_seat_apostle_price(g, pid, religion) else {
            return false;
        };
        let faith = g.players[pid].faith;
        let saving = faith + f64::EPSILON < price + reserve;
        if saving {
            think!(self.journal(), Faith, Detail,
                "Saving for an Apostle";
                "religion-seat-fields-apostles: {:.0} Faith banked against a price of {:.0} and a reserve of {:.0}; a cheaper unit bought now spends the bank the Apostle needs",
                faith, price, reserve);
        }
        saving
    }

    /// Faith worship buildings leave banked while the corps saves. Zero with
    /// the gene off, on another target, once the cap is met or when no city
    /// of ours can field an Apostle.
    pub(super) fn religion_seat_apostle_reserve(&self, g: &Game, pid: usize) -> f64 {
        let Some(religion) = g.players[pid].religion.clone() else {
            return 0.0;
        };
        if !self.religion_seat_corps_active(g, pid, true) {
            return 0.0;
        }
        let apostles = self.founder_corps_count(g, pid, "apostle", &religion);
        if apostles >= self.religion_seat_apostle_cap(g, pid, true, 2) {
            return 0.0;
        }
        // A Temple pays for itself, so an unbuilt one comes before the corps.
        let temple_owed = g.player_city_ids(pid).into_iter().any(|cid| {
            let city = &g.cities[&cid];
            g.city_religion(city) == Some(religion.as_str())
                && g.city_has_district_family(city, crate::name!("holy_site"))
                && !Self::city_holds_family(g, city, "temple")
        });
        if temple_owed {
            return 0.0;
        }
        self.religion_seat_apostle_price(g, pid, &religion)
            .unwrap_or(0.0)
    }

    /// Whether `city` holds `family` or a civilization's replacement for it.
    pub(super) fn city_holds_family(g: &Game, city: &crate::game::City, family: &str) -> bool {
        city.buildings.iter().any(|building| {
            building.as_str() == family
                || g.rules
                    .buildings
                    .get(building)
                    .and_then(|spec| spec.replaces.as_deref())
                    == Some(family)
        })
    }

    /// Whether a queue head may give way to a Faith building: anything but a
    /// unit, Walls or a Faith building already started. An idle queue always
    /// may.
    fn faith_building_may_displace(g: &Game, head: Option<&Item>) -> bool {
        match head {
            None => true,
            Some(Item::District { .. } | Item::Project { .. }) => true,
            Some(Item::Building { building }) => {
                let family = g
                    .rules
                    .buildings
                    .get(building)
                    .and_then(|spec| spec.replaces)
                    .unwrap_or(*building);
                !matches!(family.as_str(), "shrine" | "temple" | "walls")
                    && g.rules
                        .buildings
                        .get(building)
                        .is_none_or(|spec| spec.outer_defense <= 0)
            }
            _ => false,
        }
    }

    /// `religion-seat-builds-holy-sites`: the next Shrine or Temple a founder
    /// owes, bought with Faith when the bank covers it and otherwise put at
    /// the head of its city's queue; then a Holy Site in an idle city. One
    /// claim a turn. A city under attack keeps its queue, and the opening book
    /// keeps its four builds.
    pub(super) fn claim_religion_seat_faith_economy(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        self.claim_religion_seat_faith_economy_in(g, pid, plan, None);
    }

    /// The hint owns only its requested city's queue, not another city's
    /// earlier position in the empire-wide claim order.
    pub(super) fn claim_religion_seat_faith_hint(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
        city: u32,
    ) {
        self.claim_religion_seat_faith_economy_in(g, pid, plan, Some(city));
    }

    fn claim_religion_seat_faith_economy_in(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
        only_city: Option<u32>,
    ) {
        if !self.religion_seat_builds_holy_sites
            || self.base.book_pos < 4
            || self.active_victory_target(g) != Some(VictoryTarget::Religion)
        {
            return;
        }
        let Some(religion) = g.players[pid].religion.clone() else {
            return;
        };
        let cities: Vec<u32> = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| only_city.is_none_or(|wanted| wanted == *cid))
            .collect();
        for family in ["shrine", "temple"] {
            for cid in &cities {
                let city = &g.cities[cid];
                if g.city_religion(city) != Some(religion.as_str())
                    || !g.city_has_district_family(city, crate::name!("holy_site"))
                    || Self::city_holds_family(g, city, family)
                    || plan.threatened_city == Some(*cid)
                    || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
                {
                    continue;
                }
                let Some(Item::Building { building }) = BasicAi::civ_building(g, pid, *cid, family)
                else {
                    continue;
                };
                if g.building_faith_purchase_cost(pid, *cid, building.as_str())
                    .is_some_and(|cost| g.players[pid].faith + f64::EPSILON >= cost)
                    && g.apply(
                        pid,
                        &Action::BuyBuilding {
                            city: *cid,
                            building,
                            currency: "faith".to_string(),
                        },
                    )
                    .is_ok()
                {
                    think!(self.journal(), Faith, Decision,
                        "{} buys its {}", g.cities[cid].name, building;
                        "religion-seat-builds-holy-sites: Faith earns the Faith that buys the corps");
                    return;
                }
                if Self::faith_building_may_displace(g, g.cities[cid].queue.first())
                    && g.apply(
                        pid,
                        &Action::Produce {
                            city: *cid,
                            item: Item::Building { building },
                        },
                    )
                    .is_ok()
                {
                    think!(self.journal(), Faith, Decision,
                        "{} starts its {}", g.cities[cid].name, building;
                        "religion-seat-builds-holy-sites: ahead of the queue it displaced, which keeps its progress");
                    return;
                }
            }
        }
        if self.religion_seat_needs_a_site(g, pid) {
            self.claim_religion_seat_recovery_site(g, pid, plan, only_city);
            return;
        }
        if only_city.is_none() {
            self.religion_seat_claims_a_holy_site(g, pid);
        }
    }

    /// Keep the founder's legal Shrine/Temple through ordinary rescoring,
    /// including its first frame with no invested production. Siege and
    /// insolvency preemption still run before the governor's reservation.
    pub(super) fn religion_seat_faith_building_reserved(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        if !self.religion_seat_builds_holy_sites
            || self.base.book_pos < 4
            || self.active_victory_target(g) != Some(VictoryTarget::Religion)
            || plan.threatened_city == Some(cid)
        {
            return false;
        }
        let Some(religion) = g.players[pid].religion.as_deref() else {
            return false;
        };
        let city = &g.cities[&cid];
        matches!(item, Item::Building { building }
            if g.building_is_family(*building, crate::name!("shrine"))
                || g.building_is_family(*building, crate::name!("temple")))
            && (city.last_attacked == 0 || g.turn.saturating_sub(city.last_attacked) > 4)
            && g.city_religion(city) == Some(religion)
            && g.city_has_district_family(city, crate::name!("holy_site"))
            && Self::production_commitment_is_legal(g, pid, cid, item)
    }

    /// A founded Religion seat with no completed Holy Site must replace its
    /// lost source before ordinary expansion's three-city floor applies.
    fn religion_seat_needs_a_site(&self, g: &Game, pid: usize) -> bool {
        self.religion_seat_builds_holy_sites
            && self.base.book_pos >= 4
            && self.active_victory_target(g) == Some(VictoryTarget::Religion)
            && g.players[pid].religion.is_some()
            && !g
                .player_city_ids(pid)
                .into_iter()
                .any(|cid| g.city_has_district_family(&g.cities[&cid], crate::name!("holy_site")))
    }

    /// Reserve a legal recovery Holy Site through ordinary rescoring. A
    /// completed (even pillaged) site ends this special recovery path.
    pub(super) fn religion_seat_recovery_site_reserved(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        if !matches!(item, Item::District { district, .. }
            if g.district_family(*district) == "holy_site")
            || !self.religion_seat_needs_a_site(g, pid)
        {
            return false;
        }
        let city = &g.cities[&cid];
        plan.threatened_city != Some(cid)
            && (city.last_attacked == 0 || g.turn.saturating_sub(city.last_attacked) > 4)
            && g.city_religion(city) == g.players[pid].religion.as_deref()
            && Self::production_commitment_is_legal(g, pid, cid, item)
    }

    fn religion_seat_recovery_city_available(
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> bool {
        let city = &g.cities[&cid];
        plan.threatened_city != Some(cid)
            && (city.last_attacked == 0 || g.turn.saturating_sub(city.last_attacked) > 4)
            && g.city_religion(city) == g.players[pid].religion.as_deref()
            && !city
                .queue
                .first()
                .is_some_and(|item| Self::active_queue_is_defensive(g, item))
            && Self::faith_building_may_displace(g, city.queue.first())
    }

    /// Replace a lost Faith source, including in a one- or two-city remnant.
    /// Ordinary buildings/projects may wait; units, Walls and repairs keep
    /// their queues. A paused foundation is resumed before a new one is laid.
    fn claim_religion_seat_recovery_site(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
        only_city: Option<u32>,
    ) -> bool {
        if !self.religion_seat_needs_a_site(g, pid) {
            return false;
        }
        let cities = g.player_city_ids(pid);
        let mut pending = BTreeSet::new();
        for cid in &cities {
            if !Self::religion_seat_recovery_city_available(g, pid, *cid, plan) {
                continue;
            }
            let city = &g.cities[cid];
            let placed = city
                .queue
                .iter()
                .filter_map(|item| match item {
                    Item::District { district, pos }
                        if g.district_family(*district) == "holy_site" =>
                    {
                        Some((*district, *pos))
                    }
                    _ => None,
                })
                .chain(city.owned_tiles.iter().filter_map(|pos| {
                    g.map
                        .get(*pos)
                        .and_then(|tile| tile.district_foundation.as_ref())
                        .filter(|foundation| g.district_family(foundation.district) == "holy_site")
                        .map(|foundation| (foundation.district, *pos))
                }));
            for (district, pos) in placed {
                if Self::production_commitment_is_legal(
                    g,
                    pid,
                    *cid,
                    &Item::District { district, pos },
                ) {
                    pending.insert(pos);
                }
            }
        }
        let mut best: Option<(f64, f64, u32, Pos, Item)> = None;
        for cid in cities {
            if only_city.is_some_and(|wanted| wanted != cid)
                || !Self::religion_seat_recovery_city_available(g, pid, cid, plan)
            {
                continue;
            }
            for item in g.producible_items(pid, cid) {
                let Item::District { district, pos } = &item else {
                    continue;
                };
                if g.district_family(*district) != "holy_site"
                    || (!pending.is_empty() && !pending.contains(pos))
                {
                    continue;
                }
                let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
                    g.item_remaining_cost_for_city(pid, cid, &item)
                        / g.city_yields(cid).production.max(0.5)
                });
                let faith = g.district_site_yields(*district, *pos).faith;
                let better =
                    best.as_ref()
                        .is_none_or(|(old_turns, old_faith, old_city, old_pos, _)| {
                            old_turns
                                .total_cmp(&turns)
                                .then(faith.total_cmp(old_faith))
                                .then(old_city.cmp(&cid))
                                .then(old_pos.cmp(pos))
                                .is_gt()
                        });
                if better {
                    best = Some((turns, faith, cid, *pos, item));
                }
            }
        }
        let Some((turns, _, city, _, item)) = best else {
            return false;
        };
        if g.cities[&city].queue.first() == Some(&item) {
            return true;
        }
        if g.apply(pid, &Action::Produce { city, item }).is_err() {
            return false;
        }
        think!(self.journal(), Faith, Decision,
            "{} starts a replacement Holy Site", g.cities[&city].name;
            "religion-seat-builds-holy-sites: no completed Holy Site remains; restore the source before the expansion floor ({turns:.1} turns)");
        true
    }

    /// `religion-seat-builds-holy-sites`: the idle city of ours with the best
    /// Holy Site placement that has none, once founded. One claim a turn.
    pub(super) fn religion_seat_claims_a_holy_site(&self, g: &mut Game, pid: usize) {
        if !self.religion_seat_builds_holy_sites
            || self.active_victory_target(g) != Some(VictoryTarget::Religion)
            || g.players[pid].religion.is_none()
            || g.player_city_ids(pid).len() < HOLY_SITE_EMPIRE_FLOOR
        {
            return;
        }
        let family = crate::name!("holy_site");
        let mut best: Option<(f64, i32, u32, Item)> = None;
        for cid in g.player_city_ids(pid) {
            let city = &g.cities[&cid];
            if !city.queue.is_empty()
                || city.pop < HOLY_SITE_POP_FLOOR
                || g.city_has_district_family(city, family)
            {
                continue;
            }
            let pop = city.pop;
            for item in g.producible_items(pid, cid) {
                let Item::District { district, pos } = &item else {
                    continue;
                };
                if g.district_family(*district) != "holy_site" {
                    continue;
                }
                let faith = g.district_site_yields(*district, *pos).faith;
                let better = best.as_ref().is_none_or(|(f, p, c, _)| {
                    faith
                        .total_cmp(f)
                        .then(pop.cmp(p))
                        .then(c.cmp(&cid))
                        .is_gt()
                });
                if better {
                    best = Some((faith, pop, cid, item.clone()));
                }
            }
        }
        if let Some((faith, _, city, item)) = best {
            if g.apply(pid, &Action::Produce { city, item }).is_ok() {
                think!(self.journal(), Faith, Decision,
                    "{} claims a Holy Site", g.cities[&city].name;
                    "religion-seat-builds-holy-sites: Faith and religious-unit purchases come from Holy Sites, and this site yields {:.0} Faith", faith);
            }
        }
    }

    /// `religion-seat-presses-the-campaign`: whether this seat's corps yields
    /// only to a killing blow.
    pub(super) fn religion_seat_presses_on(&self, g: &Game, pid: usize) -> bool {
        self.religion_seat_presses_the_campaign
            && self.active_victory_target(g) == Some(VictoryTarget::Religion)
            && g.players[pid].religion.is_some()
    }

    /// `religion-seat-presses-the-campaign`: an Apostle beside a rival
    /// religious unit takes the winnable fight against the weakest of them.
    /// `None` with the gene off, for any other unit, or with no such fight.
    pub(super) fn religion_seat_apostle_fights_first(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !self.religion_seat_presses_on(g, pid) {
            return None;
        }
        let unit = g.units.get(&uid)?;
        if unit.kind != "apostle" || unit.moves_left <= 0.0 {
            return None;
        }
        let faith = unit.religion.clone()?;
        let target = g
            .nbrs(unit.pos)
            .into_iter()
            .filter(|position| g.theological_target_open(pid, *position))
            .filter(|position| self.theology_fight_is_winnable(g, uid, *position))
            .filter_map(|position| {
                g.unit_ids_at(position)
                    .iter()
                    .filter(|other| {
                        let other = &g.units[other];
                        other.owner != pid
                            && g.rules.units[other.kind].class == "religious"
                            && other.religion.as_deref().is_some_and(|r| r != faith)
                    })
                    .map(|other| g.units[other].hp)
                    .min()
                    .map(|hp| (hp, position))
            })
            .min()?
            .1;
        let acted = g
            .apply(pid, &Action::TheologicalAttack { unit: uid, target })
            .is_ok();
        if acted {
            think!(self.journal(), Faith, Decision,
                "Apostle {} fights at {:?}", uid, target;
                "religion-seat-presses-the-campaign: the mean exchange wins, and a kill hands our faith 250 pressure around the fight");
        }
        Some(acted)
    }

    /// `religion-seat-races-the-prophet`: the city that produces most among
    /// those with a finished Holy Site runs Prayers at the head of its queue
    /// until our Prophet is earned. One claim a turn; a city under attack
    /// keeps its queue.
    pub(super) fn claim_religion_seat_prayers(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        let player = &g.players[pid];
        if !self.religion_seat_races_the_prophet
            || self.active_victory_target(g) != Some(VictoryTarget::Religion)
            || player.religion.is_some()
            || player.prophet_pending
            || !self.prophet_race_open_for(g, pid)
        {
            return;
        }
        let prayers = Item::Project {
            project: crate::name!("holy_site_prayers"),
        };
        let family = crate::name!("holy_site");
        let best = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| {
                let city = &g.cities[cid];
                g.city_has_district_family(city, family)
                    && plan.threatened_city != Some(*cid)
                    && (city.last_attacked == 0 || g.turn.saturating_sub(city.last_attacked) > 4)
            })
            .max_by(|left, right| {
                g.city_yields(*left)
                    .production
                    .total_cmp(&g.city_yields(*right).production)
                    .then_with(|| right.cmp(left))
            });
        let Some(city) = best else {
            return;
        };
        if g.cities[&city].queue.first() == Some(&prayers) || !g.can_produce(pid, city, &prayers) {
            return;
        }
        if g.apply(
            pid,
            &Action::Produce {
                city,
                item: prayers,
            },
        )
        .is_ok()
        {
            think!(self.journal(), Faith, Decision,
                "{} runs Prayers for the Prophet", g.cities[&city].name;
                "religion-seat-races-the-prophet: {:.0} Prophet points of our own, and each Prayers completes for more of them than turns of the district's rate",
                g.players[pid].gpp.get("prophet").copied().unwrap_or(0.0));
        }
    }

    /// Whether this seat's founding should try the Faith beliefs first. Read
    /// by `BasicAi`'s founding, which cannot see the lane.
    pub(super) fn founder_takes_the_faith_beliefs(&self, g: &Game) -> bool {
        self.religion_seat_takes_the_faith_beliefs
            && self.active_victory_target(g) == Some(VictoryTarget::Religion)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod corps_fog_tests;
