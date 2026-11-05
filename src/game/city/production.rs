//! The production catalogue: what a city may produce, what it costs, and
//! what blocks it.
//!
//! Whether a city can produce an item or a unit (`can_produce`,
//! `can_produce_unit`) and the full list it may (`producible_items`); an
//! item's cost, base and in a given city (`item_cost`, `base_item_cost`,
//! `item_cost_for`, `item_cost_for_city`), what is already invested and
//! what remains, a unit's strategic-resource cost, and a building's
//! purchase price; the keys a queued item is tracked by; pausing a city's
//! production and dropping obsolete items; and what the live host says:
//! its production menu gates, production turns and purchase prices, the
//! production and purchases it blocks, and the resource prices it charges.
//!
//! Moved verbatim out of the `impl Game` block in `game/city.rs`
//! (2026-11-05). Methods the rest of `city` calls are `pub(super)`;
//! methods that were `pub(super)` (all of `game`) are
//! `pub(in crate::game)`, the same reach; the code is otherwise unchanged.

use super::*;

impl Game {
    pub fn item_cost(&self, item: &Item) -> f64 {
        self.game_speed.scale(self.base_item_cost(item))
    }

    pub(in crate::game) fn base_item_cost(&self, item: &Item) -> f64 {
        match item {
            Item::Formation { unit, formation } => {
                // UNIT_CORPS_COST_MODIFIER 1.5 and UNIT_ARMY_COST_MODIFIER 2.0.
                // Both are read against the base unit's cost, so an Army is 2x
                // rather than the 1.5 x 1.5 an extrapolated Corps rate gives.
                // The resource cost is separate and really does step 2 then 3,
                // one per unit folded in -- see unit_resource_cost.
                self.rules.units[unit].cost * if *formation >= 2 { 2.0 } else { 1.5 }
            }
            Item::Unit { unit } => self.rules.units[unit].cost,
            Item::Building { building } => self.rules.buildings[building].cost,
            Item::District { district, .. } => self.rules.districts[district].cost,
            Item::Wonder { wonder, .. } => self.rules.wonders[wonder].cost,
            Item::Repair { repair, pos } => {
                if repair == "district" {
                    self.map
                        .get(*pos)
                        .and_then(|tile| tile.district.as_ref())
                        .and_then(|district| self.rules.districts.get(district))
                        .map(|spec| spec.cost * 0.25)
                        .unwrap_or(1.0)
                } else {
                    self.rules
                        .buildings
                        .get(repair)
                        .map(|spec| spec.cost * 0.25)
                        .unwrap_or(1.0)
                }
            }
            Item::Project { project } => self.rules.projects[project].cost,
            Item::Product { .. } => 500.0,
        }
    }

    pub fn item_cost_for(&self, pid: usize, item: &Item) -> f64 {
        let standard = self.base_item_cost(item);
        match item {
            Item::Unit { unit } if unit == "settler" => self.game_speed.scale(
                standard
                    + 30.0
                        * self.players[pid]
                            .counters
                            .get("trained:settler")
                            .copied()
                            .unwrap_or(0) as f64,
            ),
            Item::Unit { unit } if unit == "builder" => self.game_speed.scale(
                standard
                    + 4.0
                        * self.players[pid]
                            .counters
                            .get("trained:builder")
                            .copied()
                            .unwrap_or(0) as f64,
            ),
            Item::Project { project } => {
                // `GameOptions::ai_space_project_cost_pct`: 1 unless set.
                let standard = standard * self.space_project_cost_scale(pid, project.as_str());
                let maximum = self.rules.projects[project].cost_progression_max_pct;
                if maximum <= 0.0 {
                    self.game_speed.scale(standard)
                } else {
                    // GAME_PROGRESS linearly interpolates from the base to
                    // Param1 percent of base; 1500 therefore caps at 15x.
                    self.game_speed
                        .scale(
                            standard
                                * (1.0 + (maximum / 100.0 - 1.0) * self.game_progress_ratio(pid)),
                        )
                        .floor()
                }
            }
            _ => self.item_cost(item),
        }
    }

    /// Return the treasury cost of purchasing an ordinary city building.
    /// Gold follows Civ VI's four-times-Production rate; the narrower Faith
    /// purchase paths use the existing two-times rate. District purchase
    /// discounts apply to either currency.
    pub fn building_purchase_cost(
        &self,
        pid: usize,
        cid: u32,
        building: &str,
        currency: &str,
    ) -> Option<f64> {
        let city = self.cities.get(&cid).filter(|city| city.owner == pid)?;
        if city
            .queue
            .iter()
            .any(|item| matches!(item, Item::Building { building: queued } if queued == building))
        {
            return None;
        }
        if currency == "gold" {
            return self.building_gold_purchase_cost(pid, cid, building);
        }
        let spec = self.rules.buildings.get(building)?;
        if currency != "faith" {
            return None;
        }
        // ★ The host's own answer first — see `building_gold_purchase_cost`.
        let asked = Item::Building {
            building: Name::new(building),
        };
        if let Some(host) = self.host_purchase_price(cid, &asked, "faith") {
            return host.filter(|_| !self.purchase_is_blocked(cid, &asked));
        }
        let discount = self.city_district_effect(city, "gold_faith_purchase_discount_pct");
        Some(spec.cost * 2.0 * (1.0 - discount / 100.0).max(0.0))
    }

    /// City-sensitive production cost. Flood Barriers scale with the number
    /// of Coastal Lowland tiles they must protect; all other items retain the
    /// civilization-wide cost rules above.
    pub fn item_cost_for_city(&self, pid: usize, cid: u32, item: &Item) -> f64 {
        // ★ The host's own cost first, when its menu carries this item for
        // this city (`StateCity::buildable`): `GetXCost(row.Index)` after
        // every modifier the engine applies. A founded district keeps the cost
        // it was founded at, exactly as below.
        if !self.host_buildable.is_empty() {
            let founded = matches!(item, Item::District { district, pos }
                if self
                    .map
                    .get(*pos)
                    .and_then(|tile| tile.district_foundation.as_ref())
                    .is_some_and(|foundation| foundation.district == *district));
            if !founded {
                if let Some(cost) = self
                    .host_buildable
                    .get(&cid)
                    .and_then(|menu| menu.get(&Self::production_block_key(item)))
                    .and_then(|entry| entry.cost)
                    .filter(|cost| *cost > 0.0)
                {
                    return cost;
                }
            }
        }
        if let Item::District { district, pos } = item {
            if let Some(foundation) = self
                .map
                .get(*pos)
                .and_then(|tile| tile.district_foundation.as_ref())
            {
                if foundation.district == *district {
                    return foundation.cost;
                }
            }
            return self
                .game_speed
                .scale(self.district_cost_for_placement(pid, district, false));
        }
        if let Item::Building { building } = item {
            let spec = &self.rules.buildings[building];
            let district = spec
                .district
                .map(|district| self.district_family(district))
                .unwrap_or(crate::name!("city_center"));
            if self.congress_effect_active("urban_development_treaty", "B", district.as_str())
                || self.congress_effect_active("global_energy_treaty", "B", building)
            {
                return f64::INFINITY;
            }
            let per_lowland = spec
                .effects
                .get("cost_per_coastal_lowland")
                .copied()
                .unwrap_or(0.0);
            if per_lowland > 0.0 {
                let count = self.coastal_lowland_tiles(&self.cities[&cid]).len().max(1);
                let flood_level = (self.climate_phase / 2) as f64;
                return self
                    .game_speed
                    .scale(per_lowland * count as f64 * (1.0 + flood_level));
            }
        }
        let mut cost = self.item_cost_for(pid, item);
        let military = match item {
            Item::Unit { unit } | Item::Formation { unit, .. } => {
                self.rules.units[unit].class == "military"
            }
            _ => false,
        };
        if military {
            if self.congress_effect_active("mercenary_companies", "A", "production") {
                cost *= 2.0;
            } else if self.congress_effect_active("mercenary_companies", "B", "production") {
                cost *= 0.5;
            }
        }
        if let Item::Building { building } = item {
            if self.congress_effect_active("global_energy_treaty", "A", building) {
                cost *= 0.5;
            }
        }
        cost
    }

    /// Work already assigned to this exact item, including a paused build.
    /// An idle city's unassigned overflow is not an investment in every menu item.
    pub(crate) fn item_invested_production(&self, cid: u32, item: &Item) -> f64 {
        let city = &self.cities[&cid];
        let saved = city
            .production_progress
            .get(&Self::item_progress_key(item))
            .copied()
            .unwrap_or(0.0);
        saved
            + if city.queue.first() == Some(item) {
                city.production
            } else {
                0.0
            }
    }

    /// Pause a city's queue while keeping its paid work on the current item.
    /// A replacement may resume its own saved work, but cannot spend this
    /// item's progress as overflow. An already empty queue keeps its overflow.
    pub fn pause_city_production(&mut self, cid: u32) -> bool {
        let Some(item) = self
            .cities
            .get(&cid)
            .and_then(|city| city.queue.first())
            .cloned()
        else {
            return false;
        };
        let paid = self.item_invested_production(cid, &item);
        let key = Self::item_progress_key(&item);
        let city = self.cities.get_mut(&cid).unwrap();
        if paid > 0.0 {
            city.production_progress.insert(key, paid);
        }
        city.production = 0.0;
        city.queue.clear();
        true
    }

    /// Production still required after active progress, item-specific paused
    /// progress, and unassigned overflow are applied. Search agents use this
    /// instead of treating a nearly complete build like a fresh one.
    pub(crate) fn item_remaining_cost_for_city(&self, pid: usize, cid: u32, item: &Item) -> f64 {
        let city = &self.cities[&cid];
        let mut invested = self.item_invested_production(cid, item);
        if city.queue.is_empty() {
            invested += city.production;
        }
        (self.item_cost_for_city(pid, cid, item) - invested).max(0.0)
    }

    pub(in crate::game) fn unit_resource_cost(&self, cid: u32, item: &Item) -> f64 {
        // The host prices the exact city/tier after all modifiers. Preserve
        // explicit zero prices and do not discount or multiply them again.
        // Empty on ordinary boards, avoiding a key allocation there.
        if !self.host_unit_resource_prices.is_empty() {
            if let Some(cost) = self
                .host_unit_resource_prices
                .get(&cid)
                .and_then(|prices| prices.get(&Self::production_block_key(item)))
                .filter(|cost| cost.is_finite() && **cost >= 0.0)
            {
                return *cost;
            }
        }
        let (unit, multiplier) = match item {
            Item::Unit { unit } => (unit, 1.0),
            Item::Formation {
                unit, formation, ..
            } => (unit, if *formation >= 2 { 3.0 } else { 2.0 }),
            _ => return 0.0,
        };
        let Some(spec) = self.rules.units.get(unit) else {
            return 0.0;
        };
        let city = &self.cities[&cid];
        let discount = self
            .governor_effect(city.owner, cid, "strategic_resource_discount_pct")
            .clamp(0.0, 100.0);
        spec.resource_cost * multiplier * (100.0 - discount) / 100.0
    }

    /// Drop queued units the owner has just retired. Without this a city that
    /// started a Slinger keeps finishing it long after Machinery, so the very
    /// technology that should have modernized the army delivers one more copy
    /// of the unit it replaced.
    pub(in crate::game) fn drop_obsolete_production(&mut self, pid: usize) {
        for cid in self.player_city_ids(pid) {
            let doomed: Vec<Item> = self.cities[&cid]
                .queue
                .iter()
                .filter(|item| match item {
                    Item::Unit { unit } | Item::Formation { unit, .. } => {
                        self.unit_is_obsolete(pid, unit)
                    }
                    _ => false,
                })
                .cloned()
                .collect();
            if doomed.is_empty() {
                continue;
            }
            // Strategic material already committed to the abandoned order goes
            // back to the stockpile rather than evaporating with the item.
            for item in &doomed {
                let key = Self::item_progress_key(item);
                let Some(unit) = (match item {
                    Item::Unit { unit } | Item::Formation { unit, .. } => Some(*unit),
                    _ => None,
                }) else {
                    continue;
                };
                let refund = self.cities[&cid]
                    .strategic_resource_commitments
                    .get(&key)
                    .copied()
                    .unwrap_or(0.0);
                let city = self.cities.get_mut(&cid).unwrap();
                city.strategic_resource_commitments.remove(&key);
                city.production_progress.remove(&key);
                if refund > 0.0 {
                    if let Some(resource) = self.rules.units[unit].requires_resource {
                        let held = self.strategic_stockpile(pid, resource);
                        let capacity = self.strategic_stockpile_capacity(pid);
                        self.players[pid]
                            .strategic_resources
                            .insert(Name::new(&resource), (held + refund).min(capacity));
                    }
                }
            }
            let city = self.cities.get_mut(&cid).unwrap();
            let head_dropped = city.queue.first().is_some_and(|item| doomed.contains(item));
            city.queue.retain(|item| !doomed.contains(item));
            if head_dropped {
                city.production = 0.0;
            }
        }
    }

    /// Unit production rules without the recursive obsolescence check. The
    /// resource credit is used only while automatically migrating an existing
    /// queue whose already-committed material can be refunded atomically.
    pub(in crate::game) fn can_produce_unit(
        &self,
        pid: usize,
        cid: u32,
        unit: impl AsName,
        check_obsolete: bool,
        resource_credit: f64,
    ) -> bool {
        let unit = unit.as_name();
        let Some(city) = self.cities.get(&cid).filter(|city| city.owner == pid) else {
            return false;
        };
        let Some(spec) = self.rules.units.get_interned(unit) else {
            return false;
        };
        if matches!(unit.as_str(), "rock_band" | "naturalist") {
            return false; // Faith purchase only (Civ VI)
        }
        // A city-state is permanently one city. This is an engine rule rather
        // than an AI preference so captured, granted, and externally commanded
        // minor Settlers cannot turn it into an ordinary empire.
        if unit == "settler"
            && (self.players[pid].is_minor || self.policy_effect(pid, "no_settling") > 0.0)
        {
            return false;
        }
        if !spec.buildable || !self.unlocked(pid, &spec.tech, &spec.civic) {
            return false;
        }
        if check_obsolete && self.unit_is_obsolete(pid, unit) {
            return false;
        }
        if unit == "spy" {
            // Counts mirrored `UNIT_SPY` units too — see `spy_agents`.
            let existing = self.spy_agents(pid);
            // Spies queued in the empire's OTHER cities. A `Produce` order
            // replaces the asking city's whole queue, so a Spy already at the
            // head of this city's queue is the claim being asked about, not a
            // second one: counting it made a queued Spy illegal the moment the
            // empire had exactly one free slot, and the AI then replaced it and
            // queued it again every turn without ever finishing it.
            let queued = self
                .cities
                .values()
                .filter(|candidate| candidate.owner == pid && candidate.id != cid)
                .filter(|candidate| {
                    candidate
                        .queue
                        .iter()
                        .any(|queued| matches!(queued, Item::Unit { unit } if unit == "spy"))
                })
                .count();
            if existing + queued >= self.spy_capacity(pid).max(0) as usize {
                return false;
            }
        }
        if unit == "settler" && city.pop < 2 {
            return false;
        }
        if spec.class == "religious" {
            return false; // faith purchase only (Civ VI)
        }
        if spec
            .unique_to
            .as_deref()
            .is_some_and(|civilization| !self.owns_civ_unique(pid, civilization))
        {
            return false;
        }
        // A civilization with a unique replacement cannot train the base.
        if self.player_unit_replacement(pid, unit) != unit {
            return false;
        }
        let item = Item::Unit { unit };
        if let Some(resource) = &spec.requires_resource {
            let resource_cost = self.unit_resource_cost(cid, &item);
            if resource_cost > 0.0
                && !self.unit_resource_is_committed(cid, &item)
                && self.strategic_stockpile(pid, resource) + resource_credit + f64::EPSILON
                    < resource_cost
            {
                return false;
            }
        }
        if spec
            .requires_building
            .as_ref()
            .is_some_and(|building| !self.city_has_building_family(city, Name::new(building)))
            || spec
                .requires_district
                .as_ref()
                .is_some_and(|district| !self.city_has_district_family(city, Name::new(district)))
        {
            return false;
        }
        if spec.domain.as_deref() == Some("sea") {
            let coastal = self.nbrs(city.pos).iter().any(|position| {
                self.map
                    .get(*position)
                    .is_some_and(|tile| self.rules.is_water(tile))
            });
            if !coastal {
                return false;
            }
        }
        true
    }

    /// Non-allocating twin of [`Game::production_block_key`], for callers
    /// that only ever compare or hash the key — never look it up in a
    /// `String`-keyed map fed by the live mirror (`blocked_production`,
    /// `blocked_purchases`, `host_buildable`, `host_purchasable`; those stay
    /// `String`-keyed because they cross the bridge's serde boundary).
    /// `Name` is already an interned, `Copy` id, so building this key touches
    /// no allocator.
    pub(crate) fn production_key(item: &Item) -> ProductionKey {
        match item {
            Item::Formation { unit, formation } => ProductionKey::Formation(*unit, *formation),
            Item::Unit { unit } => ProductionKey::Unit(*unit),
            Item::Building { building } => ProductionKey::Building(*building),
            Item::District { district, .. } => ProductionKey::District(*district),
            Item::Wonder { wonder, .. } => ProductionKey::Wonder(*wonder),
            Item::Repair { repair, .. } => ProductionKey::Repair(*repair),
            Item::Project { project } => ProductionKey::Project(*project),
            Item::Product { product } => ProductionKey::Product(*product),
        }
    }

    pub(crate) fn production_block_key(item: &Item) -> String {
        Self::production_key(item).to_block_string()
    }

    pub(crate) fn replace_blocked_production(&mut self, blocked: BTreeMap<u32, BTreeSet<String>>) {
        self.blocked_production = Arc::new(blocked);
        // This field is mirror input rather than an Action, so it does not cross the
        // ordinary successful-apply invalidation below. A menu derived before sync
        // must not survive after the host has rejected one of its entries.
        self.query_memo.producible.borrow_mut().clear();
    }

    pub(crate) fn replace_blocked_purchases(&mut self, blocked: BTreeMap<u32, BTreeSet<String>>) {
        self.blocked_purchases = Arc::new(blocked);
    }

    pub(crate) fn replace_host_unit_resource_prices(
        &mut self,
        prices: BTreeMap<u32, BTreeMap<String, f64>>,
    ) {
        self.host_unit_resource_prices = Arc::new(prices);
        self.query_memo.producible.borrow_mut().clear();
    }

    /// The host's own turns-to-complete for an item in this city
    /// (`BuildQueue:GetTurnsLeft`), when the export carried it.
    pub fn host_production_turns(&self, cid: u32, item: &Item) -> Option<f64> {
        self.host_buildable
            .get(&cid)?
            .get(&Self::production_block_key(item))?
            .turns
    }

    /// Whether the host menu is asked about this kind of item at all. Repairs
    /// and Corporation products are not among the exported families.
    pub(in crate::game) fn host_menu_gates(item: &Item) -> bool {
        !matches!(item, Item::Repair { .. } | Item::Product { .. })
    }

    /// The host's purchase price for an item in this city in a currency, when
    /// the export carried this city's purchase menu: `Some(None)` is the host
    /// declining to sell, `None` is no menu — ask the model.
    pub(in crate::game) fn host_purchase_price(
        &self,
        cid: u32,
        item: &Item,
        currency: &str,
    ) -> Option<Option<f64>> {
        let menu = self.host_purchasable.get(&cid)?;
        let entry = menu.get(&Self::production_block_key(item));
        Some(match currency {
            "gold" => entry.and_then(|entry| entry.gold),
            "faith" => entry.and_then(|entry| entry.faith),
            _ => None,
        })
    }

    /// Whether the host recently refused to let this city BUY this item.
    ///
    /// ⚠ `pub(crate)` because the legal-action enumeration is not the only way a
    /// purchase happens. `buy_gold_infrastructure`, `buy_gold_unit`,
    /// `buy_gold_military` and the missionary buyer all build an `Action::Buy*`
    /// themselves and call `apply` directly, so a gate that lives only in the
    /// enumeration never runs for them — which is exactly how the same Granary
    /// was re-bought in the same city on turns 114, 117, 121, 122, 123 and 128 of
    /// live run `civvis-20260804T091315Z`, 8 of that game's 9 purchases refused.
    pub(crate) fn purchase_is_blocked(&self, cid: u32, item: &Item) -> bool {
        let Some(blocked) = self.blocked_purchases.get(&cid) else {
            return false;
        };
        let key = Self::production_block_key(item);
        if blocked.contains(&key) {
            return true;
        }
        // The host purchase event names the unit type, not the Corps/Army wrapper.
        // One refusal therefore cools down every formation of that unit in this city.
        matches!(item, Item::Formation { unit, .. } if blocked.contains(&format!("unit:{unit}")))
    }

    pub(in crate::game) fn purchase_action_is_blocked(&self, action: &Action) -> bool {
        match action {
            Action::Buy {
                city,
                unit,
                formation,
                ..
            } => {
                let item = if *formation == 0 {
                    Item::Unit { unit: *unit }
                } else {
                    Item::Formation {
                        unit: *unit,
                        formation: *formation,
                    }
                };
                self.purchase_is_blocked(*city, &item)
            }
            Action::BuyBuilding { city, building, .. } => self.purchase_is_blocked(
                *city,
                &Item::Building {
                    building: *building,
                },
            ),
            Action::BuyDistrict {
                city,
                district,
                pos,
                ..
            } => self.purchase_is_blocked(
                *city,
                &Item::District {
                    district: *district,
                    pos: *pos,
                },
            ),
            _ => false,
        }
    }

    pub fn can_produce(&self, pid: usize, cid: u32, item: &Item) -> bool {
        // An arena builds things that fight and nothing else. Stated once
        // here, at the gate every queue, every AI production choice and every
        // client menu already asks, rather than as a rule each of them has to
        // remember separately.
        if self.is_arena() && !self.arena_allows_production(item) {
            return false;
        }
        // And nothing at all while the arena grants no Production to build
        // it with — the stock arena since the grants went to zero. A city
        // that will never finish anything has no business asking its player
        // to choose what it builds every turn; raise the grant and the
        // fighting-units menu is back. `arena_allows_production` stays the
        // roster rule, so a unique-units match still reads its roster
        // whatever the grant.
        if self.is_arena() && self.tactics.production == 0 {
            return false;
        }
        // Both maps a block key is looked up in are keyed by the live
        // mirror, and empty otherwise — so build the (allocating) `String`
        // form only when a map actually holds an entry for this city, not
        // unconditionally on every candidate item of every ordinary board.
        if self.blocked_production.get(&cid).is_some_and(|items| {
            !items.is_empty() && items.contains(Self::production_block_key(item).as_str())
        }) {
            return false;
        }
        let city = &self.cities[&cid];
        // ★ THE POSITIVE GATE. The set above says what the host REFUSED; this
        // one says what the host OFFERS — `BuildQueue:CanProduce(hash, false,
        // true)` for every family, exported per city (`StateCity::buildable`)
        // and translated onto the same keys. Empty on an ordinary board and on
        // an older export, so nothing changes there. The queue is exempt: the
        // head is the host's own `producing`, listed or not at its discretion.
        if Self::host_menu_gates(item) {
            if let Some(menu) = self.host_buildable.get(&cid) {
                // The queue scan below only ever compares keys, never looks
                // one up in a `String`-keyed map, so it stays on the
                // non-allocating `ProductionKey` throughout.
                let key = Self::production_key(item);
                if !menu.contains_key(key.to_block_string().as_str())
                    && !city
                        .queue
                        .iter()
                        .any(|queued| Self::production_key(queued) == key)
                {
                    return false;
                }
            }
            if let Item::District { district, pos } = item {
                if self
                    .host_district_plots
                    .get(&cid)
                    .and_then(|plots| plots.get(district))
                    .is_some_and(|plots| !plots.contains(pos))
                    && !city.queue.contains(item)
                {
                    return false;
                }
            }
        }
        match item {
            Item::Formation { unit, formation } => {
                let Some(spec) = self.rules.units.get(unit) else {
                    return false;
                };
                if self.players[pid].is_minor
                    || !matches!(formation, 1 | 2)
                    || spec.class != "military"
                    || spec.domain.as_deref() == Some("air")
                    || !spec.can_formations
                    || !self.formation_unlocked(pid, unit, *formation)
                {
                    return false;
                }
                let infrastructure = if spec.domain.as_deref() == Some("sea") {
                    self.city_has_building_family(city, crate::name!("seaport"))
                } else {
                    self.city_has_building_family(city, crate::name!("military_academy"))
                        || city.districts.iter().any(|(district, position)| {
                            district == "ikanda"
                                && self.district_is_active(city, district, *position)
                        })
                };
                if !infrastructure {
                    return false;
                }
                let committed = self.unit_resource_is_committed(cid, item);
                if !self.can_produce(pid, cid, &Item::Unit { unit: *unit })
                    && !(committed && spec.buildable && self.unlocked(pid, &spec.tech, &spec.civic))
                {
                    return false;
                }
                if let Some(resource) = spec.requires_resource.as_deref() {
                    let resource_cost = self.unit_resource_cost(cid, item);
                    if resource_cost > 0.0
                        && !committed
                        && self.strategic_stockpile(pid, Name::new(resource)) + f64::EPSILON
                            < resource_cost
                    {
                        return false;
                    }
                }
                true
            }
            Item::Unit { unit } => self.can_produce_unit(pid, cid, unit, true, 0.0),
            Item::Building { building } => {
                let spec = match self.rules.buildings.get(building) {
                    Some(s) => s,
                    None => return false,
                };
                let congress_district = spec
                    .district
                    .map(|district| self.district_family(district))
                    .unwrap_or(crate::name!("city_center"));
                if self.congress_effect_active(
                    "urban_development_treaty",
                    "B",
                    congress_district.as_str(),
                ) {
                    return false;
                }
                if self.congress_effect_active("global_energy_treaty", "B", building) {
                    return false;
                }
                let society_for = |candidate: &crate::rules::BuildingSpec| {
                    [
                        ("requires_hermetic_order", "hermetic_order"),
                        ("requires_owls_of_minerva", "owls_of_minerva"),
                        ("requires_voidsingers", "voidsingers"),
                    ]
                    .into_iter()
                    .find_map(|(effect, society)| {
                        (candidate.effects.get(effect).copied().unwrap_or(0.0) > 0.0)
                            .then_some(society)
                    })
                };
                let society_building = society_for(spec).is_some_and(|society| {
                    self.players[pid].secret_society.as_deref() == Some(society)
                });
                if city.buildings.contains(building)
                    || !self.unlocked(pid, &spec.tech, &spec.civic)
                    || (!spec.buildable && !society_building)
                    || spec.purchase_only
                {
                    return false;
                }
                if spec
                    .unique_to
                    .as_deref()
                    .is_some_and(|civ| !self.owns_civ_unique(pid, civ))
                    || self.rules.buildings.values().any(|candidate| {
                        candidate.replaces == Some(*building)
                            && (candidate
                                .unique_to
                                .as_deref()
                                .is_some_and(|owner| self.owns_civ_unique(pid, owner))
                                || society_for(candidate).is_some_and(|society| {
                                    self.players[pid].secret_society.as_deref() == Some(society)
                                }))
                    })
                    || !spec
                        .requires
                        .iter()
                        .all(|required| self.city_has_building_family(city, *required))
                    || (!spec.requires_any.is_empty()
                        && !spec
                            .requires_any
                            .iter()
                            .any(|required| self.city_has_building_family(city, *required)))
                    || spec
                        .excludes
                        .iter()
                        .any(|excluded| self.city_has_building_family(city, *excluded))
                {
                    return false;
                }
                for (effect, society) in [
                    ("requires_hermetic_order", "hermetic_order"),
                    ("requires_owls_of_minerva", "owls_of_minerva"),
                    ("requires_voidsingers", "voidsingers"),
                ] {
                    if spec.effects.get(effect).copied().unwrap_or(0.0) > 0.0
                        && self.players[pid].secret_society.as_deref() != Some(society)
                    {
                        return false;
                    }
                }
                if spec.outer_defense > 0
                    && (!spec
                        .requires
                        .iter()
                        .all(|required| self.city_has_building_family(city, *required))
                        || city.wall_hp < self.city_max_wall_hp(city))
                {
                    return false;
                }
                if spec
                    .effects
                    .get("protect_coastal_lowlands")
                    .copied()
                    .unwrap_or(0.0)
                    > 0.0
                    && self.coastal_lowland_tiles(city).is_empty()
                {
                    return false;
                }
                if spec.wonder && self.wonder_built(building) {
                    return false; // one per world
                }
                if spec.coastal {
                    let ok = self.nbrs(city.pos).iter().any(|n| {
                        self.map
                            .get(*n)
                            .map(|t| self.rules.is_water(t))
                            .unwrap_or(false)
                    });
                    if !ok {
                        return false;
                    }
                }
                if spec
                    .effects
                    .get("requires_river_city")
                    .copied()
                    .unwrap_or(0.0)
                    > 0.0
                    && !self.map.tiles[&city.pos].has_river()
                {
                    return false;
                }
                if spec
                    .effects
                    .get("requires_fresh_water_city")
                    .copied()
                    .unwrap_or(0.0)
                    > 0.0
                {
                    let fresh = self.map.tiles[&city.pos].has_river()
                        || self.nbrs(city.pos).iter().any(|neighbor| {
                            self.map.get(*neighbor).is_some_and(|tile| {
                                tile.terrain == "lake" || tile.feature.as_deref() == Some("oasis")
                            })
                        });
                    if !fresh {
                        return false;
                    }
                }
                match &spec.district {
                    None => true,
                    Some(d) if self.district_family(*d) == "city_center" => true,
                    Some(d) => city.districts.iter().any(|(built, position)| {
                        self.district_is_family(built, d)
                            && self.district_is_active(city, built, *position)
                            && !self.unit_ids_at(*position).iter().any(|unit| {
                                self.units[unit].owner != pid
                                    && self.is_at_war(pid, self.units[unit].owner)
                            })
                    }),
                }
            }
            Item::District { district, pos } => {
                let spec = match self.rules.districts.get(district) {
                    Some(s) => s,
                    None => return false,
                };
                if !self.unlocked(pid, &spec.tech, &spec.civic)
                    || !spec.buildable
                    || spec
                        .unique_to
                        .as_deref()
                        .is_some_and(|civ| !self.owns_civ_unique(pid, civ))
                    || self.rules.districts.values().any(|candidate| {
                        candidate.replaces == Some(*district)
                            && candidate
                                .unique_to
                                .as_deref()
                                .is_some_and(|owner| self.owns_civ_unique(pid, owner))
                    })
                {
                    return false;
                }
                self.district_sites(cid, district).contains(pos)
            }
            Item::Wonder { wonder, pos } => {
                if self.players[pid].is_minor {
                    return false;
                }
                let Some(spec) = self.rules.wonders.get(wonder) else {
                    return false;
                };
                self.unlocked(pid, &spec.tech, &spec.civic)
                    && !self.wonder_built(wonder)
                    && self.wonder_sites(cid, wonder).contains(pos)
            }
            Item::Repair { repair, pos } => {
                let Some(tile) = self.map.get(*pos) else {
                    return false;
                };
                if tile.owner_city != Some(cid)
                    || tile.district.is_none()
                    // Expansion2_InGameText: LOC_DISTRICT_REPAIR_LOCATION_FLOODED.
                    // A Flood Barrier must clear flooding before repairs can start.
                    || tile.flooded
                    || tile.submerged
                    || self.unit_ids_at(*pos).iter().any(|unit| {
                        self.units[unit].owner != pid && self.is_at_war(pid, self.units[unit].owner)
                    })
                {
                    return false;
                }
                if repair == "district" {
                    tile.pillaged
                } else {
                    // ★★★ A PILLAGED PREREQUISITE IS REPAIRED FIRST. The host
                    // refuses a Coal Power Plant repair while the Factory it
                    // requires is still pillaged ("This building requires a
                    // Factory building."). Live King 20261004T083931Z: Caracas
                    // lost its Workshop, Factory and Coal Power Plant at turn
                    // 144; the plant was the cheapest repair, so the city chose
                    // it and the host refused it every turn from 147 to 205,
                    // 59 turns with an empty queue.
                    city.pillaged_buildings
                        .iter()
                        .any(|built| *built == *repair)
                        && self.rules.buildings.get(repair).is_some_and(|building| {
                            building.district.as_ref().is_some_and(|family| {
                                tile.district.is_some_and(|district| {
                                    self.district_is_family(district, family)
                                })
                            }) && !building
                                .requires
                                .iter()
                                .any(|required| city.pillaged_buildings.contains(required))
                        })
                }
            }
            Item::Project { project } => {
                // `GameOptions::ai_space_tech_floor`: true unless set.
                if !self.space_project_tech_floor_met(pid, project.as_str()) {
                    return false;
                }
                if project == "repair_outer_defenses" {
                    let max = self.city_max_wall_hp(city);
                    return max > 0
                        && city.wall_hp < max
                        && self.turn.saturating_sub(city.last_attacked) >= 3;
                }
                if project == "repair_encampment" {
                    let max_wall = self.city_max_wall_hp(city);
                    return self.city_has_district_family(city, crate::name!("encampment"))
                        && (city.encampment_pillaged
                            || city.encampment_hp < 100
                            || city.encampment_wall_hp < max_wall)
                        && self.turn.saturating_sub(city.encampment_last_attacked) >= 3;
                }
                // Repairs keep the one city functional; ordinary district,
                // Great Person, loyalty, victory, and conversion projects are
                // major-civilization activity and are never a minor fallback.
                if self.players[pid].is_minor {
                    return false;
                }
                let spec = match self.rules.projects.get(project) {
                    Some(s) => s,
                    None => return false,
                };
                if self.players[pid].is_barbarian {
                    return false;
                }
                if spec.requires_host_competition()
                    && !spec
                        .host_competition_kinds()
                        .any(|kind| self.host_competition(pid, kind).is_some())
                {
                    return false;
                }
                if spec
                    .tech
                    .as_ref()
                    .is_some_and(|t| !self.players[pid].techs.contains(t))
                {
                    return false;
                }
                if spec
                    .civic
                    .as_ref()
                    .is_some_and(|c| !self.players[pid].civics.contains(c))
                {
                    return false;
                }
                if matches!(
                    project.as_str(),
                    "lagrange_laser_station" | "terrestrial_laser_station"
                ) && self.tree_effect(pid, "laser_station_projects") <= 0.0
                {
                    return false;
                }
                if !self.project_has_active_district(city, spec) {
                    return false;
                }
                if !spec.requires.iter().all(|required| {
                    self.players[pid]
                        .science_projects
                        .contains(required.as_str())
                }) {
                    return false;
                }
                if !spec
                    .requires_buildings
                    .iter()
                    .chain(&spec.consumes_buildings)
                    .all(|building| self.city_has_building_family(city, Name::new(building)))
                {
                    return false;
                }
                if let Some(target) = Self::converted_power_plant(project) {
                    let current = city.buildings.iter().find(|building| {
                        matches!(
                            building.as_str(),
                            "coal_power_plant" | "oil_power_plant" | "nuclear_power_plant"
                        )
                    });
                    if current.is_none_or(|plant| plant == target) {
                        return false;
                    }
                }
                spec.repeatable
                    || !self.players[pid]
                        .science_projects
                        .contains(project.as_str())
            }
            Item::Product { product } => {
                if self.players[pid].is_minor
                    || self.players[pid].is_barbarian
                    || self
                        .rules
                        .resources
                        .get(product)
                        .is_none_or(|resource| resource.class != "luxury")
                    || self.world_product_count(product) >= 5
                    || self.product_slot_city(pid, cid).is_none()
                {
                    return false;
                }
                self.city_active_economic_improvement(city)
                    .is_some_and(|(resource, corporation)| corporation && resource == *product)
            }
        }
    }

    pub fn producible_items(&self, pid: usize, cid: u32) -> Vec<Item> {
        if let Some(items) = self
            .query_memo
            .producible
            .borrow()
            .get(&(pid, cid))
            .cloned()
        {
            return items;
        }
        // Every unit, building, district and wonder in the ruleset is offered
        // to the same city in turn, and each offer re-asks that city the same
        // things.
        let _memo = self.query_memo();
        let mut items = Vec::new();
        for name in self.rules.units.keys() {
            let it = Item::Unit { unit: *name };
            if self.can_produce(pid, cid, &it) {
                items.push(it);
            }
            for formation in 1..=2 {
                let formation_item = Item::Formation {
                    unit: *name,
                    formation,
                };
                if self.can_produce(pid, cid, &formation_item) {
                    items.push(formation_item);
                }
            }
        }
        for name in self.rules.buildings.keys() {
            let it = Item::Building { building: *name };
            if self.can_produce(pid, cid, &it) {
                items.push(it);
            }
        }
        for name in self.rules.wonders.keys() {
            let mut sites = self.wonder_sites(cid, name);
            sites.sort();
            for pos in sites.into_iter().take(2) {
                // `wonder_sites` already applies the complete world-unique,
                // unlock, prerequisite, and placement validation used by the
                // `Item::Wonder` arm of `can_produce`.
                let item = Item::Wonder { wonder: *name, pos };
                if self.can_produce(pid, cid, &item) {
                    items.push(item);
                }
            }
        }
        let city = &self.cities[&cid];
        for (_, pos) in &city.districts {
            let district = self.map.tiles[pos].district.unwrap_or(crate::name!(""));
            let district_repair = Item::Repair {
                repair: crate::name!("district"),
                pos: *pos,
            };
            if self.can_produce(pid, cid, &district_repair) {
                items.push(district_repair);
            }
            for building in &city.pillaged_buildings {
                let matches_district = self.rules.buildings.get(building).is_some_and(|spec| {
                    spec.district
                        .as_ref()
                        .is_some_and(|family| self.district_is_family(district, family))
                });
                if matches_district {
                    let repair = Item::Repair {
                        repair: Name::new(building),
                        pos: *pos,
                    };
                    if self.can_produce(pid, cid, &repair) {
                        items.push(repair);
                    }
                }
            }
        }
        for name in self.rules.projects.keys() {
            let it = Item::Project { project: *name };
            if self.can_produce(pid, cid, &it) {
                items.push(it);
            }
        }
        if let Some((resource, true)) = self.city_active_economic_improvement(city) {
            let product = Item::Product { product: resource };
            if self.can_produce(pid, cid, &product) {
                items.push(product);
            }
        }
        for (name, spec) in &self.rules.districts {
            // The sites below are validated by hand rather than through
            // `can_produce`, so the arena's answer is stated here as well:
            // a battlefield builds no districts.
            if self.is_arena()
                || !spec.buildable
                || !self.unlocked(pid, &spec.tech, &spec.civic)
                || spec
                    .unique_to
                    .as_deref()
                    .is_some_and(|civ| !self.owns_civ_unique(pid, civ))
                || self.rules.districts.values().any(|candidate| {
                    candidate.replaces == Some(*name)
                        && candidate
                            .unique_to
                            .as_deref()
                            .is_some_and(|owner| self.owns_civ_unique(pid, owner))
                })
            {
                continue;
            }
            // Rank the sites on values taken once each. Deriving a site's
            // yields means walking its neighbours for adjacency, and a
            // comparator that re-derives both sides pays that for every
            // comparison in the sort rather than for every site.
            let mut ranked: Vec<(bool, f64, Pos)> = self
                .district_sites(cid, name)
                .into_iter()
                .map(|site| {
                    (
                        self.map.tiles[&site].district_foundation.is_some(),
                        // `nusantara-reads-the-coast`: the menu keeps the two
                        // best fresh sites, so the planner's Coast term must
                        // rank here too. Equal to `district_yields` otherwise.
                        self.district_site_yields(name, site).total(),
                        site,
                    )
                })
                .collect();
            ranked.sort_by(|a, b| {
                b.0.cmp(&a.0)
                    .then_with(|| b.1.partial_cmp(&a.1).unwrap())
                    .then(a.2.cmp(&b.2))
            });
            let sites: Vec<Pos> = ranked.into_iter().map(|(_, _, site)| site).collect();
            let mut fresh_sites = 0usize;
            for s in sites {
                let foundation = self.map.tiles[&s].district_foundation.is_some();
                if !foundation && fresh_sites >= 2 {
                    continue;
                }
                // The filters above and `district_sites` together are the
                // complete `Item::District` validation from `can_produce`.
                items.push(Item::District {
                    district: *name,
                    pos: s,
                });
                fresh_sites += usize::from(!foundation);
            }
        }
        self.query_memo
            .producible
            .borrow_mut()
            .insert((pid, cid), items.clone());
        items
    }
}
