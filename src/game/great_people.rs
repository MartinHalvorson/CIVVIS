//! Great People: points, offers, recruitment, patronage and their effects.
//!
//! Points per turn and the market (`great_person_points_per_turn`,
//! `refresh_great_person_offers`, `gp_cost`, patronage prices), what blocks
//! or allows an offer, activating and claiming a Great Person, and the
//! effects: district and named effects, the land units that Great Generals
//! and Admirals lead, and the recruit/patronize orders.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

impl Game {
    pub fn current_great_person(
        &self,
        kind: &str,
    ) -> Option<(&str, &crate::rules::GreatPersonSpec)> {
        self.rules
            .great_people
            .iter()
            .filter(|(id, spec)| {
                spec.kind == kind && !self.retired_great_people.contains(id.as_str())
            })
            .min_by_key(|(id, spec)| (spec.era, *id))
            .map(|(id, spec)| (id.as_str(), spec))
    }

    /// Standard-speed base price plus the stock ahead-of-era surcharge. Art
    /// people and Prophets use their listed price without that surcharge.
    fn great_person_market_cost(&self, person: &crate::rules::GreatPersonSpec) -> f64 {
        let standard = if matches!(
            person.kind.as_str(),
            "writer" | "artist" | "musician" | "prophet"
        ) {
            person.cost
        } else {
            let difference = person.era.saturating_sub(self.world_era);
            if difference == 0 {
                person.cost
            } else {
                (person.cost
                    * (1.0 + 0.3 * difference as f64)
                        .powi(difference.try_into().unwrap_or(i32::MAX)))
                .floor()
            }
        };
        self.game_speed.scale(standard)
    }

    /// Record the price of every currently exposed class offer. Existing
    /// entries are intentionally retained when the World Era advances.
    pub(super) fn refresh_great_person_offers(&mut self) {
        let offers: Vec<(String, String, f64)> = self
            .rules
            .great_people
            .values()
            .map(|person| person.kind.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter_map(|kind| {
                let (id, person) = self.current_great_person(&kind)?;
                Some((kind, id.to_string(), self.great_person_market_cost(person)))
            })
            .collect();
        for (_, id, cost) in offers {
            self.great_person_offer_costs.entry(id).or_insert(cost);
        }
    }

    /// Point cost of the named person currently offered in this global
    /// market. The legacy fallback keeps old/modded saves playable when a
    /// ruleset has no named entry for a point type.
    pub fn gp_cost(&self, pid: usize, kind: &str) -> f64 {
        if let Some((id, person)) = self.current_great_person(kind) {
            return self
                .great_person_offer_costs
                .get(id)
                .copied()
                .unwrap_or_else(|| self.great_person_market_cost(person));
        }
        let n = self.players[pid].gp_claimed.get(kind).copied().unwrap_or(0);
        60.0 * (1u64 << n.min(6) as u64) as f64
    }

    /// Exact Standard-speed patronage quote for the outstanding points.
    /// The fixed fee is charged in addition to the per-point component.
    pub fn great_person_patronage_price(
        &self,
        pid: usize,
        kind: &str,
        currency: &str,
    ) -> Option<f64> {
        if !self.great_person_class_offered_now(pid, kind) {
            return None;
        }
        self.current_great_person(kind)?;
        let points = self.players[pid].gpp.get(kind).copied().unwrap_or(0.0);
        let missing = (self.gp_cost(pid, kind) - points).max(0.0);
        if missing <= f64::EPSILON {
            return None;
        }
        match currency {
            "gold" => Some(200.0 + 15.0 * missing.floor()),
            "faith" => {
                let discount =
                    self.empire_wonder_effect(pid, "great_person_faith_patronage_discount_pct");
                Some((150.0 + 10.0 * missing.floor()) * (1.0 - discount / 100.0))
            }
            _ => None,
        }
    }

    fn has_foundable_corporation(&self, pid: usize) -> bool {
        self.cities
            .values()
            .filter(|city| city.owner == pid)
            .flat_map(|city| city.owned_tiles.iter().copied())
            .any(|position| self.can_found_corporation(pid, position))
    }

    /// Whether this empire can still recruit a Great Person of `kind`.
    ///
    /// Civilization VI stops paying a class into the bank the moment nobody of
    /// that class is left to earn: a Great Prophet once the empire holds a
    /// religion (or one is pending, or every religion the map allows has been
    /// founded), and any other class once the last named individual anywhere
    /// has been claimed. On a mirrored seat the host's own timeline is the
    /// authority for the latter (`live_great_person_exhausted`); headless
    /// games ask CIVVIS's roster. What such a class earns is not lost — see
    /// [`Self::unused_great_person_faith`].
    pub fn great_person_class_earnable(&self, pid: usize, kind: &str) -> bool {
        if kind == "prophet" {
            let player = &self.players[pid];
            if player.religion.is_some() || player.prophet_pending {
                return false;
            }
            let claimed = self.religions_founded()
                + self
                    .players
                    .iter()
                    .filter(|player| player.prophet_pending)
                    .count();
            return claimed < self.max_religions();
        }
        if let Some(exhausted) = self.players[pid].live_great_person_exhausted.as_ref() {
            return !exhausted.contains(kind);
        }
        self.current_great_person(kind).is_some()
    }

    /// Faith Civilization VI grants each turn for Great Person points nobody
    /// can spend: every point per turn of a class this empire can no longer
    /// earn (`great_person_class_earnable`) becomes one Faith, the engine's
    /// `GetFaithFromUnusedGreatPeoplePoints`. Measured on the live Settler
    /// seat across seven games: after the last Great Scientist was claimed the
    /// balance grew by the Campus points to the point (ratio 0.97–1.10, run
    /// civvis-20260816T123936Z t219–239: 23–60 a turn, more than every city
    /// together), and a Holy Site's Prophet points arrive as Faith from the
    /// turn the empire has a religion or the map runs out of them. Zero under
    /// Anarchy, when the empire collects no Faith at all.
    pub fn unused_great_person_faith(&self, pid: usize) -> f64 {
        if self.players[pid].is_minor || self.in_anarchy(pid) {
            return 0.0;
        }
        self.great_person_points_per_turn(pid)
            .iter()
            .filter(|(kind, _)| !self.great_person_class_earnable(pid, kind))
            .map(|(_, points)| points.max(0.0))
            .sum()
    }

    /// Great Person points this empire earns per turn, by class, after every
    /// multiplier (Governor, government, World Congress Patronage) — the
    /// figure Civilization VI's `GetPointsPerTurn` reports. Pure: what
    /// `process_great_people` banks each turn, readable without a turn.
    pub fn great_person_points_per_turn(&self, pid: usize) -> BTreeMap<String, f64> {
        let mut earn: BTreeMap<String, f64> = BTreeMap::new();
        if self.players[pid].is_minor {
            return earn;
        }
        for c in self.cities.values().filter(|c| c.owner == pid) {
            let mut city_earn: BTreeMap<String, f64> = BTreeMap::new();
            let scottish_happiness = self.scottish_enlightenment_happiness_scale(c);
            for position in &c.owned_tiles {
                let tile = &self.map.tiles[position];
                if !tile.pillaged && tile.improvement.as_deref() == Some("industry") {
                    *city_earn.entry("merchant".to_string()).or_insert(0.0) +=
                        self.rules.improvements["industry"]
                            .effects
                            .get("great_merchant_points")
                            .copied()
                            .unwrap_or(0.0);
                }
            }
            for (d, position) in &c.districts {
                if !self.district_is_active(c, d, *position) {
                    continue;
                }
                for (kind, points) in &self.rules.districts[d].great_person_points {
                    *city_earn.entry(kind.clone()).or_insert(0.0) += *points;
                    let cultural_alliance_route = self.routes.iter().any(|route| {
                        route.origin == c.id
                            && self.cities.get(&route.dest).is_some_and(|destination| {
                                self.alliance_with(pid, destination.owner)
                                    .is_some_and(|alliance| {
                                        alliance.kind == "cultural" && alliance.level >= 2
                                    })
                            })
                    });
                    if cultural_alliance_route {
                        *city_earn.entry(kind.clone()).or_insert(0.0) += 1.0;
                    }
                    if c.wonders.contains_key(&crate::name!("oracle")) {
                        *city_earn.entry(kind.clone()).or_insert(0.0) += self.rules.wonders
                            ["oracle"]
                            .effects
                            .get("city_district_gpp")
                            .copied()
                            .unwrap_or(0.0);
                    }
                }
                // Scottish Enlightenment's four live modifiers require the
                // exact active district type, not a district family: Happy
                // contributes one Scientist point per Campus and one Engineer
                // point per Industrial Zone; Ecstatic doubles both amounts.
                // They enter the city ledger here so Pingala, governments,
                // policies, and Patronage multiply them with every other city
                // source exactly once.
                if scottish_happiness != 0.0 {
                    match d.as_str() {
                        "campus" => {
                            *city_earn.entry("scientist".to_string()).or_insert(0.0) +=
                                scottish_happiness
                                    * self.civ_effect(pid, "happy_campus_scientist_gpp");
                        }
                        "industrial_zone" => {
                            *city_earn.entry("engineer".to_string()).or_insert(0.0) +=
                                scottish_happiness
                                    * self.civ_effect(pid, "happy_industrial_engineer_gpp");
                        }
                        _ => {}
                    }
                }
                // The building scan stays INSIDE the condition. It walks the
                // city's whole building list doing a rules lookup and a family
                // comparison per building, and this runs for every district of
                // every city every time yields are computed — so the common
                // case, where nobody is Suzerain of either city-state, must
                // stop at the two boolean checks.
                if (self.grants_city_state_unique_bonus(pid, "Stockholm")
                    || self.grants_city_state_unique_bonus(pid, "Bologna"))
                    && c.buildings.iter().any(|building| {
                        !c.pillaged_buildings.contains(building)
                            && self.rules.buildings.get(building).is_some_and(|spec| {
                                spec.district
                                    .is_some_and(|family| self.district_is_family(d, family))
                            })
                    })
                {
                    let kinds: &[&str] = match self.district_family(*d).as_str() {
                        "encampment" => &["general"],
                        "harbor" => &["admiral"],
                        "campus" => &["scientist"],
                        "holy_site" => &["prophet"],
                        "theater_square" => &["writer", "artist", "musician"],
                        "industrial_zone" => &["engineer"],
                        "commercial_hub" => &["merchant"],
                        _ => &[],
                    };
                    for kind in kinds {
                        *city_earn.entry((*kind).to_string()).or_insert(0.0) += 1.0;
                    }
                }
                if d == "lavra" {
                    if self.city_has_active_building_family(c, crate::name!("shrine")) {
                        *city_earn.entry("writer".to_string()).or_insert(0.0) += 1.0;
                    }
                    if self.city_has_active_building_family(c, crate::name!("temple")) {
                        *city_earn.entry("artist".to_string()).or_insert(0.0) += 1.0;
                    }
                    let has_worship_building = c.buildings.iter().any(|building| {
                        !c.pillaged_buildings.contains(building)
                            && self.building_district_is_active(c, building)
                            && self.rules.buildings.get(building).is_some_and(|spec| {
                                spec.district == Some(crate::name!("holy_site"))
                                    && spec.requires.iter().any(|required| required == "temple")
                            })
                    });
                    if has_worship_building {
                        *city_earn.entry("musician".to_string()).or_insert(0.0) += 1.0;
                    }
                }
            }
            for b in &c.buildings {
                if c.pillaged_buildings.contains(b) || !self.building_district_is_active(c, b) {
                    continue;
                }
                for (kind, points) in &self.rules.buildings[b].great_person_points {
                    *city_earn.entry(kind.clone()).or_insert(0.0) += *points;
                }
                let add = |earn: &mut BTreeMap<String, f64>, kind: &str, amount: f64| {
                    if amount != 0.0 {
                        *earn.entry(kind.to_string()).or_insert(0.0) += amount;
                    }
                };
                match b.as_str() {
                    "art_museum" => add(
                        &mut city_earn,
                        "artist",
                        self.policy_effect(pid, "gpp_artist_per_art_museum"),
                    ),
                    "workshop" => add(
                        &mut city_earn,
                        "engineer",
                        self.policy_effect(pid, "gpp_engineer_per_workshop"),
                    ),
                    "broadcast_center" => add(
                        &mut city_earn,
                        "musician",
                        self.policy_effect(pid, "gpp_musician_per_broadcast_center"),
                    ),
                    "armory" => add(
                        &mut city_earn,
                        "general",
                        self.policy_effect(pid, "gpp_general_per_armory"),
                    ),
                    "military_academy" => add(
                        &mut city_earn,
                        "general",
                        self.policy_effect(pid, "gpp_general_per_military_academy"),
                    ),
                    _ => {}
                }
                // Laissez-Faire and Nobel Prize grant a different amount per
                // building tier rather than one flat number per card.
                if b.as_str() == "bank" {
                    add(
                        &mut city_earn,
                        "merchant",
                        self.policy_effect(pid, "gpp_merchant_per_bank"),
                    );
                }
                if b.as_str() == "stock_exchange" {
                    add(
                        &mut city_earn,
                        "merchant",
                        self.policy_effect(pid, "gpp_merchant_per_stock_exchange"),
                    );
                }
                if b.as_str() == "shipyard" {
                    add(
                        &mut city_earn,
                        "admiral",
                        self.policy_effect(pid, "gpp_admiral_per_shipyard"),
                    );
                }
                if b.as_str() == "seaport" {
                    add(
                        &mut city_earn,
                        "admiral",
                        self.policy_effect(pid, "gpp_admiral_per_seaport"),
                    );
                }
                if b.as_str() == "university" {
                    add(
                        &mut city_earn,
                        "scientist",
                        self.policy_effect(pid, "gpp_scientist_per_university"),
                    );
                }
                if b.as_str() == "research_lab" {
                    add(
                        &mut city_earn,
                        "scientist",
                        self.policy_effect(pid, "gpp_scientist_per_research_lab"),
                    );
                }
                if b.as_str() == "factory" {
                    add(
                        &mut city_earn,
                        "engineer",
                        self.policy_effect(pid, "gpp_engineer_per_factory"),
                    );
                }
                if matches!(
                    b.as_str(),
                    "coal_power_plant" | "oil_power_plant" | "nuclear_power_plant"
                ) {
                    add(
                        &mut city_earn,
                        "engineer",
                        self.policy_effect(pid, "gpp_engineer_per_power_plant"),
                    );
                }
            }
            for wonder in c.wonders.keys() {
                for (kind, points) in &self.rules.wonders[wonder].great_person_points {
                    *city_earn.entry(kind.clone()).or_insert(0.0) += *points;
                }
            }
            let city_multiplier = 1.0 + self.governor_effect(pid, c.id, "great_people_pct") / 100.0;
            for (kind, amount) in city_earn {
                *earn.entry(kind).or_insert(0.0) += amount * city_multiplier;
            }
        }
        for kind in [
            "general",
            "admiral",
            "scientist",
            "prophet",
            "writer",
            "artist",
            "engineer",
            "merchant",
            "musician",
        ] {
            let amount = self.policy_effect(pid, &format!("gpp_{kind}"));
            if amount != 0.0 {
                *earn.entry(kind.to_string()).or_insert(0.0) += amount;
            }
        }
        // Exodus of the Evangelists' Golden Age half:
        // COMMEMORATION_RELIGIOUS_GA_GREAT_PROPHET_POINTS, +4 Great Prophet
        // points per turn at the player level. Live Rome (run
        // civvis-20260816T223457Z, t66) earned 8.04 Prophet points a turn
        // from a Holy Site, Shrine and Temple under Classical Republic —
        // (3 + 4) × 1.15 — where the model made 3.45; and with the Prophet
        // class exhausted the four became "+8 from excess Great Person
        // points" of Faith the model was short by.
        if self.dedication_active(pid, "exodus_of_the_evangelists") {
            *earn.entry("prophet".to_string()).or_insert(0.0) += 4.0;
        }
        let divine_spark = self.pantheon_effect(pid, "district_gpp");
        if divine_spark > 0.0 {
            for c in self.cities.values().filter(|c| c.owner == pid) {
                for (district, building, kind) in [
                    ("campus", Some("library"), "scientist"),
                    ("holy_site", None, "prophet"),
                    ("theater_square", Some("amphitheater"), "writer"),
                ] {
                    if self.city_has_active_district_family(c, Name::new(district))
                        && building.is_none_or(|building| {
                            self.city_has_active_building_family(c, Name::new(building))
                        })
                    {
                        let city_multiplier =
                            1.0 + self.governor_effect(pid, c.id, "great_people_pct") / 100.0;
                        *earn.entry(kind.to_string()).or_insert(0.0) +=
                            divine_spark * city_multiplier;
                    }
                }
            }
        }
        let mult = 1.0
            + (self.gov_effects(pid).great_people_pct
                + self.policy_effect(pid, "great_people_pct"))
                / 100.0;
        for (t, amt) in earn.iter_mut() {
            let congress_mult = if self.congress_effect_active("patronage", "A", t) {
                2.0
            } else if self.congress_effect_active("patronage", "B", t) {
                0.0
            } else {
                1.0
            };
            *amt *= mult * congress_mult;
        }
        earn
    }

    /// Accrue great person points and auto-claim on reaching the threshold
    /// (simplified: generic named-less great people with instant effects).
    ///
    /// Points of a class the empire can no longer earn still land in the
    /// bank — Firaxis's `GetPointsTotal` keeps counting them (773 Scientist
    /// points banked against a 660 cost, twenty turns after the last
    /// Scientist was claimed) — and are paid out again as Faith, one for one,
    /// which is where a late-game empire's Faith actually comes from
    /// (`unused_great_person_faith`).
    pub(super) fn process_great_people(&mut self, pid: usize) {
        if self.players[pid].is_minor {
            return;
        }
        let earn = self.great_person_points_per_turn(pid);
        let anarchy = self.in_anarchy(pid);
        // The World's Fair scores the points as they are generated, not the
        // people they eventually buy. Points that pay out as Faith because the
        // class has run dry were still generated, and Firaxis' `FromGreatPerson`
        // rows do not ask where they went.
        let generated: f64 = earn.values().map(|amount| amount.max(0.0)).sum();
        for (t, amt) in earn {
            if !anarchy && !self.great_person_class_earnable(pid, &t) {
                self.players[pid].faith += amt.max(0.0);
            }
            *self.players[pid].gpp.entry(t).or_insert(0.0) += amt;
        }
        self.score_great_person_point_competition(pid, generated);
        let due: Vec<String> = self.players[pid]
            .gpp
            .iter()
            .filter(|(t, pts)| **pts >= self.gp_cost(pid, t))
            .filter(|(t, _)| self.great_person_class_earnable(pid, t))
            .filter(|(t, _)| self.great_person_class_offered_now(pid, t))
            .map(|(t, _)| t.clone())
            .collect();
        for t in due {
            // A ready Great Merchant may be retired either for their named
            // effect or to found a Corporation. Preserve that player choice
            // instead of auto-activating the Merchant at turn start.
            if t == "merchant" && self.has_foundable_corporation(pid) {
                continue;
            }
            if self.current_great_person(&t).is_some_and(|(_, person)| {
                person.effects.contains_key("wonder_production") && !self.has_queued_wonder(pid)
            }) {
                continue;
            }
            let _ = self.claim_great_person(pid, &t, None);
        }
    }

    fn has_queued_wonder(&self, pid: usize) -> bool {
        self.cities.values().any(|city| {
            city.owner == pid && matches!(city.queue.first(), Some(Item::Wonder { .. }))
        })
    }

    fn has_great_person_district(&self, pid: usize, family: impl AsName) -> bool {
        self.cities
            .values()
            .any(|city| city.owner == pid && self.city_has_active_district_family(city, family))
    }

    /// A live bridge may know that Firaxis's named offer has a prerequisite
    /// CIVVIS's current generic person does not. For example, a Campus empire
    /// can model a Scientist as usable while the host is actually offering
    /// Hildegard of Bingen, who cannot activate without a Holy Site.
    ///
    /// This is deliberately player-local and defaults empty: ordinary games,
    /// saved games, and the frozen tournament anchor retain their historical
    /// offer model unless an authoritative host export supplies a blocker.
    pub fn live_great_person_offer_blocker(&self, pid: usize, kind: &str) -> Option<&str> {
        self.players[pid]
            .live_great_person_offer_blockers
            .get(kind)
            .map(String::as_str)
    }

    /// The normalized named individual presently offered by the host for a
    /// Great Person class. Unlike [`Self::current_great_person`], this is
    /// host authority rather than CIVVIS's native roster, so an AI can value
    /// the actual effect before deciding whether a project race is worthwhile.
    pub fn live_great_person_offer_individual(&self, pid: usize, kind: &str) -> Option<&str> {
        self.players[pid]
            .live_great_person_offer_individuals
            .get(kind)
            .map(String::as_str)
    }

    /// Whether this class is presently recruitable on the host's Great People
    /// screen. The live set is intentionally narrower than
    /// [`Self::great_person_class_earnable`]: a class can have someone later
    /// in Firaxis's timeline without being today's offer. Native and older
    /// mirrored boards retain the historical roster because they leave the
    /// set as `None`.
    pub fn great_person_class_offered_now(&self, pid: usize, kind: &str) -> bool {
        self.players[pid]
            .live_great_person_offers
            .as_ref()
            .is_none_or(|offers| offers.contains(kind))
    }

    /// Why this Great Person cannot be claimed right now, if they cannot.
    /// The reasons are written for a person to read: the observation hands
    /// them to the client so a card with enough points but no Recruit button
    /// can say what is missing instead of looking broken.
    pub(crate) fn great_person_blocker(&self, pid: usize, kind: &str) -> Option<String> {
        let (_, spec) = self.current_great_person(kind)?;
        self.validate_great_person_activation(pid, kind, spec).err()
    }

    pub(super) fn validate_great_person_activation(
        &self,
        pid: usize,
        kind: &str,
        spec: &crate::rules::GreatPersonSpec,
    ) -> Result<(), String> {
        if let Some(blocker) = self.live_great_person_offer_blocker(pid, kind) {
            return Err(blocker.to_string());
        }
        // The live named offer is authoritative. Its exported blocker above
        // carries the host's actual district requirement; applying CIVVIS's
        // locally-current *different* person afterward can invent a false
        // Wonder, Campus, or Industrial Zone gate. Native/older boards keep
        // the complete local activation model.
        let live_named_offer = self.live_great_person_offer_individual(pid, kind).is_some();
        if !live_named_offer
            && kind == "scientist"
            && !self.has_great_person_district(pid, crate::name!("campus"))
        {
            return Err("this Great Scientist requires an active Campus".into());
        }
        if !live_named_offer
            && kind == "engineer"
            && !spec.effects.contains_key("wonder_production")
            && !self.has_great_person_district(pid, crate::name!("industrial_zone"))
        {
            return Err("this Great Engineer requires an active Industrial Zone".into());
        }
        if kind == "prophet" {
            let claimed = self.religions_founded()
                + self
                    .players
                    .iter()
                    .filter(|player| player.prophet_pending)
                    .count();
            if self.players[pid].religion.is_some() || self.players[pid].prophet_pending {
                return Err("this civilization has already attracted a Great Prophet".into());
            }
            if claimed >= self.max_religions() {
                return Err("all available religions already have Great Prophets".into());
            }
            let has_site = self.has_great_person_district(pid, crate::name!("holy_site"))
                || self.cities.values().any(|city| {
                    city.owner == pid && city.wonders.contains_key(&crate::name!("stonehenge"))
                });
            if !has_site {
                return Err("this Great Prophet requires an active Holy Site or Stonehenge".into());
            }
        }
        if !live_named_offer
            && spec.effects.contains_key("wonder_production")
            && !self.has_queued_wonder(pid)
        {
            return Err("this Great Engineer requires a Wonder under construction".into());
        }
        if !live_named_offer
            && (spec.effects.contains_key("free_trader")
                || spec.effects.contains_key("destination_foreign_trade_gold")
                || spec.effects.contains_key("free_quadrireme")
                || spec.effects.contains_key("free_lighthouse")
                || spec.effects.contains_key("free_shipyard"))
            && self.great_person_trade_city(pid, kind).is_none()
        {
            return Err(format!(
                "this Great {} requires an active {}",
                if kind == "admiral" {
                    "Admiral"
                } else {
                    "Merchant"
                },
                if kind == "admiral" {
                    "Harbor"
                } else {
                    "Commercial Hub"
                }
            ));
        }
        if !live_named_offer
            && spec.effects.contains_key("land_unit_promotion_level")
            && self.great_person_promotion_unit(pid).is_none()
        {
            return Err("this Great General requires a promotable military land unit".into());
        }
        if !live_named_offer {
            for (effect, domain, title) in [
                ("land_unit_formation", "land", "General"),
                ("naval_unit_formation", "sea", "Admiral"),
            ] {
                if let Some(formation) = spec.effects.get(effect) {
                    if self
                        .great_person_formation_unit(pid, *formation as u8, domain)
                        .is_none()
                    {
                        return Err(format!(
                            "this Great {title} requires a military {domain} unit"
                        ));
                    }
                }
            }
        }
        if !live_named_offer {
            for (effect, work) in [
                ("great_work_writing", "writing"),
                ("great_work_art", "art"),
                ("great_work_music", "music"),
            ] {
                let count = spec.effects.get(effect).copied().unwrap_or(0.0).round() as usize;
                if count > 0 && !self.can_house_great_works(pid, work, count) {
                    return Err(format!(
                        "this Great Person requires {count} open {work} Great Work slots"
                    ));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn can_activate_current_great_person(&self, pid: usize, kind: &str) -> bool {
        self.current_great_person(kind).is_some_and(|(_, spec)| {
            self.validate_great_person_activation(pid, kind, spec)
                .is_ok()
        })
    }

    pub(super) fn claim_great_person(
        &mut self,
        pid: usize,
        kind: &str,
        patronage: Option<&str>,
    ) -> Result<(), String> {
        if !self.great_person_class_offered_now(pid, kind) {
            return Err("Great Person class is not currently offered by the live game".into());
        }
        let (id, spec) = self
            .current_great_person(kind)
            .map(|(id, spec)| (id.to_string(), spec.clone()))
            .ok_or_else(|| "no Great Person of that type is currently available".to_string())?;
        self.validate_great_person_activation(pid, kind, &spec)?;
        let points = self.players[pid].gpp.get(kind).copied().unwrap_or(0.0);
        let cost = self.gp_cost(pid, kind);
        let missing = (cost - points).max(0.0);
        match patronage {
            None if missing > 0.0 => return Err("not enough Great Person points".into()),
            Some("gold") => {
                let price = self
                    .great_person_patronage_price(pid, kind, "gold")
                    .ok_or_else(|| "cannot patronize with Gold".to_string())?;
                if missing <= 0.0 || self.players[pid].gold < price {
                    return Err("cannot patronize with Gold".into());
                }
                self.players[pid].gold -= price;
            }
            Some("faith") => {
                let price = self
                    .great_person_patronage_price(pid, kind, "faith")
                    .ok_or_else(|| "cannot patronize with Faith".to_string())?;
                if missing <= 0.0 || self.players[pid].faith < price {
                    return Err("cannot patronize with Faith".into());
                }
                self.players[pid].faith -= price;
            }
            Some(_) => return Err("patronage currency must be gold or faith".into()),
            None => {}
        }
        self.players[pid].gpp.insert(kind.to_string(), 0.0);
        if let Some(offers) = self.players[pid].live_great_person_offers.as_mut() {
            // Native recruitment creates a physical unit. Its separate
            // activation (and any recruitment bonuses) must come from the
            // next host observation, not the effects of our fallback roster.
            // Consume only this observed offer so the same plan cannot buy
            // another local individual before the native timeline refreshes.
            offers.remove(kind);
            *self.players[pid]
                .gp_claimed
                .entry(kind.to_string())
                .or_insert(0) += 1;
            let individual = self
                .live_great_person_offer_individual(pid, kind)
                .unwrap_or(kind)
                .to_string();
            self.note(
                pid,
                "People",
                format!("recruited native {individual}"),
                None,
            );
            return Ok(());
        }
        self.retired_great_people.insert(id.clone());
        self.note(pid, "People", format!("recruited {}", spec.name), None);
        self.players[pid].great_people.push(id);
        *self.players[pid]
            .gp_claimed
            .entry(kind.to_string())
            .or_insert(0) += 1;
        self.add_historic_moment(
            pid,
            if patronage == Some("faith") && missing > cost / 2.0 {
                "MOMENT_GREAT_PERSON_CREATED_PATRONAGE_FAITH_OVER_HALF"
            } else if patronage == Some("gold") && missing > cost / 2.0 {
                "MOMENT_GREAT_PERSON_CREATED_PATRONAGE_GOLD_OVER_HALF"
            } else if spec.era < self.world_era {
                "MOMENT_GREAT_PERSON_CREATED_PAST_ERA"
            } else {
                "MOMENT_GREAT_PERSON_CREATED_GAME_ERA"
            },
        );
        self.dedication_trigger(pid, "great_person", 1);
        bump(&mut self.players[pid], "great_people");
        let activations = spec.charges
            + if kind == "engineer" {
                self.empire_wonder_effect(pid, "great_engineer_charges") as usize
            } else {
                0
            };
        for _ in 0..activations {
            self.named_great_person_effect(pid, &spec);
            self.vatican_great_person_pressure(pid);
        }
        self.apply_great_person_district_effects(pid);
        self.refresh_great_person_offers();
        Ok(())
    }

    /// Great People are recruited and expended immediately in this rules
    /// model. When more than one city could host the activation, use the
    /// largest eligible city (stable city ID breaks ties).
    pub(super) fn apply_great_person_district_effects(&mut self, pid: usize) {
        let target = self
            .cities
            .values()
            .filter(|city| city.owner == pid)
            .filter(|city| self.city_district_effect(city, "border_growth_on_great_person") > 0.0)
            .max_by_key(|city| (city.pop, Reverse(city.id)))
            .map(|city| city.id);
        if let Some(city) = target {
            self.expand_borders(city);
        }
    }

    fn vatican_great_person_pressure(&mut self, pid: usize) {
        if !self.grants_city_state_unique_bonus(pid, "Vatican City") {
            return;
        }
        let Some(source) = self
            .cities
            .values()
            .filter(|city| city.owner == pid)
            .max_by_key(|city| (city.pop, Reverse(city.id)))
            .map(|city| city.id)
        else {
            return;
        };
        let religion = self.players[pid].religion.clone().or_else(|| {
            self.city_religion(&self.cities[&source])
                .map(str::to_string)
        });
        let Some(religion) = religion else {
            return;
        };
        let source_pos = self.cities[&source].pos;
        let targets: Vec<u32> = self
            .cities
            .values()
            .filter(|city| self.wdist(source_pos, city.pos) <= 10)
            .filter(|city| !self.city_ignores_foreign_religion(city, &religion))
            .map(|city| city.id)
            .collect();
        for cid in targets {
            *self
                .cities
                .get_mut(&cid)
                .unwrap()
                .pressure
                .entry(religion.clone())
                .or_insert(0.0) += 400.0;
        }
        self.check_religious_victory();
    }

    pub(super) fn named_great_person_effect(
        &mut self,
        pid: usize,
        spec: &crate::rules::GreatPersonSpec,
    ) {
        if let Some(amount) = spec.effects.get("gold") {
            self.players[pid].gold += *amount;
        }
        if let Some(amount) = spec.effects.get("envoys") {
            self.players[pid].envoys_free += *amount as i64;
        }
        if let Some(amount) = spec.effects.get("trade_capacity") {
            *self.players[pid]
                .counters
                .entry("great_person_trade_capacity".to_string())
                .or_insert(0) += *amount as i64;
        }
        if spec.effects.get("annex_tile").copied().unwrap_or(0.0) > 0.0 {
            if let Some(city) = self
                .player_city_ids(pid)
                .into_iter()
                .max_by_key(|city| (self.cities[city].pop, Reverse(*city)))
            {
                self.expand_borders(city);
            }
        }
        if let Some(city) = self.great_person_trade_city(pid, &spec.kind) {
            if spec.effects.get("free_trader").copied().unwrap_or(0.0) > 0.0 {
                self.place_new_unit("trader", pid, self.cities[&city].pos);
            }
            if spec.effects.get("free_quadrireme").copied().unwrap_or(0.0) > 0.0 {
                self.place_new_unit("quadrireme", pid, self.cities[&city].pos);
            }
            if let Some(amount) = spec.effects.get("destination_foreign_trade_gold") {
                self.cities
                    .get_mut(&city)
                    .unwrap()
                    .great_person_foreign_route_gold += *amount;
            }
            for (effect, family) in [
                ("free_lighthouse", "lighthouse"),
                ("free_shipyard", "shipyard"),
            ] {
                if spec.effects.get(effect).copied().unwrap_or(0.0) > 0.0 {
                    self.grant_free_building_family(pid, city, family);
                }
            }
        }
        for (effect, counter) in [
            ("oil_per_turn", "great_person:oil_per_turn"),
            (
                "strategic_destination_trade_gold",
                "great_person:strategic_destination_trade_gold",
            ),
            (
                "naval_flanking_bonus_pct",
                "great_person:naval_flanking_bonus_pct",
            ),
            (
                "naval_ranged_production_pct",
                "great_person:naval_ranged_production_pct",
            ),
        ] {
            if let Some(amount) = spec.effects.get(effect) {
                *self.players[pid]
                    .counters
                    .entry(counter.to_string())
                    .or_insert(0) += amount.round() as i64;
            }
        }
        if let Some(amount) = spec.effects.get("land_unit_promotion_level") {
            for _ in 0..amount.round() as usize {
                let Some(unit) = self.great_person_promotion_unit(pid) else {
                    break;
                };
                let level = self.units[&unit].level;
                self.units.get_mut(&unit).unwrap().xp =
                    Self::promotion_threshold(level).max(self.units[&unit].xp);
            }
        }
        for (effect, domain) in [
            ("land_unit_formation", "land"),
            ("naval_unit_formation", "sea"),
        ] {
            if let Some(formation) = spec.effects.get(effect) {
                let formation = (*formation as u8).min(2);
                if let Some(unit) = self.great_person_formation_unit(pid, formation, domain) {
                    self.set_unit_formation(unit, formation);
                    if formation == 1 {
                        bump(&mut self.players[pid], "corps");
                    }
                    self.note_formation_moment(pid, unit);
                }
            }
        }
        if let Some(amount) = spec.effects.get("tech_boosts") {
            self.grant_random_boosts(pid, *amount as usize, true);
        }
        if let Some(amount) = spec.effects.get("modern_atomic_tech_boosts") {
            self.grant_random_tech_boosts_in_eras(pid, *amount as usize, 5, 6);
        }
        if let Some(amount) = spec.effects.get("modern_tech_boosts") {
            self.grant_random_tech_boosts_in_eras(pid, *amount as usize, 5, 5);
        }
        if let Some(amount) = spec.effects.get("city_production") {
            if let Some(cid) = self
                .player_city_ids(pid)
                .into_iter()
                .max_by_key(|cid| self.cities[cid].pop)
            {
                self.cities.get_mut(&cid).unwrap().production += *amount;
            }
        }
        for (effect, counter) in [
            ("libraries_science", "great_person:library_science"),
            ("universities_science", "great_person:university_science"),
            ("research_labs_science", "great_person:research_lab_science"),
            ("workshops_culture", "great_person:workshop_culture"),
        ] {
            if let Some(amount) = spec.effects.get(effect) {
                *self.players[pid]
                    .counters
                    .entry(counter.to_string())
                    .or_insert(0) += amount.round() as i64;
            }
        }
        if spec.effects.contains_key("free_library") || spec.effects.contains_key("free_university")
        {
            let target = self
                .cities
                .values()
                .filter(|city| city.owner == pid)
                .filter(|city| self.city_has_active_district_family(city, crate::name!("campus")))
                .max_by_key(|city| (city.pop, Reverse(city.id)))
                .map(|city| city.id);
            if let Some(city_id) = target {
                for (effect, family) in [
                    ("free_library", "library"),
                    ("free_university", "university"),
                ] {
                    if spec.effects.get(effect).copied().unwrap_or(0.0) <= 0.0
                        || self.city_has_building_family(&self.cities[&city_id], Name::new(family))
                    {
                        continue;
                    }
                    let building = self
                        .rules
                        .buildings
                        .iter()
                        .find(|(_, candidate)| {
                            candidate.replaces == Some(Name::new(family))
                                && candidate
                                    .unique_to
                                    .as_deref()
                                    .is_some_and(|owner| self.owns_civ_unique(pid, owner))
                        })
                        .map(|(name, _)| Name::new(name))
                        .unwrap_or_else(|| Name::new(family));
                    let city = self.cities.get_mut(&city_id).unwrap();
                    city.buildings.push(building);
                    city.building_eras.insert(building, self.world_era);
                    self.note_building_completed_moments(pid, &building);
                }
            }
        }
        if let Some(amount) = spec.effects.get("wonder_production") {
            let target = self
                .player_city_ids(pid)
                .into_iter()
                .filter(|city| matches!(self.cities[city].queue.first(), Some(Item::Wonder { .. })))
                .max_by_key(|city| (self.cities[city].pop, Reverse(*city)));
            if let Some(city) = target {
                let wonder = match self.cities[&city].queue.first() {
                    Some(Item::Wonder { wonder, .. }) => *wonder,
                    _ => unreachable!(),
                };
                let wonder_spec = &self.rules.wonders[wonder];
                let wonder_era = wonder_spec
                    .tech
                    .as_deref()
                    .and_then(|technology| self.rules.techs.get(technology))
                    .map(|technology| technology.era)
                    .or_else(|| {
                        wonder_spec
                            .civic
                            .as_deref()
                            .and_then(|civic| self.rules.civics.get(civic))
                            .map(|civic| civic.era)
                    })
                    .unwrap_or(0);
                let multiplier = if wonder_era <= 1 {
                    spec.effects
                        .get("ancient_classical_wonder_multiplier")
                        .copied()
                        .unwrap_or(1.0)
                } else {
                    1.0
                };
                self.cities.get_mut(&city).unwrap().production += amount * multiplier;
            }
        }
        for (effect, kind) in [
            ("great_work_writing", "writing"),
            ("great_work_art", "art"),
            ("great_work_music", "music"),
        ] {
            if let Some(amount) = spec.effects.get(effect).copied() {
                for _ in 0..amount as usize {
                    self.grant_great_work(pid, kind, spec.era, &spec.name);
                }
            }
        }
        // The existing class effects cover Prophets and military people and
        // remain the fallback primitive for modded named entries.
        if spec.effects.contains_key("found_religion")
            || spec.effects.contains_key("military_promotion")
            || spec.effects.contains_key("naval_promotion")
        {
            self.great_person_effect(pid, &spec.kind);
        }
    }

    fn great_person_trade_city(&self, pid: usize, kind: &str) -> Option<u32> {
        let district = if kind == "admiral" {
            "harbor"
        } else {
            "commercial_hub"
        };
        self.cities
            .values()
            .filter(|city| city.owner == pid)
            .filter(|city| self.city_has_active_district_family(city, Name::new(district)))
            .max_by_key(|city| (city.pop, Reverse(city.id)))
            .map(|city| city.id)
    }

    fn great_person_land_units(&self, pid: usize) -> impl Iterator<Item = u32> + '_ {
        self.units.values().filter_map(move |unit| {
            let spec = &self.rules.units[unit.kind];
            (unit.owner == pid
                && spec.class == "military"
                && !matches!(spec.domain.as_deref(), Some("sea" | "air")))
            .then_some(unit.id)
        })
    }

    fn strongest_great_person_land_unit<I>(&self, units: I) -> Option<u32>
    where
        I: Iterator<Item = u32>,
    {
        units.max_by_key(|unit| {
            let state = &self.units[unit];
            let spec = &self.rules.units[state.kind];
            (
                (spec.strength + self.unit_formation_bonus(state)).round() as i64,
                Reverse(*unit),
            )
        })
    }

    fn great_person_promotion_unit(&self, pid: usize) -> Option<u32> {
        self.strongest_great_person_land_unit(self.great_person_land_units(pid).filter(|unit| {
            let state = &self.units[unit];
            let spec = &self.rules.units[state.kind];
            spec.earns_xp
                && !spec.promotion_class.is_empty()
                && state.level < 8
                && !self.promotion_pending(*unit)
        }))
    }

    fn great_person_formation_unit(&self, pid: usize, formation: u8, domain: &str) -> Option<u32> {
        self.units
            .values()
            .filter(|unit| {
                let spec = &self.rules.units[unit.kind];
                let domain_matches = if domain == "sea" {
                    spec.domain.as_deref() == Some("sea")
                } else {
                    !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                };
                unit.owner == pid
                    && spec.class == "military"
                    && spec.can_formations
                    && domain_matches
                    && unit.formation < formation
            })
            .max_by_key(|unit| {
                let spec = &self.rules.units[unit.kind];
                (
                    (spec.strength + self.unit_formation_bonus(unit)).round() as i64,
                    Reverse(unit.id),
                )
            })
            .map(|unit| unit.id)
    }

    pub(super) fn grant_free_building_family(&mut self, pid: usize, city_id: u32, family: &str) {
        if self.city_has_building_family(&self.cities[&city_id], Name::new(family)) {
            return;
        }
        let building = self
            .rules
            .buildings
            .iter()
            .find(|(_, candidate)| {
                candidate.replaces == Some(Name::new(family))
                    && candidate
                        .unique_to
                        .as_deref()
                        .is_some_and(|owner| self.owns_civ_unique(pid, owner))
            })
            .map(|(name, _)| Name::new(name))
            .unwrap_or_else(|| Name::new(family));
        let city = self.cities.get_mut(&city_id).unwrap();
        city.buildings.push(building);
        city.building_eras.insert(building, self.world_era);
        self.note_building_completed_moments(pid, &building);
    }

    pub(super) fn do_recruit_great_person(&mut self, pid: usize, kind: &str) -> Result<(), String> {
        self.claim_great_person(pid, kind, None)
    }

    pub(super) fn do_patronize_great_person(
        &mut self,
        pid: usize,
        kind: &str,
        currency: &str,
    ) -> Result<(), String> {
        self.claim_great_person(pid, kind, Some(currency))
    }

    /// Simplified instant retirement effects for a claimed great person.
    pub(super) fn great_person_effect(&mut self, pid: usize, kind: &str) {
        match kind {
            "scientist" => self.grant_random_boosts(pid, 2, true),
            "artist" => self.grant_random_boosts(pid, 2, false),
            "engineer" => {
                let best = self
                    .cities
                    .values()
                    .filter(|c| c.owner == pid)
                    .max_by(|a, b| {
                        a.production
                            .partial_cmp(&b.production)
                            .unwrap()
                            .then(a.id.cmp(&b.id))
                    })
                    .map(|c| c.id);
                if let Some(cid) = best {
                    self.cities.get_mut(&cid).unwrap().production += 150.0;
                }
            }
            "merchant" => {
                self.players[pid].gold += 200.0;
                self.players[pid].envoys_free += 1;
            }
            "prophet" => {
                let can_found = self.players[pid].religion.is_none()
                    && self.religions_founded() < self.max_religions()
                    && (self.has_great_person_district(pid, crate::name!("holy_site"))
                        || self.cities.values().any(|city| {
                            city.owner == pid
                                && city.wonders.contains_key(&crate::name!("stonehenge"))
                        }));
                if can_found {
                    self.players[pid].prophet_pending = true;
                } else {
                    self.players[pid].faith += 100.0;
                }
            }
            "general" | "admiral" => {
                let sea = kind == "admiral";
                for uid in self.player_unit_ids(pid) {
                    let spec = &self.rules.units[self.units[&uid].kind];
                    if spec.class == "military" && (spec.domain.as_deref() == Some("sea")) == sea {
                        let u = self.units.get_mut(&uid).unwrap();
                        u.level = (u.level + 1).min(8);
                    }
                }
                if sea {
                    self.players[pid].gold += 100.0;
                }
            }
            _ => {}
        }
    }

    fn grant_random_tech_boosts_in_eras(
        &mut self,
        pid: usize,
        n: usize,
        minimum_era: usize,
        maximum_era: usize,
    ) {
        let game_speed = self.game_speed;
        for _ in 0..n {
            let candidates: Vec<(Name, f64)> = self
                .rules
                .techs
                .iter()
                .filter(|(name, tech)| {
                    (minimum_era..=maximum_era).contains(&tech.era)
                        && !self.players[pid].techs.contains(*name)
                        && !self.players[pid].boosted_techs.contains(*name)
                })
                .map(|(name, tech)| (*name, game_speed.scale(tech.cost)))
                .collect();
            if candidates.is_empty() {
                return;
            }
            let (name, cost) = candidates[self.rng.below(candidates.len())];
            let fraction = self.node_boost_frac(pid, &name, true);
            let player = &mut self.players[pid];
            player.boosted_techs.insert(Name::new(&name));
            if player.research.as_deref() == Some(name.as_str()) {
                player.research_progress += fraction * cost;
            }
        }
    }

    // -------------------------------------------------- city-state envoys

    pub fn city_happiness(&self, city: &City) -> &'static str {
        match self.city_amenity_surplus(city) {
            5.. => "ecstatic",
            3..=4 => "happy",
            0..=2 => "content",
            -2..=-1 => "displeased",
            -4..=-3 => "unhappy",
            -6..=-5 => "unrest",
            _ => "revolt",
        }
    }

    /// Scottish Enlightenment has four city-local happiness modifiers. Its
    /// Happy values apply at a +3 surplus and its Ecstatic values are exactly
    /// double at +5; Content and lower bands receive none of the trait.
    pub(super) fn scottish_enlightenment_happiness_scale(&self, city: &City) -> f64 {
        if !self.owns_civ_unique(city.owner, "Scotland") {
            return 0.0;
        }
        match self.city_happiness(city) {
            "ecstatic" => 2.0,
            "happy" => 1.0,
            _ => 0.0,
        }
    }

    pub(crate) fn can_house_great_works(&self, pid: usize, kind: &str, additional: usize) -> bool {
        if let Some(memo) = self.query_memo.gw_housing.borrow().as_ref() {
            if let Some(answer) = memo.get(&(pid, kind.to_string(), additional)) {
                return *answer;
            }
        }
        let answer = self.can_house_great_works_uncached(pid, kind, additional);
        if let Some(memo) = self.query_memo.gw_housing.borrow_mut().as_mut() {
            memo.insert((pid, kind.to_string(), additional), answer);
        }
        answer
    }

    fn can_house_great_works_uncached(&self, pid: usize, kind: &str, additional: usize) -> bool {
        let count = |allocation: &BTreeMap<u32, BTreeMap<String, usize>>| {
            allocation
                .values()
                .flat_map(|works| works.values())
                .sum::<usize>()
        };
        let before = count(&self.housed_great_works(pid));
        count(&self.housed_great_works_with_extra(pid, Some((kind, additional))))
            == before + additional
    }

    /// Great People are recruited and retired immediately in this rules
    /// model, so they cannot physically stand beside a battle. Preserve the
    /// shipped Moment's useful semantics by letting each recruited General or
    /// Admiral oversee one future offensive in its matching domain.
    pub(super) fn note_great_person_assisted_kill(&mut self, pid: usize, attacker: &Unit) {
        if self.rules.units[attacker.kind].class != "military" {
            return;
        }
        let kind = match self.rules.units[attacker.kind].domain.as_deref() {
            Some("sea") => "admiral",
            Some("air") => return,
            _ => "general",
        };
        let person = self.players[pid].great_people.iter().find(|person| {
            self.rules
                .great_people
                .get(person)
                .is_some_and(|spec| spec.kind == kind)
                && !self.players[pid]
                    .counters
                    .contains_key(&format!("historic_moment:assisted_kill:{person}"))
        });
        if let Some(person) = person.cloned() {
            let moment = if kind == "admiral" {
                "MOMENT_UNIT_KILLED_ASSISTED_BY_ADMIRAL"
            } else {
                "MOMENT_UNIT_KILLED_ASSISTED_BY_GENERAL"
            };
            self.first_historic_moment(pid, &format!("assisted_kill:{person}"), Some(moment), None);
        }
    }
}
