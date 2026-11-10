//! Legal actions: every action a seat may take this turn.
//!
//! `legal_actions` and `legal_actions_within` enumerate them for a player
//! or a region. The per-family enumerators sit beside them: city
//! dispositions, doctrines, unit upgrades, and purchases (with the
//! per-city price memo). So do the helpers only enumeration uses: the deals
//! a seat could offer (`quick_deals`), quoting a payment or an asset trade,
//! a unit's purchase cost, whether a melee order is legal, and a rock
//! concert's tourism.
//!
//! Moved verbatim out of the `impl Game` block in `game/actions.rs`
//! (2026-11-05). Methods the rest of the action layer calls are
//! `pub(super)`; methods the rest of `game` calls keep that reach as
//! `pub(in crate::game)`; public ones stay public. The code is otherwise
//! unchanged.

use super::*;

impl Game {
    /// Captured-city decisions are mandatory and exclusive, so callers that
    /// only need to resolve one should not generate the rest of the turn's
    /// action space merely to discover whether a decision is pending.
    pub(crate) fn legal_city_disposition_actions(&self, pid: usize) -> Vec<Action> {
        if self.is_finished() || self.current != pid {
            Vec::new()
        } else {
            self.pending_city_capture_actions(pid)
        }
    }

    /// Generate only the unit-local actions used by the AI's raider and air
    /// doctrines. Calling `legal_actions` for this decision also evaluates
    /// every city's production, purchases, diplomacy, and every other unit,
    /// even though none of those actions can be selected here.
    pub(crate) fn legal_doctrine_actions(&self, pid: usize, uid: u32) -> Vec<Action> {
        if self.is_finished()
            || self.current != pid
            || !self.pending_city_capture_actions(pid).is_empty()
        {
            return Vec::new();
        }
        let Some(u) = self.units.get(&uid) else {
            return Vec::new();
        };
        if u.owner != pid || self.noncombat_action_blocked_by_zoc(uid) {
            return Vec::new();
        }
        let spec = &self.rules.units[u.kind];
        let mut actions = Vec::new();
        if spec.domain.as_deref() == Some("air") {
            if u.moves_left > 0.0 {
                let range = self.unit_attack_range(uid);
                let origin = self.air_operation_origin(uid);
                for target in self.wdisk(origin, range) {
                    if target != origin
                        && u.attacks_left > 0
                        && !self.strike_blocked(uid, target)
                        && self.enemy_air_strike_target_at(pid, target)
                    {
                        actions.push(Action::AirStrike { unit: uid, target });
                    }
                    if target != origin
                        && spec.promotion_class == "air_bomber"
                        && u.attacks_left > 0
                        && !self.strike_blocked(uid, target)
                        && (u.hp >= 50
                            || self.promotion_effect(u, "air_pillage_at_low_health") > 0.0)
                        && self.air_pillageable_at(pid, target)
                    {
                        actions.push(Action::AirPillage { unit: uid, target });
                    }
                    if target != origin
                        && u.attacks_left > 0
                        && self.unit_has_priority_target(u)
                        && self.priority_support_target_at(pid, target).is_some()
                    {
                        actions.push(Action::PriorityTarget { unit: uid, target });
                    }
                }
                for base in self.wdisk(u.pos, self.air_rebase_range(uid)) {
                    if base != u.pos && self.can_air_base_at(pid, base, Some(uid)) {
                        actions.push(Action::AirRebase {
                            unit: uid,
                            to: base,
                        });
                    }
                }
                if !spec.siege && u.attacks_left > 0 {
                    for to in self.wdisk(u.pos, spec.moves.floor() as i32) {
                        if self.can_air_patrol_at(pid, to) {
                            actions.push(Action::AirPatrol { unit: uid, to });
                        }
                    }
                }
            }
            return actions;
        }
        if spec.class == "military"
            && u.moves_left > 0.0
            && !self.is_embarked(u)
            && self.pillageable_at(pid, u.pos)
        {
            actions.push(Action::Pillage { unit: uid });
        }
        if self.can_coastal_raid(pid, u) && u.moves_left > 0.0 {
            for target in self.nbrs(u.pos) {
                if self.pillageable_at(pid, target) {
                    actions.push(Action::CoastalRaid { unit: uid, target });
                }
            }
        }
        actions
    }

    pub(crate) fn legal_unit_upgrade_actions(&self, pid: usize) -> Vec<Action> {
        if self.is_finished()
            || self.current != pid
            || !self.pending_city_capture_actions(pid).is_empty()
        {
            return Vec::new();
        }
        self.player_unit_ids(pid)
            .into_iter()
            .filter_map(|unit| {
                self.unit_gold_upgrade_offer(pid, unit)
                    .map(|_| Action::UpgradeUnit { unit })
            })
            .collect()
    }

    /// Cities whose purchase menus may be enumerated independently.
    ///
    /// A pending capture replaces the whole legal-action space, just as it
    /// does in [`legal_actions_within`]. Keeping that gate here lets the AI
    /// distribute only city-local work without first paying for the full
    /// empire action list it intends to discard.
    pub(crate) fn purchase_action_city_ids(&self, pid: usize) -> Vec<u32> {
        if self.is_finished()
            || self.current != pid
            || !self.pending_city_capture_actions(pid).is_empty()
        {
            Vec::new()
        } else {
            self.player_city_ids(pid)
        }
    }

    /// Purchase actions contributed by one city, separated into the stock
    /// PURCHASES block and the later EMPIRE block.
    ///
    /// The two vectors must stay separate: callers flatten every city's first
    /// vector before any city's second vector, reproducing the order of
    /// `legal_actions_within(PURCHASES | EMPIRE)` after non-purchase actions
    /// are filtered away. That stable order is an AI tie-break input.
    pub(crate) fn legal_purchase_actions_for_city(
        &self,
        pid: usize,
        cid: u32,
    ) -> (Vec<Action>, Vec<Action>) {
        self.legal_purchase_actions_for_city_with_price_memo(pid, cid, true)
    }

    /// The inner purchase-menu sweep, optionally sharing price quotes with an
    /// enclosing read-only query.
    ///
    /// One menu asks each ordinary `(city, unit, formation, currency)` quote
    /// once. A top-level menu therefore gains nothing by constructing and
    /// populating [`QueryCache::purchase_price`] for every one of those
    /// unique keys; callers that already hold a [`QueryMemo`] retain the
    /// cache, because a second menu in that surrounding read-only pass can
    /// reuse the quotes exactly.
    fn legal_purchase_actions_for_city_with_price_memo(
        &self,
        pid: usize,
        cid: u32,
        cache_unit_prices: bool,
    ) -> (Vec<Action>, Vec<Action>) {
        let p = &self.players[pid];
        let mut purchases = Vec::new();
        let mut plots = self
            .wdisk(self.cities[&cid].pos, 3)
            .into_iter()
            .filter_map(|position| {
                self.plot_purchase_cost(pid, cid, position)
                    .map(|cost| (position, cost))
            })
            .collect::<Vec<_>>();
        plots.sort_unstable_by_key(|(position, _)| *position);
        for (pos, cost) in plots {
            if p.gold + f64::EPSILON >= cost {
                purchases.push(Action::BuyPlot {
                    city: cid,
                    pos,
                    cost,
                });
            }
        }

        // A purchase menu does not need the complete production catalog. The
        // building and unit purchase quotes repeat the same `can_produce`
        // checks themselves, so walking every wonder, project, repair and
        // district first only to discard those entries was duplicate work on
        // every city. District purchases are the one exception: preserve the
        // stock district-site ordering when a Governor actually enables them.
        for building in self.rules.buildings.keys() {
            if self
                .building_gold_purchase_cost(pid, cid, building)
                .is_some_and(|cost| p.gold + f64::EPSILON >= cost)
            {
                purchases.push(Action::BuyBuilding {
                    city: cid,
                    building: *building,
                    currency: "gold".to_string(),
                });
            }
        }

        let faith_districts = self.governor_effect(pid, cid, "faith_purchase_districts") > 0.0;
        let gold_districts = self.governor_effect(pid, cid, "gold_purchase_districts") > 0.0;
        if faith_districts || gold_districts {
            let producible = self.producible_items(pid, cid);
            for item in &producible {
                let Item::District { district, pos } = item else {
                    continue;
                };
                if self.map.tiles[pos].district_foundation.is_some() {
                    continue;
                }
                let modelled = self
                    .game_speed
                    .scale(self.district_cost_for_placement(pid, district, true))
                    * 4.0;
                // The host's purchase menu prices a district too, when present.
                let priced = |currency: &str| match self.host_purchase_price(cid, item, currency) {
                    Some(host) => host,
                    None => Some(modelled),
                };
                if faith_districts
                    && priced("faith").is_some_and(|cost| p.faith + f64::EPSILON >= cost)
                {
                    purchases.push(Action::BuyDistrict {
                        city: cid,
                        district: *district,
                        pos: *pos,
                        currency: "faith".to_string(),
                    });
                }
                if gold_districts
                    && priced("gold").is_some_and(|cost| p.gold + f64::EPSILON >= cost)
                {
                    purchases.push(Action::BuyDistrict {
                        city: cid,
                        district: *district,
                        pos: *pos,
                        currency: "gold".to_string(),
                    });
                }
            }
        }

        for (unit, spec) in &self.rules.units {
            // A kind this player has not unlocked prices to `None` on every
            // branch of `unit_purchase_cost_for_formation_uncached` — the
            // ordinary branch through `can_produce`, the religious, Warrior
            // Monk, Rock Band and Naturalist branches through their own
            // `unlocked` checks — except two: the host's own exported price
            // for the standard formation, and Lahore's Nihang, which never
            // asks. Both are kept; every other locked kind is skipped before
            // six price derivations (three formations, two currencies) each
            // rediscover the same `None`. Most of a ruleset's unit kinds are
            // locked at any moment of a game.
            // A Spy is never bought: the derivation refuses it before it
            // reads anything, at every formation and in either currency.
            if unit == "spy" {
                continue;
            }
            if unit != "nihang" {
                let plain = Item::Unit { unit: *unit };
                let host_priced = ["gold", "faith"].iter().any(|currency| {
                    matches!(
                        self.host_purchase_price(cid, &plain, currency),
                        Some(Some(_))
                    )
                });
                if !host_priced {
                    if !self.unlocked(pid, &spec.tech, &spec.civic) {
                        continue;
                    }
                    // The ordinary branch refuses a kind this city cannot
                    // produce, and a Corps or Army is producible only if its
                    // unit is (`can_produce`, `Item::Formation`), so one
                    // `can_produce` here answers all six derivations. The
                    // religious, Warrior Monk, Rock Band and Naturalist
                    // branches never ask it and are left to price themselves.
                    let ordinary = spec.class != "religious"
                        && !matches!(unit.as_str(), "warrior_monk" | "rock_band" | "naturalist");
                    if ordinary && !self.can_produce(pid, cid, &plain) {
                        continue;
                    }
                }
            }
            for formation in 0..=2 {
                for (currency, bank) in [("gold", p.gold), ("faith", p.faith)] {
                    let cost = if cache_unit_prices {
                        self.unit_purchase_cost_for_formation(pid, cid, unit, formation, currency)
                    } else {
                        self.unit_purchase_cost_for_formation_uncached(
                            pid, cid, unit, formation, currency,
                        )
                    };
                    if cost.is_some_and(|cost| bank + f64::EPSILON >= cost) {
                        purchases.push(Action::Buy {
                            city: cid,
                            unit: *unit,
                            formation,
                            currency: currency.to_string(),
                        });
                    }
                }
            }
        }

        let mut empire = Vec::new();
        if !p.is_minor {
            for unit in ["missionary", "apostle", "guru", "inquisitor"] {
                let cost = if cache_unit_prices {
                    self.unit_purchase_cost(pid, cid, unit, "faith")
                } else {
                    self.unit_purchase_cost_for_formation_uncached(pid, cid, unit, 0, "faith")
                };
                if cost.is_some_and(|cost| p.faith + f64::EPSILON >= cost) {
                    empire.push(Action::Buy {
                        city: cid,
                        unit: Name::new(unit),
                        formation: 0,
                        currency: "faith".to_string(),
                    });
                }
            }
            for building in self.rules.buildings.keys() {
                if self
                    .building_faith_purchase_cost(pid, cid, building)
                    .is_some_and(|cost| p.faith + f64::EPSILON >= cost)
                {
                    empire.push(Action::BuyBuilding {
                        city: cid,
                        building: *building,
                        currency: "faith".to_string(),
                    });
                }
            }
        }
        purchases.retain(|action| !self.purchase_action_is_blocked(action));
        empire.retain(|action| !self.purchase_action_is_blocked(action));
        (purchases, empire)
    }

    /// Purchase-only projection of
    /// `legal_actions_within(PURCHASES | EMPIRE)`, in identical relative
    /// order and under one query-memo scope.
    pub(crate) fn legal_purchase_actions(&self, pid: usize) -> Vec<Action> {
        let city_ids = self.purchase_action_city_ids(pid);
        let memo = self.query_memo();
        // A price memo has useful hits only when a caller already holds the
        // read-only scope around this menu. Otherwise each ordinary quote is
        // unique to this one enumeration, and the BTreeMap key plus insertion
        // is pure overhead.
        let cache_unit_prices = !memo.outermost;
        let per_city = city_ids
            .into_iter()
            .map(|cid| {
                self.legal_purchase_actions_for_city_with_price_memo(pid, cid, cache_unit_prices)
            })
            .collect::<Vec<_>>();
        let mut actions = Vec::new();
        actions.extend(
            per_city
                .iter()
                .flat_map(|(purchases, _)| purchases.iter().cloned()),
        );
        actions.extend(
            per_city
                .into_iter()
                .flat_map(|(_, empire)| empire.into_iter()),
        );
        actions
    }

    /// Every action `pid` could legally take right now.
    pub fn legal_actions(&self, pid: usize) -> Vec<Action> {
        self.legal_actions_within(pid, ActionFamilies::ALL)
    }

    /// `legal_actions`, skipping the enumeration of families the caller does
    /// not need.
    ///
    /// The named families let a caller avoid unrelated work: an agent looking
    /// for purchases no longer also walks every unit, prices every upgrade,
    /// builds live visibility, or values every diplomatic offer. `EndTurn` is
    /// retained for every non-capture query; all other returned actions belong
    /// to a requested family. See [`ActionFamilies`] for the mapping.
    pub fn legal_actions_within(&self, pid: usize, families: ActionFamilies) -> Vec<Action> {
        if self.is_finished() || self.current != pid {
            return vec![];
        }
        // Enumerating what a civilization may do asks the same questions of
        // the same cities many times over — what each produces, what each is
        // worth, how content each is. One memo scope over the whole
        // enumeration answers each of them once.
        let _memo = self.query_memo();
        let capture_actions = self.pending_city_capture_actions(pid);
        if !capture_actions.is_empty() {
            return capture_actions;
        }
        // A battlefield is decided by the fighting. The empire game around
        // a war — governments and policies, governors, religion, Great
        // People, envoys and trade routes, deals, the ballots of a Congress
        // that never convenes here, declarations against a rival the arena
        // already has at war — is never put to a seat on an arena, so a
        // player there is asked for orders and nothing else. Research and
        // civics are handled below: the arena picks its own research
        // (`arena_auto_research`) and pays no Culture at all.
        let families = if self.is_arena() {
            families.without(ActionFamilies::OFF_THE_BATTLEFIELD)
        } else {
            families
        };
        let p = &self.players[pid];
        let want_core = families.has(ActionFamilies::CORE);
        let want_units = families.has(ActionFamilies::UNITS);
        let mut acts = if want_core {
            self.legal_unit_upgrade_actions(pid)
        } else {
            Vec::new()
        };
        let needs_visibility = want_core || want_units;
        let current_visibility = if needs_visibility {
            self.player_vision_frame(pid)
        } else {
            Arc::new(TileBits::default())
        };
        let visibility_viewers = if needs_visibility {
            self.visibility_viewers(pid)
        } else {
            BTreeSet::new()
        };
        if want_core {
            for spy_id in self
                .spies
                .values()
                .filter(|spy| spy.owner == pid)
                .map(|spy| spy.id)
            {
                acts.extend(self.legal_spy_actions(pid, spy_id));
            }
        }
        let unit_order_ids = if want_units {
            self.player_unit_ids(pid)
        } else {
            Vec::new()
        };
        self.push_unit_order_actions(
            pid,
            p,
            unit_order_ids,
            &current_visibility,
            &visibility_viewers,
            &mut acts,
        );
        if families.has(ActionFamilies::CORPORATIONS) {
            for city in self.cities.values().filter(|city| city.owner == pid) {
                for position in &city.owned_tiles {
                    if self.can_found_corporation(pid, *position) {
                        acts.push(Action::FoundCorporation { pos: *position });
                    }
                }
            }
        }
        if families.has(ActionFamilies::PRODUCTS) {
            for city in self.cities.values().filter(|city| city.owner == pid) {
                for product in &city.products {
                    for target in self.cities.values().filter(|target| {
                        target.owner == pid
                            && target.id != city.id
                            && target.products.len() < self.product_capacity(target)
                    }) {
                        acts.push(Action::MoveProduct {
                            from: city.id,
                            to: target.id,
                            product: Name::new(product),
                        });
                    }
                }
            }
        }
        if families.has(ActionFamilies::FORMATIONS) {
            let owned_units = self.player_unit_ids(pid);
            // Releasing an escort belongs with the action that forms it.  In
            // particular, callers that ask only for FORMATIONS must be able
            // to both create and dissolve an escort formation.
            for &uid in &owned_units {
                if self.units[&uid].linked_to.is_some()
                    && !self.noncombat_action_blocked_by_zoc(uid)
                {
                    acts.push(Action::UnlinkUnits { unit: uid });
                }
            }
            for (index, &uid) in owned_units.iter().enumerate() {
                for &other in &owned_units[index + 1..] {
                    if self.can_combine_units(pid, uid, other).is_some() {
                        acts.push(Action::CombineUnits {
                            unit: uid,
                            with: other,
                        });
                    }
                    if self.can_link_units(pid, uid, other) {
                        let uid_military =
                            self.rules.units[self.units[&uid].kind].class == "military";
                        let (unit, with) = if uid_military {
                            (uid, other)
                        } else {
                            (other, uid)
                        };
                        acts.push(Action::LinkUnits { unit, with });
                    }
                }
            }
        }
        let want_purchases = families.has(ActionFamilies::PURCHASES);
        let purchasable_units: Vec<Name> = if want_purchases {
            self.rules.units.keys().cloned().collect()
        } else {
            Vec::new()
        };
        let purchase_city_ids = if want_purchases {
            self.player_city_ids(pid)
        } else {
            Vec::new()
        };
        self.push_city_purchase_actions(pid, p, purchase_city_ids, purchasable_units, &mut acts);
        if want_core {
            self.push_core_actions(pid, p, &current_visibility, &visibility_viewers, &mut acts);
        }
        if families.has(ActionFamilies::EMPIRE) && !p.is_minor {
            self.push_empire_actions(pid, p, &mut acts);
        }
        if families.has(ActionFamilies::DEALS) && !p.is_minor && !p.is_barbarian {
            self.push_deal_actions(pid, &mut acts);
        }
        if families.has(ActionFamilies::DIPLOMACY) && !p.is_barbarian {
            self.push_diplomacy_actions(pid, p, &mut acts);
        }
        acts.push(Action::EndTurn);
        if families.has(ActionFamilies::PURCHASES) || families.has(ActionFamilies::EMPIRE) {
            acts.retain(|action| !self.purchase_action_is_blocked(action));
        }
        acts
    }

    /// The orders each of `pid`'s units could take this turn.
    fn push_unit_order_actions(
        &self,
        pid: usize,
        p: &Player,
        unit_order_ids: Vec<u32>,
        current_visibility: &Arc<TileBits>,
        visibility_viewers: &BTreeSet<usize>,
        acts: &mut Vec<Action>,
    ) {
        for uid in unit_order_ids {
            let u = self.units[&uid].clone();
            let spec = &self.rules.units[u.kind];
            let embarked = self.is_embarked(&u);
            if self.noncombat_action_blocked_by_zoc(uid) {
                continue;
            }
            for promotion in self.available_promotions(uid) {
                acts.push(Action::Promote {
                    unit: uid,
                    promotion: Name::new(&promotion),
                });
            }
            if spec.domain.as_deref() == Some("air") {
                if u.moves_left > 0.0 {
                    let range = self.unit_attack_range(uid);
                    let origin = self.air_operation_origin(uid);
                    for target in self.wdisk(origin, range) {
                        if target != origin
                            && u.attacks_left > 0
                            && !self.strike_blocked(uid, target)
                            && self.enemy_air_strike_target_at(pid, target)
                            && self.combat_target_visible_at(
                                pid,
                                target,
                                current_visibility,
                                visibility_viewers,
                            )
                        {
                            acts.push(Action::AirStrike { unit: uid, target });
                        }
                        if target != origin
                            && spec.promotion_class == "air_bomber"
                            && u.attacks_left > 0
                            && !self.strike_blocked(uid, target)
                            && (u.hp >= 50
                                || self.promotion_effect(&u, "air_pillage_at_low_health") > 0.0)
                            && self.sees(current_visibility, target)
                            && self.air_pillageable_at(pid, target)
                        {
                            acts.push(Action::AirPillage { unit: uid, target });
                        }
                        if target != origin
                            && u.attacks_left > 0
                            && self.unit_has_priority_target(&u)
                            && self.priority_support_target_at(pid, target).is_some()
                            && self.combat_target_visible_at(
                                pid,
                                target,
                                current_visibility,
                                visibility_viewers,
                            )
                        {
                            acts.push(Action::PriorityTarget { unit: uid, target });
                        }
                    }
                    for base in self.wdisk(u.pos, self.air_rebase_range(uid)) {
                        if base != u.pos && self.can_air_base_at(pid, base, Some(uid)) {
                            acts.push(Action::AirRebase {
                                unit: uid,
                                to: base,
                            });
                        }
                    }
                    if !spec.siege && u.attacks_left > 0 {
                        for to in self.wdisk(u.pos, spec.moves.floor() as i32) {
                            if self.can_air_patrol_at(pid, to) {
                                acts.push(Action::AirPatrol { unit: uid, to });
                            }
                        }
                    }
                }
                continue;
            }
            if u.moves_left > 0.0 {
                for n in self.nbrs(u.pos) {
                    if self.can_move(uid, n) {
                        acts.push(Action::Move { unit: uid, to: n });
                    }
                }
                for destination in self.airlift_destinations(pid, uid) {
                    acts.push(Action::Move {
                        unit: uid,
                        to: destination,
                    });
                }
                if spec.class == "military" {
                    if !embarked
                        && spec.has_ranged_attack()
                        && u.attacks_left > 0
                        && (!spec.siege || !u.moved || self.siege_may_attack_after_moving(&u))
                    {
                        let range = self.unit_attack_range(uid);
                        for pos in self.wdisk(u.pos, range) {
                            if pos == u.pos || !self.map.tiles.contains_key(&pos) {
                                continue;
                            }
                            if self.enemy_ranged_target_at(pid, pos)
                                && self.combat_target_visible_at(
                                    pid,
                                    pos,
                                    current_visibility,
                                    visibility_viewers,
                                )
                                && self.unit_has_line_of_sight(uid, pos)
                                && !self.strike_blocked(uid, pos)
                            {
                                acts.push(Action::Ranged {
                                    unit: uid,
                                    target: pos,
                                });
                            }
                            if self.unit_has_priority_target(&u)
                                && self.priority_support_target_at(pid, pos).is_some()
                                && self.combat_target_visible_at(
                                    pid,
                                    pos,
                                    current_visibility,
                                    visibility_viewers,
                                )
                                && self.unit_has_line_of_sight(uid, pos)
                            {
                                acts.push(Action::PriorityTarget {
                                    unit: uid,
                                    target: pos,
                                });
                            }
                        }
                    }
                    if spec.is_melee_capable() && u.attacks_left > 0 {
                        for pos in self.nbrs(u.pos) {
                            if self.melee_order_is_legal(pid, uid, pos) {
                                acts.push(Action::Attack {
                                    unit: uid,
                                    target: pos,
                                });
                            }
                        }
                    }
                }
            }
            if u.kind == "settler" && !p.is_minor && self.can_found_city(uid) {
                acts.push(Action::FoundCity { unit: uid });
            }
            // ⚠ MOVEMENT IS REQUIRED TO BUILD, AND BUILDING SPENDS ALL OF IT.
            //
            // Civilization VI refuses `BUILD_IMPROVEMENT` from a unit with no
            // movement left, and completing one zeroes the unit's movement. So a
            // builder that has already improved a tile this turn cannot improve
            // another, and CIVVIS was proposing exactly that.
            //
            // Measured on live run `civvis-20260804T091315Z`: **all 19
            // `improve_refused` events** in the game had the right improvement for
            // the terrain, on a tile we owned, with the enabling tech researched,
            // with build charges remaining, and with the builder standing on the
            // target — and every one had `moves=0` after spending a charge that
            // same turn. Nothing else distinguished them.
            //
            // The project branch four lines below already required `moves_left`;
            // this one did not, which is what made the omission visible.
            if (u.kind == "builder" || !spec.builds.is_empty())
                && u.charges > 0
                && u.moves_left > 0.0
            {
                for imp in self.valid_improvements(pid, u.pos) {
                    if (u.kind == "builder" && !self.rules.improvements[&imp].builder_buildable)
                        || (u.kind != "builder" && !spec.builds.contains(&imp))
                    {
                        continue;
                    }
                    acts.push(Action::Improve {
                        unit: uid,
                        improvement: Name::new(&imp),
                    });
                }
                if u.kind == "builder" {
                    for operation in self.builder_operations(pid, u.pos) {
                        acts.push(Action::Improve {
                            unit: uid,
                            improvement: Name::new(&operation),
                        });
                    }
                }
            }
            if u.kind == "builder" && u.charges > 0 && u.moves_left > 0.0 {
                for city in self.player_city_ids(pid) {
                    if self.can_contribute_project(pid, uid, city) {
                        acts.push(Action::ContributeProject { unit: uid, city });
                    }
                }
            }
            if u.kind == "military_engineer" && u.charges > 0 && u.moves_left > 0.0 {
                for city in self.player_city_ids(pid) {
                    if self.can_contribute_district(pid, uid, city) {
                        acts.push(Action::ContributeDistrict { unit: uid, city });
                    }
                }
            }
            if self.can_build_railroad(pid, uid) {
                acts.push(Action::BuildRailroad { unit: uid });
            }
            if self.rock_concert_tourism(pid, uid).is_some() {
                acts.push(Action::PerformConcert { unit: uid });
            }
            if u.kind == "builder"
                && u.moves_left > 0.0
                && self.map.tiles[&u.pos].pillaged
                && self.map.tiles[&u.pos].improvement.is_some()
                && self.map.tiles[&u.pos]
                    .owner_city
                    .and_then(|cid| self.cities.get(&cid))
                    .is_some_and(|city| self.builder_may_improve_territory(pid, city.owner))
            {
                acts.push(Action::RepairImprovement { unit: uid });
            }
            if spec.class == "military"
                && u.moves_left > 0.0
                && !embarked
                && self.pillageable_at(pid, u.pos)
            {
                acts.push(Action::Pillage { unit: uid });
            }
            if self.can_coastal_raid(pid, &u) && u.moves_left > 0.0 {
                for target in self.nbrs(u.pos) {
                    if self.pillageable_at(pid, target) {
                        acts.push(Action::CoastalRaid { unit: uid, target });
                    }
                }
            }
            if spec.religious_spread > 0.0 && u.charges > 0 && u.moves_left > 0.0 {
                let near_city = self.city_at(u.pos).is_some()
                    || self
                        .nbrs(u.pos)
                        .iter()
                        .any(|position| self.city_at(*position).is_some());
                if near_city {
                    acts.push(Action::Spread { unit: uid });
                }
            }
            if matches!(u.kind.as_str(), "apostle" | "inquisitor")
                && u.moves_left > 0.0
                && u.attacks_left > 0
            {
                for target in self.nbrs(u.pos) {
                    let rival = self.unit_ids_at(target).iter().any(|id| {
                        let other = &self.units[id];
                        self.rules.units[other.kind].class == "religious"
                            && other.owner != pid
                            && other.religion.is_some()
                            && u.religion.is_some()
                            && other.religion != u.religion
                    });
                    if rival && self.theological_target_open(pid, target) {
                        acts.push(Action::TheologicalAttack { unit: uid, target });
                    }
                }
            }
            if spec.class == "military" && u.moves_left > 0.0 {
                for target_unit in self.unit_ids_at(u.pos) {
                    let target = &self.units[target_unit];
                    if target.owner != pid
                        && self.rules.units[target.kind].class == "religious"
                        && (self.is_at_war(pid, target.owner)
                            || target.religion.as_deref().is_some_and(|religion| {
                                self.congress_effect_active("world_religion", "B", religion)
                            }))
                    {
                        acts.push(Action::CondemnHeretic {
                            unit: uid,
                            target_unit: *target_unit,
                        });
                    }
                }
            }
            if u.kind == "guru" && u.charges > 0 && u.moves_left > 0.0 {
                let damaged = self
                    .wdisk(u.pos, 1)
                    .into_iter()
                    .flat_map(|pos| self.unit_ids_at(pos))
                    .any(|id| self.units[id].religion == u.religion && self.units[id].hp < 100);
                if damaged {
                    acts.push(Action::HealReligious { unit: uid });
                }
            }
            if u.kind == "inquisitor"
                && u.charges > 0
                && u.moves_left > 0.0
                && self
                    .city_at(u.pos)
                    .is_some_and(|cid| self.cities[&cid].owner == pid)
            {
                acts.push(Action::RemoveHeresy { unit: uid });
            }
            if u.kind == "apostle"
                && u.moves_left > 0.0
                && p.counters.get("inquisition").copied().unwrap_or(0) == 0
                && p.holy_city
                    .and_then(|cid| self.cities.get(&cid))
                    .is_some_and(|city| self.wdist(u.pos, city.pos) <= 1)
            {
                acts.push(Action::LaunchInquisition { unit: uid });
            }
            if u.kind == "apostle"
                && u.moves_left > 0.0
                && u.religion == p.religion
                && self.religion_belief_count(pid) < 4
            {
                let taken = |belief: &str| {
                    self.players.iter().any(|player| {
                        player
                            .religion_beliefs
                            .iter()
                            .any(|chosen| chosen == belief)
                    })
                };
                let has_enhancer = p
                    .religion_beliefs
                    .iter()
                    .any(|belief| self.rules.beliefs.enhancer.contains_key(belief));
                let has_worship = p
                    .religion_beliefs
                    .iter()
                    .any(|belief| self.rules.beliefs.worship.contains_key(belief));
                for belief in self
                    .rules
                    .beliefs
                    .enhancer
                    .keys()
                    .chain(self.rules.beliefs.worship.keys())
                    .filter(|belief| {
                        !taken(belief)
                            && ((!has_enhancer
                                && self.rules.beliefs.enhancer.contains_key(*belief))
                                || (!has_worship
                                    && self.rules.beliefs.worship.contains_key(*belief)))
                    })
                {
                    acts.push(Action::EvangelizeBelief {
                        unit: uid,
                        belief: Name::new(belief),
                    });
                }
            }
            if u.kind == "apostle"
                && u.moves_left > 0.0
                && u.charges > 0
                && self.promotion_effect(&u, "convert_barbarians") > 0.0
                && self.barb_pid.is_some_and(|barbarian| {
                    self.wdisk(u.pos, 1)
                        .into_iter()
                        .flat_map(|position| self.unit_ids_at(position))
                        .any(|other| self.units[other].owner == barbarian)
                })
            {
                acts.push(Action::ConvertBarbarians { unit: uid });
            }
        }
    }

    /// What each of `pid`'s cities could buy: plots, producible items, the
    /// districts a governor opens to Gold or Faith, and units.
    fn push_city_purchase_actions(
        &self,
        pid: usize,
        p: &Player,
        purchase_city_ids: Vec<u32>,
        purchasable_units: Vec<Name>,
        acts: &mut Vec<Action>,
    ) {
        for cid in purchase_city_ids {
            let mut plots: Vec<(Pos, f64)> = self
                .wdisk(self.cities[&cid].pos, 3)
                .into_iter()
                .filter_map(|position| {
                    self.plot_purchase_cost(pid, cid, position)
                        .map(|cost| (position, cost))
                })
                .collect();
            plots.sort_unstable_by_key(|(position, _)| *position);
            for (pos, cost) in plots {
                if p.gold + f64::EPSILON >= cost {
                    acts.push(Action::BuyPlot {
                        city: cid,
                        pos,
                        cost,
                    });
                }
            }
            let producible = self.producible_items(pid, cid);
            for item in &producible {
                acts.push(Action::Produce {
                    city: cid,
                    item: item.clone(),
                });
                if let Item::Building { building } = item {
                    if self
                        .building_gold_purchase_cost(pid, cid, building)
                        .is_some_and(|cost| p.gold + f64::EPSILON >= cost)
                        && !self.purchase_is_blocked(cid, item)
                    {
                        acts.push(Action::BuyBuilding {
                            city: cid,
                            building: *building,
                            currency: "gold".to_string(),
                        });
                    }
                }
            }
            let faith_districts = self.governor_effect(pid, cid, "faith_purchase_districts") > 0.0;
            let gold_districts = self.governor_effect(pid, cid, "gold_purchase_districts") > 0.0;
            if faith_districts || gold_districts {
                for item in &producible {
                    if let Item::District { district, pos } = item {
                        if self.map.tiles[pos].district_foundation.is_some() {
                            continue;
                        }
                        if self.purchase_is_blocked(cid, item) {
                            continue;
                        }
                        let modelled = self
                            .game_speed
                            .scale(self.district_cost_for_placement(pid, district, true))
                            * 4.0;
                        // The host's purchase menu prices a district too, when present.
                        let priced =
                            |currency: &str| match self.host_purchase_price(cid, item, currency) {
                                Some(host) => host,
                                None => Some(modelled),
                            };
                        if faith_districts
                            && priced("faith").is_some_and(|cost| p.faith + f64::EPSILON >= cost)
                        {
                            acts.push(Action::BuyDistrict {
                                city: cid,
                                district: *district,
                                pos: *pos,
                                currency: "faith".to_string(),
                            });
                        }
                        if gold_districts
                            && priced("gold").is_some_and(|cost| p.gold + f64::EPSILON >= cost)
                        {
                            acts.push(Action::BuyDistrict {
                                city: cid,
                                district: *district,
                                pos: *pos,
                                currency: "gold".to_string(),
                            });
                        }
                    }
                }
            }
            for unit in &purchasable_units {
                for formation in 0..=2 {
                    let item = if formation == 0 {
                        Item::Unit { unit: *unit }
                    } else {
                        Item::Formation {
                            unit: *unit,
                            formation,
                        }
                    };
                    if self.purchase_is_blocked(cid, &item) {
                        continue;
                    }
                    for (currency, bank) in [("gold", p.gold), ("faith", p.faith)] {
                        if self
                            .unit_purchase_cost_for_formation(pid, cid, unit, formation, currency)
                            .is_some_and(|cost| bank + f64::EPSILON >= cost)
                        {
                            acts.push(Action::Buy {
                                city: cid,
                                unit: *unit,
                                formation,
                                currency: currency.to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    /// The rest of `CORE`: city-state actions, the research and civic choices,
    /// and what `pid`'s units and cities can do besides move.
    fn push_core_actions(
        &self,
        pid: usize,
        p: &Player,
        current_visibility: &Arc<TileBits>,
        visibility_viewers: &BTreeSet<usize>,
        acts: &mut Vec<Action>,
    ) {
        for city_state in self.players.iter().filter(|player| {
            player.is_minor
                && !player.is_barbarian
                && player.alive
                && self.suzerain_of(player.id) == Some(pid)
        }) {
            if self
                .levy_cost(pid, city_state.id)
                .is_some_and(|cost| p.gold >= cost)
            {
                acts.push(Action::LevyMilitary {
                    player: city_state.id,
                });
            }
        }
        if p.research.is_none() && !self.is_arena() {
            for t in self.available_techs(pid) {
                acts.push(Action::Research {
                    tech: Name::new(&t),
                });
            }
        }
        if p.civic.is_none() && !self.is_arena() {
            for c in self.available_civics(pid) {
                acts.push(Action::Civic {
                    civic: Name::new(&c),
                });
            }
        }
        for uid in self.player_unit_ids(pid) {
            let u = self.units[&uid].clone();
            if self.unit_can_fortify(&u) && u.moves_left > 0.0 && !u.fortified {
                acts.push(Action::Fortify { unit: uid });
            }
        }
        for cid in self.player_city_ids(pid) {
            if self.city_can_strike(&self.cities[&cid]) {
                let cpos = self.cities[&cid].pos;
                for pos in self.wdisk(cpos, 2) {
                    if !self.map.tiles.contains_key(&pos) {
                        continue;
                    }
                    // See `peaceful_foreign_unit_at`: a city strike is a
                    // plot order too, and the host picks its defender.
                    let hit = !self.peaceful_foreign_unit_at(pid, pos)
                        && self.unit_ids_at(pos).iter().any(|oid| {
                            let o = &self.units[oid];
                            o.owner != pid
                                && self.is_at_war(pid, o.owner)
                                && self.rules.units[o.kind].class == "military"
                        });
                    if hit
                        && self.city_at(pos).is_none()
                        && self.defending_district_at(pos).is_none()
                        && self.combat_target_visible_at(
                            pid,
                            pos,
                            current_visibility,
                            visibility_viewers,
                        )
                        && self.has_line_of_sight(cpos, pos, true)
                    {
                        acts.push(Action::CityStrike {
                            city: cid,
                            target: pos,
                        });
                    }
                }
            }
            let city = &self.cities[&cid];
            for district in self.defending_districts(city) {
                if !self.defending_district_can_strike(city, &district) {
                    continue;
                }
                let source = district.pos;
                for pos in self.wdisk(source, 2) {
                    // See `peaceful_foreign_unit_at`: same plot order, same
                    // host-chosen defender, same surprise war.
                    let hit = !self.peaceful_foreign_unit_at(pid, pos)
                        && self.unit_ids_at(pos).iter().any(|id| {
                            let other = &self.units[id];
                            other.owner != pid
                                && self.is_at_war(pid, other.owner)
                                && self.rules.units[other.kind].class == "military"
                        });
                    if hit
                        && self.city_at(pos).is_none()
                        && self.defending_district_at(pos).is_none()
                        && self.combat_target_visible_at(
                            pid,
                            pos,
                            current_visibility,
                            visibility_viewers,
                        )
                        && self.has_line_of_sight(source, pos, true)
                    {
                        acts.push(self.defending_district_strike_action(cid, &district, pos));
                    }
                }
            }
        }
    }

    /// Empire-wide choices: government and policies, trade routes, Envoys,
    /// nuclear strikes, the Moon, Great People, governors and the rest of
    /// `EMPIRE`.
    fn push_empire_actions(&self, pid: usize, p: &Player, acts: &mut Vec<Action>) {
        for (g, spec) in &self.rules.governments {
            let ok = spec
                .civic
                .as_ref()
                .map(|c| p.civics.contains(c))
                .unwrap_or(true);
            // A second change has to wait out the Anarchy the first one
            // caused.
            if ok && p.government.as_deref() != Some(g.as_str()) && p.anarchy_turns == 0 {
                acts.push(Action::Government {
                    government: Name::new(g),
                });
            }
        }
        if !self.host_policy_change_blocked(pid) {
            for card in self.available_policies(pid) {
                let mut next = p.policies.clone();
                next.insert(Name::new(&card));
                if self.policies_fit(pid, &next) {
                    acts.push(Action::SlotPolicy {
                        policy: Name::new(&card),
                    });
                }
            }
            for card in &p.policies {
                acts.push(Action::UnslotPolicy {
                    policy: Name::new(card),
                });
            }
        }
        if self.active_routes(pid) < self.trade_capacity(pid) {
            for uid in self.player_unit_ids(pid) {
                if self.units[&uid].kind != "trader" {
                    continue;
                }
                let origin = match self.city_at(self.units[&uid].pos) {
                    Some(cid) if self.cities[&cid].owner == pid => cid,
                    _ => continue,
                };
                // Ask the one validator `do_trade_route` will consult,
                // not a looser paraphrase of it. The inline filter here
                // used to skip `blocked_trade_routes` and the World
                // Congress embargo, so run civvis-20260815T081505Z
                // reported "routes=2" in its notes for 54 straight turns
                // while the trader idled — both destinations had been
                // host-refused at t25/t26 and the actual picker rightly
                // proposed nothing. An action this enumeration offers
                // must be one the engine would take.
                for dest in self.cities.keys() {
                    if !self.can_establish_trade_route(pid, origin, *dest) {
                        continue;
                    }
                    acts.push(Action::TradeRoute {
                        unit: uid,
                        city: *dest,
                    });
                }
            }
        }
        if p.envoys_free > 0 {
            for m in &self.players {
                if self.can_send_envoy(pid, m.id) {
                    acts.push(Action::SendEnvoy { player: m.id });
                }
            }
        }
        // Nuclear strikes are enumerated against enemy city centers some
        // platform can reach; arbitrary revealed tiles remain legal through
        // apply.
        for (device_key, thermonuclear, weapon) in [
            ("project_effect:nuclear_devices", false, "nuclear_device"),
            (
                "project_effect:thermonuclear_devices",
                true,
                "thermonuclear_device",
            ),
        ] {
            if p.counters.get(device_key).copied().unwrap_or(0) <= 0 {
                continue;
            }
            let range = self.rules.wmds[weapon].icbm_strike_range;
            // Hoisted once rather than rebuilt per candidate target: a
            // carrier's reach does not depend on which city the order names,
            // and rescanning every unit inside a doubly-nested city loop
            // would put the whole roster in a hot enumeration path.
            let carriers = self.wmd_carriers(pid, thermonuclear);
            let launchers: Vec<(u32, Vec<Pos>)> = self
                .cities
                .values()
                .filter(|city| city.owner == pid)
                .map(|city| {
                    let silos = city
                        .owned_tiles
                        .iter()
                        .copied()
                        .filter(|position| {
                            self.map.tiles.get(position).is_some_and(|tile| {
                                tile.improvement.as_deref() == Some("missile_silo")
                                    && !tile.pillaged
                            })
                        })
                        .collect();
                    (city.id, silos)
                })
                .collect();
            for enemy in self.cities.values() {
                if !self.is_at_war(pid, enemy.owner) || !p.explored.contains(&enemy.pos) {
                    continue;
                }
                let mut offered = false;
                for (launch, silos) in &launchers {
                    if silos
                        .iter()
                        .any(|position| self.wdist(*position, enemy.pos) <= range)
                    {
                        offered = true;
                        acts.push(Action::WmdStrike {
                            city: *launch,
                            target: enemy.pos,
                            thermonuclear,
                        });
                    }
                }
                // A carrier in range makes the target reachable whichever city
                // the order names, so offer the shot once rather than per city.
                if !offered
                    && carriers
                        .iter()
                        .any(|(_, from, reach)| self.wdist(*from, enemy.pos) <= *reach)
                {
                    if let Some((launch, _)) = launchers.first() {
                        acts.push(Action::WmdStrike {
                            city: *launch,
                            target: enemy.pos,
                            thermonuclear,
                        });
                    }
                }
            }
        }
        // The Moon, when the Modified Future Era put ore on it. Aiming is
        // offered as one order per ore per city center and a strike as one
        // per enemy city center: a driver can land on or hit any tile at
        // all, and the arbitrary ones stay legal through `apply`, but a
        // catalog with every owned hexagon in it several times over is not
        // a decision anybody — person, agent or search — can read.
        if self.mass_drivers(pid) > 0 {
            let ores: Vec<Name> = self
                .moon_ores()
                .into_iter()
                .filter(|ore| {
                    self.moon_deposit(ore) >= 1.0 && self.resource_visible_to(pid, ore.as_str())
                })
                .collect();
            if !ores.is_empty() {
                for city in self.cities.values().filter(|city| city.owner == pid) {
                    for ore in &ores {
                        if p.mass_driver_site == Some(city.pos)
                            && p.mass_driver_ore.as_ref() == Some(ore)
                        {
                            continue;
                        }
                        acts.push(Action::AimMassDriver {
                            site: city.pos,
                            ore: *ore,
                        });
                    }
                }
            }
            let armed = p
                .mass_driver_ore
                .is_some_and(|ore| self.strategic_stockpile(pid, ore) >= 1.0);
            if armed && p.mass_driver_shots < self.mass_drivers(pid) {
                for enemy in self.cities.values() {
                    if self.is_at_war(pid, enemy.owner) && p.explored.contains(&enemy.pos) {
                        acts.push(Action::MassDriverStrike { target: enemy.pos });
                    }
                }
            }
        }
        let gp_kinds: BTreeSet<String> = self
            .rules
            .great_people
            .values()
            .map(|person| person.kind.clone())
            .collect();
        for kind in gp_kinds {
            let Some(_) = self.current_great_person(&kind) else {
                continue;
            };
            if !self.great_person_class_offered_now(pid, &kind) {
                continue;
            }
            if !self.can_activate_current_great_person(pid, &kind) {
                continue;
            }
            let points = p.gpp.get(&kind).copied().unwrap_or(0.0);
            let missing = (self.gp_cost(pid, &kind) - points).max(0.0);
            if missing <= 0.0 {
                acts.push(Action::RecruitGreatPerson { kind });
            } else {
                if self
                    .great_person_patronage_price(pid, &kind, "gold")
                    .is_some_and(|price| p.gold + f64::EPSILON >= price)
                {
                    acts.push(Action::PatronizeGreatPerson {
                        kind: kind.clone(),
                        currency: "gold".to_string(),
                    });
                }
                if self
                    .great_person_patronage_price(pid, &kind, "faith")
                    .is_some_and(|price| p.faith + f64::EPSILON >= price)
                {
                    acts.push(Action::PatronizeGreatPerson {
                        kind,
                        currency: "faith".to_string(),
                    });
                }
            }
        }
        if self.game_mode("secret_societies")
            && p.secret_society.is_none()
            && p.civics.contains(&crate::name!("code_of_laws"))
        {
            for society in ["hermetic_order", "owls_of_minerva", "voidsingers"] {
                acts.push(Action::ChooseSecretSociety {
                    society: Name::new(society),
                });
            }
        }
        if !p.is_minor
            && !p.is_barbarian
            && p.pantheon.is_none()
            && p.faith >= self.pantheon_faith_cost()
        {
            for b in self.rules.beliefs.pantheon.keys() {
                if !self
                    .players
                    .iter()
                    .any(|o| o.pantheon.as_deref() == Some(b.as_str()))
                {
                    acts.push(Action::ChoosePantheon {
                        belief: Name::new(b),
                    });
                }
            }
        }
        if p.prophet_pending && self.religions_founded() < self.max_religions() {
            let taken = |b: &str| {
                self.players
                    .iter()
                    .any(|o| o.religion_beliefs.iter().any(|x| x == b))
            };
            for fo in self.rules.beliefs.follower.keys().filter(|b| !taken(b)) {
                for fu in self.rules.beliefs.founder.keys().filter(|b| !taken(b)) {
                    acts.push(Action::FoundReligion {
                        follower: Name::new(fo),
                        founder: Name::new(fu),
                    });
                }
            }
        }
        if p.governor_roster.is_empty() && p.governors.len() < self.governor_titles(pid) {
            for cid in self.player_city_ids(pid) {
                if !p.governors.contains(&cid) {
                    acts.push(Action::AssignGovernor { city: cid });
                }
            }
        }
        if p.governor_titles_spent < self.governor_titles(pid) {
            for governor in self.rules.governors.keys() {
                if !p.governor_roster.contains_key(governor) {
                    for city in self.player_city_ids(pid) {
                        if !p
                            .governor_roster
                            .values()
                            .any(|state| state.city == Some(city))
                        {
                            acts.push(Action::AppointGovernor {
                                governor: Name::new(governor),
                                city,
                            });
                        }
                    }
                } else {
                    for promotion in self.available_governor_promotions(pid, governor) {
                        acts.push(Action::PromoteGovernor {
                            governor: Name::new(governor),
                            promotion: Name::new(&promotion),
                        });
                    }
                }
            }
        }
        for (governor, state) in &p.governor_roster {
            for city in self.player_city_ids(pid) {
                if state.city != Some(city)
                    && !p
                        .governor_roster
                        .values()
                        .any(|other| other.city == Some(city))
                {
                    acts.push(Action::ReassignGovernor {
                        governor: Name::new(governor),
                        city,
                    });
                }
            }
        }
        for cid in self.player_city_ids(pid) {
            for unit in ["missionary", "apostle", "guru", "inquisitor"] {
                if self
                    .unit_purchase_cost(pid, cid, unit, "faith")
                    .is_some_and(|cost| p.faith + f64::EPSILON >= cost)
                {
                    acts.push(Action::Buy {
                        city: cid,
                        unit: Name::new(unit),
                        formation: 0,
                        currency: "faith".to_string(),
                    });
                }
            }
            for building in self.rules.buildings.keys() {
                if self
                    .building_faith_purchase_cost(pid, cid, building)
                    .is_some_and(|cost| p.faith + f64::EPSILON >= cost)
                {
                    acts.push(Action::BuyBuilding {
                        city: cid,
                        building: *building,
                        currency: "faith".to_string(),
                    });
                }
            }
        }
    }

    /// Deals to answer or offer, Dedications, and the World Congress.
    fn push_deal_actions(&self, pid: usize, acts: &mut Vec<Action>) {
        for deal in self
            .pending_deals
            .iter()
            .filter(|deal| deal.to == pid && deal.expires >= self.turn)
        {
            // A peace clause is only acceptable while the war it settles
            // is being fought and has run its minimum turns; offering the
            // acceptance anyway would advertise an action that can only
            // fail. Rejecting a dead offer stays available either way.
            let settleable = !deal.peace
                || (self.is_at_war(deal.from, deal.to)
                    && self.peace_available_at(deal.from, deal.to).is_none());
            if settleable {
                acts.push(Action::AcceptDeal { deal: deal.id });
            }
            acts.push(Action::RejectDeal { deal: deal.id });
        }
        for deal in self.quick_deals(pid) {
            acts.push(Action::Trade {
                player: deal.partner,
                offer: Box::new(deal.offer),
                request: Box::new(deal.request),
            });
        }
        for dedication in self.available_dedications(pid) {
            acts.push(Action::ChooseDedication {
                dedication: Name::new(&dedication),
            });
        }
        if let Some(congress) = self
            .congress
            .as_ref()
            .filter(|session| self.turn < session.closes)
        {
            let max_votes = self.congress_affordable_votes(pid);
            for resolution in &congress.resolutions {
                if resolution.ballots.contains_key(&pid) {
                    continue;
                }
                for choice in &resolution.choices {
                    if let Some(proposal) = self.emergency_proposal_for_resolution(&resolution.id) {
                        let (outcome, target) = Self::congress_choice_parts(choice);
                        let allowed = (proposal.eligible.contains(&pid)
                            && ((outcome, target) == ("A", "support")
                                || (outcome, target) == ("B", "oppose")))
                            || (proposal.target == pid && (outcome, target) == ("B", "oppose"));
                        if !allowed {
                            continue;
                        }
                    }
                    acts.push(Action::CongressVote {
                        resolution: Name::new(&resolution.id),
                        choice: choice.clone(),
                        votes: max_votes,
                    });
                }
            }
        }
    }

    /// The diplomatic acts open toward every civilization `pid` has met.
    fn push_diplomacy_actions(&self, pid: usize, p: &Player, acts: &mut Vec<Action>) {
        for o in &self.players {
            // Diplomacy needs a counterpart. Until the two civilizations
            // have met there is no embassy to send a demand to, no peace
            // to sign and nobody to denounce, so none of the acts below
            // exist against an empire this one has never heard of.
            if o.id != pid && o.alive && !o.is_barbarian && self.has_met(pid, o.id) {
                if self.is_at_war(pid, o.id) {
                    // A city-state can be at war only because its
                    // Suzerain is. That derived war must be ended with the
                    // Suzerain, not through an inapplicable bilateral
                    // peace action against the city-state itself.
                    if self.at_war.contains(&pair(pid, o.id))
                        && !self.emergency_war_pair(pid, o.id)
                        && self.peace_available_at(pid, o.id).is_none()
                    {
                        acts.push(Action::MakePeace { player: o.id });
                    }
                } else if !p.is_minor && !o.is_minor {
                    // A peace treaty is binding while it runs: no casus
                    // belli reopens the war before it expires. Emergency
                    // coalition partners are equally bound while they
                    // prosecute their shared objective.
                    let non_aggression = self.peace_treaty_until(pid, o.id).is_some()
                        || self.emergency_coalition_pair(pid, o.id);
                    if !non_aggression
                        && !self.host_blocks_war(pid, o.id)
                        && !self.are_friends(pid, o.id)
                        && !self.are_allied(pid, o.id)
                    {
                        if self.host_war_type_permission(pid, o.id, "surprise_war") != Some(false) {
                            acts.push(Action::DeclareWar { player: o.id });
                        }
                        for casus_belli in [
                            "formal_war",
                            "holy_war",
                            "reconquest_war",
                            "protectorate_war",
                            "liberation_war",
                            "colonial_war",
                            "territorial_war",
                            "golden_age_war",
                            "retribution_war",
                            "ideological_war",
                        ] {
                            if !self.casus_belli_available(pid, o.id, casus_belli) {
                                continue;
                            }
                            acts.push(Action::DeclareWarWithCasusBelli {
                                player: o.id,
                                casus_belli: casus_belli.to_string(),
                            });
                        }
                    }
                    if !self.denounced_active(pid, o.id) && !self.are_friends(pid, o.id) {
                        acts.push(Action::Denounce { player: o.id });
                    }
                    if p.gold + f64::EPSILON >= DELEGATION_GOLD
                        && self.diplomatic_mission_to(pid, o.id).is_none()
                    {
                        acts.push(Action::SendDelegation { player: o.id });
                    }
                    if p.gold + f64::EPSILON >= EMBASSY_GOLD
                        && p.civics.contains(&crate::name!("diplomatic_service"))
                        && !self
                            .diplomatic_mission_to(pid, o.id)
                            .is_some_and(|mission| mission.kind == "embassy")
                    {
                        acts.push(Action::SendEmbassy { player: o.id });
                    }
                    if o.gold > 0.0 && self.demand_available(pid, o.id) {
                        acts.push(Action::DemandGold {
                            player: o.id,
                            gold: o.gold.min(25.0),
                        });
                    }
                    for promise in [
                        "no_settling",
                        "no_conversion",
                        "no_spying",
                        "no_city_state_attack",
                    ] {
                        if self.promise_request_available(pid, o.id, promise) {
                            acts.push(Action::RequestPromise {
                                player: o.id,
                                promise: promise.to_string(),
                            });
                        }
                    }
                    if !self.are_friends(pid, o.id) {
                        acts.push(Action::ProposeDeal {
                            player: o.id,
                            give_gold: 0.0,
                            request_gold: 0.0,
                            open_borders: false,
                            friendship: true,
                            peace: false,
                            alliance: None,
                        });
                    }
                    if p.civics.contains(&crate::name!("civil_service"))
                        && o.civics.contains(&crate::name!("civil_service"))
                        && !self.are_allied(pid, o.id)
                    {
                        for kind in ["research", "cultural", "economic", "military", "religious"] {
                            if kind == "research"
                                && (self.tree_effect(pid, "research_agreements") <= 0.0
                                    || self.tree_effect(o.id, "research_agreements") <= 0.0)
                            {
                                continue;
                            }
                            acts.push(Action::ProposeDeal {
                                player: o.id,
                                give_gold: 0.0,
                                request_gold: 0.0,
                                open_borders: self.tree_effect(pid, "open_borders") > 0.0,
                                friendship: true,
                                peace: false,
                                alliance: Some(kind.to_string()),
                            });
                        }
                    }
                    if self.defensive_pact_available(pid, o.id) {
                        acts.push(Action::ProposeDefensivePact { player: o.id });
                    }
                    for target in self.players.iter().filter(|target| {
                        target.id != pid
                            && target.id != o.id
                            && target.alive
                            && !target.is_minor
                            && !target.is_barbarian
                    }) {
                        if self.joint_war_available(pid, o.id, target.id) {
                            acts.push(Action::ProposeJointWar {
                                player: o.id,
                                target: target.id,
                            });
                        }
                    }
                } else if !p.is_minor
                    && !self.host_blocks_war(pid, o.id)
                    && !self.are_friends(pid, o.id)
                    && !self.are_allied(pid, o.id)
                    && self.peace_treaty_until(pid, o.id).is_none()
                {
                    acts.push(Action::DeclareWar { player: o.id });
                }
            }
        }
    }

    /// Whether the engine will accept `Attack { unit: uid, target }` right
    /// now — `legal_actions_within`'s own melee predicates plus the hostile
    /// target check, exactly as `do_attack` applies them. See
    /// `ranged_order_is_legal` for why a picker asks before proposing; the
    /// same census counted 17 authoritative melee refusals ("unit cannot
    /// attack into that domain", "not enough movement to attack", "no combat
    /// target"), all from the same candidate loop.
    pub(crate) fn melee_order_is_legal(&self, pid: usize, uid: u32, target: Pos) -> bool {
        let Some(unit) = self.units.get(&uid) else {
            return false;
        };
        self.rules.units[unit.kind].is_melee_capable()
            && (!self.is_embarked(unit)
                || self
                    .map
                    .get(target)
                    .is_some_and(|tile| !self.rules.is_water(tile)))
            && unit.moves_left > 0.0
            && unit.attacks_left > 0
            && self.wdist(unit.pos, target) == 1
            && self.enemy_combat_target_at(pid, target)
            && self.unit_can_melee_target_domain(uid, target)
            && self.can_pay_melee_entry(uid, target)
            && !self.strike_blocked(uid, target)
    }

    pub(crate) fn rock_concert_tourism(&self, pid: usize, uid: u32) -> Option<f64> {
        let unit = self.units.get(&uid)?;
        if unit.owner != pid
            || unit.kind != "rock_band"
            || unit.promotions.is_empty()
            || unit.moves_left <= 0.0
        {
            return None;
        }
        self.rock_concert_venue_details(pid, Some(unit), unit.pos)
            .map(|(tourism, _, _)| {
                // Flower Power: ROCK_BAND_TOURISM_BOMB_VALUE_PEACE +50%.
                tourism * (1.0 + self.policy_effect(pid, "rock_band_concert_tourism_pct") / 100.0)
            })
    }

    /// Authoritative Gold/Faith quote for a unit purchase. Action enumeration
    /// and execution share this path, so an AI can see an action made
    /// affordable by Holy Order, Monumentality, a Product, or a generic
    /// per-unit modifier instead of learning about the discount only after it
    /// tries to apply the action.
    pub fn unit_purchase_cost(
        &self,
        pid: usize,
        cid: u32,
        unit: &str,
        currency: &str,
    ) -> Option<f64> {
        self.unit_purchase_cost_for_formation(pid, cid, unit, 0, currency)
    }

    /// [`Self::quoted_payment`], except on a quote `viewer` asks for while
    /// it is [`Game::lump_quote_viewer`]: then a payer whose treasury covers
    /// the price rounded up, above the same reserve, pays exactly that lump
    /// and nothing per turn. Otherwise the ordinary quote.
    pub(in crate::game) fn viewer_quoted_payment(
        &self,
        viewer: usize,
        payer: usize,
        price: f64,
    ) -> Option<DealItems> {
        if self.lump_quote_viewer == Some(viewer) && price.is_finite() && price > 0.0 {
            let lump = price.ceil();
            let reserve = (self.players[payer].gold * 0.30).min(40.0);
            if self.players[payer].gold - reserve >= lump {
                return Some(DealItems {
                    gold: lump,
                    ..DealItems::default()
                });
            }
        }
        self.quoted_payment(payer, price)
    }

    pub(in crate::game) fn quoted_payment(&self, payer: usize, price: f64) -> Option<DealItems> {
        if !price.is_finite() || price <= 0.0 {
            return None;
        }
        let reserve = (self.players[payer].gold * 0.30).min(40.0);
        let lump = (self.players[payer].gold - reserve)
            .max(0.0)
            .min(price)
            .floor();
        let remaining = (price - lump).max(0.0);
        let gpt = ((remaining / 25.0) * 10.0).ceil() / 10.0;
        if gpt > self.empire_gold_per_turn(payer).max(0.0) || lump + gpt > self.players[payer].gold
        {
            return None;
        }
        Some(DealItems {
            gold: lump,
            gold_per_turn: gpt,
            ..DealItems::default()
        })
    }

    pub(in crate::game) fn quote_asset_trade(
        &self,
        viewer: usize,
        partner: usize,
        category: &str,
        item: &str,
        direction: &str,
        asset: DealItems,
    ) -> Option<QuickDeal> {
        let (offer, request) = if direction == "sell" {
            let minimum = self.give_items_cost(viewer, partner, &asset);
            let maximum = self.receive_items_value(partner, viewer, &asset);
            if maximum <= minimum + 1.0 {
                return None;
            }
            let payment = self.viewer_quoted_payment(viewer, partner, (minimum + maximum) / 2.0)?;
            (asset, payment)
        } else {
            let minimum = self.give_items_cost(partner, viewer, &asset);
            let maximum = self.receive_items_value(viewer, partner, &asset);
            if maximum <= minimum + 1.0 {
                return None;
            }
            let payment = self.viewer_quoted_payment(viewer, viewer, (minimum + maximum) / 2.0)?;
            (payment, asset)
        };
        let (my_value, partner_value) = self
            .validate_trade(viewer, partner, &offer, &request)
            .ok()?;
        Some(QuickDeal {
            partner,
            category: category.to_string(),
            item: item.to_string(),
            direction: direction.to_string(),
            offer,
            request,
            my_value,
            partner_value,
        })
    }

    /// Gather every AI's acceptable resource, Great Work, Favor, and Open
    /// Borders quote and sort it by benefit to the requesting player,
    /// matching Quick Deals' compare-all-counterparties workflow.
    pub fn quick_deals(&self, viewer: usize) -> Vec<QuickDeal> {
        if viewer >= self.players.len() || !self.players[viewer].alive {
            return Vec::new();
        }
        let partners: Vec<usize> = self
            .players
            .iter()
            .filter(|player| {
                player.id != viewer
                    && player.alive
                    && !player.is_minor
                    && !player.is_barbarian
                    // There is nobody to negotiate with until the two
                    // civilizations have found each other.
                    && self.has_met(viewer, player.id)
                    && !self.is_at_war(viewer, player.id)
            })
            .map(|player| player.id)
            .collect();
        let resources: Vec<(Name, String)> = self
            .rules
            .resources
            .iter()
            .filter(|(_, spec)| matches!(spec.class.as_str(), "luxury" | "strategic"))
            .map(|(name, spec)| (*name, spec.class.clone()))
            .collect();
        // How much of each resource the asking player holds does not depend on
        // who they are asking, and counting it walks every tile of every city
        // they own — so it is counted once for the whole round of offers
        // rather than four times per counterparty per resource.
        let viewer_holds: Vec<i32> = resources
            .iter()
            .map(|(resource, _)| self.resource_access_count(viewer, resource))
            .collect();
        let mut deals = Vec::new();
        for partner in partners {
            for ((resource, class), held) in resources.iter().zip(&viewer_holds) {
                if !self.resource_visible_to(viewer, resource)
                    || !self.resource_visible_to(partner, resource)
                {
                    continue;
                }
                let quantity = if class == "strategic" { 10 } else { 1 };
                let partner_holds = self.resource_access_count(partner, resource);
                let mut asset = DealItems::default();
                asset.resources.insert(resource.to_string(), quantity);
                if *held > quantity && partner_holds == 0 {
                    if let Some(deal) = self.quote_asset_trade(
                        viewer,
                        partner,
                        class,
                        resource,
                        "sell",
                        asset.clone(),
                    ) {
                        deals.push(deal);
                    }
                }
                if partner_holds > quantity && *held == 0 {
                    if let Some(deal) =
                        self.quote_asset_trade(viewer, partner, class, resource, "buy", asset)
                    {
                        deals.push(deal);
                    }
                }
            }

            for kind in [
                "writing",
                "art",
                "religious_art",
                "artifact",
                "music",
                "relic",
            ] {
                let mut asset = DealItems::default();
                asset.great_works.insert(kind.to_string(), 1);
                // Keep a civilization's last work of each kind out of the
                // automatic market. A singleton remains manually tradable
                // when the buyer's modifiers justify the seller's scarcity
                // premium, but Quick Deals exposes only genuine duplicates.
                if self.housed_great_work_count(viewer, kind) > 1 {
                    if let Some(deal) = self.quote_asset_trade(
                        viewer,
                        partner,
                        "great_work",
                        kind,
                        "sell",
                        asset.clone(),
                    ) {
                        deals.push(deal);
                    }
                }
                if self.housed_great_work_count(partner, kind) > 1 {
                    if let Some(deal) =
                        self.quote_asset_trade(viewer, partner, "great_work", kind, "buy", asset)
                    {
                        deals.push(deal);
                    }
                }
            }

            let captive_ids: Vec<u32> = self
                .spies
                .values()
                .filter(|spy| {
                    (spy.owner == viewer && spy.captured_by == Some(partner))
                        || (spy.owner == partner && spy.captured_by == Some(viewer))
                })
                .map(|spy| spy.id)
                .collect();
            for spy_id in captive_ids {
                let mut asset = DealItems::default();
                asset.captured_spies.push(spy_id);
                let (direction, category) = if self.spies[&spy_id].owner == viewer {
                    ("buy", "recover_spy")
                } else {
                    ("sell", "release_spy")
                };
                if let Some(deal) = self.quote_asset_trade(
                    viewer,
                    partner,
                    category,
                    &spy_id.to_string(),
                    direction,
                    asset,
                ) {
                    deals.push(deal);
                }
            }

            let favor = DealItems {
                diplomatic_favor: 10.0,
                ..DealItems::default()
            };
            if self.players[viewer].diplomatic_favor >= 20.0 {
                if let Some(deal) = self.quote_asset_trade(
                    viewer,
                    partner,
                    "favor",
                    "diplomatic_favor",
                    "sell",
                    favor.clone(),
                ) {
                    deals.push(deal);
                }
            }
            if self.players[partner].diplomatic_favor >= 20.0 {
                if let Some(deal) = self.quote_asset_trade(
                    viewer,
                    partner,
                    "favor",
                    "diplomatic_favor",
                    "buy",
                    favor,
                ) {
                    deals.push(deal);
                }
            }

            if self.tree_effect(viewer, "open_borders") > 0.0
                && self.tree_effect(partner, "open_borders") > 0.0
            {
                let borders = DealItems {
                    open_borders: true,
                    ..DealItems::default()
                };
                if !self.has_open_borders(partner, viewer) {
                    if let Some(deal) = self.quote_asset_trade(
                        viewer,
                        partner,
                        "agreement",
                        "open_borders",
                        "sell",
                        borders.clone(),
                    ) {
                        deals.push(deal);
                    }
                }
                if !self.has_open_borders(viewer, partner) {
                    if let Some(deal) = self.quote_asset_trade(
                        viewer,
                        partner,
                        "agreement",
                        "open_borders",
                        "buy",
                        borders,
                    ) {
                        deals.push(deal);
                    }
                }
            }
        }
        deals.sort_by(|left, right| {
            right
                .my_value
                .partial_cmp(&left.my_value)
                .unwrap()
                .then_with(|| {
                    right
                        .partner_value
                        .partial_cmp(&left.partner_value)
                        .unwrap()
                })
                .then_with(|| left.partner.cmp(&right.partner))
                .then_with(|| left.item.cmp(&right.item))
        });
        deals
    }
}
