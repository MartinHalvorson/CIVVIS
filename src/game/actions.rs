//! The action layer: every legal action a seat may take, and what taking it
//! does.
//!
//! Carved verbatim out of the `action layer` section of the single
//! `impl Game` in `game.rs`.  `legal_actions`, `legal_actions_within` and the
//! per-family enumerators; `apply` and every `do_*` handler behind it --
//! movement, combat, air missions, WMDs, purchases, production, promotions,
//! diplomacy, deals, espionage and the trade routes.
//!
//! This is the same inherent `impl Game`, in a child module: nothing about
//! the rules moved, only the text.  See `docs/VERSION_CONTROL.md`.

use super::*;

impl Game {
    // ------------------------------------------------------- action layer

    pub(super) fn pending_city_capture_actions(&self, pid: usize) -> Vec<Action> {
        let mut actions = Vec::new();
        for city in self
            .cities
            .values()
            .filter(|city| city.owner == pid && city.captured_from.is_some())
        {
            let can_raze = self.city_can_be_razed_by(pid, city);
            if !self.players[pid].is_minor || !can_raze {
                actions.push(Action::KeepCity { city: city.id });
            }
            if can_raze {
                actions.push(Action::RazeCity { city: city.id });
            }
            if !self.players[pid].is_minor
                && city.original_owner != pid
                && city.captured_from != Some(city.original_owner)
                && self
                    .players
                    .get(city.original_owner)
                    .is_some_and(|founder| !founder.is_barbarian)
            {
                actions.push(Action::LiberateCity { city: city.id });
            }
        }
        actions
    }

    /// Original Capitals, former city-states, a civilization's own recaptured
    /// cities, and cities founded by its active ally cannot be razed in Civ VI.
    /// The latter two cases matter most after a Loyalty revolt or a third-party
    /// occupation, where ownership immediately before capture is misleading.
    pub(super) fn city_can_be_razed_by(&self, pid: usize, city: &City) -> bool {
        !city.is_capital
            && !self.players[city.original_owner].is_minor
            && city.original_owner != pid
            && !self.are_allied(pid, city.original_owner)
    }

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
                        && self.enemy_air_strike_target_at(pid, target)
                    {
                        actions.push(Action::AirStrike { unit: uid, target });
                    }
                    if target != origin
                        && spec.promotion_class == "air_bomber"
                        && u.attacks_left > 0
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
                    building: Name::new(building),
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
                        district: Name::new(district),
                        pos: *pos,
                        currency: "faith".to_string(),
                    });
                }
                if gold_districts
                    && priced("gold").is_some_and(|cost| p.gold + f64::EPSILON >= cost)
                {
                    purchases.push(Action::BuyDistrict {
                        city: cid,
                        district: Name::new(district),
                        pos: *pos,
                        currency: "gold".to_string(),
                    });
                }
            }
        }

        for unit in self.rules.units.keys() {
            for formation in 0..=2 {
                for (currency, bank) in [("gold", p.gold), ("faith", p.faith)] {
                    if self
                        .unit_purchase_cost_for_formation(pid, cid, unit, formation, currency)
                        .is_some_and(|cost| bank + f64::EPSILON >= cost)
                    {
                        purchases.push(Action::Buy {
                            city: cid,
                            unit: Name::new(unit),
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
                if self
                    .unit_purchase_cost(pid, cid, unit, "faith")
                    .is_some_and(|cost| p.faith + f64::EPSILON >= cost)
                {
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
                        building: Name::new(building),
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
        let _memo = self.query_memo();
        let per_city = city_ids
            .into_iter()
            .map(|cid| self.legal_purchase_actions_for_city(pid, cid))
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
                            && self.enemy_air_strike_target_at(pid, target)
                            && self.combat_target_visible_at(
                                pid,
                                target,
                                &current_visibility,
                                &visibility_viewers,
                            )
                        {
                            acts.push(Action::AirStrike { unit: uid, target });
                        }
                        if target != origin
                            && spec.promotion_class == "air_bomber"
                            && u.attacks_left > 0
                            && (u.hp >= 50
                                || self.promotion_effect(&u, "air_pillage_at_low_health") > 0.0)
                            && self.sees(&current_visibility, target)
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
                                &current_visibility,
                                &visibility_viewers,
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
                if spec.class == "military" && !embarked {
                    if spec.has_ranged_attack()
                        && u.attacks_left > 0
                        && (!spec.siege
                            || !u.moved
                            || self.promotion_effect(&u, "attack_after_move") > 0.0)
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
                                    &current_visibility,
                                    &visibility_viewers,
                                )
                                && self.unit_has_line_of_sight(uid, pos)
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
                                    &current_visibility,
                                    &visibility_viewers,
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
                            if self.map.tiles.contains_key(&pos)
                                && self.enemy_combat_target_at(pid, pos)
                                && self.unit_can_melee_target_domain(uid, pos)
                                && self.can_pay_melee_entry(uid, pos)
                            {
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
                    if rival {
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
                && p.religion_beliefs.len() < 4
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
                            building: Name::new(building),
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
                                district: Name::new(district),
                                pos: *pos,
                                currency: "faith".to_string(),
                            });
                        }
                        if gold_districts
                            && priced("gold").is_some_and(|cost| p.gold + f64::EPSILON >= cost)
                        {
                            acts.push(Action::BuyDistrict {
                                city: cid,
                                district: Name::new(district),
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
                                unit: Name::new(unit),
                                formation,
                                currency: currency.to_string(),
                            });
                        }
                    }
                }
            }
        }
        if want_core {
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
                            && self.encampment_at(pos).is_none()
                            && self.combat_target_visible_at(
                                pid,
                                pos,
                                &current_visibility,
                                &visibility_viewers,
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
                if self.encampment_can_strike(city) {
                    let Some(source) =
                        self.city_district_family_position(city, crate::name!("encampment"))
                    else {
                        continue;
                    };
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
                            && self.encampment_at(pos).is_none()
                            && self.combat_target_visible_at(
                                pid,
                                pos,
                                &current_visibility,
                                &visibility_viewers,
                            )
                            && self.has_line_of_sight(source, pos, true)
                        {
                            acts.push(Action::EncampmentStrike {
                                city: cid,
                                target: pos,
                            });
                        }
                    }
                }
            }
        }
        if families.has(ActionFamilies::EMPIRE) && !p.is_minor {
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
            // Nuclear strikes are enumerated against enemy city centers in
            // ICBM range; arbitrary revealed tiles remain legal through apply.
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
                // Hoisted once rather than rebuilt per candidate target: an
                // SSBN's reach does not depend on which city the order names,
                // and rescanning every unit inside a doubly-nested city loop
                // would put the whole roster in a hot enumeration path.
                let submarines: Vec<Pos> = self
                    .units
                    .values()
                    .filter(|unit| unit.owner == pid && unit.kind == "nuclear_submarine")
                    .map(|unit| unit.pos)
                    .collect();
                let launchers: Vec<(u32, Vec<Pos>)> = self
                    .cities
                    .values()
                    .filter(|city| city.owner == pid)
                    .map(|city| {
                        let mut platforms = vec![city.pos];
                        platforms.extend(city.owned_tiles.iter().copied().filter(|position| {
                            self.map.tiles.get(position).is_some_and(|tile| {
                                tile.improvement.as_deref() == Some("missile_silo")
                                    && !tile.pillaged
                            })
                        }));
                        (city.id, platforms)
                    })
                    .collect();
                for enemy in self.cities.values() {
                    if !self.is_at_war(pid, enemy.owner) || !p.explored.contains(&enemy.pos) {
                        continue;
                    }
                    let mut offered = false;
                    for (launch, platforms) in &launchers {
                        if platforms
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
                    // A boat in range makes the target reachable from anywhere,
                    // so offer the shot once rather than once per city.
                    if !offered
                        && submarines
                            .iter()
                            .any(|position| self.wdist(*position, enemy.pos) <= range)
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
                            building: Name::new(building),
                            currency: "faith".to_string(),
                        });
                    }
                }
            }
        }
        if families.has(ActionFamilies::DEALS) && !p.is_minor && !p.is_barbarian {
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
                        if let Some(proposal) =
                            self.emergency_proposal_for_resolution(&resolution.id)
                        {
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
        if families.has(ActionFamilies::DIPLOMACY) && !p.is_barbarian {
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
                            && !self.are_friends(pid, o.id)
                            && !self.are_allied(pid, o.id)
                        {
                            acts.push(Action::DeclareWar { player: o.id });
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
                            for kind in
                                ["research", "cultural", "economic", "military", "religious"]
                            {
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
                        && !self.are_friends(pid, o.id)
                        && !self.are_allied(pid, o.id)
                        && self.peace_treaty_until(pid, o.id).is_none()
                    {
                        acts.push(Action::DeclareWar { player: o.id });
                    }
                }
            }
        }
        acts.push(Action::EndTurn);
        if families.has(ActionFamilies::PURCHASES) || families.has(ActionFamilies::EMPIRE) {
            acts.retain(|action| !self.purchase_action_is_blocked(action));
        }
        acts
    }

    /// Whether a unit belonging to a player we are NOT at war with stands on
    /// `pos`. Barbarians and Free Cities read as at war through
    /// [`Game::is_at_war`], so only a real peace — or a teammate — vetoes.
    ///
    /// ★★★★★ A STRIKE IS ADDRESSED TO A PLOT, NOT TO A UNIT, AND THE HOST
    /// PICKS THE DEFENDER.
    ///
    /// `civvis_orders` emits `Attack` and `RANGE_ATTACK` as coordinates, so
    /// every plot-addressed strike is a bet that the defender Civ 6 chooses
    /// is the one we aimed at. The gates below were all existential
    /// (`.any()`): ONE at-war military unit standing there made the whole
    /// plot legal, and a unit we are at peace with beside it never got a
    /// vote. When the host then resolves the blow against THAT unit, the
    /// engine starts a war nobody declared and books 150 grievances.
    ///
    /// Measured 2026-08-29 over 95 live runs: of 3,832 deduped
    /// plot-addressed strikes, 24 were aimed at a plot whose own state frame
    /// showed a foreign unit we were at peace with — 19 a major's, 5 a
    /// minor's, and every one of them a civilian (TRADER, MISSIONARY,
    /// APOSTLE, BUILDER) stacked with or replacing a barbarian.
    /// `civvis-20260827T183146Z` t53 shot (56,35), a Brazilian MISSIONARY on
    /// a barbarian WARRIOR; `civvis-20260827T191140Z` t57 shot (56,30), a
    /// Maya TRADER with a barbarian SCOUT; `civvis-20260828T142735Z` t58
    /// shot (70,23), a Mongolian TRADER with a barbarian WARRIOR.
    ///
    /// There is no way to aim a plot order past the bystander, so the cure
    /// is to not send it. This is the veto every plot-addressed strike gate
    /// asks first.
    pub(crate) fn peaceful_foreign_unit_at(&self, pid: usize, pos: Pos) -> bool {
        self.unit_ids_at(pos).iter().any(|oid| {
            let unit = &self.units[oid];
            unit.owner != pid && !self.is_at_war(pid, unit.owner)
        })
    }

    pub(super) fn enemy_combat_target_at(&self, pid: usize, pos: Pos) -> bool {
        // The veto covers the unit scan only. A city or an Encampment on the
        // plot is always its own defender — an attack on one cannot be
        // resolved against a bystander standing in it — so the two branches
        // below stay reachable with a peaceful unit on the tile.
        if !self.peaceful_foreign_unit_at(pid, pos) {
            for oid in self.unit_ids_at(pos) {
                let unit = &self.units[oid];
                if unit.owner != pid
                    && self.is_at_war(pid, unit.owner)
                    && self.rules.units[unit.kind].class == "military"
                    && self.rules.units[unit.kind].domain.as_deref() != Some("air")
                {
                    return true;
                }
            }
        }
        if let Some(cid) = self.city_at(pos) {
            let owner = self.cities[&cid].owner;
            return owner != pid && self.is_at_war(pid, owner);
        }
        if let Some(cid) = self.encampment_at(pos) {
            let owner = self.cities[&cid].owner;
            return owner != pid && self.is_at_war(pid, owner);
        }
        false
    }

    pub(super) fn unit_has_priority_target(&self, unit: &Unit) -> bool {
        matches!(
            unit.kind.as_str(),
            "spec_ops" | "jet_fighter" | "jet_bomber"
        )
    }

    pub(crate) fn priority_support_target_at(&self, pid: usize, pos: Pos) -> Option<u32> {
        let mut supports: Vec<u32> = self
            .unit_ids_at(pos)
            .iter()
            .filter(|unit| {
                let unit = &self.units[unit];
                unit.owner != pid
                    && self.is_at_war(pid, unit.owner)
                    && self.rules.units[unit.kind].class == "support"
            })
            .copied()
            .collect();
        supports.sort_unstable();
        supports.into_iter().find(|support| {
            let owner = self.units[support].owner;
            self.unit_ids_at(pos).iter().any(|escort| {
                let escort = &self.units[escort];
                escort.owner == owner
                    && self.rules.units[escort.kind].class == "military"
                    && self.rules.units[escort.kind].domain.as_deref() != Some("air")
            })
        })
    }

    pub(super) fn enemy_air_strike_target_at(&self, pid: usize, pos: Pos) -> bool {
        if self.enemy_combat_target_at(pid, pos) {
            return true;
        }
        // Same plot, same veto: `enemy_combat_target_at` has already refused
        // the ground scan, and a support unit or a fighter's patrol station
        // does not make the bystander underneath it any safer to hit.
        if self.peaceful_foreign_unit_at(pid, pos) {
            return false;
        }
        self.units.values().any(|unit| {
            // Standing over the tile is the necessary condition, and it is two
            // field reads; deciding a war is not. Ask it first, of a scan that
            // covers every unit in the world.
            (unit.pos == pos || (unit.air_patrol && unit.air_patrol_pos == Some(pos)))
                && unit.owner != pid
                && self.is_at_war(pid, unit.owner)
                && ((unit.air_patrol
                    && unit.air_patrol_pos == Some(pos)
                    && self.rules.units[unit.kind].promotion_class == "air_fighter")
                    || (unit.pos == pos && self.rules.units[unit.kind].class == "support"))
        })
    }

    pub(super) fn enemy_ranged_target_at(&self, pid: usize, pos: Pos) -> bool {
        self.enemy_combat_target_at(pid, pos)
            // An enemy fighter's patrol station is not a reason to shell the
            // plot it covers when someone we are at peace with is standing
            // on it: the host resolves the shot against the ground unit.
            || (!self.peaceful_foreign_unit_at(pid, pos)
                && self.units.values().any(|unit| {
                    unit.air_patrol
                        && unit.air_patrol_pos == Some(pos)
                        && unit.owner != pid
                        && self.is_at_war(pid, unit.owner)
                        && self.rules.units[unit.kind].promotion_class == "air_fighter"
                }))
    }

    /// Whether the engine will accept `Ranged { unit: uid, target }` right
    /// now. These are the predicates `legal_actions_within` applies before it
    /// enumerates a Ranged order and `do_ranged` re-applies before executing
    /// one — asked here in `do_ranged`'s own order, not re-derived, so the
    /// answer cannot drift from the refusal.
    ///
    /// A controller that proposes ranged candidates on distance alone is
    /// proposing orders the engine will refuse: a census of one 150-turn
    /// six-player deployment game (seed 7700000, 2026-08-18) counted 503
    /// authoritative Ranged refusals, every one from `BasicAi::military_step`
    /// — 281 "target is not visible", 195 "line of sight blocked", 26
    /// "nothing to attack" — and each refused winner also shadowed whatever
    /// legal shot came second. `AdvancedAi`'s tactical picker asks the same
    /// predicates inline for the same reason.
    ///
    /// ⚠ The caller hoists `player_vision_now` and `visibility_viewers` once
    /// per unit and passes them in. The convenience path rebuilds a whole
    /// `TileBits` per call and measured **+6.43%** when a picker paid it per
    /// candidate tile (see `combat_target_visible`'s note).
    pub(crate) fn ranged_order_is_legal(
        &self,
        pid: usize,
        uid: u32,
        target: Pos,
        visible: &TileBits,
        viewers: &BTreeSet<usize>,
    ) -> bool {
        let Some(unit) = self.units.get(&uid) else {
            return false;
        };
        let spec = &self.rules.units[unit.kind];
        spec.has_ranged_attack()
            && !self.is_embarked(unit)
            && (!spec.siege
                || !unit.moved
                || self.promotion_effect(unit, "attack_after_move") > 0.0)
            && unit.moves_left > 0.0
            && unit.attacks_left > 0
            && self.wdist(unit.pos, target) <= self.unit_attack_range(uid)
            && self.enemy_ranged_target_at(pid, target)
            && self.combat_target_visible_at(pid, target, visible, viewers)
            && self.unit_has_line_of_sight(uid, target)
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
            && !self.is_embarked(unit)
            && unit.moves_left > 0.0
            && unit.attacks_left > 0
            && self.wdist(unit.pos, target) == 1
            && self.enemy_combat_target_at(pid, target)
            && self.unit_can_melee_target_domain(uid, target)
            && self.can_pay_melee_entry(uid, target)
    }

    /// The two strengths `do_attack` resolves a melee blow with, `(attacker,
    /// defender)`, each already through [`effective_strength`]. `None` when
    /// either unit is gone.
    ///
    /// ★★★★ THIS IS THE ENGINE'S OWN ARITHMETIC, EXPOSED — NOT A COPY OF IT.
    /// `do_attack` calls it, so a controller that prices a melee exchange
    /// *before* it happens cannot drift from the exchange the engine will
    /// actually resolve. That matters here more than for an attack scan,
    /// because the terms this function carries and a strength-only estimate
    /// does not — matchup, flanking, the amphibious and river penalties,
    /// terrain, adjacent support, and fortification — are exactly the ones
    /// that decide whether standing still beats swinging. See
    /// `ranged_order_is_legal` for the same argument about the predicates.
    ///
    /// Feed both halves to [`expected_damage`] to get the two blows of one
    /// melee round at the engine's unrandomized centre: the defender takes
    /// `expected_damage(att, def)` and the attacker takes
    /// `expected_damage(def, att)`.
    pub(crate) fn melee_exchange_strengths(&self, uid: u32, did: u32) -> Option<(f64, f64)> {
        let attacker = self.units.get(&uid)?;
        let defender = self.units.get(&did)?;
        let target = defender.pos;
        let unamphibious = self.promotion_effect(attacker, "amphibious") == 0.0;
        let mut att_base = self.unit_unembarked_strength(attacker)
            + self.matchup_bonus(uid, defender, true)
            + self.flanking_bonus(uid, target)
            + self.vs_bonus(attacker.owner, defender.owner);
        if self.is_embarked(attacker) && unamphibious {
            att_base -= 10.0;
        }
        let mut def_base = self.unit_strength(defender, true)
            + self.matchup_bonus(did, attacker, false)
            + self.tile_defense_bonus(target)
            + self.support_bonus(defender)
            + self.vs_bonus(defender.owner, attacker.owner);
        if self.crosses_river(attacker.pos, target) && unamphibious {
            def_base += 5.0;
        }
        Some((
            effective_strength(att_base, attacker.hp),
            effective_strength(def_base, defender.hp),
        ))
    }

    /// The two strengths `do_ranged` resolves a shot with, `(shooter,
    /// defender)`, each already through [`effective_strength`]. `target` is
    /// the tile fired at, which is the defender's own tile except for an air
    /// unit answered on patrol. `None` when either unit is gone.
    ///
    /// The companion of [`melee_exchange_strengths`], and the asymmetry
    /// between them is the whole point: a shot returns *nothing*. There is no
    /// second blow in `do_ranged` for a controller to price, which is why
    /// standing under one is a straight loss and standing under a melee
    /// attack need not be.
    pub(crate) fn ranged_strike_strengths(
        &self,
        uid: u32,
        did: u32,
        target: Pos,
    ) -> Option<(f64, f64)> {
        let shooter = self.units.get(&uid)?;
        let defender = self.units.get(&did)?;
        let spec = &self.rules.units[shooter.kind];
        let defender_spec = &self.rules.units[defender.kind];
        let defender_is_sea = defender_spec.domain.as_deref() == Some("sea");
        let mut att_base = self.unit_ranged_attack_strength(shooter)
            + self.matchup_bonus(uid, defender, true)
            + if defender_is_sea {
                self.promotion_effect(shooter, "ranged_vs_units")
                    + self.promotion_effect(shooter, "ranged_vs_naval")
                    + self.promotion_effect(shooter, "siege_vs_naval")
            } else {
                self.promotion_effect(shooter, "ranged_vs_land")
                    + self.promotion_effect(shooter, "ranged_vs_units")
                    + self.promotion_effect(shooter, "siege_vs_land")
            }
            + self.vs_bonus(shooter.owner, defender.owner);
        if (spec.bombard_strength > 0.0 && !defender_is_sea)
            || (spec.ranged_strength > 0.0
                && spec.domain.as_deref() != Some("sea")
                && defender_is_sea)
        {
            att_base -= 17.0;
        }
        let def_base = self.unit_strength(defender, true)
            + self.ranged_defense_bonus(defender, false)
            + if defender_spec.domain.as_deref() == Some("air") {
                0.0
            } else {
                self.tile_defense_bonus(target)
            }
            + self.vs_bonus(defender.owner, shooter.owner);
        Some((
            effective_strength(att_base, shooter.hp),
            effective_strength(def_base, defender.hp),
        ))
    }

    /// A ranged target must be in current shared vision, and a stealthed unit
    /// on that tile must be detected by at least one of the direct viewers.
    /// Range-three indirect fire ignores terrain along the shooter's ray, but
    /// still requires a friendly spotter exactly as in Civ VI.
    /// Whether `pid` may legally fire at `pos` at all, against visibility
    /// frames the caller already holds. `do_ranged`, `do_attack` and
    /// `do_city_strike` each apply exactly this before a shot, so a controller
    /// that proposes a target without asking is proposing an order the engine
    /// will refuse. Hoist `player_vision_frame` and `visibility_viewers` once per
    /// unit and pass them in; the frames cannot move while no action is applied.
    pub(crate) fn combat_target_visible_at(
        &self,
        pid: usize,
        pos: Pos,
        visible: &TileBits,
        viewers: &BTreeSet<usize>,
    ) -> bool {
        if !self.sees(visible, pos) {
            return false;
        }
        let hostile_city = self
            .city_at(pos)
            .or_else(|| self.encampment_at(pos))
            .is_some_and(|city| {
                let owner = self.cities[&city].owner;
                owner != pid && self.is_at_war(pid, owner)
            });
        hostile_city
            || self.units.values().any(|unit| {
                let observed_pos = unit.air_patrol_pos.unwrap_or(unit.pos);
                observed_pos == pos
                    && unit.owner != pid
                    && self.is_at_war(pid, unit.owner)
                    && viewers
                        .iter()
                        .any(|viewer| self.unit_visible_to(unit.id, *viewer))
            })
    }

    pub(super) fn unit_currently_visible_to(&self, uid: u32, pid: usize) -> bool {
        let Some(unit) = self.units.get(&uid) else {
            return false;
        };
        let pos = unit.air_patrol_pos.unwrap_or(unit.pos);
        self.player_can_see(pid, pos)
            && self
                .visibility_viewers(pid)
                .iter()
                .any(|viewer| self.unit_visible_to(uid, *viewer))
    }

    pub fn apply(&mut self, pid: usize, action: &Action) -> Result<(), String> {
        if self.is_finished() {
            return Err("game over".into());
        }
        if self.current != pid {
            return Err("not your turn".into());
        }
        let capture_actions = self.pending_city_capture_actions(pid);
        if !capture_actions.is_empty() && !capture_actions.contains(action) {
            return Err("resolve the captured city's fate first".into());
        }
        // The empire game is off on a battlefield, for every seat alike —
        // refused here as well as left out of `legal_actions`, so an AI that
        // picks its own research or sues for its own peace gets the same
        // answer a player would. See `Action::off_the_battlefield`.
        if self.is_arena() && action.off_the_battlefield() {
            return Err("not on a battlefield: an arena is decided by the fighting".into());
        }
        // An ordinary unit step cannot alter connected resources, ownership,
        // resource-reveal prerequisites, or Suzerain status. A tribal village
        // is the exception: its reward may complete a technology and reveal a
        // luxury. Epic Quest makes a cleared Barbarian Outpost grant the same
        // reward, so remember either site before `do_move` removes it.
        let monopoly_control_may_change = match action {
            Action::Move { to, .. } => {
                self.barb_camps.contains_key(to)
                    || self.map.get(*to).is_some_and(|tile| {
                        matches!(
                            tile.improvement.as_deref(),
                            Some("goody_hut" | "meteor_goody" | "barbarian_camp")
                        )
                    })
            }
            _ => true,
        };
        // The first-monopoly detector still runs after actions that can alter
        // research visibility, but its expensive census only needs rebuilding
        // when the connected/world holdings or suzerainty can change. In a
        // long game most successful actions are ordinary unit orders.
        let monopoly_context_may_change = !matches!(
            action,
            Action::Move { .. }
                | Action::MoveTo { .. }
                | Action::Ranged { .. }
                | Action::AirRebase { .. }
                | Action::AirStrike { .. }
                | Action::AirPatrol { .. }
                | Action::ContributeProject { .. }
                | Action::PerformConcert { .. }
                | Action::UpgradeUnit { .. }
                | Action::Fortify { .. }
                | Action::Promote { .. }
                | Action::Upgrade { .. }
                | Action::CombineUnits { .. }
                | Action::LinkUnits { .. }
                | Action::UnlinkUnits { .. }
                | Action::TradeRoute { .. }
                | Action::BuildRailroad { .. }
                | Action::Spread { .. }
                | Action::TheologicalAttack { .. }
                | Action::CondemnHeretic { .. }
                | Action::HealReligious { .. }
                | Action::RemoveHeresy { .. }
                | Action::LaunchInquisition { .. }
                | Action::EvangelizeBelief { .. }
                | Action::ConvertBarbarians { .. }
                | Action::CityStrike { .. }
                | Action::EncampmentStrike { .. }
        );
        if monopoly_context_may_change {
            self.query_memo.monopoly_context.borrow_mut().take();
        }
        let blocked_unit = match action {
            Action::Move { unit, .. }
            | Action::MoveTo { unit, .. }
            | Action::Attack { unit, .. }
            | Action::Ranged { unit, .. }
            | Action::FoundCity { unit }
            | Action::Improve { unit, .. }
            | Action::ContributeProject { unit, .. }
            | Action::ContributeDistrict { unit, .. }
            | Action::PerformConcert { unit }
            | Action::Pillage { unit }
            | Action::RepairImprovement { unit }
            | Action::CoastalRaid { unit, .. }
            | Action::AirRebase { unit, .. }
            | Action::AirStrike { unit, .. }
            | Action::AirPillage { unit, .. }
            | Action::PriorityTarget { unit, .. }
            | Action::AirPatrol { unit, .. }
            | Action::Fortify { unit }
            | Action::UpgradeUnit { unit }
            | Action::Promote { unit, .. }
            | Action::Upgrade { unit, .. }
            | Action::UnlinkUnits { unit }
            | Action::TradeRoute { unit, .. }
            | Action::Spread { unit }
            | Action::TheologicalAttack { unit, .. }
            | Action::CondemnHeretic { unit, .. }
            | Action::HealReligious { unit }
            | Action::RemoveHeresy { unit }
            | Action::LaunchInquisition { unit }
            | Action::EvangelizeBelief { unit, .. }
            | Action::ConvertBarbarians { unit } => self.noncombat_action_blocked_by_zoc(*unit),
            Action::CombineUnits { unit, with } | Action::LinkUnits { unit, with } => {
                self.noncombat_action_blocked_by_zoc(*unit)
                    || self.noncombat_action_blocked_by_zoc(*with)
            }
            _ => false,
        };
        if blocked_unit {
            return Err("non-combat unit cannot act after entering zone of control".into());
        }
        let r = match action {
            Action::Move { unit, to } => self.do_move(pid, *unit, *to),
            Action::MoveTo { unit, to } => self.do_move_to(pid, *unit, *to),
            Action::Swap { unit, other } => self.do_swap(pid, *unit, *other),
            Action::Attack { unit, target } => self
                .do_attack(pid, *unit, *target)
                .inspect(|()| self.accrue_combat_weariness(pid, *target)),
            Action::Ranged { unit, target } => self
                .do_ranged(pid, *unit, *target)
                .inspect(|()| self.accrue_combat_weariness(pid, *target)),
            Action::FoundCity { unit } => self.do_found_city(pid, *unit),
            Action::Improve { unit, improvement } => self.do_improve(pid, *unit, improvement),
            Action::FoundCorporation { pos } => self.do_found_corporation(pid, *pos),
            Action::MoveProduct { from, to, product } => {
                self.do_move_product(pid, *from, *to, product)
            }
            Action::ContributeProject { unit, city } => {
                self.do_contribute_project(pid, *unit, *city)
            }
            Action::ContributeDistrict { unit, city } => {
                self.do_contribute_district(pid, *unit, *city)
            }
            Action::PerformConcert { unit } => self.do_perform_concert(pid, *unit),
            Action::Pillage { unit } => self.do_pillage(pid, *unit),
            Action::RepairImprovement { unit } => self.do_repair_improvement(pid, *unit),
            Action::CoastalRaid { unit, target } => self.do_coastal_raid(pid, *unit, *target),
            Action::AirRebase { unit, to } => self.do_air_rebase(pid, *unit, *to),
            Action::AirStrike { unit, target } => self
                .do_air_strike(pid, *unit, *target)
                .inspect(|()| self.accrue_combat_weariness(pid, *target)),
            Action::AirPillage { unit, target } => self.do_air_pillage(pid, *unit, *target),
            Action::PriorityTarget { unit, target } => self.do_priority_target(pid, *unit, *target),
            Action::AirPatrol { unit, to } => self.do_air_patrol(pid, *unit, *to),
            Action::Produce { city, item } => self.do_produce(pid, *city, item),
            Action::Buy {
                city,
                unit,
                formation,
                currency,
            } => self.do_buy_formation(pid, *city, unit, *formation, currency),
            Action::BuyBuilding {
                city,
                building,
                currency,
            } => self.do_buy_building(pid, *city, building, currency),
            Action::BuyDistrict {
                city,
                district,
                pos,
                currency,
            } => self.do_buy_district(pid, *city, district, *pos, currency),
            Action::BuyPlot { city, pos, .. } => self.do_buy_plot(pid, *city, *pos),
            Action::Research { tech } => self.do_research(pid, tech),
            Action::Civic { civic } => self.do_civic(pid, civic),
            Action::DeclareWar { player } => self.do_declare_war(pid, *player),
            Action::DeclareWarWithCasusBelli {
                player,
                casus_belli,
            } => self.do_declare_war_with_casus_belli(pid, *player, casus_belli),
            Action::MakePeace { player } => self.do_make_peace(pid, *player),
            Action::Denounce { player } => self.do_denounce(pid, *player),
            Action::SendDelegation { player } => self.do_send_delegation(pid, *player),
            Action::SendEmbassy { player } => self.do_send_embassy(pid, *player),
            Action::ProposeDefensivePact { player } => self.do_propose_defensive_pact(pid, *player),
            Action::ProposeJointWar { player, target } => {
                self.do_propose_joint_war(pid, *player, *target)
            }
            Action::RequestPromise { player, promise } => {
                self.do_request_promise(pid, *player, promise)
            }
            Action::DemandGold { player, gold } => self.do_demand_gold(pid, *player, *gold),
            Action::ProposeDeal {
                player,
                give_gold,
                request_gold,
                open_borders,
                friendship,
                peace,
                alliance,
            } => self.do_propose_deal(
                pid,
                *player,
                *give_gold,
                *request_gold,
                *open_borders,
                *friendship,
                *peace,
                alliance.as_deref(),
            ),
            Action::AcceptDeal { deal } => self.do_accept_deal(pid, *deal),
            Action::RejectDeal { deal } => self.do_reject_deal(pid, *deal),
            Action::Trade {
                player,
                offer,
                request,
            } => self.do_trade(pid, *player, offer, request),
            Action::CongressVote {
                resolution,
                choice,
                votes,
            } => self.do_congress_vote(pid, resolution, choice, *votes),
            Action::AssignSpy { spy, city } => self.do_assign_spy(pid, *spy, *city),
            Action::SpyMission {
                spy,
                mission,
                target,
            } => self.do_spy_mission(pid, *spy, mission, *target),
            Action::PromoteSpy { spy, promotion } => self.do_promote_spy(pid, *spy, promotion),
            Action::ChooseDedication { dedication } => self.do_choose_dedication(pid, dedication),
            Action::Fortify { unit } => self.do_fortify(pid, *unit),
            Action::UpgradeUnit { unit } => self.do_upgrade_unit(pid, *unit),
            Action::Promote { unit, promotion } => self.do_promote(pid, *unit, promotion),
            Action::Upgrade { unit, to } => self.do_upgrade(pid, *unit, to),
            Action::CombineUnits { unit, with } => self.do_combine_units(pid, *unit, *with),
            Action::LinkUnits { unit, with } => self.do_link_units(pid, *unit, *with),
            Action::UnlinkUnits { unit } => self.do_unlink_units(pid, *unit),
            Action::Government { government } => self.do_government(pid, government),
            Action::SlotPolicy { policy } => self.do_slot_policy(pid, policy),
            Action::UnslotPolicy { policy } => self.do_unslot_policy(pid, policy),
            Action::TradeRoute { unit, city } => self.do_trade_route(pid, *unit, *city),
            Action::SendEnvoy { player } => self.do_send_envoy(pid, *player),
            Action::LevyMilitary { player } => self.do_levy_military(pid, *player),
            Action::RecruitGreatPerson { kind } => self.do_recruit_great_person(pid, kind),
            Action::PatronizeGreatPerson { kind, currency } => {
                self.do_patronize_great_person(pid, kind, currency)
            }
            Action::ChoosePantheon { belief } => self.do_choose_pantheon(pid, belief),
            Action::ChooseSecretSociety { society } => self.do_choose_secret_society(pid, society),
            Action::AssignGovernor { city } => self.do_assign_governor(pid, *city),
            Action::AppointGovernor { governor, city } => {
                self.do_appoint_governor(pid, governor, *city)
            }
            Action::ReassignGovernor { governor, city } => {
                self.do_reassign_governor(pid, governor, *city)
            }
            Action::PromoteGovernor {
                governor,
                promotion,
            } => self.do_promote_governor(pid, governor, promotion),
            Action::FoundReligion { follower, founder } => {
                self.do_found_religion(pid, follower, founder)
            }
            Action::Spread { unit } => self.do_spread(pid, *unit),
            Action::TheologicalAttack { unit, target } => {
                self.do_theological_attack(pid, *unit, *target)
            }
            Action::CondemnHeretic { unit, target_unit } => {
                self.do_condemn_heretic(pid, *unit, *target_unit)
            }
            Action::HealReligious { unit } => self.do_heal_religious(pid, *unit),
            Action::RemoveHeresy { unit } => self.do_remove_heresy(pid, *unit),
            Action::LaunchInquisition { unit } => self.do_launch_inquisition(pid, *unit),
            Action::EvangelizeBelief { unit, belief } => {
                self.do_evangelize_belief(pid, *unit, belief)
            }
            Action::ConvertBarbarians { unit } => self.do_convert_barbarians(pid, *unit),
            Action::CityStrike { city, target } => self
                .do_city_strike(pid, *city, *target)
                .inspect(|()| self.accrue_combat_weariness(pid, *target)),
            Action::WmdStrike {
                city,
                target,
                thermonuclear,
            } => self
                .do_wmd_strike(pid, *city, *target, *thermonuclear)
                // WAR_WEARINESS_PER_WMD_LAUNCHED: launching is the single
                // most expensive thing you can do to your own people.
                .inspect(|()| self.add_war_weariness(pid, 10.0)),
            Action::AimMassDriver { site, ore } => self.do_aim_mass_driver(pid, *site, *ore),
            Action::MassDriverStrike { target } => self.do_mass_driver_strike(pid, *target),
            Action::BuildRailroad { unit } => self.do_build_railroad(pid, *unit),
            Action::EncampmentStrike { city, target } => {
                self.do_encampment_strike(pid, *city, *target)
            }
            Action::KeepCity { city } => self.do_keep_city(pid, *city),
            Action::RazeCity { city } => self.do_raze_city(pid, *city),
            Action::LiberateCity { city } => self.do_liberate_city(pid, *city),
            Action::EndTurn => {
                self.do_end_turn();
                Ok(())
            }
        };
        if r.is_ok() {
            // `producible_items` is intentionally retained across short
            // read-only decision helpers, rather than only one `QueryMemo`.
            // Once an action succeeds any one of its prerequisites may have
            // changed, so the next helper must derive a fresh catalog.
            self.query_memo.producible.borrow_mut().clear();
            // A planning world's EndTurn keeps only what planning reads.
            // On a seat's private copy the close is the last thing the world
            // ever does: the plan harvest reads the actions logged before it,
            // so the monopoly census and the every-seat visibility sweep
            // below would be for nobody. On the rolling prepare world the
            // walk is an upkeep-only forward: with fog memory off the sweep's
            // one gameplay product is contact recording, and the authoritative
            // commit runs the very same sweep on the real world; a contact
            // that would first arise from mid-walk upkeep reaches the next
            // cycle's planning worlds one cycle later instead. Mid-turn
            // actions keep everything — the planning agent still reads its
            // own world while deliberating.
            let discarded_close =
                matches!(action, Action::EndTurn) && self.planning_role != PlanningRole::Off;
            if monopoly_control_may_change && !discarded_close {
                self.note_first_monopoly_moments();
            }
            // The war infobox is live during a turn, not only after End Turn.
            // Refresh after actions that can damage, create, transfer, upgrade,
            // or otherwise change the unit-only military total.
            if matches!(
                action,
                Action::Attack { .. }
                    | Action::Ranged { .. }
                    | Action::Pillage { .. }
                    | Action::CoastalRaid { .. }
                    | Action::AirStrike { .. }
                    | Action::AirPillage { .. }
                    | Action::PriorityTarget { .. }
                    | Action::Buy { .. }
                    | Action::BuyBuilding { .. }
                    | Action::BuyDistrict { .. }
                    | Action::UpgradeUnit { .. }
                    | Action::Upgrade { .. }
                    | Action::CombineUnits { .. }
                    | Action::Government { .. }
                    | Action::SlotPolicy { .. }
                    | Action::UnslotPolicy { .. }
                    | Action::SendEnvoy { .. }
                    | Action::LevyMilitary { .. }
                    | Action::AssignGovernor { .. }
                    | Action::AppointGovernor { .. }
                    | Action::ReassignGovernor { .. }
                    | Action::PromoteGovernor { .. }
                    | Action::ConvertBarbarians { .. }
                    | Action::CityStrike { .. }
                    | Action::WmdStrike { .. }
                    | Action::MassDriverStrike { .. }
                    | Action::EncampmentStrike { .. }
                    | Action::KeepCity { .. }
                    | Action::RazeCity { .. }
                    | Action::LiberateCity { .. }
            ) {
                // Mid-turn freshness for the spectator's war infobox only;
                // headless rollouts switch it off and rely on the
                // unconditional syncs at declarations, peaces, and turn
                // boundaries.
                if self.track_war_ledger {
                    self.sync_war_log();
                }
            }
            if !discarded_close {
                self.defer_or_refresh_visibility(pid, matches!(action, Action::EndTurn));
            }
            self.log.push(pid, action.clone());
        }
        r
    }

    pub(super) fn own_unit(&self, pid: usize, uid: u32) -> Result<Unit, String> {
        match self.units.get(&uid) {
            Some(u) if u.owner == pid => Ok(u.clone()),
            _ => Err("not your unit".into()),
        }
    }

    pub(super) fn airlift_destinations(&self, pid: usize, uid: u32) -> Vec<Pos> {
        let Some(unit) = self.units.get(&uid) else {
            return vec![];
        };
        let spec = &self.rules.units[unit.kind];
        let Some(origin_tile) = self.map.get(unit.pos) else {
            return vec![];
        };
        let Some(origin) = origin_tile
            .owner_city
            .and_then(|city| self.cities.get(&city))
        else {
            return vec![];
        };
        let Some(origin_district) = origin_tile.district else {
            return vec![];
        };
        if unit.owner != pid
            || unit.moves_left <= 0.0
            || unit.linked_to.is_some()
            || spec
                .domain
                .as_deref()
                .is_some_and(|domain| domain != "land")
            || self.tree_effect(pid, "airport_transfer") <= 0.0
            || !self.district_is_family(Name::new(&origin_district), crate::name!("aerodrome"))
            || !self.district_is_active(origin, origin_district, unit.pos)
            || self.city_building_effect(origin, "airlift") <= 0.0
        {
            return vec![];
        }
        self.cities
            .values()
            .filter_map(|city| {
                if city.owner != pid
                    || city.id == origin.id
                    || self.city_building_effect(city, "airlift") <= 0.0
                {
                    return None;
                }
                let destination =
                    self.city_active_district_family_position(city, crate::name!("aerodrome"))?;
                (!self.unit_ids_at(destination).iter().any(|other| {
                    let other = &self.units[other];
                    other.owner != pid || self.rules.units[other.kind].class == spec.class
                }))
                .then_some(destination)
            })
            .collect()
    }

    pub(super) fn do_move(&mut self, pid: usize, uid: u32, to: Pos) -> Result<(), String> {
        self.do_move_step(pid, uid, to, true)
    }

    /// One hex of movement. `stopping` says whether the unit is arriving here
    /// or merely crossing on its way further along a path; see
    /// [`Game::can_move_step`]. Everything else about the step — cost, zone of
    /// control, carriers, linked escorts, capture — is identical either way.
    pub(super) fn do_move_step(
        &mut self,
        pid: usize,
        uid: u32,
        to: Pos,
        stopping: bool,
    ) -> Result<(), String> {
        let u = self.own_unit(pid, uid)?;
        if u.moves_left <= 0.0 {
            return Err("no moves left".into());
        }
        if self.formation_movement_locked_by_zoc(uid) {
            return Err("stopped by zone of control".into());
        }
        if self.airlift_destinations(pid, uid).contains(&to) {
            self.relocate(uid, to);
            let unit = self.units.get_mut(&uid).unwrap();
            unit.moves_left = 0.0;
            unit.acted = true;
            unit.moved = true;
            unit.fortified = false;
            unit.fortify_turns = 0;
            return Ok(());
        }
        if !self.can_move_step(uid, to, stopping) {
            return Err("invalid move".into());
        }
        self.resolve_entered_units(uid, to);
        let cost = self.unit_step_cost(uid, u.pos, to);
        let linked = if self.is_linked_leader(uid) {
            u.linked_to
        } else {
            None
        };
        let carrier_aircraft: Vec<u32> = if u.kind == "aircraft_carrier" {
            self.unit_ids_at(u.pos)
                .iter()
                .filter(|other| {
                    **other != uid
                        && self.units[other].owner == pid
                        && self.rules.units[self.units[other].kind].domain.as_deref() == Some("air")
                })
                .copied()
                .collect()
        } else {
            Vec::new()
        };
        {
            let mu = self.units.get_mut(&uid).unwrap();
            mu.fortified = false;
            mu.fortify_turns = 0;
            mu.acted = true;
            mu.moved = true;
        }
        self.relocate(uid, to);
        for aircraft in carrier_aircraft {
            self.relocate(aircraft, to);
        }
        if let Some(peer) = linked {
            self.relocate(peer, to);
            let escort_speed = self.unit_shares_escort_movement(&self.units[&uid]);
            let peer_cost = if escort_speed {
                cost
            } else {
                self.unit_step_cost(peer, u.pos, to)
            };
            let peer_max = self.unit_max_moves(peer);
            let passenger = self.units.get_mut(&peer).unwrap();
            passenger.moves_left = (passenger.moves_left - peer_cost).max(0.0).min(peer_max);
            passenger.acted = true;
            passenger.moved = true;
        }
        let remaining = (self.units[&uid].moves_left - cost)
            .max(0.0)
            .min(self.unit_max_moves(uid));
        self.units.get_mut(&uid).unwrap().moves_left = remaining;
        if self.formation_enters_enemy_zoc(uid, to) {
            // A linked formation stops when either member is affected. Apply
            // class-specific movement loss to both occupants so a passenger
            // cannot unlink and act after being dragged into ZOC.
            self.stop_unit_by_zoc(uid);
            if let Some(peer) = linked {
                self.stop_unit_by_zoc(peer);
            }
        }
        self.maybe_clear_camp(uid);
        self.maybe_goody_hut(uid);
        self.record_move_step(pid, uid, u.pos, to);
        Ok(())
    }

    /// Chain one executed step onto the walked-route ledger. Consecutive steps
    /// of one unit in one turn extend a single trail, so a whole `MoveTo`
    /// reads as one route however many `do_move` calls carried it out.
    pub(super) fn record_move_step(&mut self, pid: usize, uid: u32, from: Pos, to: Pos) {
        // Only a walked, adjacent hop belongs on a route. `do_move`'s airlift
        // branch returns before recording, and this keeps any future
        // teleport-style relocation from drawing as a march across the map.
        if self.wdist(from, to) != 1 {
            return;
        }
        if let Some(last) = self.unit_move_trails.last_mut() {
            if last.unit == uid && last.turn == self.turn && last.path.last() == Some(&from) {
                last.path.push(to);
                return;
            }
        }
        self.unit_move_trails.push(UnitMoveTrail {
            unit: uid,
            owner: pid,
            turn: self.turn,
            path: vec![from, to],
        });
        // A client draws the last few turns of this list; the whole walking
        // history of a long game is not worth carrying in every frame.
        const TRAILS_KEPT: usize = 512;
        if self.unit_move_trails.len() > TRAILS_KEPT {
            let excess = self.unit_move_trails.len() - TRAILS_KEPT;
            self.unit_move_trails.drain(..excess);
        }
    }

    pub(crate) fn tile_defense_bonus(&self, pos: Pos) -> f64 {
        let t = &self.map.tiles[&pos];
        let mut bonus = 0.0;
        if t.hills {
            bonus += 3.0;
        }
        // The shipped `Features.DefenseModifier`, read from the ruleset rather
        // than listed here. The hand-written match this replaces carried the
        // ordinary features and none of the Natural Wonders, so Ha Long Bay's
        // +15 and Gobustan's, Chocolate Hills' and Ubsunur Hollow's rows never
        // reached a defender.
        bonus += t
            .feature
            .as_deref()
            .and_then(|feature| self.rules.features.get(feature))
            .map(|spec| spec.defense)
            .unwrap_or(0.0);
        if t.wonder.as_deref().is_some_and(|wonder| {
            self.rules.wonders[wonder]
                .effects
                .get("defensive_structure")
                .copied()
                .unwrap_or(0.0)
                > 0.0
        }) {
            // Alhambra functions as a Fort for an occupying land unit.
            bonus += 4.0;
        }
        if !t.pillaged {
            bonus += t
                .improvement
                .as_deref()
                .and_then(|improvement| self.rules.improvements.get(improvement))
                .and_then(|improvement| improvement.effects.get("defense"))
                .copied()
                .unwrap_or(0.0);
        }
        bonus
    }

    /// Advanced Power Cells fits the Giant Death Robot with the Particle Beam
    /// Siege Cannon: +30 Ranged Strength against cities and Encampments, and
    /// those attacks are fully effective against walls rather than half.
    pub(super) fn gdr_siege_bonus(&self, unit: &Unit) -> f64 {
        if unit.kind == "giant_death_robot" {
            self.tree_effect(unit.owner, "gdr_ranged_vs_district")
        } else {
            0.0
        }
    }

    pub(super) fn gdr_full_wall_damage(&self, unit: &Unit) -> bool {
        unit.kind == "giant_death_robot"
            && self.tree_effect(unit.owner, "gdr_full_wall_damage") > 0.0
    }

    pub(super) fn matchup_bonus(&self, uid: u32, opponent: &Unit, attacking: bool) -> f64 {
        let u = &self.units[&uid];
        let spec = &self.rules.units[u.kind];
        let other = &self.rules.units[opponent.kind];
        let mut bonus = 0.0;
        if self.active_emergencies.iter().any(|emergency| {
            emergency.ends > self.turn
                && emergency.target == opponent.owner
                && emergency.members.contains(&u.owner)
        }) {
            bonus += 2.0;
        }
        bonus += 3.0
            * (self.diplomatic_visibility(u.owner, opponent.owner)
                - self.diplomatic_visibility(opponent.owner, u.owner))
            .max(0.0);
        if self.players[u.owner].religion.is_some()
            && self.players[opponent.owner].religion.is_some()
            && self.players[u.owner].religion != self.players[opponent.owner].religion
        {
            bonus += self.policy_effect(u.owner, "different_religion_combat");
        }
        // Cyber Warfare: CYBER_WARFARE_INFO_FUTURE_BUFF, +10 against a
        // civilization in the Information Era or later.
        if self.player_era(opponent.owner) >= 7 {
            bonus += self.policy_effect(u.owner, "combat_vs_information_era");
        }
        if spec.promotion_class == "anti_cavalry"
            && (matches!(
                other.promotion_class.as_str(),
                "light_cavalry" | "heavy_cavalry"
            ) || (other.cavalry && other.promotion_class == "ranged"))
            && opponent.kind != "war_cart"
        {
            bonus += 10.0;
        }
        if spec.promotion_class == "melee" && other.promotion_class == "anti_cavalry" {
            bonus += 5.0;
        }
        if u.kind == "nihang" && other.promotion_class == "anti_cavalry" {
            bonus += 5.0;
        }
        if u.kind == "hoplite"
            && self.nbrs(u.pos).into_iter().any(|p| {
                self.unit_ids_at(p).iter().any(|id| {
                    *id != uid
                        && self.units[id].owner == u.owner
                        && self.units[id].kind == "hoplite"
                })
            })
        {
            bonus += 10.0;
        }
        if attacking && self.has_ability(u.owner, "killer_of_cyrus") && opponent.hp < 100 {
            bonus += 5.0;
        }
        if attacking {
            if matches!(other.promotion_class.as_str(), "melee" | "ranged") {
                bonus += self.promotion_effect(u, "attack_vs_melee_ranged");
            }
            if other.promotion_class == "melee" {
                bonus += self.promotion_effect(u, "vs_melee");
            }
            if other.promotion_class == "anti_cavalry" {
                bonus += self.promotion_effect(u, "vs_anti_cavalry");
            }
            if matches!(
                other.promotion_class.as_str(),
                "light_cavalry" | "heavy_cavalry"
            ) {
                bonus += self.promotion_effect(u, "vs_cavalry");
            }
            if matches!(other.promotion_class.as_str(), "ranged" | "siege") {
                bonus += self.promotion_effect(u, "attack_vs_ranged_siege");
            }
            if other.promotion_class == "siege" {
                bonus += self.promotion_effect(u, "vs_siege");
            }
            if other.promotion_class == "heavy_cavalry" {
                bonus += self.promotion_effect(u, "vs_heavy_cavalry");
            }
            if opponent.hp < 100 {
                bonus += self.promotion_effect(u, "vs_damaged");
            }
            if opponent.fortify_turns > 0 {
                bonus += self.promotion_effect(u, "vs_fortified");
            }
            let tile = &self.map.tiles[&opponent.pos];
            if self.city_at(opponent.pos).is_some() || tile.district.is_some() {
                bonus += self.promotion_effect(u, "vs_unit_in_district");
                bonus += self.promotion_effect(u, "district_melee");
            }
            if other.domain.as_deref() == Some("sea") {
                bonus += self.promotion_effect(u, "vs_naval");
            }
            if other.promotion_class == "naval_raider" {
                bonus += self.promotion_effect(u, "vs_naval_raider");
            }
        } else {
            if other.promotion_class == "melee" {
                bonus += self.promotion_effect(u, "defend_melee");
            }
            if matches!(
                other.promotion_class.as_str(),
                "heavy_cavalry" | "anti_cavalry"
            ) {
                bonus += self.promotion_effect(u, "defend_heavy_anti");
            }
            let tile = &self.map.tiles[&u.pos];
            if self.city_at(u.pos).is_some() || tile.district.is_some() {
                bonus += self.promotion_effect(u, "district_melee");
            }
        }
        if matches!(
            other.promotion_class.as_str(),
            "light_cavalry" | "heavy_cavalry"
        ) {
            bonus += self
                .nbrs(u.pos)
                .into_iter()
                .flat_map(|pos| self.unit_ids_at(pos))
                .filter(|id| {
                    let ally = &self.units[id];
                    ally.owner == u.owner
                        && self.rules.units[ally.kind].promotion_class != spec.promotion_class
                })
                .map(|id| self.promotion_effect(&self.units[id], "adjacent_vs_cavalry"))
                .sum::<f64>();
        }
        bonus
    }

    pub(super) fn eagle_capture_chance(&self, uid: u32, opponent: &Unit) -> f64 {
        let unit = &self.units[&uid];
        if unit.kind != "eagle_warrior"
            || self.players[opponent.owner].is_barbarian
            || self.rules.units[opponent.kind].class != "military"
        {
            return 0.0;
        }
        let attacker = self.rules.units[unit.kind].strength;
        let defender = self.rules.units[opponent.kind].strength;
        (50.0 + (attacker - defender) * 2.5).clamp(0.0, 100.0)
    }

    /// Everything a defeated unit pays its killer.
    ///
    /// ⚠ Named for promotions and never only about them — the policy card
    /// `earlier_era_kill_gold_pct` and the building `heal_on_unit_kill` were
    /// already here — and God of War makes the pantheon the third source, so
    /// the name is now what the function does.
    pub(super) fn kill_rewards(&mut self, attacker: &Unit, defeated: &Unit) {
        let defeated_spec = &self.rules.units[defeated.kind];
        let defeated_era = defeated_spec
            .tech
            .as_ref()
            .and_then(|node| self.rules.techs.get(node).map(|spec| spec.era))
            .or_else(|| {
                defeated_spec
                    .civic
                    .as_ref()
                    .and_then(|node| self.rules.civics.get(node).map(|spec| spec.era))
            })
            .unwrap_or(0);
        if self.player_era(attacker.owner) > defeated_era {
            let pct = self.policy_effect(attacker.owner, "earlier_era_kill_gold_pct");
            self.players[attacker.owner].gold += defeated_spec.strength * pct / 100.0;
        }
        let faith_pct = self.promotion_effect(attacker, "faith_on_kill_strength_pct");
        if faith_pct > 0.0 {
            self.players[attacker.owner].faith += defeated_spec.strength * faith_pct / 100.0;
        }
        // God of War: GOD_OF_WAR_FAITH_KILLS_MODIFIER,
        // `MODIFIER_PLAYER_UNITS_ADJUST_POST_COMBAT_YIELD` with
        // `PercentDefeatedStrength 50` and `YieldType YIELD_FAITH`, over
        // `PLOT_EIGHT_INCLUDE_HOLY_SITE`. The same arithmetic as the promotion
        // above — a percentage of the dead unit's Combat Strength — with a plot
        // test instead of a promotion, so the two share this shape rather than
        // each inventing one. Not in `Expansion2_RemoveData.xml`.
        //
        // ⚠ Two things the shipped rows say that a reading from memory does
        // not. The requirement names no owner, and the text agrees by omission
        // — "within 8 tiles of a Holy Site district" — so a rival's Holy Site
        // pays as well as our own. And the text ends "(on Standard Speed)",
        // Civilization VI's marker for a one-off yield that scales with the
        // game speed, which `GameSpeed::scale` is.
        let war_pct = self.pantheon_effect(attacker.owner, "faith_on_kill_near_holy_site_pct");
        if war_pct > 0.0 && defeated_spec.class == "military" {
            let strength = defeated_spec.strength;
            let near_a_holy_site = self
                .wdisk(defeated.pos, GOD_OF_WAR_HOLY_SITE_RANGE)
                .into_iter()
                .any(|position| {
                    self.map.get(position).is_some_and(|tile| {
                        tile.district.is_some_and(|district| {
                            self.district_is_family(district, crate::name!("holy_site"))
                        })
                    })
                });
            if near_a_holy_site {
                self.players[attacker.owner].faith +=
                    self.game_speed.scale(strength * war_pct / 100.0);
            }
        }
        if self.rules.units[defeated.kind].domain.as_deref() == Some("sea") {
            let pct = self.promotion_effect(attacker, "gold_from_naval_kill_pct");
            if pct > 0.0 {
                let strength = self.rules.units[defeated.kind].strength;
                self.players[attacker.owner].gold += strength * pct / 100.0;
            }
        }
        let heal = self
            .cities
            .values()
            .filter(|city| city.owner == attacker.owner)
            .map(|city| self.city_building_effect(city, "heal_on_unit_kill"))
            .sum::<f64>() as i32;
        if heal > 0 {
            if let Some(unit) = self.units.get_mut(&attacker.id) {
                unit.hp = (unit.hp + heal).min(100);
            }
        }
        let pressure = self.promotion_effect(attacker, "religious_pressure_on_kill");
        if pressure > 0.0 {
            if let Some(religion) = attacker.religion.as_ref() {
                let cities: Vec<u32> = self
                    .cities
                    .values()
                    .filter(|city| self.wdist(city.pos, defeated.pos) <= 6)
                    .map(|city| city.id)
                    .collect();
                for cid in cities {
                    *self
                        .cities
                        .get_mut(&cid)
                        .unwrap()
                        .pressure
                        .entry(religion.clone())
                        .or_insert(0.0) += pressure;
                }
            }
        }
    }

    pub(super) fn flanking_support_unlocked(&self, owner: usize) -> bool {
        if self.players[owner].is_barbarian {
            let majors: Vec<&Player> = self
                .players
                .iter()
                .filter(|p| !p.is_minor && !p.is_barbarian)
                .collect();
            !majors.is_empty()
                && 2 * majors
                    .iter()
                    .filter(|p| self.tree_effect(p.id, "flanking_support") > 0.0)
                    .count()
                    >= majors.len()
        } else {
            self.tree_effect(owner, "flanking_support") > 0.0
        }
    }

    pub(super) fn flanking_bonus(&self, uid: u32, target: Pos) -> f64 {
        let owner = self.units[&uid].owner;
        let additional = self
            .nbrs(target)
            .into_iter()
            .flat_map(|p| self.unit_ids_at(p))
            .filter(|id| **id != uid)
            .filter(|id| {
                let u = &self.units[id];
                u.owner == owner
                    && self.rules.units[u.kind].class == "military"
                    && !self.is_embarked(u)
                    && !self.crosses_river(u.pos, target)
            })
            .count();
        let tegh = if additional > 0 {
            self.promotion_effect(&self.units[&uid], "flanking_combat_strength")
        } else {
            0.0
        };
        if !self.flanking_support_unlocked(owner) {
            return tegh;
        }
        let multiplier = self
            .promotion_effect(&self.units[&uid], "flanking_multiplier")
            .max(1.0);
        let naval_multiplier =
            if self.rules.units[self.units[&uid].kind].domain.as_deref() == Some("sea") {
                1.0 + self.players[owner]
                    .counters
                    .get("great_person:naval_flanking_bonus_pct")
                    .copied()
                    .unwrap_or(0) as f64
                    / 100.0
            } else {
                1.0
            };
        2.0 * additional as f64 * multiplier * naval_multiplier + tegh
    }

    /// Melee attacks pay the movement cost of entering the defender's tile.
    /// As with ordinary movement, a unit that has all of its Movement may
    /// always perform one attack even when the terrain costs more than its
    /// maximum Movement.
    /// This is also the exact movement preflight for an `AdvancedAi` melee
    /// candidate, so its tactical evaluator does not price a strike the game
    /// will reject for an expensive terrain or river entry.
    pub(crate) fn can_pay_melee_entry(&self, uid: u32, target: Pos) -> bool {
        let u = &self.units[&uid];
        if !self.map.tiles.contains_key(&target) {
            return false;
        }
        if !self.unit_can_cross_cliff(uid, u.pos, target) {
            return false;
        }
        u.moves_left >= self.unit_max_moves(uid)
            || u.moves_left >= self.unit_step_cost(uid, u.pos, target)
    }

    pub(super) fn support_bonus(&self, defender: &Unit) -> f64 {
        if !self.flanking_support_unlocked(defender.owner) {
            return 0.0;
        }
        let adjacent = self
            .nbrs(defender.pos)
            .into_iter()
            .flat_map(|p| self.unit_ids_at(p))
            .filter(|id| {
                let u = &self.units[id];
                u.owner == defender.owner && self.rules.units[u.kind].class == "military"
            })
            .count();
        let multiplier = self
            .promotion_effect(defender, "support_multiplier")
            .max(1.0);
        2.0 * adjacent as f64 * multiplier
    }

    pub(super) fn consume_unit_attack(&mut self, uid: u32) {
        let move_after = self.promotion_effect(&self.units[&uid], "move_after_attack") > 0.0;
        let unit = self.units.get_mut(&uid).unwrap();
        unit.attacks_left = (unit.attacks_left - 1).max(0);
        if unit.attacks_left == 0 && !move_after {
            unit.moves_left = 0.0;
        }
        unit.fortified = false;
        unit.fortify_turns = 0;
        unit.acted = true;
    }

    /// Apply one unit's damage to another and credit only health really lost.
    ///
    /// Combat rolls are allowed to exceed the target's remaining health so
    /// the existing death checks can continue to read `hp <= 0`. Lifetime
    /// output is an economic measurement, though, and must not count that
    /// overkill. Keeping the subtraction and the credit in one helper also
    /// prevents melee counter-damage, interceptions, and ranged attacks from
    /// drifting into different definitions of "damage dealt".
    pub(super) fn apply_unit_damage(&mut self, attacker: u32, defender: u32, rolled: i32) {
        let rolled = rolled.max(0);
        let actual = self.units[&defender].hp.max(0).min(rolled) as u64;
        self.units.get_mut(&defender).unwrap().hp -= rolled;
        if let Some(unit) = self.units.get_mut(&attacker) {
            unit.damage_dealt = unit.damage_dealt.saturating_add(actual);
        }
    }

    pub(super) fn consume_melee_attack(&mut self, uid: u32, target: Pos) {
        let cost = self.unit_step_cost(uid, self.units[&uid].pos, target);
        let remaining = (self.units[&uid].moves_left - cost).max(0.0);
        self.units.get_mut(&uid).unwrap().moves_left = remaining;
        self.consume_unit_attack(uid);
    }

    pub(super) fn pillage_encampment(&mut self, uid: u32, cid: u32, target: Pos) {
        let defender = self.cities[&cid].owner;
        let garrison: Vec<u32> = self
            .unit_ids_at(target)
            .iter()
            .filter(|id| {
                self.units[id].owner == defender
                    && self.rules.units[self.units[id].kind].class == "military"
            })
            .copied()
            .collect();
        for id in garrison {
            self.remove_unit(id);
            self.on_unit_lost(defender);
        }
        let city = self.cities.get_mut(&cid).unwrap();
        city.encampment_hp = 0;
        city.encampment_wall_hp = 0;
        city.encampment_pillaged = true;
        self.enter_tile(uid, target);
    }

    pub(super) fn do_encampment_melee(
        &mut self,
        pid: usize,
        uid: u32,
        cid: u32,
        target: Pos,
        embarked: bool,
    ) -> Result<(), String> {
        let defender = self.cities[&cid].owner;
        let participant = self.units[&uid].clone();
        self.record_war_unit_participation(&participant, defender);
        if self.cities[&cid].encampment_hp <= 0 {
            self.consume_melee_attack(uid, target);
            self.pillage_encampment(uid, cid, target);
            return Ok(());
        }
        let attacker = self.units[&uid].clone();
        let spec = self.rules.units[attacker.kind].clone();
        let mut attack_base =
            self.unit_unembarked_strength(&attacker) + self.vs_bonus(pid, self.cities[&cid].owner);
        if embarked && self.promotion_effect(&attacker, "amphibious") == 0.0 {
            attack_base -= 10.0;
        }
        let mut defense = self.encampment_strength(cid);
        if self.crosses_river(attacker.pos, target)
            && self.promotion_effect(&attacker, "amphibious") == 0.0
        {
            defense += 5.0;
        }
        let attack = effective_strength(attack_base, attacker.hp);
        let dealt = damage(attack, defense, &mut self.rng);
        let received = damage(defense, attack, &mut self.rng);
        let (ram, tower) = self.siege_support_effects(pid, cid, target, &spec.promotion_class);
        self.encampment_take_damage(pid, cid, dealt, if ram { 1.0 } else { 0.15 }, tower);
        self.units.get_mut(&uid).unwrap().hp -= received;
        self.consume_melee_attack(uid, target);
        if self.units[&uid].hp <= 0 {
            self.remove_unit(uid);
            self.on_unit_lost(pid);
            self.cities.get_mut(&cid).unwrap().encampment_hp =
                self.cities[&cid].encampment_hp.max(1);
            return Ok(());
        }
        if self.cities[&cid].encampment_hp <= 0 {
            self.award_initiated_combat_xp(uid, 10.0);
            self.pillage_encampment(uid, cid, target);
        } else {
            self.award_initiated_combat_xp(uid, 3.0);
        }
        Ok(())
    }

    /// Active Battering Ram and Siege Tower effects against a City Center or
    /// Encampment. Their wall-era limits also apply to replacement buildings,
    /// and both support units are ineffective against Steel's Urban Defenses.
    pub(super) fn siege_support_effects(
        &self,
        attacker: usize,
        cid: u32,
        target: Pos,
        promotion_class: &str,
    ) -> (bool, bool) {
        let city = &self.cities[&cid];
        if !matches!(promotion_class, "melee" | "anti_cavalry")
            || self.tree_effect(city.owner, "urban_defenses") > 0.0
        {
            return (false, false);
        }
        let adjacent_support = |kind: &str| {
            self.nbrs(target).into_iter().any(|position| {
                self.unit_ids_at(position)
                    .iter()
                    .any(|id| self.units[id].owner == attacker && self.units[id].kind == kind)
            })
        };
        let ram = adjacent_support("battering_ram")
            && self.city_building_effect(city, "battering_ram_immunity") <= 0.0;
        let tower = adjacent_support("siege_tower")
            && self.city_building_effect(city, "siege_support_immunity") <= 0.0;
        (ram, tower)
    }

    pub(super) fn do_encampment_ranged(
        &mut self,
        pid: usize,
        uid: u32,
        cid: u32,
        _target: Pos,
    ) -> Result<(), String> {
        let defender = self.cities[&cid].owner;
        let participant = self.units[&uid].clone();
        self.record_war_unit_participation(&participant, defender);
        if self.cities[&cid].encampment_hp <= 0 {
            self.consume_unit_attack(uid);
            self.cities.get_mut(&cid).unwrap().encampment_hp = 0;
            return Ok(());
        }
        let attacker = self.units[&uid].clone();
        let spec = self.rules.units[attacker.kind].clone();
        let mut attack_base = self.unit_ranged_attack_strength(&attacker)
            + self.vs_bonus(pid, self.cities[&cid].owner)
            + self.promotion_effect(&attacker, "ranged_vs_district")
            + self.gdr_siege_bonus(&attacker);
        if spec.ranged_strength > 0.0 && spec.domain.as_deref() != Some("sea") {
            attack_base -= 17.0;
        }
        let attack = effective_strength(attack_base, attacker.hp);
        let dealt = damage(attack, self.encampment_strength(cid), &mut self.rng);
        self.encampment_take_damage(pid, cid, dealt, if spec.siege { 1.0 } else { 0.5 }, false);
        self.consume_unit_attack(uid);
        if self.cities[&cid].encampment_hp <= 0 {
            if spec.siege {
                self.cities.get_mut(&cid).unwrap().encampment_hp = 0;
                self.award_initiated_combat_xp(uid, 10.0);
            } else {
                self.cities.get_mut(&cid).unwrap().encampment_hp = 1;
                self.award_initiated_combat_xp(uid, 3.0);
            }
        } else {
            self.award_initiated_combat_xp(uid, 3.0);
        }
        Ok(())
    }

    pub(super) fn do_attack(&mut self, pid: usize, uid: u32, target: Pos) -> Result<(), String> {
        let u = self.own_unit(pid, uid)?;
        let spec = self.rules.units[u.kind].clone();
        if !spec.is_melee_capable() {
            return Err("unit cannot melee attack".into());
        }
        let amphibious = self.is_embarked(&u);
        if u.moves_left <= 0.0 || u.attacks_left <= 0 {
            return Err("no moves left".into());
        }
        if self.wdist(u.pos, target) != 1 {
            return Err("target not adjacent".into());
        }
        if !self.unit_can_melee_target_domain(uid, target) {
            return Err("unit cannot attack into that domain".into());
        }
        if !self.can_pay_melee_entry(uid, target) {
            return Err("not enough movement to attack".into());
        }
        if amphibious
            && self
                .map
                .get(target)
                .map(|t| self.rules.is_water(t))
                .unwrap_or(true)
        {
            return Err("embarked units can only attack onto land".into());
        }
        if let Some(cid) = self.encampment_at(target) {
            let owner = self.cities[&cid].owner;
            if owner != pid && self.is_at_war(pid, owner) {
                return self.do_encampment_melee(pid, uid, cid, target, amphibious);
            }
        }
        let enemy_ids: Vec<u32> = self
            .unit_ids_at(target)
            .iter()
            .filter(|id| {
                let owner = self.units[id].owner;
                owner != pid && self.is_at_war(pid, owner)
            })
            .copied()
            .collect();
        let mut city_id = self.city_at(target);
        if let Some(cid) = city_id {
            let owner = self.cities[&cid].owner;
            if owner == pid || !self.is_at_war(pid, owner) {
                city_id = None;
            }
        }
        // See `peaceful_foreign_unit_at`. The enumeration above no longer
        // offers this plot; refuse it here too so a hand-written order — or a
        // replay of one the host already mangled — cannot take the same shot.
        // A city is its own defender, so only the unit fight is vetoed.
        if city_id.is_none() && self.peaceful_foreign_unit_at(pid, target) {
            return Err("a unit we are at peace with stands there".into());
        }
        if enemy_ids.is_empty() && city_id.is_none() {
            return Err("nothing to attack".into());
        }
        let military: Vec<u32> = enemy_ids
            .iter()
            .cloned()
            .filter(|id| {
                let spec = &self.rules.units[self.units[id].kind];
                spec.class == "military" && spec.domain.as_deref() != Some("air")
            })
            .collect();
        if military.is_empty() && city_id.is_none() {
            return Err("no combat target".into());
        }
        self.consume_melee_attack(uid, target);
        // A unit garrisoned in a City Center cannot be targeted directly;
        // attacks hit the city and the garrison only affects its strength.
        if city_id.is_none() && !military.is_empty() {
            let did = *military
                .iter()
                .max_by(|a, b| {
                    let ea = effective_strength(
                        self.unit_strength(&self.units[*a], true),
                        self.units[*a].hp,
                    );
                    let eb = effective_strength(
                        self.unit_strength(&self.units[*b], true),
                        self.units[*b].hp,
                    );
                    ea.partial_cmp(&eb).unwrap()
                })
                .unwrap();
            let d = self.units[&did].clone();
            let attacker = self.units[&uid].clone();
            self.record_war_unit_participation(&attacker, d.owner);
            self.record_war_unit_participation(&d, attacker.owner);
            // The strengths live in `melee_exchange_strengths` so a controller
            // can price this exact exchange before choosing to take it. The
            // unit has been consumed above, but nothing `consume_melee_attack`
            // touches (attacks, movement, fortification) enters an attacker's
            // strength, and it does not move the unit — so the shared reading
            // is the reading this line always had.
            let (att, ds) = self
                .melee_exchange_strengths(uid, did)
                .expect("both combatants exist at the blow");
            let dmg_out = damage(att, ds, &mut self.rng);
            let dmg_in = damage(ds, att, &mut self.rng);
            self.apply_unit_damage(uid, did, dmg_out);
            self.apply_unit_damage(did, uid, dmg_in);
            let d_dead = self.units[&did].hp <= 0;
            let downer = self.units[&did].owner;
            self.record_emergency_combat(pid, downer, d_dead);
            if d_dead && self.has_ability(pid, "killer_of_cyrus") {
                if let Some(mu) = self.units.get_mut(&uid) {
                    mu.hp = (mu.hp + 30).min(100); // Tomyris
                }
            }
            let attacker_dead = self.units[&uid].hp <= 0;
            if attacker_dead && !d_dead && self.has_ability(downer, "killer_of_cyrus") {
                if let Some(defender) = self.units.get_mut(&did) {
                    defender.hp = (defender.hp + 30).min(100);
                }
            }
            if !attacker_dead {
                self.award_unit_combat_xp(uid, &d, false, true, d_dead);
            }
            if !d_dead {
                self.award_unit_combat_xp(did, &attacker, false, false, attacker_dead);
            }
            // A survivor that loses to a Winged Hussar is pushed off its tile,
            // and pays for standing its ground when there is nowhere to go.
            if !d_dead
                && !attacker_dead
                && self.rules.units[attacker.kind]
                    .effects
                    .get("force_retreat")
                    .copied()
                    .unwrap_or(0.0)
                    > 0.0
            {
                match self.forced_retreat_tile(did, attacker.pos) {
                    Some(to) => self.relocate(did, to),
                    None => {
                        // Cornered. Civilization VI charges the defender for the
                        // retreat it could not make; this can finish it, so the
                        // death is handled by the same path every other kill
                        // takes rather than a second copy of it here.
                        self.apply_unit_damage(uid, did, FORCED_RETREAT_BLOCKED_DAMAGE);
                    }
                }
            }
            let capture_chance = self.eagle_capture_chance(uid, &d);
            let captured_as_builder = d_dead
                && !attacker_dead
                && capture_chance > 0.0
                && self.rng.uniform(0.0, 100.0) < capture_chance;
            if d_dead {
                self.note_underdog_kill(pid, &attacker, &d);
                self.note_great_person_assisted_kill(pid, &attacker);
                self.record_kill(pid, Some(&attacker.kind), &d);
                self.kill_rewards(&attacker, &d);
                self.remove_unit(did);
                self.on_unit_lost(downer);
                if captured_as_builder {
                    self.spawn_unit("builder", pid, target);
                }
            }
            if attacker_dead {
                if self.units.contains_key(&uid) {
                    self.note_underdog_kill(downer, &d, &attacker);
                    self.record_kill(downer, Some(&d.kind), &attacker);
                    self.remove_unit(uid);
                    self.on_unit_lost(pid);
                }
                return Ok(());
            }
            if d_dead {
                let enemy_military_left = self.unit_ids_at(target).iter().any(|id| {
                    let o = &self.units[id];
                    o.owner != pid && self.rules.units[o.kind].class == "military"
                });
                if !enemy_military_left {
                    let city_blocks = match city_id {
                        Some(cid) => self.cities.get(&cid).map(|c| c.hp > 0).unwrap_or(false),
                        None => false,
                    };
                    if !city_blocks {
                        self.enter_tile(uid, target);
                    }
                }
            }
        } else if let Some(cid) = city_id {
            if self.cities[&cid].hp > 0 {
                let attacker = self.units[&uid].clone();
                let defender = self.cities[&cid].owner;
                self.record_war_unit_participation(&attacker, defender);
                self.record_war_city_garrison_participation(cid, attacker.owner);
                let mut att_base = self.unit_unembarked_strength(&attacker)
                    + self.vs_bonus(pid, self.cities[&cid].owner);
                if amphibious && self.promotion_effect(&attacker, "amphibious") == 0.0 {
                    att_base -= 10.0;
                }
                let att = effective_strength(att_base, attacker.hp);
                let cs = self.city_strength(cid)
                    + if self.crosses_river(u.pos, target)
                        && self.promotion_effect(&attacker, "amphibious") == 0.0
                    {
                        5.0
                    } else {
                        0.0
                    };
                let dmg_out = damage(att, cs, &mut self.rng);
                let dmg_in = damage(cs, att, &mut self.rng);
                // battering ram: full melee damage vs ancient walls;
                // siege tower: only melee/anti-cavalry pour through the walls
                let (ram, tower) =
                    self.siege_support_effects(pid, cid, target, &spec.promotion_class);
                let basil_cavalry = self.has_ability(pid, "taxis")
                    && spec.cavalry
                    && self.players[pid]
                        .religion
                        .as_deref()
                        .is_some_and(|religion| {
                            self.city_religion(&self.cities[&cid]) == Some(religion)
                        });
                let akkad = self.grants_city_state_unique_bonus(pid, "Akkad")
                    && matches!(spec.promotion_class.as_str(), "melee" | "anti_cavalry");
                let mult = if ram || basil_cavalry || akkad {
                    1.0
                } else {
                    0.15
                };
                self.city_take_damage(pid, cid, dmg_out, mult, tower);
                self.units.get_mut(&uid).unwrap().hp -= dmg_in;
                if self.units[&uid].hp <= 0 {
                    self.remove_unit(uid);
                    self.on_unit_lost(pid);
                    let c = self.cities.get_mut(&cid).unwrap();
                    c.hp = c.hp.max(1);
                    return Ok(());
                }
                if self.cities[&cid].hp <= 0 {
                    if self.players[pid].is_barbarian {
                        self.award_initiated_combat_xp(uid, 3.0);
                        self.cities.get_mut(&cid).unwrap().hp = 1;
                    } else {
                        self.award_initiated_combat_xp(uid, 10.0);
                        self.capture_city(cid, pid);
                        self.apply_capture_formation_upgrade(pid, uid);
                        self.enter_tile(uid, target);
                    }
                } else {
                    self.award_initiated_combat_xp(uid, 3.0);
                }
            } else if self.players[pid].is_barbarian {
                self.cities.get_mut(&cid).unwrap().hp = 1;
            } else {
                // A previous ranged attack may have depleted the garrison
                // health. The melee unit captures it but earns no XP for an
                // attack made after the city was already at 0 HP.
                self.capture_city(cid, pid);
                self.apply_capture_formation_upgrade(pid, uid);
                self.enter_tile(uid, target);
            }
        }
        Ok(())
    }

    pub(super) fn enter_tile(&mut self, uid: u32, pos: Pos) {
        let linked = self
            .units
            .get(&uid)
            .and_then(|unit| unit.linked_to)
            .filter(|_| self.is_linked_leader(uid));
        self.resolve_entered_units(uid, pos);
        self.relocate(uid, pos);
        let linked = linked.filter(|peer| {
            self.units
                .get(&uid)
                .is_some_and(|unit| unit.linked_to == Some(*peer))
                && self.units.get(peer).is_some_and(|unit| {
                    unit.linked_to == Some(uid) && unit.owner == self.units[&uid].owner
                })
        });
        if let Some(peer) = linked {
            self.relocate(peer, pos);
            let peer_max = self.unit_max_moves(peer);
            self.units.get_mut(&peer).unwrap().moves_left =
                self.units[&peer].moves_left.min(peer_max);
        }
        let unit_max = self.unit_max_moves(uid);
        self.units.get_mut(&uid).unwrap().moves_left = self.units[&uid].moves_left.min(unit_max);
        if self.formation_enters_enemy_zoc(uid, pos) {
            self.stop_unit_by_zoc(uid);
            if let Some(peer) = linked {
                self.stop_unit_by_zoc(peer);
            }
        }
        self.maybe_clear_camp(uid);
        self.maybe_goody_hut(uid);
    }

    /// Resolve undefended units when a combat unit enters their tile.
    /// Settlers and Builders are captured; Traders and support units are
    /// destroyed. Religious units are neither automatically captured nor
    /// destroyed (they use theological combat/Condemn Heretic instead).
    pub(super) fn resolve_entered_units(&mut self, uid: u32, pos: Pos) {
        let owner = self.units[&uid].owner;
        let mover_spec = &self.rules.units[self.units[&uid].kind];
        let military = mover_spec.class == "military";
        if !military {
            return;
        }
        let can_capture = mover_spec.domain.as_deref() == Some("sea")
            || !self
                .map
                .get(pos)
                .map(|tile| self.rules.is_water(tile))
                .unwrap_or(false);
        // Free Cities also carry `is_barbarian`; only the true barbarian
        // seat's takings belong on the raid ledger.
        let mover_is_barbarian =
            self.players[owner].is_barbarian && !self.players[owner].is_free_city;
        let mut affected_owners = BTreeSet::new();
        for oid in self.units_at(pos) {
            if oid == uid || self.units[&oid].owner == owner {
                continue;
            }
            let kind = self.units[&oid].kind;
            let class = self.rules.units[kind].class.as_str();
            if can_capture && matches!(kind.as_str(), "builder" | "settler") {
                let old = self.units[&oid].owner;
                affected_owners.insert(old);
                if mover_is_barbarian {
                    bump(&mut self.players[old], "civilians_lost_to_barbarians");
                }
                // `captured:settler` / `captured:builder`: how many of each
                // this seat has taken from a rival by entering their tile;
                // `rescued:*` is the same take from a barbarian that had
                // taken it first. An evaluator row can say whether a raid
                // ever paid, not only that it was declared.
                let from_barbarian = self.players[old].is_barbarian;
                let key = if from_barbarian {
                    "rescued"
                } else {
                    "captured"
                };
                bump(&mut self.players[owner], &format!("{key}:{kind}"));
                self.transfer_unit_owner(oid, owner);
            } else if matches!(class, "civilian" | "support") {
                let old = self.units[&oid].owner;
                affected_owners.insert(old);
                if mover_is_barbarian {
                    bump(&mut self.players[old], "civilians_lost_to_barbarians");
                }
                self.remove_unit(oid);
            }
        }
        for old in affected_owners {
            self.on_unit_lost(old);
        }
    }

    pub(super) fn do_ranged(&mut self, pid: usize, uid: u32, target: Pos) -> Result<(), String> {
        let u = self.own_unit(pid, uid)?;
        let spec = self.rules.units[u.kind].clone();
        if !spec.has_ranged_attack() {
            return Err("unit has no ranged attack".into());
        }
        if self.is_embarked(&u) {
            return Err("cannot attack while embarked".into());
        }
        if spec.siege && u.moved && self.promotion_effect(&u, "attack_after_move") == 0.0 {
            return Err("siege units cannot move and attack in the same turn".into());
        }
        if u.moves_left <= 0.0 || u.attacks_left <= 0 {
            return Err("no moves left".into());
        }
        let range = self.unit_attack_range(uid);
        if self.wdist(u.pos, target) > range {
            return Err("out of range".into());
        }
        let visible = self.player_vision_frame(pid);
        let viewers = self.visibility_viewers(pid);
        if !self.combat_target_visible_at(pid, target, visible.as_ref(), &viewers) {
            return Err("target is not visible".into());
        }
        if !self.unit_has_line_of_sight(uid, target) {
            return Err("line of sight blocked".into());
        }
        if let Some(cid) = self.encampment_at(target) {
            let owner = self.cities[&cid].owner;
            if owner != pid && self.is_at_war(pid, owner) {
                return self.do_encampment_ranged(pid, uid, cid, target);
            }
        }
        let mut enemy_ids: Vec<u32> = self
            .unit_ids_at(target)
            .iter()
            .filter(|id| {
                let owner = self.units[id].owner;
                owner != pid
                    && self.is_at_war(pid, owner)
                    && self.unit_currently_visible_to(**id, pid)
            })
            .copied()
            .collect();
        enemy_ids.extend(self.units.values().filter_map(|unit| {
            (unit.air_patrol
                && unit.air_patrol_pos == Some(target)
                && unit.owner != pid
                && self.is_at_war(pid, unit.owner)
                && self.rules.units[unit.kind].promotion_class == "air_fighter")
                .then_some(unit.id)
                .filter(|unit| self.unit_currently_visible_to(*unit, pid))
        }));
        enemy_ids.sort_unstable();
        enemy_ids.dedup();
        let mut city_id = self.city_at(target);
        if let Some(cid) = city_id {
            let owner = self.cities[&cid].owner;
            if owner == pid || !self.is_at_war(pid, owner) {
                city_id = None;
            }
        }
        // See `peaceful_foreign_unit_at`, and `do_attack` for why the city
        // branch stays open.
        if city_id.is_none() && self.peaceful_foreign_unit_at(pid, target) {
            return Err("a unit we are at peace with stands there".into());
        }
        let military: Vec<u32> = enemy_ids
            .iter()
            .cloned()
            .filter(|id| {
                let spec = &self.rules.units[self.units[id].kind];
                spec.class == "military"
                    && (spec.domain.as_deref() != Some("air")
                        || self.units[id].air_patrol_pos == Some(target))
            })
            .collect();
        if military.is_empty() && city_id.is_none() {
            return Err("nothing to attack".into());
        }
        self.consume_unit_attack(uid);
        // City Center garrisons are protected while the city stands.
        if city_id.is_none() && !military.is_empty() {
            let did = *military
                .iter()
                .max_by(|a, b| {
                    let ea = effective_strength(
                        self.unit_strength(&self.units[*a], true),
                        self.units[*a].hp,
                    );
                    let eb = effective_strength(
                        self.unit_strength(&self.units[*b], true),
                        self.units[*b].hp,
                    );
                    ea.partial_cmp(&eb).unwrap()
                })
                .unwrap();
            let defender = self.units[&did].clone();
            let attacker = self.units[&uid].clone();
            let downer = defender.owner;
            self.record_war_unit_participation(&attacker, downer);
            self.record_war_unit_participation(&defender, attacker.owner);
            // As in `do_attack`: the strengths are stated once, in
            // `ranged_strike_strengths`, so a controller deciding whether to
            // stand under this shot prices the shot the engine will fire.
            let (att, ds) = self
                .ranged_strike_strengths(uid, did, target)
                .expect("both combatants exist at the shot");
            let dmg = damage(att, ds, &mut self.rng);
            self.apply_unit_damage(uid, did, dmg);
            let defender_dead = self.units[&did].hp <= 0;
            self.record_emergency_combat(pid, downer, defender_dead);
            self.award_unit_combat_xp(uid, &defender, true, true, defender_dead);
            if !defender_dead {
                self.award_unit_combat_xp(did, &attacker, true, false, false);
            }
            if defender_dead {
                self.note_underdog_kill(pid, &attacker, &defender);
                self.note_great_person_assisted_kill(pid, &attacker);
                self.record_kill(pid, Some(&attacker.kind), &defender);
                self.kill_rewards(&attacker, &defender);
                if self.has_ability(pid, "killer_of_cyrus") {
                    if let Some(attacker) = self.units.get_mut(&uid) {
                        attacker.hp = (attacker.hp + 30).min(100);
                    }
                }
                let downer = self.units[&did].owner;
                self.remove_unit(did);
                self.on_unit_lost(downer);
            }
        } else if let Some(cid) = city_id {
            let starting_hp = self.cities[&cid].hp;
            let defender = self.cities[&cid].owner;
            let attacker = self.units[&uid].clone();
            self.record_war_unit_participation(&attacker, defender);
            self.record_war_city_garrison_participation(cid, attacker.owner);
            let mut att_base = self.unit_ranged_attack_strength(&self.units[&uid])
                + self.promotion_effect(&self.units[&uid], "ranged_vs_district")
                + self.gdr_siege_bonus(&self.units[&uid])
                + self.vs_bonus(pid, self.cities[&cid].owner);
            if spec.ranged_strength > 0.0 && spec.domain.as_deref() != Some("sea") {
                att_base -= 17.0;
            }
            let att = effective_strength(att_base, self.units[&uid].hp);
            let cs = self.city_strength(cid);
            let dmg = damage(att, cs, &mut self.rng);
            let mult = if spec.siege || self.gdr_full_wall_damage(&self.units[&uid]) {
                1.0
            } else {
                0.5
            };
            self.city_take_damage(pid, cid, dmg, mult, false);
            if starting_hp <= 0 {
                // Shots after a Bombard attack has depleted the garrison
                // grant no XP and must not revive the city.
                self.cities.get_mut(&cid).unwrap().hp = 0;
            } else if self.cities[&cid].hp <= 0 && spec.siege {
                // Bombard-class shots may deplete a city, but still cannot
                // capture it. The depleting shot earns the city final-blow XP.
                self.cities.get_mut(&cid).unwrap().hp = 0;
                self.siege.left_depleted += 1;
                let city_pos = self.cities[&cid].pos;
                let taker_ready = self.units.values().any(|unit| {
                    unit.owner == pid
                        && unit.moves_left > 0.0
                        && self.wdist(unit.pos, city_pos) <= 1
                        && self.rules.units[unit.kind].is_melee_capable()
                });
                if taker_ready {
                    self.siege.depleted_with_a_taker_ready += 1;
                }
                self.award_initiated_combat_xp(uid, 10.0);
            } else {
                // Ordinary ranged attacks cannot reduce Garrison Health
                // below 1 and earn the normal city-attack XP.
                self.cities.get_mut(&cid).unwrap().hp = self.cities[&cid].hp.max(1);
                self.award_initiated_combat_xp(uid, 3.0);
            }
        }
        Ok(())
    }

    pub(super) fn do_found_city(&mut self, pid: usize, uid: u32) -> Result<(), String> {
        let u = self.own_unit(pid, uid)?;
        if u.kind != "settler" {
            return Err("only settlers found cities".into());
        }
        // Nobody founds a city on a battlefield. No Tactics seat is dropped
        // in with a Settler, so this is the rule stated where it can be
        // relied on rather than left as a property of the opening roster: a
        // Settler that reached an arena some other way still does not turn
        // the battle into a Civ game.
        if self.is_arena() {
            return Err("no cities are founded on a battlefield".into());
        }
        if self.players[pid].is_barbarian {
            return Err("barbarians do not found cities".into());
        }
        if self.players[pid].is_minor {
            return Err("city-states do not found cities".into());
        }
        if !self.can_found_city(uid) {
            return Err("cannot found city here".into());
        }
        if self.policy_effect(pid, "no_settling") > 0.0 {
            return Err("Isolationism forbids settling new cities".into());
        }
        let cid = self.found_city_for(pid, u.pos, None);
        self.break_promises_on_settlement(pid, self.cities[&cid].pos);
        if self.empire_building_sum(pid, |building| {
            building
                .effects
                .get("free_builder_new_city")
                .copied()
                .unwrap_or(0.0)
        }) > 0.0
        {
            self.place_new_unit("builder", pid, self.cities[&cid].pos);
        }
        self.remove_unit(uid);
        Ok(())
    }

    /// Place a city for `pid` at `pos`.
    ///
    /// Public so a mirrored game can be given the empire it is mirroring.
    /// `mirror::rebuild_game` rebuilds the MAP; without the cities, every score
    /// that reads spacing or owned territory is evaluated against an empty world —
    /// which made CIVVIS's settle ranking recommend plots two tiles from the real
    /// capital, illegal at Civilization VI's own `CITY_MIN_RANGE` of 3.
    pub fn place_city(&mut self, pid: usize, pos: Pos, name: Option<String>) -> u32 {
        self.found_city_for(pid, pos, name)
    }

    pub(crate) fn found_city_for(&mut self, pid: usize, pos: Pos, name: Option<String>) -> u32 {
        let p_civ = self.players[pid].civ.clone();
        let is_minor = self.players[pid].is_minor;
        let name = name.unwrap_or_else(|| {
            let names = city_names(&p_civ);
            let n_mine = self
                .cities
                .values()
                .filter(|c| c.original_owner == pid)
                .count();
            if n_mine < names.len() {
                names[n_mine].to_string()
            } else {
                format!("{} {}", p_civ, n_mine + 1)
            }
        });
        let is_capital = !is_minor
            && !self
                .cities
                .values()
                .any(|c| c.original_owner == pid && c.is_capital);
        let cid = self.next_id;
        self.next_id += 1;
        let mut city = City {
            id: cid,
            name,
            owner: pid,
            pos,
            pop: 1,
            food: 0.0,
            production: 0.0,
            production_progress: BTreeMap::new(),
            strategic_resource_commitments: BTreeMap::new(),
            border_culture: 0.0,
            hp: 200,
            buildings: Vec::new(),
            products: Vec::new(),
            building_eras: BTreeMap::new(),
            pillaged_buildings: BTreeSet::new(),
            atheist_pressure: starting_atheist_pressure(),
            districts: Districts::default(),
            wonders: BTreeMap::new(),
            owned_tiles: Vec::new(),
            queue: Vec::new(),
            original_owner: pid,
            is_capital,
            struck: false,
            extra_strikes_used: 0,
            wall_hp: 0,
            encampment_hp: 0,
            encampment_wall_hp: 0,
            encampment_struck: false,
            encampment_extra_strikes_used: 0,
            encampment_last_attacked: 0,
            encampment_pillaged: false,
            last_attacked: 0,
            pressure: BTreeMap::new(),
            loyalty: 100.0,
            free_city_pressure: BTreeMap::new(),
            captured_from: None,
            occupied_from: None,
            occupation_grievance: None,
            reactor_age: 0,
            great_person_foreign_route_gold: 0.0,
        };
        if !is_minor
            && self.dedication_active(pid, "hic_sunt_dracones")
            && self.on_foreign_continent(pid, pos)
        {
            city.pop = 3;
        }
        if let Some(religion) = self.players[pid].religion.clone() {
            if self.religion_belief_effect(&religion, "new_city_religion") > 0.0 {
                city.pressure.insert(religion, 1000.0);
            }
        }
        {
            let center = self.map.tiles.get_mut(&pos).unwrap();
            center.feature = None;
            center.improvement = None;
        }
        // Every city opens owning its centre and the whole first ring, a
        // city-state included. `CivilizationLevels.StartingTilesForCity` is 6
        // for a full civilization and 5 for a city-state — calibrated by
        // Russia's `MODIFIER_PLAYER_ADJUST_CITY_TILES` Amount 5, which reads
        // as "+5 tiles beyond the ring", so 6 is the ring and the shipped
        // minor really does open one tile short. CIVVIS diverges here on
        // purpose: a deliberately neutral hole inside a city's own first ring
        // is unreadable on the map, and which of the six it should be is not
        // in the database anyway. The rule that keeps a city-state small is
        // the annexation gate, not this one tile — a minor still never takes
        // ground with its own Culture or Gold and grows only on Envoys.
        let mut claim = vec![pos];
        claim.extend(self.nbrs(pos));
        for tpos in claim {
            if let Some(t) = self.map.tiles.get_mut(&tpos) {
                if t.owner_city.is_none() {
                    t.owner_city = Some(cid);
                    city.owned_tiles.push(tpos);
                }
            }
        }
        if self.has_ability(pid, "trajans_column") && !is_minor {
            city.buildings.push(crate::name!("monument")); // Trajan's Column
            city.building_eras
                .insert(crate::name!("monument"), self.world_era);
        }
        self.city_by_pos.insert(pos, cid);
        let founded = city.name.clone();
        self.cities.insert(cid, city);
        if is_capital {
            if let Some(continent) = self.map.tiles[&pos].continent {
                self.players[pid]
                    .counters
                    .insert(format!("discovered_continent:{continent}"), 1);
            }
        }
        self.reveal(pid, pos, 3);
        self.note(pid, "Cities", format!("founded {founded}"), Some(pos));
        if !is_minor {
            self.note_city_founding_moments(pid, pos);
        }
        cid
    }

    pub(super) fn note_city_founding_moments(&mut self, pid: usize, pos: Pos) {
        let Some(tile) = self.map.get(pos) else {
            return;
        };
        let terrain = tile.terrain;
        if terrain.starts_with("desert") {
            self.add_historic_moment(pid, "MOMENT_CITY_BUILT_ON_DESERT");
        } else if terrain.starts_with("snow") {
            self.add_historic_moment(pid, "MOMENT_CITY_BUILT_ON_SNOW");
        } else if terrain.starts_with("tundra") {
            self.add_historic_moment(pid, "MOMENT_CITY_BUILT_ON_TUNDRA");
        }
        let (floodable_river, volcano, natural_wonder) = {
            let nearby: Vec<&crate::world::Tile> = self
                .map
                .tiles
                .values()
                .filter(|near| self.wdist(near.pos, pos) <= 2)
                .collect();
            (
                nearby.iter().any(|near| {
                    near.has_river()
                        && matches!(
                            near.feature.as_deref(),
                            Some("floodplains" | "grassland_floodplains" | "plains_floodplains")
                        )
                }),
                nearby.iter().any(|near| self.rules.is_volcano(near)),
                nearby.iter().any(|near| self.tile_is_natural_wonder(near)),
            )
        };
        if floodable_river {
            self.add_historic_moment(pid, "MOMENT_CITY_BUILT_NEAR_FLOODABLE_RIVER");
        }
        if volcano {
            self.add_historic_moment(pid, "MOMENT_CITY_BUILT_NEAR_VOLCANO");
        }
        if natural_wonder {
            self.add_historic_moment(pid, "MOMENT_CITY_BUILT_NEAR_NATURAL_WONDER");
        }
        if self.cities.values().any(|other| {
            other.owner != pid
                && !self.players[other.owner].is_minor
                && !self.players[other.owner].is_barbarian
                && self.wdist(other.pos, pos) <= 5
        }) {
            self.add_historic_moment(pid, "MOMENT_CITY_BUILT_NEAR_OTHER_CIV_CITY");
        }
        if self.on_foreign_continent(pid, pos) {
            if let Some(continent) = self.map.tiles[&pos].continent {
                self.first_historic_moment(
                    pid,
                    &format!("settlement_on_continent:{continent}"),
                    Some("MOMENT_CITY_BUILT_NEW_CONTINENT"),
                    None,
                );
            }
        }
        let city_count = self.player_city_ids(pid).len();
        let largest_other = self
            .players
            .iter()
            .filter(|player| {
                player.id != pid && player.alive && !player.is_minor && !player.is_barbarian
            })
            .map(|player| self.player_city_ids(player.id).len())
            .max()
            .unwrap_or(0);
        if city_count >= largest_other.saturating_add(3) {
            self.first_historic_moment(
                pid,
                "largest_civilization_by_three_cities",
                Some("MOMENT_CITY_BUILT_BECAME_LARGEST_CIV_BY_MARGIN"),
                None,
            );
        }
    }

    pub(super) fn feature_removal_unlocked(&self, pid: usize, feature: &str) -> bool {
        match feature {
            "forest" => self.tree_effect(pid, "chop_woods") > 0.0,
            "jungle" => self.tree_effect(pid, "chop_rainforest") > 0.0,
            "marsh" => self.tree_effect(pid, "clear_marsh") > 0.0,
            _ => true,
        }
    }

    pub(crate) fn builder_operations(&self, pid: usize, pos: Pos) -> Vec<String> {
        let Some(tile) = self.map.get(pos) else {
            return vec![];
        };
        if tile
            .owner_city
            .and_then(|cid| self.cities.get(&cid))
            .is_none_or(|city| city.owner != pid)
            || tile.district.is_some()
            || tile.district_foundation.is_some()
            || tile.wonder.is_some()
            || self.city_at(pos).is_some()
        {
            return vec![];
        }
        let mut operations = Vec::new();
        match tile.feature.as_deref() {
            Some("forest")
                if self.tree_effect(pid, "chop_woods") > 0.0
                    && !self.congress_effect_active("deforestation_treaty", "B", "forest") =>
            {
                operations.push("chop_woods".to_string())
            }
            Some("jungle")
                if self.tree_effect(pid, "chop_rainforest") > 0.0
                    && !self.congress_effect_active("deforestation_treaty", "B", "jungle") =>
            {
                operations.push("chop_rainforest".to_string())
            }
            Some("marsh")
                if self.tree_effect(pid, "clear_marsh") > 0.0
                    && !self.congress_effect_active("deforestation_treaty", "B", "marsh") =>
            {
                operations.push("clear_marsh".to_string())
            }
            _ => {}
        }
        if let Some(resource) = tile.resource.as_deref() {
            // Only the resources with a shipped Resource_Harvests row can be
            // harvested, each from its own technology on.
            if let Some(harvest) = self
                .rules
                .resources
                .get(resource)
                .and_then(|spec| spec.harvest.as_ref())
            {
                if harvest
                    .tech
                    .as_deref()
                    .is_none_or(|tech| self.players[pid].techs.contains(&Name::new(tech)))
                {
                    operations.push("harvest_resource".to_string());
                }
            }
        }
        if tile.feature.is_none()
            && tile.resource.is_none()
            && matches!(tile.terrain.as_str(), "grassland" | "plains" | "tundra")
            && self.tree_effect(pid, "plant_woods") > 0.0
        {
            operations.push("plant_woods".to_string());
        }
        operations
    }

    pub(super) fn do_builder_operation(
        &mut self,
        pid: usize,
        uid: u32,
        operation: &str,
    ) -> Result<(), String> {
        let unit = self.own_unit(pid, uid)?;
        if unit.kind != "builder"
            || unit.charges <= 0
            || unit.moves_left <= 0.0
            || !self
                .builder_operations(pid, unit.pos)
                .iter()
                .any(|candidate| candidate == operation)
        {
            return Err("builder cannot perform that operation".into());
        }
        let cid = self.map.tiles[&unit.pos].owner_city.unwrap();
        // The shipped Feature_Removes and Resource_Harvests base yields scale
        // with the era, and Magnus applies to harvests and removals alike.
        let scale = (self.world_era as f64 + 1.0)
            * (1.0 + self.governor_effect(pid, cid, "harvest_pct") / 100.0);
        let removed_feature = self.map.tiles[&unit.pos].feature;
        let mut payouts: Vec<(String, f64)> = Vec::new();
        match operation {
            "chop_woods" | "chop_rainforest" | "clear_marsh" => {
                let feature = removed_feature.ok_or_else(|| "no feature to clear".to_string())?;
                for (yield_type, base) in &self.rules.features[feature].chop {
                    payouts.push((yield_type.clone(), base * scale));
                }
                self.map.tiles.get_mut(&unit.pos).unwrap().feature = None;
            }
            "plant_woods" => {
                self.map.tiles.get_mut(&unit.pos).unwrap().feature = Some(crate::name!("forest"));
            }
            "harvest_resource" => {
                let resource = self
                    .map
                    .tiles
                    .get_mut(&unit.pos)
                    .unwrap()
                    .resource
                    .take()
                    .unwrap();
                let harvest = self.rules.resources[resource]
                    .harvest
                    .clone()
                    .ok_or_else(|| "that resource cannot be harvested".to_string())?;
                payouts.push((harvest.yield_type.clone(), harvest.amount * scale));
            }
            _ => return Err("unknown builder operation".into()),
        }
        for (yield_type, amount) in payouts {
            match yield_type.as_str() {
                "production" => self.cities.get_mut(&cid).unwrap().production += amount,
                "gold" => self.players[pid].gold += amount,
                _ => self.cities.get_mut(&cid).unwrap().food += amount,
            }
        }
        if removed_feature.as_deref().is_some_and(|feature| {
            self.congress_effect_active("deforestation_treaty", "A", feature)
        }) && matches!(operation, "chop_woods" | "chop_rainforest" | "clear_marsh")
        {
            let total: f64 = removed_feature
                .as_deref()
                .map(|feature| self.rules.features[feature].chop.values().sum::<f64>() * scale)
                .unwrap_or(0.0);
            self.players[pid].gold += total;
        }
        let builder = self.units.get_mut(&uid).unwrap();
        builder.charges -= 1;
        builder.moves_left = 0.0;
        builder.acted = true;
        if builder.charges <= 0 {
            self.remove_unit(uid);
        }
        Ok(())
    }

    pub(super) fn do_improve(&mut self, pid: usize, uid: u32, imp: &str) -> Result<(), String> {
        if matches!(
            imp,
            "chop_woods" | "chop_rainforest" | "clear_marsh" | "harvest_resource" | "plant_woods"
        ) {
            return self.do_builder_operation(pid, uid, imp);
        }
        let u = self.own_unit(pid, uid)?;
        let can_build = (u.kind == "builder" && self.rules.improvements[imp].builder_buildable)
            || self.rules.units[u.kind]
                .builds
                .iter()
                .any(|built| built == imp);
        if !can_build || u.charges <= 0 {
            return Err("unit cannot build that improvement".into());
        }
        // ⚠⚠ THE ENUMERATION IS NOT THE ONLY WAY AN IMPROVEMENT IS ORDERED.
        //
        // #1107 added this rule to `legal_actions_within`, and live testing on
        // run `civvis-20260804T173018Z` showed it had NOT taken: all three
        // `improve_refused` events still had `moves == 0`, the exact pattern the
        // gate was meant to stop. The reason is that `builder_step` computes
        // `valid_improvements` itself and calls `apply` DIRECTLY, never touching
        // the enumeration — the same shape as the purchase block in #1105, where
        // the gold buyers bypassed the layer the gate lived in.
        //
        // So the rule belongs HERE, in the one function every path reaches:
        // `legal_actions_within`, `builder_step`, `naturalist_step`,
        // `archaeologist_step` and anything added later.
        //
        // Civilization VI refuses BUILD_IMPROVEMENT from a unit with no movement,
        // and completing one spends all of it — which is why a builder that has
        // already improved a tile this turn cannot improve another.
        if u.moves_left <= 0.0 {
            return Err("unit has no movement left to build".into());
        }
        if !self.valid_improvements(pid, u.pos).iter().any(|i| i == imp) {
            return Err("invalid improvement here".into());
        }
        let national_park = (imp == "national_park")
            .then(|| self.national_park_site_at(pid, u.pos))
            .flatten();
        if imp == "national_park" && national_park.is_none() {
            return Err("no valid four-tile National Park here".into());
        }
        let first_mahavihara = imp == "mahavihara"
            && !self
                .cities
                .values()
                .filter(|city| city.owner == pid)
                .flat_map(|city| city.owned_tiles.iter())
                .any(|position| {
                    self.map.tiles[position].improvement.as_deref() == Some("mahavihara")
                });
        let removes = self.rules.improvements[imp].removes_feature;
        let excavates_artifact = matches!(imp, "archaeological_dig" | "shipwreck_excavation");
        let improvement_position = u.pos;
        let improved_disaster_fertility = self.map.tiles[&improvement_position].disaster_food > 0.0
            || self.map.tiles[&improvement_position].disaster_production > 0.0;
        if let Some(positions) = national_park {
            for position in positions {
                let tile = self.map.tiles.get_mut(&position).unwrap();
                tile.improvement = Some(crate::name!("national_park"));
                tile.pillaged = false;
            }
            bump(&mut self.players[pid], "national_park");
            self.repeatable_world_first_moment(
                pid,
                "national_park_created",
                "MOMENT_NATIONAL_PARK_CREATED",
                "MOMENT_NATIONAL_PARK_CREATED_FIRST_IN_WORLD",
            );
        } else {
            let t = self.map.tiles.get_mut(&u.pos).unwrap();
            // Excavation consumes the Antiquity Site/Shipwreck and immediately
            // transfers its Artifact to an active compatible Great Work slot. It
            // is an Archaeologist action, not a persistent tile improvement.
            if excavates_artifact {
                t.resource = None;
                t.improvement = None;
            } else {
                t.improvement = Some(Name::new(imp));
            }
            t.pillaged = false;
            if removes {
                t.feature = None;
            }
        }
        let mu = self.units.get_mut(&uid).unwrap();
        mu.charges -= 1;
        mu.moves_left = 0.0;
        mu.acted = true;
        bump(&mut self.players[pid], "improvements");
        if first_mahavihara {
            let technologies = self.rules.improvements[imp]
                .effects
                .get("first_build_random_tech")
                .copied()
                .unwrap_or(0.0) as usize;
            if technologies > 0 {
                self.complete_random_nodes(pid, technologies, true);
                self.note(
                    pid,
                    "Science",
                    "Nalanda's first Mahavihara granted a random technology",
                    Some(improvement_position),
                );
            }
        }
        if excavates_artifact {
            // A dig raises something from a past era, left by one of the
            // world's peoples - the shipped sites record whose history
            // happened there, and museum theming asks for variety.
            let era = self.rng.below(self.world_era.max(1));
            let civs: Vec<String> = self
                .players
                .iter()
                .filter(|player| !player.is_barbarian)
                .map(|player| player.civ.clone())
                .collect();
            let origin = civs[self.rng.below(civs.len().max(1))].clone();
            self.grant_great_work(pid, "artifact", era, &origin);
            self.add_historic_moment(pid, "MOMENT_ARTIFACT_EXTRACTED");
            if imp == "shipwreck_excavation" {
                self.first_historic_moment(
                    pid,
                    "shipwreck_excavated",
                    Some("MOMENT_ARTIFACT_EXTRACTED_SHIPWRECK_FIRST"),
                    Some("MOMENT_ARTIFACT_EXTRACTED_SHIPWRECK_FIRST_IN_WORLD"),
                );
            }
            self.dedication_trigger(pid, "artifact", 1);
        } else if national_park.is_none() {
            if self.rules.improvements[imp]
                .unique_to
                .as_deref()
                .is_some_and(|civilization| self.owns_civ_unique(pid, civilization))
            {
                self.first_historic_moment(
                    pid,
                    &format!("unique_improvement:{imp}"),
                    Some("MOMENT_IMPROVEMENT_CONSTRUCTED_FIRST_UNIQUE"),
                    None,
                );
            }
            match imp {
                "mountain_tunnel" => {
                    self.first_historic_moment(
                        pid,
                        "mountain_tunnel",
                        Some("MOMENT_IMPROVEMENT_CONSTRUCTED_MOUNTAIN_TUNNEL_FIRST"),
                        Some("MOMENT_IMPROVEMENT_CONSTRUCTED_MOUNTAIN_TUNNEL_FIRST_IN_WORLD"),
                    );
                }
                "seaside_resort" => {
                    self.first_historic_moment(
                        pid,
                        "seaside_resort",
                        Some("MOMENT_IMPROVEMENT_CONSTRUCTED_SEASIDE_RESORT_FIRST"),
                        Some("MOMENT_IMPROVEMENT_CONSTRUCTED_SEASIDE_RESORT_FIRST_IN_WORLD"),
                    );
                }
                "wind_farm" | "solar_farm" | "offshore_wind_farm" | "geothermal_plant" => {
                    self.first_historic_moment(
                        pid,
                        "renewable_improvement",
                        Some("MOMENT_IMPROVEMENT_CONSTRUCTED_RENEWABLE_ENERGY_FIRST"),
                        Some("MOMENT_IMPROVEMENT_CONSTRUCTED_RENEWABLE_ENERGY_FIRST_IN_WORLD"),
                    );
                }
                _ => {}
            }
            if improved_disaster_fertility {
                self.first_historic_moment(
                    pid,
                    "disaster_fertility_improved",
                    Some("MOMENT_IMPROVEMENT_CONSTRUCTED_ON_DISASTER_YIELD_TILE_FIRST"),
                    None,
                );
            }
            if imp == "industry" {
                self.first_historic_moment(
                    pid,
                    "first_industry",
                    Some("MOMENT_FIRST_INDUSTRY"),
                    Some("MOMENT_FIRST_INDUSTRY_IN_WORLD"),
                );
            }
        }
        if self.units[&uid].charges <= 0 {
            self.remove_unit(uid);
        }
        Ok(())
    }

    /// District tile on which a Builder may spend Royal Society charges for
    /// this city's current project. Keeping the target query separate lets AI
    /// players route Builders there without enumerating the global action set.
    pub(crate) fn project_contribution_target(&self, pid: usize, cid: u32) -> Option<Pos> {
        let city = self.cities.get(&cid)?;
        if city.owner != pid
            || self.empire_building_sum(pid, |building| {
                building
                    .effects
                    .get("builder_charge_space_project_pct")
                    .copied()
                    .unwrap_or(0.0)
            }) <= 0.0
            || self.players[pid]
                .counters
                .get(&format!("royal_society_city:{cid}"))
                .is_some_and(|last_turn| *last_turn == self.turn as i64)
        {
            return None;
        }
        let Some(Item::Project { project }) = city.queue.first() else {
            return None;
        };
        if project.starts_with("repair_") {
            return None;
        }
        let spec = &self.rules.projects[project];
        let district = spec.district?;
        std::iter::once(district)
            .chain(
                spec.alternate_districts
                    .iter()
                    .map(|name| Name::new(name.as_str())),
            )
            .find_map(|family| self.city_active_district_family_position(city, family))
    }

    pub(crate) fn can_contribute_project(&self, pid: usize, uid: u32, cid: u32) -> bool {
        let Some(unit) = self.units.get(&uid) else {
            return false;
        };
        unit.owner == pid
            && unit.kind == "builder"
            && unit.charges > 0
            && unit.moves_left > 0.0
            && self.project_contribution_target(pid, cid) == Some(unit.pos)
    }

    /// Royal Society consumes every remaining Builder charge to advance the
    /// district project beneath it. Each charge contributes the data-defined
    /// percentage of that project's base cost, and each city may receive the
    /// contribution only once per turn.
    pub(super) fn do_contribute_project(
        &mut self,
        pid: usize,
        uid: u32,
        cid: u32,
    ) -> Result<(), String> {
        if !self.can_contribute_project(pid, uid, cid) {
            return Err("builder cannot contribute to that project".into());
        }
        let charges = self.units[&uid].charges as f64;
        let item = self.cities[&cid].queue[0].clone();
        let percent = self.empire_building_sum(pid, |building| {
            building
                .effects
                .get("builder_charge_space_project_pct")
                .copied()
                .unwrap_or(0.0)
        });
        let cost = self.item_cost_for_city(pid, cid, &item);
        self.cities.get_mut(&cid).unwrap().production += cost * percent * charges / 100.0;
        self.players[pid]
            .counters
            .insert(format!("royal_society_city:{cid}"), self.turn as i64);
        self.remove_unit(uid);

        if self.cities[&cid].production + f64::EPSILON >= cost
            && self.complete_item(pid, cid, &item)
        {
            let city = self.cities.get_mut(&cid).unwrap();
            city.production = (city.production - cost).max(0.0);
            if city.queue.first() == Some(&item) {
                city.queue.remove(0);
            }
        }
        Ok(())
    }

    /// Tile on which a Military Engineer may accelerate the city's current
    /// engineering district. District foundations are represented by the
    /// queued item until completion, so the position comes from that item.
    pub(crate) fn district_contribution_target(&self, pid: usize, cid: u32) -> Option<Pos> {
        let city = self.cities.get(&cid)?;
        if city.owner != pid {
            return None;
        }
        let Some(Item::District { district, pos }) = city.queue.first() else {
            return None;
        };
        matches!(
            self.district_family(*district).as_str(),
            "aqueduct" | "canal" | "dam"
        )
        .then_some(*pos)
    }

    pub(crate) fn can_build_railroad(&self, pid: usize, uid: u32) -> bool {
        let Some(unit) = self.units.get(&uid) else {
            return false;
        };
        let tile = &self.map.tiles[&unit.pos];
        unit.owner == pid
            && unit.kind == "military_engineer"
            && unit.moves_left > 0.0
            && self.players[pid]
                .techs
                .contains(&crate::name!("steam_power"))
            && !self.rules.is_water(tile)
            && tile.road < 5
            && self.strategic_stockpile(pid, crate::name!("iron")) >= 1.0
            && self.strategic_stockpile(pid, crate::name!("coal")) >= 1.0
    }

    /// Gathering Storm Railroads: engineer-built only (BuildOnlyWithUnit),
    /// gated on Steam Power (Routes_XP2 PrereqTech), free of build charges
    /// (BuildWithUnitChargeCost 0), and costing 1 Iron and 1 Coal per tile
    /// (Route_ResourceCosts).
    pub(super) fn has_railroad_city_connection(&self, pid: usize) -> bool {
        let city_positions: BTreeSet<Pos> = self
            .cities
            .values()
            .filter(|city| city.owner == pid && self.map.tiles[&city.pos].road >= 5)
            .map(|city| city.pos)
            .collect();
        for start in &city_positions {
            let mut seen = BTreeSet::from([*start]);
            let mut queue = VecDeque::from([*start]);
            while let Some(position) = queue.pop_front() {
                if position != *start && city_positions.contains(&position) {
                    return true;
                }
                for neighbor in self.nbrs(position) {
                    if self.map.tiles[&neighbor].road >= 5 && seen.insert(neighbor) {
                        queue.push_back(neighbor);
                    }
                }
            }
        }
        false
    }

    pub(super) fn do_build_railroad(&mut self, pid: usize, uid: u32) -> Result<(), String> {
        if !self.can_build_railroad(pid, uid) {
            return Err("cannot lay a railroad here".to_string());
        }
        for resource in ["iron", "coal"] {
            if let Some(stock) = self.players[pid]
                .strategic_resources
                .get_mut(&Name::new(resource))
            {
                *stock -= 1.0;
            }
        }
        let pos = self.units[&uid].pos;
        self.map.tiles.get_mut(&pos).unwrap().road = 5;
        self.units.get_mut(&uid).unwrap().moves_left = 0.0;
        if self.has_railroad_city_connection(pid) {
            self.first_historic_moment(
                pid,
                "railroad_city_connection",
                Some("MOMENT_ROUTE_CREATED_RAILROAD_CONNECTS_TWO_CITIES"),
                Some("MOMENT_ROUTE_CREATED_RAILROAD_CONNECTS_TWO_CITIES_FIRST_IN_WORLD"),
            );
        }
        Ok(())
    }

    pub(crate) fn can_contribute_district(&self, pid: usize, uid: u32, cid: u32) -> bool {
        let Some(unit) = self.units.get(&uid) else {
            return false;
        };
        unit.owner == pid
            && unit.kind == "military_engineer"
            && unit.charges > 0
            && unit.moves_left > 0.0
            && unit.linked_to.is_none()
            && self.district_contribution_target(pid, cid) == Some(unit.pos)
    }

    /// Gathering Storm Military Engineers spend one charge to add 20% of an
    /// Aqueduct, Dam, or Canal's cost. Workshop of the World doubles this to
    /// 40% for England. Contributions may finish the district immediately and
    /// preserve ordinary base-production overflow.
    pub(super) fn do_contribute_district(
        &mut self,
        pid: usize,
        uid: u32,
        cid: u32,
    ) -> Result<(), String> {
        if !self.can_contribute_district(pid, uid, cid) {
            return Err("military engineer cannot contribute to that district".into());
        }
        let item = self.cities[&cid].queue[0].clone();
        let cost = self.item_cost_for_city(pid, cid, &item);
        let percent = if self.players[pid].civ == "England" {
            40.0
        } else {
            20.0
        };
        self.cities.get_mut(&cid).unwrap().production += cost * percent / 100.0;
        {
            let engineer = self.units.get_mut(&uid).unwrap();
            engineer.charges -= 1;
            engineer.moves_left = 0.0;
            engineer.acted = true;
            engineer.moved = true;
        }

        if self.cities[&cid].production + f64::EPSILON >= cost
            && self.complete_item(pid, cid, &item)
        {
            let city = self.cities.get_mut(&cid).unwrap();
            city.production = (city.production - cost).max(0.0);
            if city.queue.first() == Some(&item) {
                city.queue.remove(0);
            }
        }
        if self.units.get(&uid).is_some_and(|unit| unit.charges <= 0) {
            self.remove_unit(uid);
        }
        Ok(())
    }

    pub(super) fn rock_concert_venue_details(
        &self,
        pid: usize,
        band: Option<&Unit>,
        position: Pos,
    ) -> Option<(f64, i32, u32)> {
        if let Some(unit) = band {
            if self.players[pid]
                .counters
                .get(&format!(
                    "rock_concert:{}:{},{}",
                    unit.id, position.0, position.1
                ))
                .copied()
                .unwrap_or(0)
                > 0
            {
                return None;
            }
        }
        let tile = self.map.get(position)?;
        let city = tile
            .owner_city
            .and_then(|city_id| self.cities.get(&city_id))?;
        if city.owner == pid || self.players[city.owner].is_minor || self.is_at_war(pid, city.owner)
        {
            return None;
        }
        let effect = |name| band.map_or(0.0, |unit| self.promotion_effect(unit, name));
        if tile.wonder.is_some() {
            return Some((1_000.0, effect("rock_wonder_levels") as i32, city.id));
        }
        let natural_wonder = tile.feature.as_ref().is_some_and(|feature| {
            self.rules
                .features
                .get(feature)
                .is_some_and(|spec| spec.natural_wonder)
        });
        if (natural_wonder || tile.improvement.as_deref() == Some("national_park"))
            && effect("rock_nature_venue") > 0.0
        {
            return Some((1_000.0, effect("rock_nature_levels") as i32, city.id));
        }
        if !tile.pillaged
            && tile.improvement.as_deref() == Some("seaside_resort")
            && effect("rock_surf_venue") > 0.0
        {
            return Some((500.0, effect("rock_surf_levels") as i32, city.id));
        }
        let district = tile.district?;
        if tile.pillaged {
            return None;
        }
        let family = self.district_family(district);
        let building_tourism = city
            .buildings
            .iter()
            .filter(|building| !city.pillaged_buildings.contains(*building))
            .filter(|building| {
                self.rules.buildings[building]
                    .district
                    .is_some_and(|family| self.district_is_family(district, family))
            })
            .filter_map(|building| {
                self.rules.buildings[building]
                    .effects
                    .get("rock_concert_tourism")
                    .copied()
            })
            .fold(0.0_f64, f64::max);
        let (base, level_bonus) = match family.as_str() {
            "entertainment_complex" if building_tourism > 0.0 => {
                (building_tourism, effect("rock_entertainment_levels"))
            }
            "theater_square" if building_tourism > 0.0 => {
                (building_tourism, effect("rock_theater_levels"))
            }
            "water_park" if building_tourism > 0.0 => {
                (building_tourism, effect("rock_water_park_levels"))
            }
            "campus" if effect("rock_space_venue") > 0.0 => {
                (500.0 + building_tourism, effect("rock_space_levels"))
            }
            "spaceport" if effect("rock_space_venue") > 0.0 => (500.0, effect("rock_space_levels")),
            "harbor" if effect("rock_surf_venue") > 0.0 => {
                (500.0 + building_tourism, effect("rock_surf_levels"))
            }
            _ => return None,
        };
        Some((base, level_bonus as i32, city.id))
    }

    pub(super) fn rock_performance_weights(effective_level: i32) -> [f64; 6] {
        match effective_level.clamp(1, 6) {
            1 => [18.4, 26.5, 26.5, 18.4, 8.2, 2.0],
            2 => [12.1, 22.3, 26.2, 22.3, 12.1, 4.9],
            3 => [7.6, 17.0, 24.5, 24.5, 17.0, 9.4],
            4 => [4.2, 11.6, 21.4, 25.1, 21.4, 16.3],
            5 => [1.9, 7.4, 16.7, 24.1, 24.1, 25.9],
            _ => [0.5, 4.2, 11.6, 21.3, 25.0, 37.5],
        }
    }

    pub(super) fn rock_performance_tier_for_roll(effective_level: i32, roll: f64) -> u8 {
        let mut cumulative = 0.0;
        for (index, weight) in Self::rock_performance_weights(effective_level)
            .into_iter()
            .enumerate()
        {
            cumulative += weight;
            if roll < cumulative {
                return index as u8 + 1;
            }
        }
        6
    }

    pub(crate) fn rock_concert_ai_value(&self, pid: usize, uid: u32, position: Pos) -> Option<f64> {
        let unit = self.units.get(&uid)?;
        if unit.owner != pid || unit.kind != "rock_band" || unit.promotions.is_empty() {
            return None;
        }
        let (base, venue_levels, _) = self.rock_concert_venue_details(pid, Some(unit), position)?;
        let effective_level = (unit.level + venue_levels).clamp(1, 6);
        let bombs = [-25.0, 100.0, -25.0, 150.0, 0.0, 200.0];
        let weights = Self::rock_performance_weights(effective_level);
        let album_multiplier = unit.album_sales as f64 / 100.0;
        let expected_multiplier = weights
            .iter()
            .zip(bombs)
            .map(|(probability, bomb)| {
                probability / 100.0 * (1.0 + bomb / 100.0 + album_multiplier)
            })
            .sum::<f64>();
        let survival = 1.0 - (weights[0] + weights[1]) / 100.0;
        Some(base * (expected_multiplier + 2.0 * survival))
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

    pub(super) fn add_targeted_tourism(&mut self, pid: usize, target: usize, tourism: f64) {
        if tourism <= 0.0 || pid == target {
            return;
        }
        let pressure = self.tourism_pressure_against(pid, target)
            + tourism * self.international_tourism_multiplier(pid, target, false);
        self.players[pid].tourism_lifetime += tourism;
        *self.players[pid]
            .targeted_tourism
            .entry(target)
            .or_insert(0.0) += tourism;
        self.players[pid].tourism_pressure.insert(target, pressure);
    }

    pub(super) fn do_perform_concert(&mut self, pid: usize, uid: u32) -> Result<(), String> {
        let band = self
            .units
            .get(&uid)
            .filter(|unit| {
                unit.owner == pid
                    && unit.kind == "rock_band"
                    && !unit.promotions.is_empty()
                    && unit.moves_left > 0.0
            })
            .cloned()
            .ok_or_else(|| "rock band cannot perform now".to_string())?;
        let position = band.pos;
        let (venue_tourism, venue_levels, host_city) = self
            .rock_concert_venue_details(pid, Some(&band), position)
            .ok_or_else(|| "rock band has no valid concert venue here".to_string())?;
        let host = self.cities[&host_city].owner;
        let effective_level = (band.level + venue_levels).clamp(1, 6);
        let tier =
            Self::rock_performance_tier_for_roll(effective_level, self.rng.uniform(0.0, 100.0));
        let (tourism_bomb, album_sales, promotes, retires) = match tier {
            1 => (-25.0, 0, false, true),
            2 => (100.0, 0, false, true),
            3 => (-25.0, 50, false, false),
            4 => (150.0, 100, false, false),
            5 => (0.0, 150, true, false),
            _ => (200.0, 200, true, false),
        };
        let tourism =
            venue_tourism * (1.0 + tourism_bomb / 100.0 + band.album_sales as f64 / 100.0);
        self.add_targeted_tourism(pid, host, tourism);
        self.players[pid].counters.insert(
            format!("rock_concert:{uid}:{},{}", position.0, position.1),
            1,
        );
        self.players[pid]
            .counters
            .insert(format!("rock_concert_tier:{uid}"), tier as i64);
        self.first_historic_moment(
            pid,
            "rock_concert",
            Some("MOMENT_UNIT_TOURISM_BOMB"),
            Some("MOMENT_UNIT_TOURISM_BOMB_FIRST_IN_WORLD"),
        );

        let nearby_pct = self.promotion_effect(&band, "rock_nearby_tourism_pct") / 100.0;
        if nearby_pct > 0.0 {
            let nearby: BTreeSet<usize> = self
                .cities
                .values()
                .filter(|city| {
                    city.owner != pid
                        && city.owner != host
                        && self.players[city.owner].alive
                        && !self.players[city.owner].is_minor
                        && !self.players[city.owner].is_barbarian
                        && self.wdist(position, city.pos) <= 10
                })
                .map(|city| city.owner)
                .collect();
            for target in nearby {
                self.add_targeted_tourism(pid, target, tourism * nearby_pct);
            }
        }
        self.players[pid].gold += tourism * self.promotion_effect(&band, "rock_gold_pct") / 100.0;
        let loyalty_loss = self.promotion_effect(&band, "rock_loyalty_loss");
        if loyalty_loss > 0.0 {
            let loyalty = (self.cities[&host_city].loyalty - loyalty_loss).max(0.0);
            self.cities.get_mut(&host_city).unwrap().loyalty = loyalty;
        }
        let converts_religion = self.promotion_effect(&band, "rock_convert_city") > 0.0;
        if converts_religion {
            if let Some(religion) = self.players[pid].religion.clone() {
                // The promotion converts the host city outright, so the
                // religion must clear every rival faith and the city's
                // atheists put together rather than only the largest rival.
                let city = &self.cities[&host_city];
                let competing = city
                    .pressure
                    .iter()
                    .filter(|(faith, _)| **faith != religion)
                    .map(|(_, pressure)| pressure.max(0.0))
                    .sum::<f64>()
                    + city.atheist_pressure.max(0.0);
                self.cities
                    .get_mut(&host_city)
                    .unwrap()
                    .pressure
                    .insert(religion, competing + 1.0);
            }
        }
        if converts_religion {
            self.check_religious_victory();
        }

        if retires {
            self.remove_unit(uid);
        } else {
            let unit = self.units.get_mut(&uid).unwrap();
            unit.album_sales += album_sales;
            unit.moves_left = 0.0;
            unit.acted = true;
            if promotes && unit.level < 4 {
                unit.level += 1;
                unit.xp = Self::promotion_threshold(unit.level);
            }
        }
        Ok(())
    }

    pub(crate) fn pillageable_at(&self, pid: usize, pos: Pos) -> bool {
        self.pillageable_at_with(pid, pos, true)
    }

    /// `pillageable_at` with the state of war assumed: what a declaration on
    /// the tile's owner would put on the table. The advanced controller's
    /// opportunistic war prices a surprise war on this before opening it.
    pub(crate) fn pillageable_after_declaring(&self, pid: usize, pos: Pos) -> bool {
        self.pillageable_at_with(pid, pos, false)
    }

    pub(super) fn pillageable_at_with(&self, pid: usize, pos: Pos, require_war: bool) -> bool {
        let Some(tile) = self.map.get(pos) else {
            return false;
        };
        if tile.improvement.as_deref() == Some("barbarian_camp") {
            return Some(pid) != self.barb_pid;
        }
        let Some(cid) = tile.owner_city else {
            return false;
        };
        let Some(city) = self.cities.get(&cid) else {
            return false;
        };
        if city.owner == pid
            || (require_war && !self.is_at_war(pid, city.owner))
            || self.city_at(pos).is_some()
        {
            return false;
        }
        if let Some(improvement) = tile.improvement.as_deref() {
            return !tile.pillaged
                && self
                    .rules
                    .improvements
                    .get(improvement)
                    .is_some_and(|spec| spec.unit_pillageable);
        }
        let Some(district) = tile.district else {
            return false;
        };
        if self.district_is_family(district, crate::name!("encampment"))
            || self.unit_ids_at(pos).iter().any(|id| {
                self.units[id].owner == city.owner
                    && self.rules.units[self.units[id].kind].class == "military"
                    && self.rules.units[self.units[id].kind].domain.as_deref() != Some("air")
            })
        {
            return false;
        }
        if !tile.pillaged {
            return true;
        }
        city.buildings.iter().any(|building| {
            !city.pillaged_buildings.contains(building)
                && self.rules.buildings.get(building).is_some_and(|spec| {
                    spec.district
                        .as_ref()
                        .is_some_and(|family| self.district_is_family(district, family))
                })
        })
    }

    /// Air Pillage follows the normal ownership, war, garrison, and remaining
    /// layer rules, but barbarian camps are captured by entering their tile
    /// rather than bombed for the camp-clear reward.
    pub(super) fn air_pillageable_at(&self, pid: usize, pos: Pos) -> bool {
        self.map
            .get(pos)
            .is_some_and(|tile| tile.improvement.as_deref() != Some("barbarian_camp"))
            && self.pillageable_at(pid, pos)
    }

    pub(super) fn scaled_pillage_amount(&self, pid: usize, yield_type: &str, base: f64) -> f64 {
        if yield_type == "heal" {
            return base;
        }
        self.game_speed.scale(base)
            * (self.world_era as f64 + 1.0)
            * (1.0 + self.policy_effect(pid, "pillage_yield_pct") / 100.0)
    }

    pub(super) fn grant_pillage_yield(
        &mut self,
        pid: usize,
        uid: u32,
        yield_type: &str,
        base: f64,
    ) {
        let amount = self.scaled_pillage_amount(pid, yield_type, base);
        match yield_type {
            "" | "none" => {}
            "heal" => {
                if let Some(unit) = self.units.get_mut(&uid) {
                    unit.hp = (unit.hp + amount.round() as i32).min(100);
                }
            }
            "science" => self.players[pid].research_overflow += amount,
            "faith" => self.players[pid].faith += amount,
            "culture" => self.players[pid].civic_overflow += amount,
            "gold" => self.players[pid].gold += amount,
            unknown => panic!("unknown pillage yield {unknown:?}"),
        }
    }

    pub(super) fn grant_pillage_reward(
        &mut self,
        pid: usize,
        uid: u32,
        source: &str,
        improvement: bool,
        coastal: bool,
    ) {
        let (yield_type, base, bonuses) = if improvement {
            let spec = &self.rules.improvements[source];
            (
                spec.plunder_type.clone().unwrap_or_default(),
                spec.plunder_amount,
                spec.bonus_pillage.clone(),
            )
        } else {
            let family = self.district_family(Name::new(source));
            let spec = &self.rules.districts[family];
            (
                spec.plunder_type.clone().unwrap_or_default(),
                spec.plunder_amount,
                BTreeMap::new(),
            )
        };
        self.grant_pillage_yield(pid, uid, &yield_type, base);
        for (ability, bonus) in bonuses {
            if self.has_ability(pid, &ability) {
                self.grant_pillage_yield(pid, uid, &bonus.yield_type, bonus.amount);
            }
        }

        let chapel_effect = if improvement {
            "pillage_improvement_faith"
        } else {
            "pillage_district_faith"
        };
        let chapel_faith = self
            .cities
            .values()
            .filter(|city| city.owner == pid)
            .map(|city| self.city_building_effect(city, chapel_effect))
            .sum::<f64>();
        if chapel_faith > 0.0 {
            self.grant_pillage_yield(pid, uid, "faith", chapel_faith);
        }

        // Loot is a flat Standard-speed +50 Gold from every coastal raid; it
        // is not a percentage of a Gold reward and also applies to heal raids.
        if coastal {
            let loot = self.promotion_effect(&self.units[&uid], "coastal_raid_gold");
            self.players[pid].gold += self.game_speed.scale(loot);
        }
    }

    pub(super) fn pillage_tile(
        &mut self,
        pid: usize,
        uid: u32,
        pos: Pos,
        coastal: bool,
        award_spoils: bool,
    ) -> Result<(), String> {
        if !self.pillageable_at(pid, pos) {
            return Err("nothing pillageable there".into());
        }
        // `pillages`: tiles and district layers this seat has pillaged,
        // barbarian camps included. The same evaluator reading as the
        // capture counters above.
        bump(&mut self.players[pid], "pillages");
        let enemy = self.map.tiles[&pos]
            .owner_city
            .and_then(|city| self.cities.get(&city))
            .map(|city| city.owner);
        if let Some(enemy) = enemy {
            let participant = self.units[&uid].clone();
            self.record_war_unit_participation(&participant, enemy);
        }
        if self.map.tiles[&pos].improvement.as_deref() == Some("barbarian_camp") {
            return self
                .clear_barbarian_camp(uid, pos, coastal && award_spoils)
                .then_some(())
                .ok_or_else(|| "barbarian camp is no longer active".to_string());
        }
        let (source, improvement) = if let Some(improvement) = self.map.tiles[&pos].improvement {
            self.map.tiles.get_mut(&pos).unwrap().pillaged = true;
            (improvement, true)
        } else {
            let district = self.map.tiles[&pos].district.unwrap();
            let cid = self.map.tiles[&pos].owner_city.unwrap();
            let building = self.cities[&cid]
                .buildings
                .iter()
                .filter(|building| !self.cities[&cid].pillaged_buildings.contains(*building))
                .filter(|building| {
                    self.rules.buildings[building]
                        .district
                        .as_ref()
                        .is_some_and(|family| self.district_is_family(district, family))
                })
                .max_by(|a, b| {
                    self.rules.buildings[a]
                        .cost
                        .partial_cmp(&self.rules.buildings[b].cost)
                        .unwrap()
                        .then(a.cmp(b))
                })
                .cloned();
            if let Some(building) = building {
                self.cities
                    .get_mut(&cid)
                    .unwrap()
                    .pillaged_buildings
                    .insert(building);
                (district, false)
            } else if !self.map.tiles[&pos].pillaged {
                self.map.tiles.get_mut(&pos).unwrap().pillaged = true;
                (district, false)
            } else {
                return Err("district is already fully pillaged".to_string());
            }
        };
        self.scatter_aircraft_from(pos);
        if award_spoils {
            self.grant_pillage_reward(pid, uid, &source, improvement, coastal);
        }
        Ok(())
    }

    pub(super) fn do_pillage(&mut self, pid: usize, uid: u32) -> Result<(), String> {
        let unit = self.own_unit(pid, uid)?;
        let spec = &self.rules.units[unit.kind];
        if spec.class != "military"
            || spec.domain.as_deref() == Some("air")
            || self.is_embarked(&unit)
            || unit.moves_left <= 0.0
        {
            return Err("unit cannot pillage".into());
        }
        self.pillage_tile(pid, uid, unit.pos, false, true)?;
        let cost = if self.promotion_effect(&unit, "pillage_cost") > 0.0 {
            1.0
        } else {
            3.0
        };
        let unit = self.units.get_mut(&uid).unwrap();
        unit.moves_left = (unit.moves_left - cost).max(0.0);
        unit.acted = true;
        Ok(())
    }

    pub(super) fn do_repair_improvement(&mut self, pid: usize, uid: u32) -> Result<(), String> {
        let builder = self.own_unit(pid, uid)?;
        let tile = self
            .map
            .get(builder.pos)
            .ok_or_else(|| "unit is off map".to_string())?;
        if builder.kind != "builder"
            || builder.moves_left <= 0.0
            || !tile.pillaged
            || tile.improvement.is_none()
            || tile
                .owner_city
                .and_then(|cid| self.cities.get(&cid))
                .is_none_or(|city| !self.builder_may_improve_territory(pid, city.owner))
        {
            return Err("builder cannot repair this improvement".into());
        }
        self.map.tiles.get_mut(&builder.pos).unwrap().pillaged = false;
        let builder = self.units.get_mut(&uid).unwrap();
        builder.moves_left = 0.0;
        builder.acted = true;
        Ok(())
    }

    pub(super) fn do_coastal_raid(
        &mut self,
        pid: usize,
        uid: u32,
        target: Pos,
    ) -> Result<(), String> {
        let unit = self.own_unit(pid, uid)?;
        if !self.can_coastal_raid(pid, &unit)
            || unit.moves_left <= 0.0
            || self.wdist(unit.pos, target) != 1
            || self
                .map
                .get(target)
                .is_none_or(|tile| self.rules.is_water(tile))
        {
            return Err("unit cannot coastal raid that tile".into());
        }
        self.pillage_tile(pid, uid, target, true, true)?;
        let unit = self.units.get_mut(&uid).unwrap();
        unit.moves_left = (unit.moves_left - 3.0).max(0.0);
        unit.acted = true;
        Ok(())
    }

    pub(super) fn can_coastal_raid(&self, pid: usize, unit: &Unit) -> bool {
        let spec = &self.rules.units[unit.kind];
        spec.promotion_class == "naval_raider"
            || (spec.promotion_class == "naval_melee" && self.has_ability(pid, "knarr"))
    }

    pub(super) fn air_capacity_at(&self, pid: usize, pos: Pos) -> i32 {
        let mut capacity = 0;
        if let Some(cid) = self.city_at(pos) {
            let city = &self.cities[&cid];
            if city.owner == pid {
                capacity += 1;
            }
        }
        if let Some(tile) = self.map.get(pos) {
            if let Some(city) = tile.owner_city.and_then(|cid| self.cities.get(&cid)) {
                if city.owner == pid {
                    if let Some(district) = tile.district {
                        if self.district_is_family(district, crate::name!("aerodrome"))
                            && !tile.pillaged
                        {
                            capacity += self.rules.districts[district].air_slots;
                            capacity += city
                                .buildings
                                .iter()
                                .filter(|building| {
                                    !city.pillaged_buildings.contains(*building)
                                        && self.building_district_is_active(city, building)
                                })
                                .map(|building| {
                                    self.rules.buildings[building]
                                        .effects
                                        .get("air_slots")
                                        .copied()
                                        .unwrap_or(0.0) as i32
                                })
                                .sum::<i32>();
                        }
                    }
                    if tile.improvement.as_deref() == Some("airstrip") && !tile.pillaged {
                        capacity += self.rules.improvements["airstrip"]
                            .effects
                            .get("air_slots")
                            .copied()
                            .unwrap_or(3.0) as i32;
                    }
                }
            }
        }
        capacity += self
            .unit_ids_at(pos)
            .iter()
            .filter(|id| self.units[id].owner == pid && self.units[id].kind == "aircraft_carrier")
            .map(|id| 2 + self.promotion_effect(&self.units[id], "aircraft_slots") as i32)
            .sum::<i32>();
        capacity
    }

    pub(super) fn air_units_at(&self, pid: usize, pos: Pos) -> i32 {
        self.unit_ids_at(pos)
            .iter()
            .filter(|id| {
                self.units[id].owner == pid
                    && self.rules.units[self.units[id].kind].domain.as_deref() == Some("air")
            })
            .count() as i32
    }

    pub(super) fn can_air_base_at(&self, pid: usize, pos: Pos, moving: Option<u32>) -> bool {
        let occupied = self.air_units_at(pid, pos)
            - moving
                .and_then(|uid| self.units.get(&uid))
                .is_some_and(|unit| unit.pos == pos) as i32;
        self.air_capacity_at(pid, pos) > occupied
    }

    /// A pillaged base loses every aircraft slot immediately. Aircraft beyond
    /// the remaining capacity force-rebase to the nearest valid base inside
    /// their ordinary rebase radius; if none exists, they are destroyed.
    pub(super) fn scatter_aircraft_from(&mut self, pos: Pos) {
        let mut aircraft: Vec<u32> = self
            .unit_ids_at(pos)
            .iter()
            .filter(|unit| self.rules.units[self.units[unit].kind].domain.as_deref() == Some("air"))
            .copied()
            .collect();
        aircraft.sort_unstable();
        let retained = aircraft
            .first()
            .map(|unit| self.air_capacity_at(self.units[unit].owner, pos).max(0) as usize)
            .unwrap_or(0);
        for uid in aircraft.into_iter().skip(retained) {
            if !self.units.contains_key(&uid) {
                continue;
            }
            let owner = self.units[&uid].owner;
            let range = self.air_rebase_range(uid);
            let mut bases: Vec<Pos> = self
                .map
                .tiles
                .keys()
                .copied()
                .filter(|base| *base != pos && self.wdist(pos, *base) <= range)
                .filter(|base| self.can_air_base_at(owner, *base, Some(uid)))
                .collect();
            bases.sort_by_key(|base| (self.wdist(pos, *base), *base));
            if let Some(base) = bases.first().copied() {
                self.relocate(uid, base);
                let unit = self.units.get_mut(&uid).unwrap();
                unit.moves_left = 0.0;
                unit.attacks_left = 0;
                unit.acted = true;
                unit.air_patrol = false;
                unit.air_patrol_pos = None;
            } else {
                self.remove_unit(uid);
                self.on_unit_lost(owner);
            }
        }
    }

    pub(super) fn do_air_rebase(&mut self, pid: usize, uid: u32, to: Pos) -> Result<(), String> {
        let unit = self.own_unit(pid, uid)?;
        let spec = &self.rules.units[unit.kind];
        if spec.domain.as_deref() != Some("air")
            || unit.moves_left <= 0.0
            || unit.pos == to
            || self.wdist(unit.pos, to) > self.air_rebase_range(uid)
            || !self.can_air_base_at(pid, to, Some(uid))
        {
            return Err("aircraft cannot rebase there".into());
        }
        self.relocate(uid, to);
        let aircraft = self.units.get_mut(&uid).unwrap();
        aircraft.moves_left = 0.0;
        aircraft.attacks_left = 0;
        aircraft.acted = true;
        aircraft.air_patrol = false;
        aircraft.air_patrol_pos = None;
        Ok(())
    }

    pub(super) fn unit_anti_air_strength(&self, unit: &Unit) -> f64 {
        let spec = &self.rules.units[unit.kind];
        let governor = if spec.class == "support" {
            self.map.tiles[&unit.pos]
                .owner_city
                .filter(|city_id| {
                    self.cities
                        .get(city_id)
                        .is_some_and(|city| city.owner == unit.owner)
                })
                .map(|city_id| self.governor_effect(unit.owner, city_id, "air_defense"))
                .unwrap_or(0.0)
        } else {
            0.0
        };
        // Since the Maya & Gran Colombia update, both formation tiers add a
        // flat +7 Anti-Air Strength rather than their +10/+17 combat bonus.
        spec.anti_air_strength
            + if unit.formation > 0 { 7.0 } else { 0.0 }
            + if spec.class == "military" {
                // Intercepting an incoming strike is defending.
                self.government_combat_bonus(unit, false)
                    + self.congress_military_strength_bonus(unit)
            } else {
                0.0
            }
            + if unit.kind == "giant_death_robot"
                && self.tree_effect(unit.owner, "gdr_air_defense") > 0.0
            {
                self.tree_effect(unit.owner, "gdr_anti_air_strength")
            } else {
                0.0
            }
            + governor
    }

    pub(super) fn air_matchup_bonus(&self, unit: &Unit, opponent: &Unit) -> f64 {
        match self.rules.units[opponent.kind].promotion_class.as_str() {
            "air_fighter" => self.promotion_effect(unit, "vs_air_fighter"),
            "air_bomber" => self.promotion_effect(unit, "vs_air_bomber"),
            _ => 0.0,
        }
    }

    pub(super) fn air_strike_unit_bonus(&self, attacker: &Unit, defender: &Unit) -> f64 {
        let attacker_spec = &self.rules.units[attacker.kind];
        let defender_spec = &self.rules.units[defender.kind];
        let mut bonus = self.air_matchup_bonus(attacker, defender);
        if defender_spec.domain.as_deref() == Some("sea") {
            bonus += self.promotion_effect(attacker, "air_vs_naval");
        } else if defender_spec.domain.as_deref() != Some("air") {
            bonus += self.promotion_effect(attacker, "air_vs_land");
        }
        if attacker_spec.promotion_class == "air_fighter"
            && matches!(
                defender_spec.promotion_class.as_str(),
                "light_cavalry" | "heavy_cavalry"
            )
        {
            bonus += self.promotion_effect(attacker, "vs_cavalry");
        } else if attacker_spec.promotion_class == "air_fighter"
            && defender_spec.domain.as_deref() != Some("air")
        {
            bonus += self.promotion_effect(attacker, "vs_non_cavalry");
        }
        bonus
    }

    pub(super) fn air_strike_unit_strength(&self, attacker: &Unit, defender: &Unit) -> f64 {
        let attacker_spec = &self.rules.units[attacker.kind];
        let defender_spec = &self.rules.units[defender.kind];
        self.unit_ranged_attack_strength(attacker) + self.air_strike_unit_bonus(attacker, defender)
            - if (attacker_spec.bombard_strength > 0.0
                && defender_spec.domain.as_deref() != Some("sea"))
                || (attacker_spec.ranged_strength > 0.0
                    && defender_spec.domain.as_deref() == Some("sea"))
            {
                17.0
            } else {
                0.0
            }
    }

    /// Strongest single interception layer at a target. Kept as a read-only
    /// diagnostic for rules tests and combat previews; mission resolution
    /// below can apply both the non-air and fighter layers independently.
    #[cfg(test)]
    pub(super) fn air_interception_strength(&self, attacker: &Unit, target: Pos) -> f64 {
        self.units
            .values()
            .filter(|unit| {
                unit.owner != attacker.owner && self.is_at_war(attacker.owner, unit.owner)
            })
            .filter_map(|unit| {
                let spec = &self.rules.units[unit.kind];
                if spec.domain.as_deref() == Some("air")
                    && spec.promotion_class == "air_fighter"
                    && unit.air_patrol
                    && unit
                        .air_patrol_pos
                        .is_some_and(|patrol| self.wdist(patrol, target) <= 1)
                {
                    Some(
                        self.unit_unembarked_strength(unit)
                            + self.air_matchup_bonus(unit, attacker),
                    )
                } else if spec.domain.as_deref() != Some("air")
                    && spec.anti_air_strength > 0.0
                    && self.wdist(unit.pos, target) <= spec.anti_air_range.max(1)
                {
                    Some(self.unit_anti_air_strength(unit))
                } else {
                    None
                }
            })
            .fold(0.0, f64::max)
    }

    /// Resolve the strongest non-air interceptor and the strongest patrolling
    /// fighter separately. Civ VI permits one interception from each category
    /// on the same mission; adjacent same-category defenses add +5 support.
    /// Returns whether the attacker was destroyed and whether a fighter-class
    /// attacker was diverted into a dogfight instead of reaching its target.
    pub(super) fn resolve_air_interceptions(
        &mut self,
        attacker_id: u32,
        target: Pos,
    ) -> (bool, bool) {
        let attacker = self.units[&attacker_id].clone();
        let mut ground: Vec<(f64, u32)> = Vec::new();
        let mut fighters: Vec<(f64, u32)> = Vec::new();
        for unit in self.units.values().filter(|unit| {
            unit.owner != attacker.owner && self.is_at_war(attacker.owner, unit.owner)
        }) {
            let spec = &self.rules.units[unit.kind];
            if spec.domain.as_deref() == Some("air")
                && spec.promotion_class == "air_fighter"
                && unit.air_patrol
                && unit
                    .air_patrol_pos
                    .is_some_and(|patrol| self.wdist(patrol, target) <= 1)
            {
                let support = self
                    .units
                    .values()
                    .filter(|other| {
                        other.id != unit.id
                            && other.owner == unit.owner
                            && other.air_patrol
                            && self.rules.units[other.kind].promotion_class == "air_fighter"
                            && other
                                .air_patrol_pos
                                .is_some_and(|patrol| self.wdist(patrol, target) == 1)
                    })
                    .count() as f64
                    * 5.0;
                fighters.push((
                    self.unit_unembarked_strength(unit)
                        + self.air_matchup_bonus(unit, &attacker)
                        + support,
                    unit.id,
                ));
            } else if spec.domain.as_deref() != Some("air")
                && spec.anti_air_strength > 0.0
                && self.wdist(unit.pos, target) <= spec.anti_air_range.max(1)
            {
                let support = self
                    .units
                    .values()
                    .filter(|other| {
                        other.id != unit.id
                            && other.owner == unit.owner
                            && self.rules.units[other.kind].domain.as_deref() != Some("air")
                            && self.rules.units[other.kind].anti_air_strength > 0.0
                            && self.wdist(other.pos, target) == 1
                    })
                    .count() as f64
                    * 5.0;
                ground.push((self.unit_anti_air_strength(unit) + support, unit.id));
            }
        }
        ground.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        fighters.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));

        let mut fighter_engaged = false;
        for (strength, interceptor_id, air_interceptor) in ground
            .first()
            .map(|(strength, id)| (*strength, *id, false))
            .into_iter()
            .chain(
                fighters
                    .first()
                    .map(|(strength, id)| (*strength, *id, true)),
            )
        {
            if !self.units.contains_key(&attacker_id) {
                return (true, fighter_engaged);
            }
            let current_attacker = self.units[&attacker_id].clone();
            let interceptor = self.units[&interceptor_id].clone();
            self.record_war_unit_participation(&current_attacker, interceptor.owner);
            self.record_war_unit_participation(&interceptor, current_attacker.owner);
            let mut defense = self.unit_unembarked_strength(&current_attacker);
            if air_interceptor {
                defense += self.promotion_effect(&current_attacker, "defend_air_fighter")
                    + self.air_matchup_bonus(&current_attacker, &self.units[&interceptor_id]);
            } else {
                defense += self.promotion_effect(&current_attacker, "defend_anti_air");
            }
            let incoming = damage(
                effective_strength(strength, self.units[&interceptor_id].hp),
                effective_strength(defense.max(1.0), current_attacker.hp),
                &mut self.rng,
            );

            if air_interceptor {
                self.units.get_mut(&interceptor_id).unwrap().acted = true;
            }

            if air_interceptor
                && self.rules.units[current_attacker.kind].promotion_class == "air_fighter"
            {
                fighter_engaged = true;
                let counter_strength = self.unit_unembarked_strength(&current_attacker)
                    + self.air_matchup_bonus(&current_attacker, &interceptor);
                let interceptor_defense = self.unit_unembarked_strength(&interceptor)
                    + self.air_matchup_bonus(&interceptor, &current_attacker);
                let counter = damage(
                    effective_strength(counter_strength, current_attacker.hp),
                    effective_strength(interceptor_defense, interceptor.hp),
                    &mut self.rng,
                );
                self.apply_unit_damage(attacker_id, interceptor_id, counter);
                if self.units[&interceptor_id].hp <= 0 {
                    let owner = self.units[&interceptor_id].owner;
                    self.remove_unit(interceptor_id);
                    self.on_unit_lost(owner);
                }
            }

            self.apply_unit_damage(interceptor_id, attacker_id, incoming);
            if self.units[&attacker_id].hp <= 0 {
                self.remove_unit(attacker_id);
                self.on_unit_lost(attacker.owner);
                return (true, fighter_engaged);
            }
        }
        (false, fighter_engaged)
    }

    pub(super) fn priority_damage_support(&mut self, pid: usize, uid: u32, defender_id: u32) {
        let attacker = self.units[&uid].clone();
        let defender = self.units[&defender_id].clone();
        self.record_war_unit_participation(&attacker, defender.owner);
        self.record_war_unit_participation(&defender, attacker.owner);
        self.apply_unit_damage(uid, defender_id, 65);
        let killed = self.units[&defender_id].hp <= 0;
        self.record_emergency_combat(pid, defender.owner, killed);
        self.award_unit_combat_xp(uid, &defender, true, true, killed);
        if killed {
            bump(&mut self.players[pid], "kills");
            self.remove_unit(defender_id);
            self.on_unit_lost(defender.owner);
        }
    }

    pub(super) fn do_air_strike(
        &mut self,
        pid: usize,
        uid: u32,
        target: Pos,
    ) -> Result<(), String> {
        let attacker = self.own_unit(pid, uid)?;
        let spec = self.rules.units[attacker.kind].clone();
        if spec.domain.as_deref() != Some("air")
            || attacker.moves_left <= 0.0
            || attacker.attacks_left <= 0
            || self.wdist(self.air_operation_origin(uid), target) > self.unit_attack_range(uid)
            || !self.enemy_air_strike_target_at(pid, target)
        {
            return Err("invalid air strike".into());
        }
        let visible = self.player_vision_frame(pid);
        let viewers = self.visibility_viewers(pid);
        if !self.combat_target_visible_at(pid, target, visible.as_ref(), &viewers) {
            return Err("invalid air strike".into());
        }
        let (destroyed, fighter_engaged) = self.resolve_air_interceptions(uid, target);
        if destroyed {
            return Ok(());
        }
        if fighter_engaged {
            self.consume_unit_attack(uid);
            if let Some(aircraft) = self.units.get_mut(&uid) {
                aircraft.air_patrol = false;
                aircraft.air_patrol_pos = None;
            }
            return Ok(());
        }
        let district_attack = effective_strength(
            self.unit_ranged_attack_strength(&self.units[&uid])
                - if spec.ranged_strength > 0.0 {
                    17.0
                } else {
                    0.0
                },
            self.units[&uid].hp,
        );
        if let Some(cid) = self.city_at(target) {
            if self.cities[&cid].owner != pid && self.is_at_war(pid, self.cities[&cid].owner) {
                self.record_war_unit_participation(&attacker, self.cities[&cid].owner);
                self.record_war_city_garrison_participation(cid, attacker.owner);
                let dealt = damage(district_attack, self.city_strength(cid), &mut self.rng);
                self.city_take_damage(pid, cid, dealt, if spec.siege { 1.0 } else { 0.5 }, false);
                if self.cities[&cid].hp <= 0 {
                    self.cities.get_mut(&cid).unwrap().hp = 1;
                }
            }
        } else if let Some(cid) = self.encampment_at(target) {
            if self.cities[&cid].owner != pid && self.is_at_war(pid, self.cities[&cid].owner) {
                self.record_war_unit_participation(&attacker, self.cities[&cid].owner);
                let dealt = damage(
                    district_attack,
                    self.encampment_strength(cid),
                    &mut self.rng,
                );
                let effectiveness = if spec.siege || self.gdr_full_wall_damage(&attacker) {
                    1.0
                } else {
                    0.5
                };
                self.encampment_take_damage(pid, cid, dealt, effectiveness, false);
                if self.cities[&cid].encampment_hp <= 0 {
                    self.cities.get_mut(&cid).unwrap().encampment_hp = 1;
                }
            }
        } else if let Some(defender_id) = self.units_at(target).into_iter().find(|id| {
            self.units[id].owner != pid
                && self.is_at_war(pid, self.units[id].owner)
                && self.rules.units[self.units[id].kind].class == "military"
                && self.rules.units[self.units[id].kind].domain.as_deref() != Some("air")
                && self.unit_currently_visible_to(*id, pid)
        }) {
            let defender = self.units[&defender_id].clone();
            self.record_war_unit_participation(&attacker, defender.owner);
            self.record_war_unit_participation(&defender, attacker.owner);
            let anti_air = self.promotion_effect(&defender, "defend_air");
            let defender_spec = &self.rules.units[defender.kind];
            let defense_base = if defender_spec.anti_air_strength > 0.0 {
                self.unit_anti_air_strength(&defender)
            } else {
                self.unit_strength(&defender, true)
            } + anti_air;
            let specialized_attack = effective_strength(
                self.air_strike_unit_strength(&self.units[&uid], &defender),
                self.units[&uid].hp,
            );
            let defense = effective_strength(defense_base, defender.hp);
            let dealt = damage(specialized_attack, defense, &mut self.rng);
            self.apply_unit_damage(uid, defender_id, dealt);
            let killed = self.units[&defender_id].hp <= 0;
            self.record_emergency_combat(pid, defender.owner, killed);
            self.award_unit_combat_xp(uid, &defender, true, true, killed);
            if killed {
                let weapon = self.units[&uid].kind;
                self.note_underdog_kill(pid, &attacker, &defender);
                self.note_great_person_assisted_kill(pid, &attacker);
                self.record_kill(pid, Some(&weapon), &defender);
                self.remove_unit(defender_id);
                self.on_unit_lost(defender.owner);
            }
        } else if let Some(defender_id) = self.unit_ids_at(target).iter().find(|id| {
            self.units[id].owner != pid
                && self.is_at_war(pid, self.units[id].owner)
                && self.rules.units[self.units[id].kind].class == "support"
                && self.unit_currently_visible_to(**id, pid)
        }) {
            self.priority_damage_support(pid, uid, *defender_id);
        }
        self.consume_unit_attack(uid);
        if let Some(aircraft) = self.units.get_mut(&uid) {
            aircraft.air_patrol = false;
            aircraft.air_patrol_pos = None;
        }
        Ok(())
    }

    pub(super) fn do_priority_target(
        &mut self,
        pid: usize,
        uid: u32,
        target: Pos,
    ) -> Result<(), String> {
        let attacker = self.own_unit(pid, uid)?;
        let spec = &self.rules.units[attacker.kind];
        let air = spec.domain.as_deref() == Some("air");
        if !self.unit_has_priority_target(&attacker)
            || attacker.moves_left <= 0.0
            || attacker.attacks_left <= 0
            || (!air && self.is_embarked(&attacker))
            || self.wdist(
                if air {
                    self.air_operation_origin(uid)
                } else {
                    attacker.pos
                },
                target,
            ) > self.unit_attack_range(uid)
            || (!air && !self.unit_has_line_of_sight(uid, target))
        {
            return Err("unit cannot priority target there".into());
        }
        let visible = self.player_vision_frame(pid);
        let viewers = self.visibility_viewers(pid);
        if !self.combat_target_visible_at(pid, target, visible.as_ref(), &viewers) {
            return Err("unit cannot priority target there".into());
        }
        let Some(defender_id) = self.priority_support_target_at(pid, target) else {
            return Err("no escorted support unit to priority target".into());
        };

        if air {
            let (destroyed, fighter_engaged) = self.resolve_air_interceptions(uid, target);
            if destroyed {
                return Ok(());
            }
            if fighter_engaged {
                self.consume_unit_attack(uid);
                if let Some(aircraft) = self.units.get_mut(&uid) {
                    aircraft.air_patrol = false;
                    aircraft.air_patrol_pos = None;
                }
                return Ok(());
            }
        }
        self.priority_damage_support(pid, uid, defender_id);
        self.consume_unit_attack(uid);
        if air {
            let aircraft = self.units.get_mut(&uid).unwrap();
            aircraft.air_patrol = false;
            aircraft.air_patrol_pos = None;
        }
        Ok(())
    }

    pub(super) fn do_air_pillage(
        &mut self,
        pid: usize,
        uid: u32,
        target: Pos,
    ) -> Result<(), String> {
        let bomber = self.own_unit(pid, uid)?;
        let spec = &self.rules.units[bomber.kind];
        if spec.domain.as_deref() != Some("air")
            || spec.promotion_class != "air_bomber"
            || bomber.moves_left <= 0.0
            || bomber.attacks_left <= 0
            || (bomber.hp < 50
                && self.promotion_effect(&bomber, "air_pillage_at_low_health") <= 0.0)
            || self.wdist(self.air_operation_origin(uid), target) > self.unit_attack_range(uid)
            || !self.player_can_see(pid, target)
            || !self.air_pillageable_at(pid, target)
        {
            return Err("invalid air pillage".into());
        }

        let (destroyed, _) = self.resolve_air_interceptions(uid, target);
        if destroyed {
            return Ok(());
        }
        if self.units[&uid].hp < 50
            && self.promotion_effect(&self.units[&uid], "air_pillage_at_low_health") <= 0.0
        {
            // The official operation resolves interception before checking
            // whether an ordinary bomber still has the required 50 HP.
            self.consume_unit_attack(uid);
            return Ok(());
        }
        // This applies exactly one normal pillage layer, including scattering
        // aircraft from a disabled Aerodrome, but Air Pillage awards no loot.
        self.pillage_tile(pid, uid, target, false, false)?;
        self.consume_unit_attack(uid);
        if let Some(aircraft) = self.units.get_mut(&uid) {
            aircraft.air_patrol = false;
            aircraft.air_patrol_pos = None;
        }
        Ok(())
    }

    pub(super) fn do_air_patrol(&mut self, pid: usize, uid: u32, to: Pos) -> Result<(), String> {
        let unit = self.own_unit(pid, uid)?;
        let spec = &self.rules.units[unit.kind];
        if spec.domain.as_deref() != Some("air")
            || spec.siege
            || unit.moves_left <= 0.0
            || unit.attacks_left <= 0
            || self.map.get(to).is_none()
            || self.wdist(unit.pos, to) > spec.moves.floor() as i32
            || !self.can_air_patrol_at(pid, to)
        {
            return Err("aircraft cannot patrol".into());
        }
        let unit = self.units.get_mut(&uid).unwrap();
        unit.air_patrol = true;
        unit.air_patrol_pos = Some(to);
        unit.moves_left = 0.0;
        unit.attacks_left = 0;
        unit.acted = true;
        self.reveal(pid, to, spec.sight);
        Ok(())
    }

    pub(super) fn place_district_foundation(
        &mut self,
        pid: usize,
        cid: u32,
        district: &str,
        pos: Pos,
    ) -> bool {
        if self.map.get(pos).is_some_and(|tile| {
            tile.district_foundation
                .as_ref()
                .is_some_and(|foundation| foundation.district == district)
        }) {
            return true;
        }
        if !self.district_sites(cid, Name::new(district)).contains(&pos) {
            return false;
        }
        let cost = self
            .game_speed
            .scale(self.district_cost_for_placement(pid, district, false));
        let spec = &self.rules.districts[district];
        let preserve_feature = self.players[pid].civ == "Vietnam" && spec.specialty;
        let tile = self.map.tiles.get_mut(&pos).unwrap();
        tile.district_foundation = Some(DistrictFoundation {
            district: Name::new(district),
            cost,
        });
        tile.improvement = None;
        tile.pillaged = false;
        if !preserve_feature {
            tile.feature = None;
        }
        true
    }

    pub(super) fn do_produce(&mut self, pid: usize, cid: u32, item: &Item) -> Result<(), String> {
        match self.cities.get(&cid) {
            Some(c) if c.owner == pid => {}
            _ => return Err("not your city".into()),
        }
        if !self.can_produce(pid, cid, item) {
            return Err("cannot produce that".into());
        }
        if let Item::District { district, pos } = item {
            if !self.place_district_foundation(pid, cid, district, *pos) {
                return Err("district placement failed".into());
            }
        }
        let old = self.cities[&cid].queue.first().cloned();
        if old.as_ref() == Some(item) {
            return Ok(());
        }
        if !self.commit_unit_resource(pid, cid, item) {
            return Err("insufficient strategic resources".into());
        }
        let new_key = Self::item_progress_key(item);
        let city = self.cities.get_mut(&cid).unwrap();
        if let Some(old_item) = old {
            let old_key = Self::item_progress_key(&old_item);
            city.production_progress.insert(old_key, city.production);
            city.production = city.production_progress.remove(&new_key).unwrap_or(0.0);
        } else {
            let overflow = city.production;
            city.production = city.production_progress.remove(&new_key).unwrap_or(0.0) + overflow;
        }
        city.queue = vec![item.clone()];
        Ok(())
    }

    pub(super) fn rock_band_purchase_cost(&self, pid: usize) -> f64 {
        self.game_speed.scale(
            self.rules.units["rock_band"].cost
                + 100.0
                    * self.players[pid]
                        .counters
                        .get("purchased:rock_band")
                        .copied()
                        .unwrap_or(0) as f64,
        )
    }

    pub(crate) fn naturalist_purchase_cost(&self, pid: usize) -> f64 {
        self.game_speed.scale(
            self.rules.units["naturalist"].cost
                + 100.0
                    * self.players[pid]
                        .counters
                        .get("purchased:naturalist")
                        .copied()
                        .unwrap_or(0) as f64,
        )
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

    /// Firaxis buys ordinary land-combat units into the City Center's combat
    /// layer. Unlike production completion, purchase does not search adjacent
    /// tiles: an existing land-combat unit, or one completing from the active
    /// queue, makes the purchase button refuse with "Too many units of the
    /// same class in this location." Keep this deliberately scoped to the
    /// live-proven layer; civilian, religious, naval, and air purchases use
    /// different placement rules and districts.
    pub(super) fn land_combat_purchase_slot_open(&self, pid: usize, city: &City) -> bool {
        let is_land_combat = |unit: &Name| {
            self.rules.units.get(unit).is_some_and(|spec| {
                spec.class == "military" && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
            })
        };
        if self.unit_ids_at(city.pos).iter().any(|uid| {
            let other = &self.units[uid];
            other.owner != pid || is_land_combat(&other.kind)
        }) {
            return false;
        }
        !city.queue.first().is_some_and(|item| match item {
            Item::Unit { unit } | Item::Formation { unit, .. } => is_land_combat(unit),
            _ => false,
        })
    }

    /// Memoized front door for [`Game::unit_purchase_cost_for_formation_uncached`],
    /// scoped like [`Game::tile_appeal`] rather than like `producible_items`:
    /// cached only for the life of one [`QueryMemo`] guard (see
    /// `QueryCache::purchase_price`), because the price reads live unit
    /// occupancy and the production queue head, and a `QueryMemo` guard is
    /// the one span the codebase already guarantees nothing mutates those
    /// out from under a `&self` query. Outside any guard every call still
    /// derives fresh, exactly as before this cache existed.
    ///
    /// `legal_purchase_actions_for_city` calls the uncached derivation six
    /// times per unit kind per city (three formations, two currencies), and
    /// `legal_actions_within`'s purchase family repeats the identical sweep
    /// under its own guard — sharing one answer per
    /// `(pid, cid, unit, formation, currency)` removes the duplication
    /// between call sites that both run inside the same decision's guard.
    pub(super) fn unit_purchase_cost_for_formation(
        &self,
        pid: usize,
        cid: u32,
        unit: &str,
        formation: u8,
        currency: &str,
    ) -> Option<f64> {
        let currency_is_gold = match currency {
            "gold" => true,
            "faith" => false,
            // The uncached path also refuses any other currency; skip the
            // lookups and the memo entirely rather than cache a key no real
            // caller asks for.
            _ => return None,
        };
        let key = (pid, cid, Name::new(unit), formation, currency_is_gold);
        if let Some(memo) = self.query_memo.purchase_price.borrow().as_ref() {
            if let Some(cached) = memo.get(&key) {
                return *cached;
            }
        }
        let cost =
            self.unit_purchase_cost_for_formation_uncached(pid, cid, unit, formation, currency);
        if let Some(memo) = self.query_memo.purchase_price.borrow_mut().as_mut() {
            memo.insert(key, cost);
        }
        cost
    }

    pub(super) fn unit_purchase_cost_for_formation_uncached(
        &self,
        pid: usize,
        cid: u32,
        unit: &str,
        formation: u8,
        currency: &str,
    ) -> Option<f64> {
        let player = self.players.get(pid)?;
        let city = self.cities.get(&cid).filter(|city| city.owner == pid)?;
        let spec = self.rules.units.get(unit)?;
        if unit == "spy" || formation > 2 || !matches!(currency, "gold" | "faith") {
            return None;
        }
        // ★ The host's own answer first, for the standard formation the menu
        // prices (`StateCity::purchasable`). Corps and Army purchases are not
        // exported and keep the model's arithmetic below.
        if formation == 0 {
            let plain = Item::Unit {
                unit: Name::new(unit),
            };
            if let Some(host) = self.host_purchase_price(cid, &plain, currency) {
                return host.filter(|_| !self.purchase_is_blocked(cid, &plain));
            }
        }
        if unit == "settler" && (city.pop < 2 || self.policy_effect(pid, "no_settling") > 0.0) {
            return None;
        }

        let religious = spec.class == "religious";
        let rock_band = unit == "rock_band";
        let naturalist = unit == "naturalist";
        let nihang = unit == "nihang"
            && formation == 0
            && currency == "faith"
            && self.grants_city_state_unique_bonus(pid, "Lahore");
        if formation > 0 && (religious || rock_band || naturalist) {
            return None;
        }
        if !nihang
            && spec.class == "military"
            && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
            && !self.land_combat_purchase_slot_open(pid, city)
        {
            return None;
        }
        if rock_band || naturalist {
            if currency != "faith" || !self.unlocked(pid, &spec.tech, &spec.civic) {
                return None;
            }
        } else if religious {
            if currency != "faith"
                || !self.city_has_district_family(city, crate::name!("holy_site"))
                || self.city_religion(city).is_none()
                || !self.unlocked(pid, &spec.tech, &spec.civic)
                || spec.requires_building.as_ref().is_some_and(|building| {
                    !self.city_has_building_family(city, Name::new(building))
                })
                || (unit == "inquisitor"
                    && player.counters.get("inquisition").copied().unwrap_or(0) == 0)
            {
                return None;
            }
        } else {
            let item = if formation == 0 {
                Item::Unit {
                    unit: Name::new(unit),
                }
            } else {
                Item::Formation {
                    unit: Name::new(unit),
                    formation,
                }
            };
            if !nihang && !self.can_produce(pid, cid, &item) {
                return None;
            }
            if currency == "faith" && !nihang {
                let monumentality = matches!(unit, "builder" | "settler")
                    && self.dedication_active(pid, "monumentality");
                let faith_land_combat = spec.class == "military"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && spec.faith_purchasable
                    && (player.government.as_deref() == Some("theocracy")
                        || self
                            .cities
                            .values()
                            .filter(|city| city.owner == pid)
                            .map(|city| {
                                self.city_building_effect(city, "faith_purchase_land_units")
                            })
                            .sum::<f64>()
                            > 0.0);
                if !monumentality && !faith_land_combat {
                    return None;
                }
            }
        }

        let mut purchase_discount = self
            .city_district_effect(city, "gold_faith_purchase_discount_pct")
            + self.unit_purchase_modifier_discount(city, unit);
        if currency == "gold" {
            purchase_discount += self.gov_effects(pid).gold_purchase_discount_pct;
        } else {
            purchase_discount += self.gov_effects(pid).faith_purchase_discount_pct;
        }
        // Flower Power: every unit costs +100% to buy (ADJUST_UNITS_PURCHASE_COST
        // -100 with IncludeCivilian) except a Rock Band, which is free.
        purchase_discount -= self.policy_effect(pid, "unit_purchase_cost_pct");
        if unit == "rock_band" {
            purchase_discount += self.policy_effect(pid, "unit_purchase_cost_pct")
                + self.policy_effect(pid, "rock_band_purchase_discount_pct");
        }
        if religious && currency == "faith" {
            if let Some(religion) = self.city_religion(city) {
                purchase_discount +=
                    self.religion_belief_effect(religion, "religious_unit_faith_discount_pct");
            }
            if unit == "guru" {
                purchase_discount += self.empire_wonder_effect(pid, "guru_purchase_discount_pct");
            }
        }
        if currency == "faith"
            && matches!(unit, "builder" | "settler")
            && self.dedication_active(pid, "monumentality")
        {
            purchase_discount += 30.0;
        }
        if self.grants_city_state_unique_bonus(pid, "Ngazargamu")
            && (spec.class == "support"
                || (spec.class == "military"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))))
        {
            let encampment_buildings = city
                .buildings
                .iter()
                .filter(|building| {
                    !city.pillaged_buildings.contains(*building)
                        && self.building_district_is_active(city, building)
                        && self
                            .rules
                            .buildings
                            .get(building)
                            .is_some_and(|building_spec| {
                                building_spec.district.is_some_and(|district| {
                                    self.district_is_family(district, crate::name!("encampment"))
                                })
                            })
                })
                .count() as f64;
            purchase_discount += 20.0 * encampment_buildings;
        }

        let (base_cost, multiplier) = if rock_band {
            (self.rock_band_purchase_cost(pid), 1.0)
        } else if naturalist {
            (self.naturalist_purchase_cost(pid), 1.0)
        } else {
            let item = Item::Unit {
                unit: Name::new(unit),
            };
            (
                self.item_cost_for(pid, &item),
                if currency == "gold" { 4.0 } else { 2.0 },
            )
        };
        // Buying a formation pays for every constituent. Direct Production
        // instead uses the separate 150%/200% formation cost.
        let formation_multiplier = match formation {
            1 => 2.0,
            2.. => 3.0,
            _ => 1.0,
        };
        let mut cost = base_cost
            * multiplier
            * formation_multiplier
            * (1.0 - purchase_discount / 100.0).max(0.0);
        if spec.class == "military" {
            if self.congress_effect_active("mercenary_companies", "A", currency) {
                cost *= 2.0;
            } else if self.congress_effect_active("mercenary_companies", "B", currency) {
                cost *= 0.5;
            }
        }
        Some(cost)
    }

    #[cfg(test)]
    pub(super) fn do_buy(
        &mut self,
        pid: usize,
        cid: u32,
        unit: &str,
        currency: &str,
    ) -> Result<(), String> {
        self.do_buy_formation(pid, cid, unit, 0, currency)
    }

    pub(super) fn do_buy_formation(
        &mut self,
        pid: usize,
        cid: u32,
        unit: &str,
        formation: u8,
        currency: &str,
    ) -> Result<(), String> {
        match self.cities.get(&cid) {
            Some(city) if city.owner == pid => {}
            _ => return Err("not your city".into()),
        }
        // ★★★★★ THE GATE BELONGS HERE, IN THE ONE FUNCTION EVERY BUYER REACHES.
        //
        // `purchase_is_blocked` already says this in its own doc — "the missionary
        // buyer [and the gold buyers] all build an `Action::Buy*` themselves and
        // call `apply` directly, so a gate that lives only in the enumeration
        // never runs for them" — and the gate was still only in the enumeration
        // (`purchases.retain`, `acts.retain`). So it never ran for them.
        //
        // Measured on live run civvis-20260811T230324Z: **181 refused
        // `UNIT_MISSIONARY` faith purchases in one game**, in one city, on 177
        // CONSECUTIVE turns — from turn 58 to the end, against a cooldown of
        // eight. That single item was 60% of every refusal the run recorded, and
        // the run's total refusal rate (119.6 per 100 turns) was five times the
        // day's median because of it.
        //
        // ⚠ This is the identical repair `do_improve` carries a few thousand
        // lines above, for the identical reason: `builder_step` computed its own
        // options and called `apply` directly, so the rule added to
        // `legal_actions_within` never fired.
        //
        // ⚠ Safe in an ordinary CIVVIS game: `blocked_purchases` is populated
        // only by the live mirror and is empty otherwise, so simulated play is
        // unchanged. And the block is a TTL cooldown, not a verdict — the worst
        // a wrong entry costs is an eight-turn delay.
        let item = if formation == 0 {
            Item::Unit {
                unit: Name::new(unit),
            }
        } else {
            Item::Formation {
                unit: Name::new(unit),
                formation,
            }
        };
        if self.purchase_is_blocked(cid, &item) {
            return Err("the host refused this purchase recently".into());
        }
        let Some(spec) = self.rules.units.get(unit) else {
            return Err("no such unit".into());
        };
        let religious = spec.class == "religious";
        let rock_band = unit == "rock_band";
        let naturalist = unit == "naturalist";
        let nihang = unit == "nihang"
            && formation == 0
            && currency == "faith"
            && self.grants_city_state_unique_bonus(pid, "Lahore");
        if !matches!(currency, "gold" | "faith") {
            return Err("unknown purchase currency".into());
        }
        if formation > 2 {
            return Err("unknown formation tier".into());
        }
        if unit == "spy" {
            return Err("Spies cannot be purchased with Gold or Faith".into());
        }
        if formation > 0 && (rock_band || naturalist || religious) {
            return Err("that unit cannot be purchased as a formation".into());
        }
        // Isolationism closes the frontier to Gold and Faith as well as to
        // Production, which is what makes it a real cost rather than a detour.
        if unit == "settler" && self.policy_effect(pid, "no_settling") > 0.0 {
            return Err("Isolationism forbids buying Settlers".into());
        }
        if rock_band || naturalist {
            if currency != "faith" || !self.unlocked(pid, &spec.tech, &spec.civic) {
                return Err(format!("{unit} is unlocked and purchased with faith"));
            }
        } else if religious {
            if currency != "faith" {
                return Err("religious units are bought with faith".into());
            }
            if !self.city_has_district_family(&self.cities[&cid], crate::name!("holy_site")) {
                return Err("needs a holy site".into());
            }
            if self.city_religion(&self.cities[&cid]).is_none() {
                return Err("city has no majority religion".into());
            }
            if !self.unlocked(pid, &spec.tech, &spec.civic) {
                return Err("not unlocked".into());
            }
            if spec.requires_building.as_ref().is_some_and(|building| {
                !self.city_has_building_family(&self.cities[&cid], Name::new(building))
            }) {
                return Err("required religious building is missing".into());
            }
            if unit == "inquisitor"
                && self.players[pid]
                    .counters
                    .get("inquisition")
                    .copied()
                    .unwrap_or(0)
                    == 0
            {
                return Err("inquisition has not been launched".into());
            }
        } else {
            let item = if formation == 0 {
                Item::Unit {
                    unit: Name::new(unit),
                }
            } else {
                Item::Formation {
                    unit: Name::new(unit),
                    formation,
                }
            };
            if !nihang && !self.can_produce(pid, cid, &item) {
                return Err("cannot buy that".into());
            }
            if currency == "faith" && !nihang {
                let monumentality = matches!(unit, "builder" | "settler")
                    && self.dedication_active(pid, "monumentality");
                let faith_land_combat = spec.class == "military"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && spec.faith_purchasable
                    && (self.players[pid].government.as_deref() == Some("theocracy")
                        || self
                            .cities
                            .values()
                            .filter(|city| city.owner == pid)
                            .map(|city| {
                                self.city_building_effect(city, "faith_purchase_land_units")
                            })
                            .sum::<f64>()
                            > 0.0);
                if !monumentality && !faith_land_combat {
                    return Err("faith cannot purchase that unit".into());
                }
            }
        }
        if unit == "settler" && self.cities[&cid].pop < 2 {
            return Err("city too small for settler".into());
        }
        let item = if formation == 0 {
            Item::Unit {
                unit: Name::new(unit),
            }
        } else {
            Item::Formation {
                unit: Name::new(unit),
                formation,
            }
        };
        let cost = self
            .unit_purchase_cost_for_formation(pid, cid, unit, formation, currency)
            .ok_or_else(|| "unit cannot be purchased that way".to_string())?;
        let bank = if currency == "gold" {
            self.players[pid].gold
        } else {
            self.players[pid].faith
        };
        if bank + f64::EPSILON < cost {
            return Err("cannot afford".into());
        }
        let strategic_payment = self.rules.units[unit]
            .requires_resource
            .as_ref()
            .map(|resource| (*resource, self.unit_resource_cost(cid, &item)))
            .filter(|(_, amount)| *amount > 0.0);
        if strategic_payment
            .as_ref()
            .is_some_and(|(resource, amount)| {
                self.strategic_stockpile(pid, resource) + f64::EPSILON < *amount
            })
        {
            return Err("insufficient strategic resources".into());
        }
        let pos = self.cities[&cid].pos;
        let placed = self
            .place_new_unit(unit, pid, pos)
            .ok_or_else(|| "no space to place unit".to_string())?;
        self.set_unit_formation(placed, formation);
        self.apply_training_district_effects(cid, placed);
        if unit == "builder" {
            self.units.get_mut(&placed).unwrap().charges +=
                self.governor_effect(pid, cid, "builder_charges") as i32;
        }
        if religious {
            self.units.get_mut(&placed).unwrap().religion =
                self.city_religion(&self.cities[&cid]).map(str::to_string);
        }
        if currency == "gold" {
            self.players[pid].gold -= cost;
        } else {
            self.players[pid].faith -= cost;
        }
        if let Some((resource, amount)) = strategic_payment {
            let stock = self.strategic_stockpile(pid, resource);
            self.players[pid]
                .strategic_resources
                .insert(Name::new(&resource), stock - amount);
        }
        if unit == "settler" && self.settler_consumes_population(pid, cid) {
            self.cities.get_mut(&cid).unwrap().pop -= 1;
        }
        bump(&mut self.players[pid], &format!("trained:{unit}"));
        if formation == 1 {
            bump(&mut self.players[pid], "corps");
        }
        self.note_formation_moment(pid, placed);
        if rock_band {
            bump(&mut self.players[pid], "purchased:rock_band");
        }
        if naturalist {
            bump(&mut self.players[pid], "purchased:naturalist");
        }
        Ok(())
    }

    pub(super) fn do_buy_building(
        &mut self,
        pid: usize,
        cid: u32,
        building: &str,
        currency: &str,
    ) -> Result<(), String> {
        if self.cities.get(&cid).is_none_or(|city| city.owner != pid) {
            return Err("not your city".into());
        }
        let item = Item::Building {
            building: Name::new(building),
        };
        let spec = self
            .rules
            .buildings
            .get(building)
            .cloned()
            .ok_or_else(|| "no such building".to_string())?;
        let congress_district = spec
            .district
            .map(|district| self.district_family(district))
            .unwrap_or(crate::name!("city_center"));
        if self.congress_effect_active("urban_development_treaty", "B", congress_district.as_str())
        {
            return Err("World Congress prohibits buildings in that district".into());
        }
        if self.congress_effect_active("global_energy_treaty", "B", building) {
            return Err("World Congress prohibits that power plant".into());
        }
        if self.cities[&cid]
            .buildings
            .iter()
            .any(|owned| owned == building)
        {
            return Err("building cannot be purchased that way".into());
        }
        if currency == "gold" {
            let cost = self
                .building_gold_purchase_cost(pid, cid, building)
                .ok_or_else(|| "building cannot be purchased with Gold".to_string())?;
            if self.players[pid].gold + f64::EPSILON < cost {
                return Err("cannot afford".into());
            }
            if !self.complete_item(pid, cid, &item) {
                return Err("building purchase failed".into());
            }
            self.players[pid].gold -= cost;
            self.finish_purchased_building(cid, &item);
            return Ok(());
        }
        if currency != "faith" {
            return Err("building cannot be purchased that way".into());
        }
        let cost = self
            .building_faith_purchase_cost(pid, cid, building)
            .ok_or_else(|| "building cannot be purchased with Faith".to_string())?;
        if self.players[pid].faith < cost {
            return Err("cannot afford".into());
        }
        if !self.complete_item(pid, cid, &item) {
            return Err("building purchase failed".into());
        }
        self.players[pid].faith -= cost;
        self.finish_purchased_building(cid, &item);
        Ok(())
    }

    /// The half of a building purchase gate that does not depend on the
    /// currency.
    ///
    /// ⚠ One engine, two purchase paths. `building_faith_purchase_cost` and
    /// `building_gold_purchase_cost` asked the same three questions in two
    /// hand-copied preambles, and the last time those drifted the Faith path
    /// sold a city defence the Gold path had always refused — 99 purchases the
    /// live host rejected, recorded in the comment below. They now ask them
    /// once.
    ///
    /// `Err` is an answer the gate has already settled: the host's own price
    /// (or its refusal), or a building the shipped ruleset sells for no
    /// currency at all. `Ok` hands back the item and its district family for
    /// the currency's own rules to price.
    pub(super) fn building_purchase_gate(
        &self,
        cid: u32,
        building: &str,
        spec: &crate::rules::BuildingSpec,
        currency: &str,
    ) -> Result<(Item, Name), Option<f64>> {
        let item = Item::Building {
            building: Name::new(building),
        };
        // ★ The host's own answer first. When the export carried this city's
        // purchase menu (`StateCity::purchasable`) the price is the engine's
        // `GetPurchaseCost` and an item off the menu is not for sale; the
        // model's rules below are what the board falls back on without it.
        if let Some(host) = self.host_purchase_price(cid, &item, currency) {
            return Err(host.filter(|_| !self.purchase_is_blocked(cid, &item)));
        }
        let district = spec
            .district
            .map(|district| self.district_family(district))
            .unwrap_or(crate::name!("city_center"));
        // ★★★ CIVILIZATION VI SELLS A CITY DEFENCE FOR NO CURRENCY AT ALL.
        //
        // Every purchasable building in the shipped ruleset declares a
        // `PurchaseYield` -- Monument, Barracks, Granary, Library, Shrine, all
        // of them. `BUILDING_WALLS`, `BUILDING_CASTLE`, `BUILDING_STAR_FORT`
        // and Georgia's `BUILDING_TSIKHE` declare none, in any base or
        // expansion file, so no currency can buy them. Valletta's suzerain
        // bonus is `MODIFIER_PLAYER_CITIES_ENABLE_BUILDING_FAITH_PURCHASE` with
        // `DistrictType = DISTRICT_CITY_CENTER`: it changes which currency a
        // purchasable building takes, not whether an unpurchasable one becomes
        // buyable. The three `..._PURCHASE_CHEAPER_WALLS/CASTLE/STAR_BONUS`
        // cost modifiers beside it are vestigial -- they discount a purchase
        // the game never offers.
        //
        // The Faith path granted the opposite until 2026-08-18, while the Gold
        // path had always refused the same buildings for the same reason. The
        // live seat paid for it: runs `civvis-20260818T113115Z` and `104654Z`,
        // the two whose minors list carries Valletta with this seat as
        // suzerain, issued 99 Faith purchases of `BUILDING_CASTLE` (53),
        // `BUILDING_STAR_FORT` (32) and `BUILDING_WALLS` (14). The host refused
        // every one, with no reason text and with all four of the mod's probed
        // parameter shapes answering `can = false`. The other six runs, which
        // have no Valletta, refused none. Two copies of a gate is how that
        // happens; this is the copy that is left.
        if spec.outer_defense > 0 {
            return Err(None);
        }
        Ok((item, district))
    }

    /// Faith price for a building unlocked by a Worship belief, Jesuit
    /// Education, or Valletta. Valletta uses the normal 2:1 Faith conversion
    /// for City Center and Encampment buildings, while the three wall tiers
    /// (including unique replacements) receive its 50% discount.
    pub(crate) fn building_faith_purchase_cost(
        &self,
        pid: usize,
        cid: u32,
        building: &str,
    ) -> Option<f64> {
        let city = self.cities.get(&cid).filter(|city| city.owner == pid)?;
        let spec = self.rules.buildings.get(building)?;
        if city.buildings.iter().any(|owned| owned == building) {
            return None;
        }
        let (item, district) = match self.building_purchase_gate(cid, building, spec, "faith") {
            Ok(gate) => gate,
            Err(settled) => return settled,
        };
        let valletta = self.grants_city_state_unique_bonus(pid, "Valletta")
            && matches!(district.as_str(), "city_center" | "encampment")
            && self.can_produce(pid, cid, &item);
        let religion = self.city_religion(city);
        let worship = religion.is_some_and(|religion| {
            spec.worship_belief.as_ref().is_some_and(|belief| {
                self.religion_founder(religion).is_some_and(|founder| {
                    self.players[founder]
                        .religion_beliefs
                        .iter()
                        .any(|chosen| chosen == belief)
                })
            })
        });
        let jesuit = religion.is_some_and(|religion| {
            self.religion_belief_effect(religion, "faith_purchase_science_culture_buildings") > 0.0
                && matches!(district.as_str(), "campus" | "theater_square")
        });
        let religious_requirements = spec
            .district
            .as_ref()
            .is_none_or(|required| self.city_has_district_family(city, *required))
            && spec
                .requires
                .iter()
                .all(|required| self.city_has_building_family(city, *required));
        if !valletta && (!(worship || jesuit) || !religious_requirements) {
            return None;
        }
        // The halved conversion existed only for the wall tiers above, which
        // are now refused outright; every remaining Faith purchase pays the
        // stock two-for-one.
        let conversion = 2.0;
        let discount = self
            .city_district_effect(city, "gold_faith_purchase_discount_pct")
            .clamp(0.0, 100.0);
        Some(self.item_cost_for_city(pid, cid, &item) * conversion * (1.0 - discount / 100.0))
    }

    /// Stock Gold purchase price for an ordinary building. City defenses,
    /// Flood Barriers, Government Plaza buildings, and wonders remain
    /// Production-only; all normal unlock, district, prerequisite, unique,
    /// and Congress gates are inherited from `can_produce`.
    pub(crate) fn building_gold_purchase_cost(
        &self,
        pid: usize,
        cid: u32,
        building: &str,
    ) -> Option<f64> {
        let spec = self.rules.buildings.get(building)?;
        let (item, district) = match self.building_purchase_gate(cid, building, spec, "gold") {
            Ok(gate) => gate,
            Err(settled) => return settled,
        };
        if spec.wonder
            || district == "government_plaza"
            || spec
                .effects
                .get("protect_coastal_lowlands")
                .copied()
                .unwrap_or(0.0)
                > 0.0
            || !self.can_produce(pid, cid, &item)
            // ⚠ `can_produce` reads `blocked_production`; a PURCHASE refusal lands
            // in `blocked_purchases`, a different set with a different meaning —
            // "the host will not sell this here right now" rather than "this city
            // cannot build it". Without this the two never met and a refused
            // purchase was re-offered every turn.
            || self.purchase_is_blocked(cid, &item)
        {
            return None;
        }
        let discount = (self
            .city_district_effect(&self.cities[&cid], "gold_faith_purchase_discount_pct")
            + self.gov_effects(pid).gold_purchase_discount_pct)
            .clamp(0.0, 100.0);
        Some(self.item_cost_for_city(pid, cid, &item) * 4.0 * (1.0 - discount / 100.0))
    }

    pub(super) fn finish_purchased_building(&mut self, cid: u32, item: &Item) {
        let key = Self::item_progress_key(item);
        let city = self.cities.get_mut(&cid).unwrap();
        let was_active = city.queue.first() == Some(item);
        city.queue.retain(|queued| queued != item);
        city.production_progress.remove(&key);
        if was_active {
            city.production = 0.0;
        }
    }

    pub(super) fn do_buy_district(
        &mut self,
        pid: usize,
        cid: u32,
        district: &str,
        pos: Pos,
        currency: &str,
    ) -> Result<(), String> {
        if self.cities.get(&cid).is_none_or(|city| city.owner != pid) {
            return Err("not your city".into());
        }
        let allowed = match currency {
            "faith" => self.governor_effect(pid, cid, "faith_purchase_districts") > 0.0,
            "gold" => self.governor_effect(pid, cid, "gold_purchase_districts") > 0.0,
            _ => false,
        };
        if !allowed {
            return Err("district cannot be purchased that way".into());
        }
        let item = Item::District {
            district: Name::new(district),
            pos,
        };
        if !self.can_produce(pid, cid, &item) {
            return Err("cannot purchase that district".into());
        }
        if self.map.tiles[&pos].district_foundation.is_some() {
            return Err("a placed district must be completed with production".into());
        }
        let discount = if currency == "gold" {
            self.gov_effects(pid).gold_purchase_discount_pct
        } else {
            0.0
        };
        let cost = self
            .game_speed
            .scale(self.district_cost_for_placement(pid, district, true))
            * 4.0
            * (1.0 - discount / 100.0).max(0.0);
        let bank = if currency == "faith" {
            self.players[pid].faith
        } else {
            self.players[pid].gold
        };
        if bank + f64::EPSILON < cost {
            return Err("cannot afford".into());
        }
        if !self.complete_item(pid, cid, &item) {
            return Err("district placement failed".into());
        }
        if currency == "faith" {
            self.players[pid].faith -= cost;
        } else {
            self.players[pid].gold -= cost;
        }
        Ok(())
    }

    pub(super) fn do_buy_plot(&mut self, pid: usize, cid: u32, pos: Pos) -> Result<(), String> {
        let cost = self
            .plot_purchase_cost(pid, cid, pos)
            .ok_or_else(|| "plot cannot be purchased by that city".to_string())?;
        if self.players[pid].gold + f64::EPSILON < cost {
            return Err("cannot afford".into());
        }
        self.players[pid].gold -= cost;
        self.map.tiles.get_mut(&pos).unwrap().owner_city = Some(cid);
        let city = self.cities.get_mut(&cid).unwrap();
        if !city.owned_tiles.contains(&pos) {
            city.owned_tiles.push(pos);
        }
        Ok(())
    }

    pub(super) fn do_research(&mut self, pid: usize, tech: &str) -> Result<(), String> {
        // Not a seat's decision on an arena: the tree is climbed the same
        // way by both sides, cheapest technology first, and nobody is asked
        // — see `arena_auto_research`.
        if self.is_arena() {
            return Err("an arena researches on its own".into());
        }
        if self.players[pid].research.is_some() {
            return Err("already researching".into());
        }
        if !self.available_techs(pid).iter().any(|t| t == tech) {
            return Err("tech unavailable".into());
        }
        self.begin_research(pid, tech);
        Ok(())
    }

    /// Put a technology under study: banked overflow and any eureka the
    /// side has earned go on the new node. The caller has checked that
    /// nothing is being studied and that the technology is open.
    pub(super) fn begin_research(&mut self, pid: usize, tech: &str) {
        let cost = self.tech_cost(tech);
        let p = &mut self.players[pid];
        p.research = Some(tech.to_string());
        p.research_progress = p.research_overflow;
        p.research_overflow = 0.0;
        let f = self.node_boost_frac(pid, tech, true);
        if self.players[pid].boosted_techs.contains(&Name::new(tech)) {
            self.players[pid].research_progress += f * cost;
        }
    }

    pub(super) fn do_civic(&mut self, pid: usize, civic: &str) -> Result<(), String> {
        // An arena pays no Culture and has no civics tree to spend it on.
        if self.is_arena() {
            return Err("an arena has no civics tree".into());
        }
        if self.players[pid].civic.is_some() {
            return Err("already working a civic".into());
        }
        if !self.available_civics(pid).iter().any(|c| c == civic) {
            return Err("civic unavailable".into());
        }
        let cost = self.civic_cost(civic);
        let p = &mut self.players[pid];
        p.civic = Some(civic.to_string());
        p.civic_progress = p.civic_overflow;
        p.civic_overflow = 0.0;
        let f = self.node_boost_frac(pid, civic, false);
        if self.players[pid].boosted_civics.contains(&Name::new(civic)) {
            self.players[pid].civic_progress += f * cost;
        }
        Ok(())
    }

    pub(super) fn do_fortify(&mut self, pid: usize, uid: u32) -> Result<(), String> {
        let u = self.own_unit(pid, uid)?;
        if !self.unit_can_fortify(&u) {
            return Err("only unembarked land military units can fortify".into());
        }
        let mu = self.units.get_mut(&uid).unwrap();
        mu.fortified = true;
        if !mu.acted {
            mu.fortify_turns = mu.fortify_turns.max(1);
        }
        mu.moves_left = 0.0;
        Ok(())
    }

    pub(super) fn do_promote(
        &mut self,
        pid: usize,
        uid: u32,
        promotion: &str,
    ) -> Result<(), String> {
        let unit = self.own_unit(pid, uid)?;
        if unit.moves_left <= 0.0 {
            return Err("unit has no movement left".into());
        }
        if !self
            .available_promotions(uid)
            .iter()
            .any(|name| name == promotion)
        {
            return Err("promotion unavailable".into());
        }
        let extra_charges = self.rules.promotions[promotion]
            .effects
            .get("religious_charges")
            .copied()
            .unwrap_or(0.0)
            + self.rules.promotions[promotion]
                .effects
                .get("natural_wonder_charges")
                .copied()
                .unwrap_or(0.0);
        let rock_band = unit.kind == "rock_band";
        let distinguished = {
            let unit = self.units.get_mut(&uid).unwrap();
            let extra_first_promotion = unit.extra_first_promotion;
            unit.extra_first_promotion = false;
            unit.promotions.insert(Name::new(promotion));
            unit.charges += extra_charges as i32;
            if rock_band {
                // Concert outcomes raise Cultural Communicator level before the
                // newly earned promotion is chosen. Consuming the choice must not
                // raise it a second time; the initial promotion likewise leaves a
                // new band at level 1.
                unit.xp = 0;
            } else {
                unit.level = (unit.level + 1).min(8);
                if extra_first_promotion {
                    unit.xp = unit.xp.max(Self::promotion_threshold(unit.level));
                }
            }
            unit.hp = (unit.hp + 50).min(100);
            unit.moves_left = 0.0;
            unit.attacks_left = 0;
            unit.acted = true;
            unit.fortified = false;
            unit.fortify_turns = 0;
            !rock_band && unit.level == 4
        };
        if distinguished {
            let counter = self.players[pid]
                .counters
                .entry("distinguished_units".to_string())
                .or_insert(0);
            let first = *counter == 0;
            *counter += 1;
            self.add_historic_moment(
                pid,
                if first {
                    "MOMENT_UNIT_HIGH_LEVEL_FIRST"
                } else {
                    "MOMENT_UNIT_HIGH_LEVEL"
                },
            );
        }
        Ok(())
    }

    pub(super) fn do_upgrade(
        &mut self,
        pid: usize,
        uid: u32,
        requested: &str,
    ) -> Result<(), String> {
        let (target, _, _) = self
            .unit_gold_upgrade_offer(pid, uid)
            .ok_or_else(|| "unit cannot upgrade here or the cost is unaffordable".to_string())?;
        if target != requested {
            return Err(format!("unit upgrades to {target}, not {requested}"));
        }
        self.do_upgrade_unit(pid, uid)
    }

    /// Return the resulting formation size if these two units may combine.
    pub(super) fn can_combine_units(&self, pid: usize, a: u32, b: u32) -> Option<u8> {
        let (a, b) = (self.units.get(&a)?, self.units.get(&b)?);
        if a.owner != pid
            || b.owner != pid
            || a.id == b.id
            || a.kind != b.kind
            || self.players[pid].is_minor
            || a.levied_from.is_some()
            || b.levied_from.is_some()
            || a.linked_to.is_some()
            || b.linked_to.is_some()
            || a.moves_left <= 0.0
            || b.moves_left <= 0.0
            || self.wdist(a.pos, b.pos) > 1
            || self.rules.units[a.kind].class != "military"
            || self.rules.units[a.kind].domain.as_deref() == Some("air")
            || !self.rules.units[a.kind].can_formations
            || !self.rules.units[a.kind].can_combine
        {
            return None;
        }
        match (a.formation, b.formation) {
            (0, 0) if self.formation_unlocked(pid, &a.kind, 1) => Some(1),
            (0, 1) | (1, 0) if self.formation_unlocked(pid, &a.kind, 2) => Some(2),
            _ => None,
        }
    }

    pub(super) fn do_combine_units(&mut self, pid: usize, a: u32, b: u32) -> Result<(), String> {
        let formation = self
            .can_combine_units(pid, a, b)
            .ok_or_else(|| "units cannot form a Corps or Army".to_string())?;
        let ua = self.units[&a].clone();
        let ub = self.units[&b].clone();
        // The most experienced constituent keeps the identity, level and XP;
        // Gathering Storm unions both promotion trees and keeps the highest
        // training-city XP modifier.
        let a_key = (ua.xp, ua.level, ua.promotions.len(), Reverse(ua.id));
        let b_key = (ub.xp, ub.level, ub.promotions.len(), Reverse(ub.id));
        let (survivor, consumed) = if a_key >= b_key { (a, b) } else { (b, a) };
        let destination = ub.pos;
        let a_constituents = ua.formation as i32 + 1;
        let b_constituents = ub.formation as i32 + 1;
        let hp =
            (ua.hp * a_constituents + ub.hp * b_constituents) / (a_constituents + b_constituents);
        let promotions = ua.promotions.union(&ub.promotions).cloned().collect();
        let xp_bonus_pct = ua.xp_bonus_pct.max(ub.xp_bonus_pct);
        let damage_dealt = ua.damage_dealt.saturating_add(ub.damage_dealt);
        let production_cost = self.unit_accounting_cost(&ua) + self.unit_accounting_cost(&ub);
        self.remove_combined_unit(consumed);
        if self.units[&survivor].pos != destination {
            self.relocate(survivor, destination);
        }
        let unit = self.units.get_mut(&survivor).unwrap();
        unit.formation = formation;
        unit.damage_dealt = damage_dealt;
        unit.production_cost = production_cost;
        unit.hp = hp;
        unit.promotions = promotions;
        unit.xp_bonus_pct = xp_bonus_pct;
        unit.moves_left = 0.0;
        unit.attacks_left = 0;
        unit.acted = true;
        if formation == 1 {
            bump(&mut self.players[pid], "corps");
        }
        self.note_formation_moment(pid, survivor);
        Ok(())
    }

    /// Isibongo upgrades the unit which actually takes a city to the highest
    /// formation tier Shaka's current civics permit.
    pub(super) fn apply_capture_formation_upgrade(&mut self, pid: usize, uid: u32) {
        if self.civ_effect(pid, "capture_formation_upgrade") <= 0.0 {
            return;
        }
        let Some(unit) = self.units.get(&uid) else {
            return;
        };
        let spec = &self.rules.units[unit.kind];
        if spec.class != "military" || spec.domain.as_deref() == Some("air") || !spec.can_formations
        {
            return;
        }
        let target = if self.formation_unlocked(pid, &unit.kind, 2) {
            2
        } else if self.formation_unlocked(pid, &unit.kind, 1) {
            1
        } else {
            0
        };
        let old = unit.formation;
        if target > old {
            self.set_unit_formation(uid, target);
            if old == 0 && target == 1 {
                bump(&mut self.players[pid], "corps");
            }
            self.note_formation_moment(pid, uid);
        }
    }

    pub(super) fn can_link_units(&self, pid: usize, a: u32, b: u32) -> bool {
        let (Some(a), Some(b)) = (self.units.get(&a), self.units.get(&b)) else {
            return false;
        };
        if a.owner != pid
            || b.owner != pid
            || a.id == b.id
            || a.pos != b.pos
            || a.linked_to.is_some()
            || b.linked_to.is_some()
            || self.noncombat_action_blocked_by_zoc(a.id)
            || self.noncombat_action_blocked_by_zoc(b.id)
        {
            return false;
        }
        let (aspec, bspec) = (&self.rules.units[a.kind], &self.rules.units[b.kind]);
        let ordinary = (aspec.class == "military"
            && matches!(bspec.class.as_str(), "civilian" | "support" | "religious"))
            || (bspec.class == "military"
                && matches!(aspec.class.as_str(), "civilian" | "support" | "religious"));
        let naval_escort = (aspec.domain.as_deref() == Some("sea")
            && bspec.class == "military"
            && self.is_embarked(b))
            || (bspec.domain.as_deref() == Some("sea")
                && aspec.class == "military"
                && self.is_embarked(a));
        ordinary || naval_escort
    }

    pub(super) fn do_link_units(&mut self, pid: usize, a: u32, b: u32) -> Result<(), String> {
        if !self.can_link_units(pid, a, b) {
            return Err("units cannot form a linked formation".into());
        }
        self.units.get_mut(&a).unwrap().linked_to = Some(b);
        self.units.get_mut(&b).unwrap().linked_to = Some(a);
        Ok(())
    }

    pub(super) fn do_unlink_units(&mut self, pid: usize, uid: u32) -> Result<(), String> {
        let unit = self.own_unit(pid, uid)?;
        let peer = unit
            .linked_to
            .ok_or_else(|| "unit is not linked".to_string())?;
        self.units.get_mut(&uid).unwrap().linked_to = None;
        if let Some(other) = self.units.get_mut(&peer) {
            other.linked_to = None;
        }
        Ok(())
    }

    /// Adopt a form of government. Civ 6 charges nothing for a form the
    /// civilization has never run — "your people are enthusiastic to try this
    /// new form of government" — and charges Anarchy for going back to one
    /// they have already lived under. Anarchy is `GOVERNMENT_BASE_ANARCHY_TURNS`
    /// turns without a government at all: no Science, Culture, Gold or Faith,
    /// no policy slots, and no second change until it ends.
    pub(super) fn do_government(&mut self, pid: usize, g: &str) -> Result<(), String> {
        let spec = self
            .rules
            .governments
            .get(g)
            .ok_or_else(|| "government unavailable".to_string())?;
        let p = &self.players[pid];
        if let Some(c) = &spec.civic {
            if !p.civics.contains(c) {
                return Err("government unavailable".into());
            }
        }
        if p.anarchy_turns > 0 {
            return Err("the empire is in Anarchy".into());
        }
        if p.government.as_deref() == Some(g) {
            return Err("already that government".into());
        }
        if p.past_governments.contains(g) {
            self.players[pid].anarchy_turns = GOVERNMENT_BASE_ANARCHY_TURNS;
            self.players[pid].pending_government = Some(g.to_string());
            self.players[pid].government = None;
            self.note(
                pid,
                "General",
                format!("fell into Anarchy on the way back to {}", pretty(g)),
                None,
            );
            return Ok(());
        }
        self.install_government(pid, g);
        Ok(())
    }

    /// Seat a government and reseat the cards under its slot layout.
    pub(super) fn install_government(&mut self, pid: usize, g: &str) {
        self.players[pid].government = Some(g.to_string());
        self.players[pid].past_governments.insert(g.to_string());
        self.note_government_tier_moment(pid, g);
        self.players[pid].pending_government = None;
        self.players[pid].anarchy_turns = 0;
        // new slot layout: drop slotted cards until they fit again
        while !self.policies_fit(pid, &self.players[pid].policies)
            && !self.players[pid].policies.is_empty()
        {
            let drop = *self.players[pid].policies.iter().next_back().unwrap();
            self.players[pid].policies.remove(&drop);
        }
        self.note(pid, "General", format!("adopted {}", pretty(g)), None);
    }

    /// Is this civilization between governments? Anarchy suspends the
    /// government's own bonus, its policy slots and the empire's Science,
    /// Culture, Gold and Faith until the new government takes power.
    pub fn in_anarchy(&self, pid: usize) -> bool {
        self.players
            .get(pid)
            .is_some_and(|player| player.anarchy_turns > 0)
    }

    /// Serve a turn of Anarchy, seating the waiting government when the last
    /// one is served.
    pub(super) fn process_anarchy(&mut self, pid: usize) {
        if self.players[pid].anarchy_turns == 0 {
            return;
        }
        self.players[pid].anarchy_turns -= 1;
        if self.players[pid].anarchy_turns > 0 {
            return;
        }
        if let Some(government) = self.players[pid].pending_government.clone() {
            self.install_government(pid, &government);
        }
    }

    pub(super) fn do_slot_policy(&mut self, pid: usize, policy: &str) -> Result<(), String> {
        if !self.available_policies(pid).iter().any(|c| c == policy) {
            return Err("policy unavailable".into());
        }
        let mut next = self.players[pid].policies.clone();
        next.insert(Name::new(policy));
        if !self.policies_fit(pid, &next) {
            return Err("no free slot for that card".into());
        }
        self.players[pid].policies = next;
        Ok(())
    }

    pub(super) fn do_unslot_policy(&mut self, pid: usize, policy: &str) -> Result<(), String> {
        if !self.players[pid].policies.remove(&Name::new(policy)) {
            return Err("policy not slotted".into());
        }
        Ok(())
    }

    /// Every platform of `pid`'s that could deliver a device to `target`,
    /// nearest first, as (distance, what carried it, where it fired from).
    ///
    /// A device is national stockpile rather than a unit, so the order names a
    /// city and the question is only which of that civilization's platforms is
    /// closest: the city center, one of that city's working Missile Silos, or —
    /// and this is the whole point of an SSBN — a Nuclear Submarine, which
    /// carries the device's range wherever it sails instead of waiting for the
    /// target to come inside a silo's reach.
    pub(super) fn wmd_launch_platforms(
        &self,
        pid: usize,
        cid: u32,
        target: Pos,
    ) -> Vec<(i32, &'static str, Pos)> {
        let mut platforms: Vec<(i32, &'static str, Pos)> = Vec::new();
        if let Some(city) = self.cities.get(&cid).filter(|city| city.owner == pid) {
            platforms.push((self.wdist(city.pos, target), "city", city.pos));
            for position in &city.owned_tiles {
                let silo = self.map.tiles.get(position).is_some_and(|tile| {
                    tile.improvement.as_deref() == Some("missile_silo") && !tile.pillaged
                });
                if silo {
                    platforms.push((self.wdist(*position, target), "missile_silo", *position));
                }
            }
        }
        for unit in self.units.values() {
            if unit.owner == pid && unit.kind == "nuclear_submarine" {
                platforms.push((self.wdist(unit.pos, target), "nuclear_submarine", unit.pos));
            }
        }
        platforms.sort_by_key(|(distance, platform, position)| {
            (*distance, *platform, position.0, position.1)
        });
        platforms
    }

    /// Launch a stockpiled device. Range, blast radius and fallout duration
    /// are the shipped WMDs rows; the per-ring unit damage is the one number
    /// the database does not carry.
    pub(super) fn do_wmd_strike(
        &mut self,
        pid: usize,
        cid: u32,
        target: Pos,
        thermonuclear: bool,
    ) -> Result<(), String> {
        if self
            .cities
            .get(&cid)
            .filter(|city| city.owner == pid)
            .is_none()
        {
            return Err("launch city must be yours".into());
        }
        let device_key = if thermonuclear {
            "project_effect:thermonuclear_devices"
        } else {
            "project_effect:nuclear_devices"
        };
        if self.players[pid]
            .counters
            .get(device_key)
            .copied()
            .unwrap_or(0)
            <= 0
        {
            return Err("no device of that type in the stockpile".into());
        }
        let spec = self.rules.wmds[if thermonuclear {
            "thermonuclear_device"
        } else {
            "nuclear_device"
        }]
        .clone();
        if !self.map.tiles.contains_key(&target) {
            return Err("no such tile".into());
        }
        if !self.players[pid].explored.contains(&target) {
            return Err("target tile is unrevealed".into());
        }
        // The closest platform carries the shot.
        let (_, platform, launched_from) = self
            .wmd_launch_platforms(pid, cid, target)
            .into_iter()
            .find(|(distance, _, _)| *distance <= spec.icbm_strike_range)
            .ok_or_else(|| "target out of ICBM range".to_string())?;
        // Nuking a major you are at peace with is not a legal order.
        let blast: Vec<Pos> = self.wdisk(target, spec.blast_radius);
        let mut targeted_owners = BTreeSet::new();
        for position in &blast {
            let victims = self
                .unit_ids_at(*position)
                .iter()
                .map(|uid| self.units[uid].owner)
                .chain(self.city_at(*position).map(|c| self.cities[&c].owner))
                .chain(self.encampment_at(*position).map(|c| self.cities[&c].owner));
            for owner in victims {
                let victim = &self.players[owner];
                if owner != pid && !victim.is_barbarian && !self.is_at_war(pid, owner) {
                    return Err("cannot nuke a civilization you are at peace with".into());
                }
                if owner != pid && !victim.is_barbarian {
                    targeted_owners.insert(owner);
                }
            }
        }
        let fallout_until = self.turn.saturating_add(spec.fallout_duration);
        *self.players[pid]
            .counters
            .entry(device_key.to_string())
            .or_insert(0) -= 1;
        bump(&mut self.players[pid], "wmd_strikes");
        let mut aggrieved: BTreeSet<usize> = BTreeSet::new();
        // The cities under the blast, named for the war ledger before the
        // detonation halves them.
        let struck_cities: Vec<(u32, usize, String)> = blast
            .iter()
            .filter_map(|position| self.city_at(*position))
            .filter_map(|cid| self.cities.get(&cid).map(|city| (cid, city)))
            .filter(|(_, city)| city.owner != pid)
            .map(|(cid, city)| (cid, city.owner, city.name.clone()))
            .collect();
        // Everything standing in the radius before the blast, so the record can
        // report what the device actually killed rather than what it aimed at.
        let exposed: Vec<u32> = blast
            .iter()
            .flat_map(|position| self.units_at(*position))
            .collect();
        let exposed_military = exposed
            .iter()
            .filter_map(|unit| self.units.get(unit))
            .filter(|unit| unit.owner != pid && self.rules.units[unit.kind].class == "military")
            .cloned()
            .collect::<Vec<_>>();
        for defender in &exposed_military {
            self.record_war_unit_participation(defender, pid);
        }
        let launch_unit = if platform == "nuclear_submarine" {
            self.units
                .values()
                .find(|unit| {
                    unit.owner == pid
                        && unit.kind == "nuclear_submarine"
                        && unit.pos == launched_from
                })
                .cloned()
        } else {
            None
        };
        for position in blast.iter().copied() {
            if let Some(tile) = self.map.tiles.get_mut(&position) {
                tile.fallout_until = tile.fallout_until.max(fallout_until);
            }
            // Ground zero is lethal to full-health units; the outer rings
            // wound severely.
            let ring = self.wdist(position, target);
            let unit_damage = match ring {
                0 => 100,
                1 => 80,
                _ => 60,
            };
            if let Some(owner) = self
                .map
                .get(position)
                .and_then(|tile| tile.owner_city)
                .and_then(|c| self.cities.get(&c))
                .map(|c| c.owner)
            {
                if owner != pid {
                    aggrieved.insert(owner);
                }
            }
            self.damage_tile_area(position, unit_damage, Some(pid));
            if let Some(struck) = self.city_at(position) {
                let city = self.cities.get_mut(&struck).unwrap();
                // Half the population and the Outer Defenses go with the blast.
                city.pop = (city.pop - (city.pop / 2)).max(1);
                city.wall_hp = if ring == 0 { 0 } else { city.wall_hp / 2 };
            }
        }
        for owner in &aggrieved {
            // The world does not forgive a nuclear strike quickly.
            self.add_grievances(*owner, pid, 150.0);
        }
        // A detonation belongs in the account of the war it was used in — on
        // the city it hit where it hit one, and otherwise on the front whose
        // land took it.
        let mut struck_owners: BTreeSet<usize> = BTreeSet::new();
        let mut struck_names: Vec<String> = Vec::new();
        for (_, owner, name) in struck_cities {
            struck_owners.insert(owner);
            struck_names.push(name.clone());
            self.record_war_moment(pid, owner, "nuclear_strike", Some(name), Some(target));
        }
        for owner in &aggrieved {
            if !struck_owners.contains(owner) {
                self.record_war_moment(pid, *owner, "nuclear_strike", None, Some(target));
            }
        }
        let units_destroyed = exposed
            .into_iter()
            .filter(|uid| !self.units.contains_key(uid))
            .count() as u32;
        let victims: BTreeSet<usize> = targeted_owners
            .union(&aggrieved)
            .copied()
            .filter(|owner| *owner != pid)
            .collect();
        if let Some(launcher) = &launch_unit {
            for owner in &victims {
                self.record_war_unit_participation(launcher, *owner);
            }
        }
        let strike = NuclearStrike {
            id: self.allocate_conflict_id(),
            turn: self.turn,
            attacker: pid,
            target,
            thermonuclear,
            platform: platform.to_string(),
            launched_from,
            blast_radius: spec.blast_radius,
            fallout_until,
            victims: victims.clone(),
            cities: struck_names.clone(),
            units_destroyed,
        };
        self.announce_nuclear_strike(&strike, &struck_names);
        self.nuclear_strikes.push(strike);
        // A client animates the tail of this list and a log describes it; the
        // full history of a long nuclear war is not worth carrying in every
        // frame forever.
        const STRIKES_KEPT: usize = 32;
        if self.nuclear_strikes.len() > STRIKES_KEPT {
            let excess = self.nuclear_strikes.len() - STRIKES_KEPT;
            self.nuclear_strikes.drain(..excess);
        }
        Ok(())
    }

    /// Tell the world. A detonation is not private news: the launcher's own
    /// log, every civilization that lost something to it, and every other
    /// living major all get an entry, and all three are marked important so a
    /// log scrolling at speed holds them instead of letting the largest event
    /// in the game slide past between two city-growth notices.
    pub(super) fn announce_nuclear_strike(
        &mut self,
        strike: &NuclearStrike,
        struck_names: &[String],
    ) {
        let attacker = strike.attacker;
        let weapon = if strike.thermonuclear {
            "thermonuclear device"
        } else {
            "nuclear device"
        };
        // Name the place it landed on: the city if it hit one, otherwise the
        // civilization whose ground took it, otherwise bare coordinates.
        let place = struck_names.first().cloned().or_else(|| {
            strike
                .victims
                .iter()
                .next()
                .map(|victim| format!("{} territory", self.civ_name(*victim)))
        });
        let where_it_landed = match &place {
            Some(place) => format!(" on {place}"),
            None => String::new(),
        };
        let toll = match strike.units_destroyed {
            0 => String::new(),
            1 => " · 1 unit lost".to_string(),
            count => format!(" · {count} units lost"),
        };
        let from = match strike.platform.as_str() {
            "missile_silo" => " from a Missile Silo",
            "nuclear_submarine" => " from a Nuclear Submarine",
            _ => "",
        };
        self.note_important(
            attacker,
            "Nuclear",
            format!("detonated a {weapon}{where_it_landed}{from}{toll}"),
            Some(strike.target),
        );
        let aggressor = self.civ_name(attacker);
        for victim in strike.victims.iter().copied() {
            // A device that lands on open ground has no city to name, and
            // "…: was struck" is not a sentence. Name the ground instead.
            let hit = match struck_names.first() {
                Some(city) => format!("{city} was struck"),
                None => format!("{} territory was struck", self.civ_name(victim)),
            };
            self.note_important(
                victim,
                "Nuclear",
                format!("{aggressor} detonated a {weapon}: {hit}{toll}"),
                Some(strike.target),
            );
        }
        // Nobody misses a mushroom cloud, whatever the fog says.
        let bystanders: Vec<usize> = self
            .players
            .iter()
            .filter(|player| {
                player.alive
                    && !player.is_minor
                    && !player.is_barbarian
                    && player.id != attacker
                    && !strike.victims.contains(&player.id)
            })
            .map(|player| player.id)
            .collect();
        for bystander in bystanders {
            self.note_important(
                bystander,
                "Nuclear",
                format!("{aggressor} detonated a {weapon}{where_it_landed}"),
                Some(strike.target),
            );
        }
    }

    pub(super) fn do_city_strike(
        &mut self,
        pid: usize,
        cid: u32,
        target: Pos,
    ) -> Result<(), String> {
        match self.cities.get(&cid) {
            Some(c) if c.owner == pid => {}
            _ => return Err("not your city".into()),
        }
        if !self.city_can_strike(&self.cities[&cid]) {
            return Err("city cannot strike".into());
        }
        if self.wdist(self.cities[&cid].pos, target) > 2 {
            return Err("out of range".into());
        }
        if self.city_at(target).is_some() || self.encampment_at(target).is_some() {
            return Err("cities cannot strike defensible districts".into());
        }
        let visible = self.player_vision_frame(pid);
        let viewers = self.visibility_viewers(pid);
        if !self.combat_target_visible_at(pid, target, visible.as_ref(), &viewers) {
            return Err("target is not visible".into());
        }
        if !self.has_line_of_sight(self.cities[&cid].pos, target, true) {
            return Err("line of sight blocked".into());
        }
        // See `peaceful_foreign_unit_at`. The scan below is existential and
        // would happily name the barbarian standing beside a rival's Trader.
        if self.peaceful_foreign_unit_at(pid, target) {
            return Err("a unit we are at peace with stands there".into());
        }
        let enemies: Vec<u32> = self
            .unit_ids_at(target)
            .iter()
            .filter(|id| {
                let o = &self.units[id];
                o.owner != pid
                    && self.is_at_war(pid, o.owner)
                    && self.unit_currently_visible_to(**id, pid)
            })
            .copied()
            .collect();
        if enemies.is_empty() {
            return Err("no enemy target".into());
        }
        let military: Vec<u32> = enemies
            .iter()
            .cloned()
            .filter(|id| self.rules.units[self.units[id].kind].class == "military")
            .collect();
        if military.is_empty() {
            return Err("no enemy military target".into());
        }
        let did = *military
            .iter()
            .max_by(|a, b| {
                let ea = effective_strength(
                    self.unit_strength(&self.units[*a], true),
                    self.units[*a].hp,
                );
                let eb = effective_strength(
                    self.unit_strength(&self.units[*b], true),
                    self.units[*b].hp,
                );
                ea.partial_cmp(&eb).unwrap()
            })
            .unwrap();
        let d = self.units[&did].clone();
        self.record_war_unit_participation(&d, pid);
        let ds = effective_strength(
            self.unit_strength(&d, true) + self.ranged_defense_bonus(&d, true),
            d.hp,
        ) + self.tile_defense_bonus(target);
        let naval = self.rules.units[d.kind].domain.as_deref() == Some("sea");
        let emergency_bonus = self.players[pid]
            .counters
            .get(&format!("emergency_city_strike_vs:{}", d.owner))
            .copied()
            .unwrap_or(0) as f64;
        let att = self.city_ranged_strength(cid) + emergency_bonus - if naval { 17.0 } else { 0.0 };
        let dmg = damage(att, ds, &mut self.rng);
        self.units.get_mut(&did).unwrap().hp -= dmg;
        let defender_dead = self.units[&did].hp <= 0;
        self.record_emergency_combat(pid, d.owner, defender_dead);
        if !defender_dead {
            self.award_xp(did, 2.0);
        } else {
            self.record_kill(pid, None, &d);
            let downer = self.units[&did].owner;
            self.remove_unit(did);
            self.on_unit_lost(downer);
        }
        let city = self.cities.get_mut(&cid).unwrap();
        if city.struck {
            city.extra_strikes_used += 1;
        }
        city.struck = true;
        Ok(())
    }

    pub(super) fn do_encampment_strike(
        &mut self,
        pid: usize,
        cid: u32,
        target: Pos,
    ) -> Result<(), String> {
        let city = self
            .cities
            .get(&cid)
            .filter(|city| city.owner == pid)
            .ok_or_else(|| "not your city".to_string())?;
        let position = city
            .districts
            .iter()
            .find_map(|(district, position)| {
                self.district_is_family(district, crate::name!("encampment"))
                    .then_some(*position)
            })
            .ok_or_else(|| "city has no Encampment".to_string())?;
        if !self.encampment_can_strike(city) {
            return Err("Encampment cannot strike".into());
        }
        if self.wdist(position, target) > 2 || !self.has_line_of_sight(position, target, true) {
            return Err("target out of range or sight".into());
        }
        let visible = self.player_vision_frame(pid);
        let viewers = self.visibility_viewers(pid);
        if !self.combat_target_visible_at(pid, target, visible.as_ref(), &viewers) {
            return Err("target is not visible".into());
        }
        if self.city_at(target).is_some() || self.encampment_at(target).is_some() {
            return Err("defensible districts cannot target each other".into());
        }
        // See `peaceful_foreign_unit_at`.
        if self.peaceful_foreign_unit_at(pid, target) {
            return Err("a unit we are at peace with stands there".into());
        }
        let defender_id = self
            .units_at(target)
            .into_iter()
            .filter(|id| {
                let unit = &self.units[id];
                unit.owner != pid
                    && self.is_at_war(pid, unit.owner)
                    && self.rules.units[unit.kind].class == "military"
                    && self.unit_currently_visible_to(*id, pid)
            })
            .max_by(|a, b| {
                let a_strength =
                    effective_strength(self.unit_strength(&self.units[a], true), self.units[a].hp);
                let b_strength =
                    effective_strength(self.unit_strength(&self.units[b], true), self.units[b].hp);
                a_strength.partial_cmp(&b_strength).unwrap()
            })
            .ok_or_else(|| "no enemy military target".to_string())?;
        let defender = self.units[&defender_id].clone();
        self.record_war_unit_participation(&defender, pid);
        let defense = effective_strength(
            self.unit_strength(&defender, true) + self.ranged_defense_bonus(&defender, false),
            defender.hp,
        ) + self.tile_defense_bonus(target);
        let naval = self.rules.units[defender.kind].domain.as_deref() == Some("sea");
        let emergency_bonus = self.players[pid]
            .counters
            .get(&format!("emergency_city_strike_vs:{}", defender.owner))
            .copied()
            .unwrap_or(0) as f64;
        let attack =
            self.city_ranged_strength(cid) + emergency_bonus - if naval { 17.0 } else { 0.0 };
        let dealt = damage(attack, defense, &mut self.rng);
        self.units.get_mut(&defender_id).unwrap().hp -= dealt;
        let defender_dead = self.units[&defender_id].hp <= 0;
        self.record_emergency_combat(pid, defender.owner, defender_dead);
        if !defender_dead {
            self.award_xp(defender_id, 2.0);
        } else {
            self.record_kill(pid, None, &defender);
            let owner = self.units[&defender_id].owner;
            self.remove_unit(defender_id);
            self.on_unit_lost(owner);
        }
        let city = self.cities.get_mut(&cid).unwrap();
        if city.encampment_struck {
            city.encampment_extra_strikes_used += 1;
        }
        city.encampment_struck = true;
        Ok(())
    }

    pub fn are_friends(&self, first: usize, second: usize) -> bool {
        self.same_team(first, second)
            || (first < self.players.len()
                && second < self.players.len()
                && self.players[first]
                    .friends_until
                    .get(&second)
                    .is_some_and(|until| *until > self.turn))
    }

    pub fn alliance_with(&self, first: usize, second: usize) -> Option<&AllianceState> {
        self.players
            .get(first)?
            .alliances
            .get(&second)
            .filter(|alliance| alliance.ends > self.turn)
    }

    /// Typed diplomatic alliances expire and level; a pre-game team is an
    /// untyped permanent alliance. Callers that only need allied/not-allied
    /// semantics should use this predicate.
    pub fn are_allied(&self, first: usize, second: usize) -> bool {
        self.same_team(first, second) || self.alliance_with(first, second).is_some()
    }

    /// The expiry of an active, reciprocal Defensive Pact.  A typed Alliance
    /// is a prerequisite to signing one; it is not itself an automatic call
    /// to war.  Keeping the distinction here prevents every alliance from
    /// behaving like an invisible defensive pact.
    pub fn defensive_pact_until(&self, first: usize, second: usize) -> Option<u32> {
        let until = self
            .players
            .get(first)?
            .defensive_pacts
            .get(&second)
            .copied()?;
        (until > self.turn
            && self.are_allied(first, second)
            && self
                .players
                .get(second)
                .and_then(|player| player.defensive_pacts.get(&first))
                .is_some_and(|other_until| *other_until > self.turn))
        .then_some(until)
    }

    pub fn has_defensive_pact(&self, first: usize, second: usize) -> bool {
        self.defensive_pact_until(first, second).is_some()
    }

    /// An outgoing delegation/embassy has one visibility level.  The precise
    /// mission kind remains observable because an Embassy replaces, rather
    /// than stacks with, the earlier Delegation.
    pub fn diplomatic_mission_to(
        &self,
        source: usize,
        target: usize,
    ) -> Option<&DiplomaticMission> {
        self.players.get(source)?.diplomatic_missions.get(&target)
    }

    /// A relationship-facing attitude that joins the persistent diplomatic
    /// ledger to the leader's agenda.  `agenda_opinion` intentionally stays a
    /// narrow leader-preference score for backwards-compatible AI callers;
    /// this is the wider state a diplomacy screen needs.
    pub fn relationship_opinion(&self, observer: usize, subject: usize) -> f64 {
        if observer == subject || observer >= self.players.len() || subject >= self.players.len() {
            return 0.0;
        }
        let mut opinion = self.agenda_opinion(observer, subject);
        opinion -= self.players[observer]
            .grievances
            .get(&subject)
            .copied()
            .unwrap_or(0.0)
            .min(100.0);
        if self.players[observer]
            .denounced_until
            .get(&subject)
            .is_some_and(|until| *until > self.turn)
        {
            opinion -= 50.0;
        }
        if self.is_at_war(observer, subject) {
            opinion -= 100.0;
        } else if self.are_allied(observer, subject) {
            opinion += 50.0;
        } else if self.are_friends(observer, subject) {
            opinion += 30.0;
        }
        // A delegation/embassy is sent *to* the observer.  It improves the
        // recipient's attitude towards its sender; it does not make the
        // sender automatically like the recipient in return.
        if self.diplomatic_mission_to(subject, observer).is_some() {
            opinion += 5.0;
        }
        opinion.clamp(-100.0, 100.0)
    }

    /// The directional diplomatic state that a leader presents to one other
    /// leader.  Friendly/Neutral/Unfriendly are inferred from the live
    /// relationship ledger; the named treaty and conflict states take
    /// precedence exactly as they do in the Civ VI ribbon.
    pub fn relationship_state(&self, observer: usize, subject: usize) -> &'static str {
        if self.is_at_war(observer, subject) {
            "war"
        } else if self.are_allied(observer, subject) {
            "allied"
        } else if self.are_friends(observer, subject) {
            "declared_friend"
        } else if self.players.get(observer).is_some_and(|player| {
            player
                .denounced_until
                .get(&subject)
                .is_some_and(|until| *until > self.turn)
        }) {
            "denounced"
        } else if self.relationship_opinion(observer, subject) >= 15.0 {
            "friendly"
        } else if self.relationship_opinion(observer, subject) <= -15.0 {
            "unfriendly"
        } else {
            "neutral"
        }
    }

    pub(super) fn denounced_active(&self, denouncer: usize, target: usize) -> bool {
        self.players
            .get(denouncer)
            .and_then(|player| player.denounced_until.get(&target))
            .is_some_and(|until| *until > self.turn)
    }

    pub(super) fn denounced_long_enough(&self, denouncer: usize, target: usize) -> bool {
        if !self.denounced_active(denouncer, target) {
            return false;
        }
        if let Some(since) = self.players[denouncer].denounced_since.get(&target) {
            return self.turn >= *since + self.standard_duration(5);
        }
        // A pre-state field save still contains the expiry.  A denunciation
        // lasts thirty standard-scaled turns, so a remaining twenty-five (or
        // fewer) is the faithful legacy reconstruction of the five-turn wait.
        self.players[denouncer]
            .denounced_until
            .get(&target)
            .is_some_and(|until| *until <= self.turn + self.standard_duration(25))
    }

    pub(super) fn valid_promise_kind(kind: &str) -> bool {
        matches!(
            kind,
            "no_settling" | "no_conversion" | "no_spying" | "no_city_state_attack"
        )
    }

    /// Record conduct that makes a Discuss promise available to the affected
    /// leader.  Incidents are deliberately directional: a city converted for
    /// America is not a reason for England to request a promise from the
    /// converter, unless England was itself affected by a later action.
    pub(super) fn record_diplomatic_incident(
        &mut self,
        affected: usize,
        offender: usize,
        kind: &str,
    ) {
        let major = |player: usize| {
            self.players.get(player).is_some_and(|leader| {
                leader.alive && !leader.is_minor && !leader.is_barbarian && !leader.is_free_city
            })
        };
        if affected == offender
            || !major(affected)
            || !major(offender)
            || !Self::valid_promise_kind(kind)
        {
            return;
        }
        self.players[affected]
            .diplomatic_incidents
            .entry(offender)
            .or_default()
            .insert(
                kind.to_string(),
                DiplomaticIncident {
                    occurred: self.turn,
                    // A new incident is a fresh reason to Discuss, even if
                    // an older request was recently refused.
                    requestable_at: self.turn,
                },
            );
    }

    pub(super) fn promise_request_incident_exists(
        &self,
        requester: usize,
        promisor: usize,
        kind: &str,
    ) -> bool {
        Self::valid_promise_kind(kind)
            && self
                .players
                .get(requester)
                .and_then(|player| player.diplomatic_incidents.get(&promisor))
                .is_some_and(|incidents| incidents.contains_key(kind))
    }

    /// Whether the Discuss panel should offer this promise right now. A
    /// promise must be grounded in a prior incident, not generated as a
    /// generic pre-emptive diplomatic action; pending, active, and recently
    /// refused requests stay out until their normal thirty-turn cadence ends.
    pub(super) fn promise_request_available(
        &self,
        requester: usize,
        promisor: usize,
        kind: &str,
    ) -> bool {
        self.major_diplomatic_counterpart(requester, promisor)
            && !self.is_at_war(requester, promisor)
            && self.promise_request_incident_exists(requester, promisor, kind)
            && self.players[requester]
                .diplomatic_incidents
                .get(&promisor)
                .and_then(|incidents| incidents.get(kind))
                .is_some_and(|incident| incident.requestable_at <= self.turn)
            && !self.promise_active(promisor, requester, kind)
            && !self.pending_deals.iter().any(|deal| {
                deal.from == requester
                    && deal.to == promisor
                    && deal.promise.as_deref() == Some(kind)
                    && deal.expires >= self.turn
            })
    }

    pub(super) fn reserve_promise_request(
        &mut self,
        requester: usize,
        promisor: usize,
        kind: &str,
    ) {
        let requestable_at = self.turn + self.standard_duration(STANDARD_DEAL_TURNS);
        if let Some(incident) = self.players[requester]
            .diplomatic_incidents
            .get_mut(&promisor)
            .and_then(|incidents| incidents.get_mut(kind))
        {
            incident.requestable_at = requestable_at;
        }
    }

    /// Demands are available only once the relationship has become openly
    /// hostile, or after either leader has denounced the other. Unlike an
    /// ordinary offer, the demanded Gold is not required to be mutually
    /// valued; the target may still refuse and create grievances.
    pub(super) fn demand_available(&self, demander: usize, target: usize) -> bool {
        self.major_diplomatic_counterpart(demander, target)
            && !self.is_at_war(demander, target)
            && (self.relationship_state(demander, target) == "unfriendly"
                || self.relationship_state(target, demander) == "unfriendly"
                || self.denounced_active(demander, target)
                || self.denounced_active(target, demander))
    }

    /// Democracy's route package is limited to allied civilizations and
    /// city-states whose Suzerain owns the route. Both endpoint cities use
    /// this same predicate so their yields cannot drift apart.
    pub(super) fn government_trade_partner(
        &self,
        route_owner: usize,
        destination_owner: usize,
    ) -> bool {
        self.are_allied(route_owner, destination_owner)
            || self.players.get(destination_owner).is_some_and(|player| {
                player.is_minor
                    && !player.is_barbarian
                    && self.suzerain_of(destination_owner) == Some(route_owner)
            })
    }

    pub(super) fn alliance_partner(
        &self,
        pid: usize,
        kind: &str,
        minimum_level: i32,
    ) -> Option<usize> {
        self.players[pid]
            .alliances
            .iter()
            .find(|(_, alliance)| {
                alliance.ends > self.turn
                    && alliance.kind == kind
                    && alliance.level >= minimum_level
            })
            .map(|(partner, _)| *partner)
    }

    pub(super) fn alliance_points_key(partner: usize, kind: &str) -> String {
        format!("alliance_points:{partner}:{kind}")
    }

    pub(super) fn research_alliance_boost_candidate(
        &self,
        recipient: usize,
        source: usize,
    ) -> Option<Name> {
        self.players[source]
            .techs
            .iter()
            .chain(self.players[source].boosted_techs.iter())
            .filter(|tech| {
                !self.players[recipient].techs.contains(*tech)
                    && !self.players[recipient].boosted_techs.contains(*tech)
            })
            .min_by(|left, right| {
                let left_spec = &self.rules.techs[*left];
                let right_spec = &self.rules.techs[*right];
                left_spec
                    .era
                    .cmp(&right_spec.era)
                    .then_with(|| left_spec.cost.partial_cmp(&right_spec.cost).unwrap())
                    .then_with(|| left.cmp(right))
            })
            .cloned()
    }

    pub(super) fn share_research_alliance_boosts(&mut self, first: usize, second: usize) {
        let first_boost = self.research_alliance_boost_candidate(first, second);
        let second_boost = self.research_alliance_boost_candidate(second, first);
        if let Some(technology) = first_boost {
            self.players[first]
                .boosted_techs
                .insert(Name::new(&technology));
        }
        if let Some(technology) = second_boost {
            self.players[second]
                .boosted_techs
                .insert(Name::new(&technology));
        }
    }

    pub(super) fn at_war_with_any_major(&self, pid: usize) -> bool {
        self.players.iter().any(|other| {
            other.alive && !other.is_minor && !other.is_barbarian && self.is_at_war(pid, other.id)
        })
    }

    pub(super) fn religious_alliance_blocks_pressure(&self, city: &City, incoming: &str) -> bool {
        let Some(dominant) = self.city_religion(city) else {
            return false;
        };
        let Some(dominant_founder) = self.religion_founder(dominant) else {
            return false;
        };
        let Some(incoming_founder) = self.religion_founder(incoming) else {
            return false;
        };
        dominant_founder != incoming_founder
            && self
                .alliance_with(dominant_founder, incoming_founder)
                .is_some_and(|alliance| alliance.kind == "religious")
    }

    pub(super) fn religious_alliance_combat_bonus(
        &self,
        pid: usize,
        opposing_religion: &str,
    ) -> f64 {
        let Some(partner) = self.alliance_partner(pid, "religious", 2) else {
            return 0.0;
        };
        let own = self.players[pid].religion.as_deref();
        let allied = self.players[partner].religion.as_deref();
        if Some(opposing_religion) != own && Some(opposing_religion) != allied {
            10.0
        } else {
            0.0
        }
    }

    /// Book one ledger entry without spreading it to the rest of the world.
    /// Explicit global consequences (eliminating a civilization or city-state)
    /// use this boundary so their already-enumerated observers do not multiply
    /// the same event a second time through relationship propagation.
    pub(super) fn add_direct_grievances(&mut self, aggrieved: usize, offender: usize, amount: f64) {
        if aggrieved == offender
            || aggrieved >= self.players.len()
            || offender >= self.players.len()
            || amount <= 0.0
        {
            return;
        }
        let target = |player: usize| player.to_string();
        let touches_target = |outcome| {
            self.congress_effect_active("public_relations", outcome, &target(aggrieved))
                || self.congress_effect_active("public_relations", outcome, &target(offender))
        };
        let multiplier = if touches_target("A") {
            2.0
        } else if touches_target("B") {
            0.5
        } else {
            1.0
        };
        // A Civ VI grievance ledger is one balance for each pair, not two
        // independent piles of anger.  A retaliation first spends the
        // offender's existing balance, only becoming a new grievance in the
        // opposite direction once it crosses neutral.  Normalizing legacy
        // saves here also means a pre-state save that happened to contain both
        // directional entries recovers the same single-balance invariant on
        // its next diplomatic event.
        let adjusted = amount * multiplier;
        let forward = self.players[aggrieved]
            .grievances
            .remove(&offender)
            .unwrap_or(0.0)
            .max(0.0);
        let reverse = self.players[offender]
            .grievances
            .remove(&aggrieved)
            .unwrap_or(0.0)
            .max(0.0);
        let balance = forward + adjusted - reverse;
        if balance > 0.0 {
            self.players[aggrieved].grievances.insert(offender, balance);
        } else if balance < 0.0 {
            self.players[offender]
                .grievances
                .insert(aggrieved, -balance);
        }
    }

    /// Apply goodwill to one ledger entry without creating an artificial
    /// reverse grievance. Returning or liberating a city reduces the amount a
    /// leader already holds against its benefactor; it never makes that leader
    /// newly aggrieved by being helped.
    pub(super) fn relieve_direct_grievances(
        &mut self,
        aggrieved: usize,
        benefactor: usize,
        amount: f64,
    ) {
        if aggrieved == benefactor
            || aggrieved >= self.players.len()
            || benefactor >= self.players.len()
            || amount <= 0.0
        {
            return;
        }
        let target = |player: usize| player.to_string();
        let touches_target = |outcome| {
            self.congress_effect_active("public_relations", outcome, &target(aggrieved))
                || self.congress_effect_active("public_relations", outcome, &target(benefactor))
        };
        let multiplier = if touches_target("A") {
            2.0
        } else if touches_target("B") {
            0.5
        } else {
            1.0
        };
        let remaining = self.players[aggrieved]
            .grievances
            .remove(&benefactor)
            .unwrap_or(0.0)
            - amount * multiplier;
        if remaining > 0.0 {
            self.players[aggrieved]
                .grievances
                .insert(benefactor, remaining);
        }
    }

    /// Requests begin at 25 grievances and grow by another 25 each time the
    /// same counterpart repeats the conduct. Broken promises use the higher
    /// 100-point first transgression, then the same 25-point escalation.
    /// Counters are durable save state, which lets the escalation survive a
    /// reload without widening the Player schema for an otherwise private
    /// bookkeeping detail.
    pub(super) fn escalating_grievance(
        &mut self,
        aggrieved: usize,
        offender: usize,
        category: &str,
        first: f64,
        repeated: f64,
    ) -> f64 {
        let key = format!("grievance_escalation:{category}:{offender}");
        let previous = self.players[aggrieved]
            .counters
            .get(&key)
            .copied()
            .unwrap_or(0)
            .max(0) as f64;
        *self.players[aggrieved].counters.entry(key).or_insert(0) += 1;
        first + repeated * previous
    }

    /// Add the direct grievance and the diplomatic spillover it causes.
    ///
    /// Gathering Storm records grievances pair-by-pair, but a leader's allies
    /// and declared friends also take a fixed share when they know both sides.
    /// The shares are computed from the relationship *before* any rows are
    /// written, and then booked directly, so a friend of a friend never turns
    /// a single offence into an accidental cascade.
    pub(super) fn add_grievances(&mut self, aggrieved: usize, offender: usize, amount: f64) {
        if aggrieved == offender
            || aggrieved >= self.players.len()
            || offender >= self.players.len()
            || amount <= 0.0
        {
            return;
        }
        let observers: Vec<(usize, f64)> = self
            .players
            .iter()
            .filter(|observer| {
                observer.id != aggrieved
                    && observer.id != offender
                    && observer.alive
                    && !observer.is_minor
                    && !observer.is_barbarian
                    && self.has_met(observer.id, aggrieved)
                    && self.has_met(observer.id, offender)
            })
            .filter_map(|observer| {
                if self.are_allied(observer.id, aggrieved) {
                    Some((observer.id, ALLIED_GRIEVANCE_SHARE))
                } else if self.are_friends(observer.id, aggrieved) {
                    Some((observer.id, FRIEND_GRIEVANCE_SHARE))
                } else {
                    None
                }
            })
            .collect();
        self.add_direct_grievances(aggrieved, offender, amount);
        for (observer, share) in observers {
            self.add_direct_grievances(observer, offender, amount * share);
        }
    }

    /// Declaring on a city-state has its own witnesses in Gathering Storm:
    /// the Suzerain receives 100 grievances, while every other major with an
    /// Envoy there receives 50. These are explicitly enumerated rather than
    /// relationship spillover, so a third party is charged once even when it
    /// is also friends with the Suzerain.
    pub(super) fn city_state_declaration_grievances(
        &mut self,
        city_state: usize,
        declarers: &[usize],
    ) {
        if !self.players.get(city_state).is_some_and(|player| {
            player.alive && player.is_minor && !player.is_barbarian && !player.is_free_city
        }) {
            return;
        }
        let suzerain = self.suzerain_of(city_state);
        let witnesses: Vec<(usize, f64)> = self
            .players
            .iter()
            .filter(|player| {
                player.alive && !player.is_minor && !player.is_barbarian && !player.is_free_city
            })
            .filter_map(|player| {
                if Some(player.id) == suzerain {
                    Some((player.id, CITY_STATE_SUZERAIN_WAR_GRIEVANCES))
                } else if self.envoys_at(player.id, city_state) > 0 {
                    Some((player.id, CITY_STATE_ENVOY_WAR_GRIEVANCES))
                } else {
                    None
                }
            })
            .collect();
        for declarer in declarers {
            for (witness, amount) in &witnesses {
                if *witness != *declarer {
                    self.add_direct_grievances(*witness, *declarer, *amount);
                }
            }
        }
    }

    /// The turn a war between these two can first be settled, while it is
    /// still too young to end. Declaring war is a commitment in Civ VI: the
    /// shipped `DIPLOMACY_WAR_MIN_TURNS` keeps it from being undone the turn
    /// after it is made, which is what turned this AI's diplomacy into a
    /// stutter of one-turn wars nobody fought.
    pub fn peace_available_at(&self, a: usize, b: usize) -> Option<u32> {
        let (first, second) = self.war_sides(a, b)?;
        let war = self.wars.get(&pair(first, second))?;
        let earliest = (war.started + self.standard_duration(WAR_MIN_TURNS))
            .max(war.joint_war_until.unwrap_or(0));
        (self.turn < earliest).then_some(earliest)
    }

    /// The turn a peace treaty between two civilizations expires, while one
    /// is still in force. A city-state answers for its Suzerain here too: the
    /// treaty its patron signed is the one that binds it.
    pub fn peace_treaty_until(&self, a: usize, b: usize) -> Option<u32> {
        let principals = |player: usize| {
            let mut seats = vec![player];
            if self.players[player].is_minor && !self.players[player].is_barbarian {
                seats.extend(self.suzerain_of(player));
            }
            seats
        };
        principals(a)
            .into_iter()
            .flat_map(|first| {
                principals(b)
                    .into_iter()
                    .filter_map(move |second| (first != second).then_some(pair(first, second)))
            })
            .filter_map(|key| self.peace_treaties.get(&key).copied())
            .filter(|until| *until > self.turn)
            .max()
    }

    pub(super) fn player_tech_era(&self, pid: usize) -> usize {
        self.players[pid]
            .techs
            .iter()
            .filter_map(|name| self.rules.techs.get(name).map(|spec| spec.era))
            .max()
            .unwrap_or(self.start_era)
            .max(self.start_era)
    }

    pub(super) fn government_tier(&self, pid: usize) -> Option<u8> {
        match self.players.get(pid)?.government.as_deref()? {
            "autocracy" | "oligarchy" | "classical_republic" => Some(1),
            "monarchy" | "merchant_republic" | "theocracy" => Some(2),
            "communism" | "democracy" | "fascism" => Some(3),
            "corporate_libertarianism" | "digital_democracy" | "synthetic_technocracy" => Some(4),
            _ => None,
        }
    }

    pub(super) fn territorial_war_available(&self, pid: usize, other: usize) -> bool {
        let mine = self.player_city_ids(pid);
        let theirs = self.player_city_ids(other);
        theirs.iter().any(|first_theirs| {
            mine.iter().any(|first_mine| {
                self.wdist(self.cities[first_theirs].pos, self.cities[first_mine].pos) <= 10
                    && theirs.iter().any(|second_theirs| {
                        second_theirs != first_theirs
                            && mine.iter().any(|second_mine| {
                                second_mine != first_mine
                                    && self.wdist(
                                        self.cities[second_theirs].pos,
                                        self.cities[second_mine].pos,
                                    ) <= 10
                            })
                    })
            })
        })
    }

    pub(super) fn casus_belli_available(
        &self,
        pid: usize,
        other: usize,
        casus_belli: &str,
    ) -> bool {
        let Some(profile) = casus_belli_profile(casus_belli) else {
            return false;
        };
        if pid == other
            || pid >= self.players.len()
            || other >= self.players.len()
            || !self.players[pid].alive
            || !self.players[other].alive
            || self.players[pid].is_minor
            || self.players[other].is_minor
            || self.players[pid].is_barbarian
            || self.players[other].is_barbarian
            || !self.has_met(pid, other)
        {
            return false;
        }
        let waited = self.denounced_long_enough(pid, other);
        match profile.id {
            "formal_war" => waited,
            "holy_war" => {
                self.players[pid]
                    .civics
                    .contains(&crate::name!("diplomatic_service"))
                    && waited
                    && self.players[other]
                        .religion
                        .as_deref()
                        .is_some_and(|religion| {
                            self.cities.values().any(|city| {
                                city.owner == pid && self.city_religion(city) == Some(religion)
                            })
                        })
            }
            "reconquest_war" => {
                self.players[pid]
                    .civics
                    .contains(&crate::name!("defensive_tactics"))
                    && waited
                    && self
                        .cities
                        .values()
                        .any(|city| city.owner == other && city.original_owner == pid)
            }
            "protectorate_war" => {
                self.players[pid]
                    .civics
                    .contains(&crate::name!("defensive_tactics"))
                    && waited
                    && self.players.iter().any(|city_state| {
                        city_state.alive
                            && city_state.is_minor
                            && !city_state.is_barbarian
                            && self.suzerain_of(city_state.id) == Some(pid)
                            && self.is_at_war(other, city_state.id)
                    })
            }
            "liberation_war" => {
                self.players[pid]
                    .civics
                    .contains(&crate::name!("diplomatic_service"))
                    && waited
                    && self.cities.values().any(|city| {
                        city.owner == other
                            && city.original_owner != pid
                            && self
                                .players
                                .get(city.original_owner)
                                .is_some_and(|founder| {
                                    founder.alive
                                        && !founder.is_minor
                                        && !founder.is_barbarian
                                        && (self.are_friends(pid, founder.id)
                                            || self.are_allied(pid, founder.id))
                                })
                    })
            }
            "colonial_war" => {
                self.players[pid]
                    .civics
                    .contains(&crate::name!("nationalism"))
                    && waited
                    && self.player_tech_era(pid) >= self.player_tech_era(other).saturating_add(2)
            }
            "territorial_war" => {
                self.players[pid]
                    .civics
                    .contains(&crate::name!("mobilization"))
                    && waited
                    && self.territorial_war_available(pid, other)
            }
            "golden_age_war" => {
                self.dedication_active(pid, "to_arms") && self.denounced_active(pid, other)
            }
            "retribution_war" => {
                self.players[pid]
                    .civics
                    .contains(&crate::name!("early_empire"))
                    && waited
                    && self.players[other]
                        .broken_promises_until
                        .get(&pid)
                        .is_some_and(|until| *until > self.turn)
            }
            "ideological_war" => {
                self.players[pid].civics.contains(&crate::name!("ideology"))
                    && waited
                    && self.government_tier(pid) == Some(3)
                    && self.government_tier(other) == Some(3)
                    && self.players[pid].government != self.players[other].government
            }
            // Joint Wars are negotiated through a partner proposal; accepting
            // an arbitrary direct casus action would skip that consent.
            "joint_war" | "surprise_war" => false,
            _ => false,
        }
    }

    pub(super) fn end_bilateral_relations_for_war(&mut self, first: usize, second: usize) {
        self.cancel_routes_with(first, second);
        self.cancel_trade_deals_with(first, second);
        self.players[first].open_borders_until.remove(&second);
        self.players[second].open_borders_until.remove(&first);
        self.players[first].friends_until.remove(&second);
        self.players[second].friends_until.remove(&first);
        self.players[first].alliances.remove(&second);
        self.players[second].alliances.remove(&first);
        self.players[first].defensive_pacts.remove(&second);
        self.players[second].defensive_pacts.remove(&first);
        self.players[first].diplomatic_missions.remove(&second);
        self.players[second].diplomatic_missions.remove(&first);
        self.players[first].promises.remove(&second);
        self.players[second].promises.remove(&first);
    }

    pub(super) fn start_war(
        &mut self,
        pid: usize,
        other: usize,
        profile: CasusBelliProfile,
        joint_partner: Option<usize>,
    ) -> Result<(), String> {
        if other == pid || other >= self.players.len() || !self.players[other].alive {
            return Err("invalid war target".into());
        }
        if self.players[pid].is_barbarian || self.players[other].is_barbarian {
            return Err("barbarians are always at war".into());
        }
        if self.players[pid].is_minor {
            return Err("city-states do not declare war independently".into());
        }
        if !self.has_met(pid, other) {
            return Err("cannot declare war before contact".into());
        }
        if self.is_at_war(pid, other) {
            return Err("already at war".into());
        }
        if self.are_allied(pid, other) || self.are_friends(pid, other) {
            return Err("friendship and alliance declarations must expire before war".into());
        }
        if self.emergency_coalition_pair(pid, other) {
            return Err(
                "active Emergency coalition members cannot declare war on each other".into(),
            );
        }
        if let Some(until) = self.peace_treaty_until(pid, other) {
            return Err(format!("a peace treaty holds until turn {until}"));
        }
        let mut declared_principals = vec![pid];
        if let Some(partner) = joint_partner {
            if profile.id != "joint_war"
                || partner == pid
                || partner == other
                || partner >= self.players.len()
                || !self.players[partner].alive
                || self.players[partner].is_minor
                || self.players[partner].is_barbarian
                || !self.has_met(partner, other)
                || self.is_at_war(partner, other)
                || self.are_friends(partner, other)
                || self.are_allied(partner, other)
                || self.emergency_coalition_pair(partner, other)
                || self.peace_treaty_until(partner, other).is_some()
            {
                return Err("joint-war partner cannot declare against that target".into());
            }
            declared_principals.push(partner);
        }
        // Gathering Storm's `WORLD_CONGRESS_REQUEST_FOR_MILITARY_AID_GRIEVANCES_MIN`
        // tests the target's grievance ledger when a declarer begins the war.
        // Capture that pre-declaration state before the war's own grievance
        // accounting can make an ordinary declaration appear eligible.
        let military_aid_request = declared_principals.iter().any(|declarer| {
            self.players[other]
                .grievances
                .get(declarer)
                .is_some_and(|grievances| *grievances >= MILITARY_AID_REQUEST_GRIEVANCES_MIN)
        });
        let attackers: BTreeSet<usize> = declared_principals
            .iter()
            .flat_map(|principal| self.team_members(*principal))
            .filter(|member| self.players[*member].alive)
            .collect();
        if attackers.is_empty() {
            return Err("no living attacker".into());
        }
        let mut defenders: BTreeSet<usize> = self
            .team_members(other)
            .into_iter()
            .filter(|member| self.players[*member].alive)
            .collect();
        let initial_defenders: Vec<usize> = defenders.iter().copied().collect();
        // Only an explicit Defensive Pact responds to a declaration.  Its
        // response is deliberately one hop: a pact-holder's own pact does not
        // chain a globe-spanning automatic war.
        let defensive_allies: Vec<usize> = initial_defenders
            .iter()
            .flat_map(|defender| {
                self.players[*defender]
                    .defensive_pacts
                    .iter()
                    .filter(|(ally, until)| {
                        **until > self.turn && self.has_defensive_pact(*defender, **ally)
                    })
                    .map(|(ally, _)| *ally)
            })
            .collect();
        for ally in defensive_allies {
            if attackers.contains(&ally) {
                continue;
            }
            defenders.extend(
                self.team_members(ally)
                    .into_iter()
                    .filter(|member| self.players[*member].alive),
            );
        }
        // A treaty can reject the declaration against one of its actual
        // targets.  It must not, however, reject a legal primary declaration
        // merely because an optional Defensive-Pact response would create a
        // treaty-protected *derived* front; that response simply stays out of
        // the war below.
        for attacker in &attackers {
            for defender in &initial_defenders {
                if attacker != defender
                    && !self.same_team(*attacker, *defender)
                    && self.peace_treaty_until(*attacker, *defender).is_some()
                {
                    return Err("a peace treaty blocks one front of this war".into());
                }
            }
        }
        let declaration_grievances = profile.declaration_grievances();
        for declarer in &declared_principals {
            self.add_grievances(other, *declarer, declaration_grievances);
        }
        if self.players[other].is_minor {
            self.city_state_declaration_grievances(other, &declared_principals);
            // Every Joint-War signatory made the choice to attack. Each one
            // therefore breaks its own promise to protect the city-state;
            // charging only the proposer would make the partner evade the
            // very diplomatic consequence the agreement was meant to share.
            for declarer in &declared_principals {
                self.break_promises_on_city_state_attack(*declarer, other);
            }
        }
        // Eureka bookkeeping: Defensive Tactics wants a war declared on you,
        // Nationalism a war declared with a named justification.
        for defender in &defenders {
            bump(&mut self.players[*defender], "received_dow");
        }
        if profile.id != "surprise_war" {
            for declarer in &declared_principals {
                bump(&mut self.players[*declarer], "casus_belli");
            }
        }
        let (aggressor, defender) = (self.civ_name(pid), self.civ_name(other));
        let message = format!("{aggressor} declared war on {defender}");
        for participant in attackers.iter().copied().chain(defenders.iter().copied()) {
            self.note(participant, "War", message.clone(), None);
        }
        let conflict = self.allocate_conflict_id();
        let joint_war_until = (profile.id == "joint_war")
            .then(|| self.turn + self.standard_duration(STANDARD_DEAL_TURNS));
        for attacker in attackers {
            for defender in defenders.iter().copied() {
                if attacker == defender || self.same_team(attacker, defender) {
                    continue;
                }
                // A defensive pact can widen the declared war, but it cannot
                // make one of those derived fronts violate a peace treaty.
                // The principal declaration was checked above; every expanded
                // attacker/ally pair needs the same protection independently.
                if self.peace_treaty_until(attacker, defender).is_some() {
                    continue;
                }
                let front = pair(attacker, defender);
                // Nobody learns who they are fighting from the battlefield.
                // A war widened by a pact introduces its new belligerents, so
                // an ally dragged in is on the ledger of everyone it now
                // fights whether or not it has seen them.
                self.record_contact(attacker, defender);
                let opened = self.at_war.insert(front);
                self.end_bilateral_relations_for_war(attacker, defender);
                if self.players[defender].is_minor {
                    self.players[attacker]
                        .envoys
                        .retain(|(minor, _)| *minor != defender);
                }
                if opened {
                    // Only the agreed signatories' fronts against the named
                    // target carry the Joint War's 30-turn commitment. A
                    // defensive-pact responder is a real belligerent, but it
                    // did not sign the joint-war deal and may make peace on
                    // the normal war timetable.
                    let committed_joint_war = joint_war_until.filter(|_| {
                        declared_principals.contains(&attacker)
                            && initial_defenders.contains(&defender)
                    });
                    self.open_war_front(
                        attacker,
                        defender,
                        WarDeclaration {
                            conflict,
                            declarer: pid,
                            target: other,
                            declared_front: attacker == pid && defender == other,
                            casus_belli: Some(profile.id.to_string()),
                            joint_war_until: committed_joint_war,
                        },
                    );
                }
            }
        }
        self.sync_war_log();
        if military_aid_request {
            self.open_native_aid_request(other, NativeCompetitionTrigger::WarWithGrievances);
        }
        Ok(())
    }

    pub(super) fn do_declare_war(&mut self, pid: usize, other: usize) -> Result<(), String> {
        self.start_war(
            pid,
            other,
            casus_belli_profile("surprise_war").unwrap(),
            None,
        )
    }

    pub(super) fn do_declare_war_with_casus_belli(
        &mut self,
        pid: usize,
        other: usize,
        casus_belli: &str,
    ) -> Result<(), String> {
        let Some(profile) = casus_belli_profile(casus_belli) else {
            return Err("unknown casus belli".into());
        };
        if !self.casus_belli_available(pid, other, profile.id) {
            return Err("casus belli requirements are not met".into());
        }
        self.start_war(pid, other, profile, None)?;
        self.add_historic_moment(pid, "MOMENT_WAR_DECLARED_USING_CASUS_BELLI");
        Ok(())
    }

    pub(super) fn do_denounce(&mut self, pid: usize, other: usize) -> Result<(), String> {
        if other == pid
            || other >= self.players.len()
            || !self.players[other].alive
            || !self.has_met(pid, other)
            || self.players[other].is_barbarian
            || self.players[pid].is_barbarian
            || self.is_at_war(pid, other)
            || self.are_friends(pid, other)
            || self.alliance_with(pid, other).is_some()
        {
            return Err("cannot denounce that player".into());
        }
        if self.players[pid]
            .denounced_until
            .get(&other)
            .is_some_and(|until| *until > self.turn)
        {
            return Err("player is already denounced".into());
        }
        let until = self.turn + self.standard_duration(30);
        self.players[pid].denounced_until.insert(other, until);
        self.players[pid].denounced_since.insert(other, self.turn);
        self.add_grievances(other, pid, 25.0);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn do_propose_deal(
        &mut self,
        pid: usize,
        other: usize,
        give_gold: f64,
        request_gold: f64,
        open_borders: bool,
        friendship: bool,
        peace: bool,
        alliance: Option<&str>,
    ) -> Result<(), String> {
        if other == pid
            || other >= self.players.len()
            || !self.players[other].alive
            || !self.has_met(pid, other)
            || self.players[other].is_minor
            || self.players[other].is_barbarian
            || self.players[pid].is_minor
            || self.players[pid].is_barbarian
            || !give_gold.is_finite()
            || !request_gold.is_finite()
            || give_gold < 0.0
            || request_gold < 0.0
            || give_gold > self.players[pid].gold
            || request_gold > self.players[other].gold
            || (peace && !self.is_at_war(pid, other))
            || (peace && self.emergency_war_pair(pid, other))
            || (peace && self.peace_available_at(pid, other).is_some())
            || ((friendship || open_borders || alliance.is_some()) && self.is_at_war(pid, other))
            || (open_borders
                && (self.tree_effect(pid, "open_borders") <= 0.0
                    || self.tree_effect(other, "open_borders") <= 0.0))
            || (friendship
                && (self.players[pid]
                    .denounced_until
                    .get(&other)
                    .is_some_and(|until| *until > self.turn)
                    || self.players[other]
                        .denounced_until
                        .get(&pid)
                        .is_some_and(|until| *until > self.turn)))
        {
            return Err("invalid diplomatic deal".into());
        }
        if let Some(kind) = alliance {
            if !matches!(
                kind,
                "research" | "cultural" | "economic" | "military" | "religious"
            ) || !self.players[pid]
                .civics
                .contains(&crate::name!("civil_service"))
                || !self.players[other]
                    .civics
                    .contains(&crate::name!("civil_service"))
                || (kind == "research"
                    && (self.tree_effect(pid, "research_agreements") <= 0.0
                        || self.tree_effect(other, "research_agreements") <= 0.0))
                || self.players[pid]
                    .alliances
                    .values()
                    .any(|alliance| alliance.ends > self.turn && alliance.kind == kind)
                || self.players[other]
                    .alliances
                    .values()
                    .any(|alliance| alliance.ends > self.turn && alliance.kind == kind)
            {
                return Err("alliance is unavailable".into());
            }
        }
        // A proposal that only gives Gold is a gift (legal, buys nothing —
        // see `validate_trade`); one that only asks is a demand and goes
        // through `Action::DemandGold`; an exchange goes through the trade
        // lane, where both sides must gain.
        let gift = give_gold > 0.0 && request_gold <= 0.0;
        if !open_borders && !friendship && !peace && alliance.is_none() && !gift {
            return Err("economic exchanges must use mutually favorable trade terms".into());
        }
        let id = self.next_deal_id;
        self.next_deal_id = self.next_deal_id.saturating_add(1);
        self.pending_deals.push(DiplomaticDeal {
            id,
            from: pid,
            to: other,
            give_gold,
            request_gold,
            open_borders,
            friendship,
            peace,
            alliance: alliance.map(str::to_string),
            defensive_pact: false,
            joint_war_target: None,
            promise: None,
            demand: false,
            expires: self.turn + self.standard_duration(10),
        });
        Ok(())
    }

    pub(super) fn major_diplomatic_counterpart(&self, pid: usize, other: usize) -> bool {
        pid != other
            && pid < self.players.len()
            && other < self.players.len()
            && self.players[pid].alive
            && self.players[other].alive
            && !self.players[pid].is_minor
            && !self.players[other].is_minor
            && !self.players[pid].is_barbarian
            && !self.players[other].is_barbarian
            && self.has_met(pid, other)
    }

    pub(super) fn queue_special_diplomatic_deal(
        &mut self,
        from: usize,
        to: usize,
        defensive_pact: bool,
        joint_war_target: Option<usize>,
        promise: Option<String>,
        demand_gold: Option<f64>,
    ) {
        let id = self.next_deal_id;
        self.next_deal_id = self.next_deal_id.saturating_add(1);
        self.pending_deals.push(DiplomaticDeal {
            id,
            from,
            to,
            give_gold: 0.0,
            request_gold: demand_gold.unwrap_or(0.0),
            open_borders: false,
            friendship: false,
            peace: false,
            alliance: None,
            defensive_pact,
            joint_war_target,
            promise,
            demand: demand_gold.is_some(),
            expires: self.turn + self.standard_duration(10),
        });
    }

    pub(super) fn do_send_diplomatic_mission(
        &mut self,
        pid: usize,
        other: usize,
        kind: &str,
    ) -> Result<(), String> {
        if !self.major_diplomatic_counterpart(pid, other) || self.is_at_war(pid, other) {
            return Err("cannot establish a diplomatic mission with that player".into());
        }
        let (cost, permitted) = match kind {
            "delegation" => (DELEGATION_GOLD, true),
            "embassy" => (
                EMBASSY_GOLD,
                self.players[pid]
                    .civics
                    .contains(&crate::name!("diplomatic_service")),
            ),
            _ => return Err("unknown diplomatic mission".into()),
        };
        if !permitted {
            return Err("Resident Embassies require Diplomatic Service".into());
        }
        if self.players[pid].gold + f64::EPSILON < cost {
            return Err("not enough gold for that diplomatic mission".into());
        }
        if let Some(mission) = self.players[pid].diplomatic_missions.get(&other) {
            // An Embassy deliberately replaces a Delegation, but an existing
            // Embassy cannot be downgraded by replaying an older delegation
            // action. The legal-action surface already hides both cases;
            // retain the invariant at the direct handler boundary too.
            if mission.kind == kind || kind == "delegation" {
                return Err("that diplomatic mission is already established".into());
            }
        }
        self.players[pid].gold -= cost;
        self.players[other].gold += cost;
        self.players[pid].diplomatic_missions.insert(
            other,
            DiplomaticMission {
                kind: kind.to_string(),
                sent: self.turn,
            },
        );
        let mission_name = if kind == "embassy" {
            "Resident Embassy"
        } else {
            "Delegation"
        };
        let message = format!(
            "{} sent a {} to {}",
            self.civ_name(pid),
            mission_name,
            self.civ_name(other)
        );
        self.note(pid, "Diplomacy", message.clone(), None);
        self.note(other, "Diplomacy", message, None);
        Ok(())
    }

    pub(super) fn do_send_delegation(&mut self, pid: usize, other: usize) -> Result<(), String> {
        self.do_send_diplomatic_mission(pid, other, "delegation")
    }

    pub(super) fn do_send_embassy(&mut self, pid: usize, other: usize) -> Result<(), String> {
        self.do_send_diplomatic_mission(pid, other, "embassy")
    }

    pub(super) fn defensive_pact_available(&self, first: usize, second: usize) -> bool {
        self.major_diplomatic_counterpart(first, second)
            && !self.is_at_war(first, second)
            && self.are_allied(first, second)
            && self.players[first]
                .civics
                .contains(&crate::name!("mobilization"))
            && self.players[second]
                .civics
                .contains(&crate::name!("mobilization"))
            && !self.has_defensive_pact(first, second)
    }

    pub(super) fn do_propose_defensive_pact(
        &mut self,
        pid: usize,
        other: usize,
    ) -> Result<(), String> {
        if !self.defensive_pact_available(pid, other) {
            return Err("a Defensive Pact is unavailable".into());
        }
        self.queue_special_diplomatic_deal(pid, other, true, None, None, None);
        Ok(())
    }

    pub(super) fn joint_war_available(&self, first: usize, second: usize, target: usize) -> bool {
        first != second
            && first != target
            && second != target
            && self.major_diplomatic_counterpart(first, second)
            && self.major_diplomatic_counterpart(first, target)
            && self.major_diplomatic_counterpart(second, target)
            && self.players[first]
                .civics
                .contains(&crate::name!("foreign_trade"))
            && self.players[second]
                .civics
                .contains(&crate::name!("foreign_trade"))
            && !self.is_at_war(first, target)
            && !self.is_at_war(second, target)
            && !self.are_friends(first, target)
            && !self.are_friends(second, target)
            && !self.are_allied(first, target)
            && !self.are_allied(second, target)
            && !self.emergency_coalition_pair(first, target)
            && !self.emergency_coalition_pair(second, target)
            && self.peace_treaty_until(first, target).is_none()
            && self.peace_treaty_until(second, target).is_none()
    }

    pub(super) fn do_propose_joint_war(
        &mut self,
        pid: usize,
        partner: usize,
        target: usize,
    ) -> Result<(), String> {
        if !self.joint_war_available(pid, partner, target) {
            return Err("a Joint War is unavailable".into());
        }
        self.queue_special_diplomatic_deal(pid, partner, false, Some(target), None, None);
        Ok(())
    }

    pub(super) fn do_request_promise(
        &mut self,
        pid: usize,
        other: usize,
        promise: &str,
    ) -> Result<(), String> {
        if !self.promise_request_available(pid, other, promise) {
            return Err("that diplomatic promise is unavailable".into());
        }
        self.reserve_promise_request(pid, other, promise);
        self.queue_special_diplomatic_deal(
            pid,
            other,
            false,
            None,
            Some(promise.to_string()),
            None,
        );
        Ok(())
    }

    pub(super) fn do_demand_gold(
        &mut self,
        pid: usize,
        other: usize,
        gold: f64,
    ) -> Result<(), String> {
        if !self.demand_available(pid, other)
            || !gold.is_finite()
            || gold <= 0.0
            || gold > self.players[other].gold
        {
            return Err("that demand is unavailable".into());
        }
        self.queue_special_diplomatic_deal(pid, other, false, None, None, Some(gold));
        Ok(())
    }

    pub(super) fn do_accept_deal(&mut self, pid: usize, deal_id: u32) -> Result<(), String> {
        let index = self
            .pending_deals
            .iter()
            .position(|deal| deal.id == deal_id && deal.to == pid)
            .ok_or_else(|| "no such incoming deal".to_string())?;
        // Clone first and validate the live state before removing an offer.
        // An expired or stale agreement remains rejectable; accepting it must
        // not make it vanish simply because a precondition changed.
        let deal = self.pending_deals[index].clone();
        if deal.expires < self.turn
            || self.players[deal.from].gold < deal.give_gold
            || self.players[deal.to].gold < deal.request_gold
        {
            return Err("deal can no longer be fulfilled".into());
        }
        let special_count = deal.defensive_pact as u8
            + deal.joint_war_target.is_some() as u8
            + deal.promise.is_some() as u8
            + deal.demand as u8;
        if special_count > 1 {
            return Err("diplomatic proposal combines incompatible requests".into());
        }
        if deal.defensive_pact && !self.defensive_pact_available(deal.from, deal.to) {
            return Err("a Defensive Pact is no longer available".into());
        }
        if let Some(target) = deal.joint_war_target {
            if !self.joint_war_available(deal.from, deal.to, target) {
                return Err("a Joint War is no longer available".into());
            }
        }
        if let Some(promise) = deal.promise.as_deref() {
            if !Self::valid_promise_kind(promise)
                || !self.major_diplomatic_counterpart(deal.from, deal.to)
                || self.is_at_war(deal.from, deal.to)
                || !self.promise_request_incident_exists(deal.from, deal.to, promise)
            {
                return Err("that diplomatic promise is no longer available".into());
            }
        }
        if deal.demand && !self.demand_available(deal.from, deal.to) {
            return Err("that demand is no longer available".into());
        }
        if deal.peace && self.emergency_war_pair(deal.from, deal.to) {
            return Err("active Emergency members cannot make peace with its target".into());
        }
        // An offer outlives the war it was written for. The sides can settle,
        // watch the treaty lapse and open a second war while the first offer
        // is still pending, so the peace clause has to answer for the war
        // actually being fought. Proposing already refuses both of these; an
        // acceptance that did not re-check them signed away a war on the turn
        // it was declared, and the ledger read as a dozen one-turn wars.
        if deal.peace {
            if !self.is_at_war(deal.from, deal.to) {
                return Err("that war is already over".into());
            }
            if let Some(until) = self.peace_available_at(deal.from, deal.to) {
                return Err(format!("this war cannot be settled before turn {until}"));
            }
        }
        self.pending_deals.remove(index);
        let mut peace_terms = Vec::new();
        if deal.give_gold > 0.0 {
            peace_terms.push(format!(
                "{} paid {} Gold to {}",
                self.civ_name(deal.from),
                deal.give_gold.round() as i64,
                self.civ_name(deal.to)
            ));
        }
        if deal.request_gold > 0.0 {
            peace_terms.push(format!(
                "{} paid {} Gold to {}",
                self.civ_name(deal.to),
                deal.request_gold.round() as i64,
                self.civ_name(deal.from)
            ));
        }
        if deal.open_borders {
            peace_terms.push(format!(
                "{} granted Open Borders to {}",
                self.civ_name(deal.from),
                self.civ_name(deal.to)
            ));
        }
        if deal.friendship {
            peace_terms.push("Declaration of Friendship".to_string());
        }
        if let Some(alliance) = deal.alliance.as_deref() {
            peace_terms.push(format!("{} Alliance", pretty(alliance)));
        }
        self.players[deal.from].gold += deal.request_gold - deal.give_gold;
        self.players[deal.to].gold += deal.give_gold - deal.request_gold;
        if Self::diplomatic_deal_is_gift(&deal) {
            self.record_gift(deal.from, deal.to);
        }
        if deal.peace {
            self.conclude_peace(deal.from, deal.to, peace_terms);
        }
        if deal.open_borders {
            let until = self.turn + self.standard_duration(30);
            self.players[deal.from]
                .open_borders_until
                .insert(deal.to, until);
        }
        if deal.friendship || deal.alliance.is_some() {
            let until = self.turn + self.standard_duration(30);
            self.players[deal.from].friends_until.insert(deal.to, until);
            self.players[deal.to].friends_until.insert(deal.from, until);
        }
        if let Some(kind) = deal.alliance {
            let from_key = Self::alliance_points_key(deal.to, &kind);
            let to_key = Self::alliance_points_key(deal.from, &kind);
            let stored_quarter_points = self.players[deal.from]
                .counters
                .get(&from_key)
                .copied()
                .unwrap_or(0)
                .max(
                    self.players[deal.to]
                        .counters
                        .get(&to_key)
                        .copied()
                        .unwrap_or(0),
                );
            let points = stored_quarter_points as f64 / 4.0;
            let state = AllianceState {
                kind,
                points,
                level: if points >= 240.0 {
                    3
                } else if points >= 80.0 {
                    2
                } else {
                    1
                },
                ends: self.turn + self.standard_duration(STANDARD_DEAL_TURNS),
            };
            self.players[deal.from]
                .alliances
                .insert(deal.to, state.clone());
            self.players[deal.to].alliances.insert(deal.from, state);
        }
        if deal.defensive_pact {
            let until = self.turn + self.standard_duration(STANDARD_DEAL_TURNS);
            self.players[deal.from]
                .defensive_pacts
                .insert(deal.to, until);
            self.players[deal.to]
                .defensive_pacts
                .insert(deal.from, until);
            let message = format!(
                "{} and {} signed a Defensive Pact",
                self.civ_name(deal.from),
                self.civ_name(deal.to)
            );
            self.note(deal.from, "Diplomacy", message.clone(), None);
            self.note(deal.to, "Diplomacy", message, None);
        }
        if let Some(target) = deal.joint_war_target {
            self.start_war(
                deal.from,
                target,
                casus_belli_profile("joint_war").unwrap(),
                Some(deal.to),
            )?;
            self.add_historic_moment(deal.from, "MOMENT_WAR_DECLARED_USING_CASUS_BELLI");
            self.add_historic_moment(deal.to, "MOMENT_WAR_DECLARED_USING_CASUS_BELLI");
        }
        if let Some(promise) = deal.promise {
            let until = self.turn + self.standard_duration(STANDARD_DEAL_TURNS);
            self.players[deal.to]
                .promises
                .entry(deal.from)
                .or_default()
                .insert(promise.clone(), until);
            let message = format!(
                "{} promised {} to {}",
                self.civ_name(deal.to),
                pretty(&promise),
                self.civ_name(deal.from)
            );
            self.note(deal.from, "Diplomacy", message.clone(), None);
            self.note(deal.to, "Diplomacy", message, None);
        }
        if deal.demand {
            let message = format!(
                "{} met {}'s Gold demand",
                self.civ_name(deal.to),
                self.civ_name(deal.from)
            );
            self.note(deal.from, "Diplomacy", message.clone(), None);
            self.note(deal.to, "Diplomacy", message, None);
        }
        Ok(())
    }

    pub(super) fn do_reject_deal(&mut self, pid: usize, deal_id: u32) -> Result<(), String> {
        let index = self
            .pending_deals
            .iter()
            .position(|deal| deal.id == deal_id && deal.to == pid)
            .ok_or_else(|| "no such incoming deal".to_string())?;
        let deal = self.pending_deals.remove(index);
        if deal.demand || deal.promise.is_some() {
            let category = deal.promise.as_deref().unwrap_or("demand");
            let grievances = self.escalating_grievance(
                deal.from,
                pid,
                &format!("request_refusal:{category}"),
                REQUEST_REFUSAL_FIRST_GRIEVANCES,
                REQUEST_REFUSAL_REPEAT_GRIEVANCES,
            );
            self.add_grievances(deal.from, pid, grievances);
            let subject = if deal.demand {
                "demand"
            } else {
                "promise request"
            };
            let message = format!(
                "{} refused {}'s {}",
                self.civ_name(pid),
                self.civ_name(deal.from),
                subject
            );
            self.note(deal.from, "Diplomacy", message.clone(), None);
            self.note(pid, "Diplomacy", message, None);
        }
        Ok(())
    }

    pub(super) fn promise_active(&self, promisor: usize, requester: usize, kind: &str) -> bool {
        self.players
            .get(promisor)
            .and_then(|player| player.promises.get(&requester))
            .and_then(|promises| promises.get(kind))
            .is_some_and(|until| *until > self.turn)
    }

    /// End one promise at its first violation and retain the victim's
    /// Retribution window.  The promise itself is removed before grievances
    /// are booked, so repeatedly performing the same forbidden action cannot
    /// farm an unlimited casus belli out of a single pledge.
    pub(super) fn break_promise(&mut self, promisor: usize, requester: usize, kind: &str) -> bool {
        if !self.promise_active(promisor, requester, kind) {
            return false;
        }
        let empty = {
            let promises = self.players[promisor]
                .promises
                .get_mut(&requester)
                .expect("active promise has an owner bucket");
            promises.remove(kind);
            promises.is_empty()
        };
        if empty {
            self.players[promisor].promises.remove(&requester);
        }
        let retribution_until = self.turn + self.standard_duration(STANDARD_DEAL_TURNS);
        self.players[promisor]
            .broken_promises_until
            .insert(requester, retribution_until);
        let grievances = self.escalating_grievance(
            requester,
            promisor,
            &format!("promise_broken:{kind}"),
            PROMISE_BROKEN_FIRST_GRIEVANCES,
            PROMISE_BROKEN_REPEAT_GRIEVANCES,
        );
        self.add_grievances(requester, promisor, grievances);
        let message = format!(
            "{} broke its promise of {} to {}",
            self.civ_name(promisor),
            pretty(kind),
            self.civ_name(requester)
        );
        self.note(promisor, "Diplomacy", message.clone(), None);
        self.note(requester, "Diplomacy", message, None);
        true
    }

    pub(super) fn break_promises_on_settlement(&mut self, founder: usize, position: Pos) {
        let requesters: Vec<usize> = self
            .players
            .iter()
            .filter(|player| {
                player.id != founder
                    && player.alive
                    && !player.is_minor
                    && !player.is_barbarian
                    && !player.is_free_city
                    && self.has_met(player.id, founder)
            })
            .filter(|player| {
                self.player_city_ids(player.id)
                    .into_iter()
                    .any(|city| self.wdist(self.cities[&city].pos, position) <= 9)
            })
            .map(|player| player.id)
            .collect();
        for requester in requesters {
            self.record_diplomatic_incident(requester, founder, "no_settling");
            self.break_promise(founder, requester, "no_settling");
        }
    }

    pub(super) fn break_promises_on_conversion(&mut self, converter: usize, city_owner: usize) {
        if converter != city_owner {
            self.record_diplomatic_incident(city_owner, converter, "no_conversion");
            self.break_promise(converter, city_owner, "no_conversion");
        }
    }

    pub(super) fn break_promises_on_spying(&mut self, spy_owner: usize, city_owner: usize) {
        if spy_owner != city_owner {
            self.record_diplomatic_incident(city_owner, spy_owner, "no_spying");
            self.break_promise(spy_owner, city_owner, "no_spying");
        }
    }

    pub(super) fn break_promises_on_city_state_attack(
        &mut self,
        attacker: usize,
        city_state: usize,
    ) {
        if !self
            .players
            .get(city_state)
            .is_some_and(|player| player.is_minor && !player.is_barbarian && !player.is_free_city)
        {
            return;
        }
        // The promise is scoped to city-states the requester actually
        // controls. Attacking an unrelated independent city-state is not a
        // breach of a promise made to a different Suzerain.
        if let Some(requester) = self.suzerain_of(city_state) {
            self.record_diplomatic_incident(requester, attacker, "no_city_state_attack");
            self.break_promise(attacker, requester, "no_city_state_attack");
        }
    }

    pub(super) fn empire_gold_per_turn(&self, pid: usize) -> f64 {
        // Deal quoting runs for many counterparties in headless games. Use a
        // conservative liquidity proxy rather than recursively rebuilding
        // every citizen plan and city yield for every candidate contract.
        let city_income = self.player_city_ids(pid).len() as f64 * 4.0;
        let treasury_income = self.players[pid].gold.max(0.0) / 100.0;
        (city_income + treasury_income
            - self.unit_gold_maintenance(pid)
            - self.infrastructure_gold_maintenance(pid)
            - self.nuclear_gold_maintenance(pid))
        .max(0.0)
    }

    /// Civ VI charges each unit its own upkeep. Corps/Fleets cost 150% of a
    /// base unit and Armies/Armadas cost 200%; policy reductions then apply
    /// once because each formation is a single unit on the map.
    pub(super) fn unit_gold_maintenance_cost(&self, pid: usize, unit: &Unit) -> f64 {
        // An arena has no empire behind it — no cities, no income, no
        // treasury — so an army on one costs nothing to keep. Charged the
        // ordinary upkeep, a Tactics side would run a deficit it could never
        // close and bankruptcy would disband the very units the mode exists
        // to fight with, one every ten turns, before either army reached the
        // other.
        if self.is_arena() {
            return 0.0;
        }
        let formation = match unit.formation {
            1 => 1.5,
            2.. => 2.0,
            _ => 1.0,
        };
        let surcharge = if self.rules.units[unit.kind].class == "military" {
            self.policy_effect(pid, "unit_maintenance_surcharge")
        } else {
            0.0
        };
        // The host's own per-type bill for this unit (`GetUnitMaintenance`
        // by formation, `ReportScreen.lua:314-334`) in place of the ruleset's
        // `maintenance × formation`; the discount and surcharge stay the
        // board's, exactly as that screen subtracts `GetMaintDiscountPerUnit`
        // afterwards. Absent in a native game and on an older mod.
        let base = match self
            .host_unit_facts
            .get(&unit.id)
            .and_then(|facts| facts.maintenance)
        {
            Some(bill) => bill.max(0.0),
            None => self.rules.units[unit.kind].maintenance * formation,
        };
        (base - self.policy_effect(pid, "unit_maintenance_discount")).max(0.0) + surcharge
    }

    pub(crate) fn unit_gold_maintenance(&self, pid: usize) -> f64 {
        // The treasury's own unit bill (`PlayerTreasury:GetUnitMaintenance`,
        // `ToolTipHelper_PlayerYields.lua:26`) when an authoritative host
        // exported it: every discount, every Spy, every formation as the host
        // charges them. The sum below is the board's model of that figure and
        // is what a native game, or an older mod, still pays.
        if let Some(total) = self.host_maintenance.get(&pid).and_then(|bill| bill.units) {
            return total.max(0.0);
        }
        let units = self
            .player_unit_ids(pid)
            .into_iter()
            .map(|uid| self.unit_gold_maintenance_cost(pid, &self.units[&uid]))
            .sum::<f64>();
        units
            + self.spies.values().filter(|spy| spy.owner == pid).count() as f64
                * self.rules.units["spy"].maintenance
    }

    pub(super) fn building_district_is_active(&self, city: &City, building: impl AsName) -> bool {
        let Some(family) = self.rules.buildings[building.as_name()].district else {
            return true;
        };
        if family == "city_center" {
            return true;
        }
        let wanted = self.district_family(family);
        city.districts.iter().any(|(district, position)| {
            self.district_family(*district) == wanted
                && self.district_is_active(city, district, *position)
        })
    }

    /// Pillaged districts and buildings are disabled and stop charging upkeep.
    /// Flood Barriers use their base 1 Gold multiplied by exposed lowlands and
    /// the current whole-meter sea-level multiplier.
    pub(super) fn city_infrastructure_gold_maintenance(&self, city: &City) -> f64 {
        let districts = city
            .districts
            .iter()
            .filter(|(district, position)| self.district_is_active(city, district, **position))
            .map(|(district, _)| self.rules.districts[district].maintenance)
            .sum::<f64>();
        let buildings = city
            .buildings
            .iter()
            .filter(|building| !city.pillaged_buildings.contains(*building))
            .filter(|building| self.building_district_is_active(city, building))
            .map(|building| {
                let base = self.rules.buildings[building].maintenance;
                if building == "flood_barrier" {
                    base * self.coastal_lowland_tiles(city).len() as f64
                        * (1 + self.climate_phase / 2) as f64
                } else {
                    base
                }
            })
            .sum::<f64>();
        districts + buildings
    }

    pub(crate) fn infrastructure_gold_maintenance(&self, pid: usize) -> f64 {
        // `GetBuildingMaintenance` + `GetDistrictMaintenance`
        // (`ToolTipHelper_PlayerYields.lua:22-24`) when both crossed; the
        // per-city sum otherwise.
        if let Some(bill) = self.host_maintenance.get(&pid) {
            if let (Some(buildings), Some(districts)) = (bill.buildings, bill.districts) {
                return (buildings + districts).max(0.0);
            }
        }
        self.cities
            .values()
            .filter(|city| city.owner == pid)
            .map(|city| self.city_infrastructure_gold_maintenance(city))
            .sum()
    }

    /// Snapshot the per-city infrastructure answers before a completion that
    /// may have empire-wide effects. Wonders are rare, so the defensive
    /// snapshot is cheaper than making ordinary upkeep derivation carry a
    /// global mutation ledger through every completion path.
    pub(super) fn city_upkeep_snapshot(&self, pid: usize) -> BTreeMap<u32, f64> {
        self.cities
            .values()
            .filter(|city| city.owner == pid)
            .map(|city| (city.id, self.city_infrastructure_gold_maintenance(city)))
            .collect()
    }

    pub(super) fn nuclear_gold_maintenance(&self, pid: usize) -> f64 {
        let player = &self.players[pid];
        let nuclear = player
            .counters
            .get("project_effect:nuclear_devices")
            .copied()
            .unwrap_or(0)
            .max(0) as f64;
        let thermonuclear = player
            .counters
            .get("project_effect:thermonuclear_devices")
            .copied()
            .unwrap_or(0)
            .max(0) as f64;
        (self.rules.wmds["nuclear_device"].maintenance * nuclear
            + self.rules.wmds["thermonuclear_device"].maintenance * thermonuclear)
            * (1.0 - self.policy_effect(pid, "nuclear_maintenance_discount_pct") / 100.0)
    }

    pub(super) fn contracted_gold_per_turn(&self, pid: usize) -> f64 {
        self.active_trade_deals
            .iter()
            .filter(|deal| deal.ends > self.turn)
            .map(|deal| {
                let outgoing = if deal.from == pid {
                    deal.offer.gold_per_turn
                } else if deal.to == pid {
                    deal.request.gold_per_turn
                } else {
                    0.0
                };
                let incoming = if deal.to == pid {
                    deal.offer.gold_per_turn
                } else if deal.from == pid {
                    deal.request.gold_per_turn
                } else {
                    0.0
                };
                incoming - outgoing
            })
            .sum()
    }

    /// Apply the recurring budget after city income and maintenance have been
    /// calculated. Civ VI never stores a negative treasury, and the two
    /// bankruptcy penalties start on different lines: the shipped
    /// `GOLD_NEGATIVE_BALANCE_AMENITY_LOSS_LINE` is **0**, so an empire in the
    /// red loses an Amenity everywhere the moment its balance goes negative,
    /// while `GOLD_NEGATIVE_BALANCE_DISBAND_UNIT_LINE` is **-10** and no unit
    /// is disbanded until then. Both step every -10 from their own line
    /// (`GOLD_NEGATIVE_BALANCE_SUBSEQUENT_AMENITY_LOSS` /
    /// `..._SUBSEQUENT_DISBAND_UNIT`), so the Amenity is always one ahead.
    pub(super) fn settle_gold_budget(&mut self, pid: usize, local_gold: f64) {
        let budget = local_gold + self.contracted_gold_per_turn(pid);
        let treasury = (self.players[pid].gold + local_gold).max(0.0);
        let deficit = if !self.players[pid].is_barbarian && treasury <= f64::EPSILON && budget < 0.0
        {
            -budget
        } else {
            0.0
        };
        let disbands = (deficit / 10.0).floor() as i64;
        let penalty = if deficit > 0.0 { 1 + disbands } else { 0 };
        {
            let player = &mut self.players[pid];
            player.gold = treasury;
            player.gold_per_turn = budget;
            player.bankruptcy_amenity_penalty = penalty;
        }
        if disbands <= 0 {
            return;
        }

        // The commercial game does not expose a player choice here. Pick the
        // costliest maintained units first, then by ID, so saves and replays
        // remain deterministic while each disband materially relieves upkeep.
        let mut candidates: Vec<(u32, f64)> = self
            .player_unit_ids(pid)
            .into_iter()
            .filter_map(|uid| {
                let maintenance = self.unit_gold_maintenance_cost(pid, &self.units[&uid]);
                (maintenance > 0.0).then_some((uid, maintenance))
            })
            .collect();
        candidates.sort_by(|left, right| {
            right
                .1
                .total_cmp(&left.1)
                .then_with(|| left.0.cmp(&right.0))
        });
        for (unit, _) in candidates.into_iter().take(disbands as usize) {
            self.remove_unit(unit);
        }
    }

    pub(super) fn primary_resource_value(&self, pid: usize, resource: &str) -> f64 {
        let Some(spec) = self.rules.resources.get(resource) else {
            return 0.0;
        };
        match spec.class.as_str() {
            "luxury" => {
                // One unique Luxury can serve four cities (six for Aztec), so
                // larger empires value a missing type more without a costly
                // full Amenity-allocation simulation inside every quote.
                let cities = self.player_city_ids(pid).len() as f64;
                135.0 + 10.0 * cities.min(8.0)
            }
            "strategic" => {
                let useful_units = self
                    .rules
                    .units
                    .values()
                    .filter(|unit| unit.requires_resource.as_deref() == Some(resource))
                    .filter(|unit| self.unlocked(pid, &unit.tech, &unit.civic))
                    .count() as f64;
                70.0 + 18.0 * useful_units.min(5.0) + 5.0 * self.player_era(pid) as f64
            }
            _ => 0.0,
        }
    }

    pub(super) fn resource_receive_value(&self, pid: usize, resource: &str, amount: i32) -> f64 {
        if amount <= 0 {
            return 0.0;
        }
        let first = self.resource_access_count(pid, resource) == 0;
        let primary = self.primary_resource_value(pid, resource);
        let duplicate = match self.rules.resources[resource].class.as_str() {
            "luxury" => 8.0,
            "strategic" => 24.0,
            _ => 0.0,
        };
        if first {
            primary + duplicate * (amount - 1).max(0) as f64
        } else {
            duplicate * amount as f64
        }
    }

    pub(super) fn resource_give_cost(&self, pid: usize, resource: &str, amount: i32) -> f64 {
        let available = self.resource_access_count(pid, resource);
        if amount <= 0 || available < amount {
            return f64::INFINITY;
        }
        let surplus_cost = match self.rules.resources[resource].class.as_str() {
            "luxury" => 24.0,
            "strategic" => 20.0,
            _ => return f64::INFINITY,
        };
        if available - amount > 0 {
            surplus_cost * amount as f64
        } else {
            self.primary_resource_value(pid, resource) * 1.05
        }
    }

    pub(super) fn favor_unit_value(&self, pid: usize) -> f64 {
        1.1 + 0.14 * self.world_era as f64 + 0.07 * self.players[pid].dvp.max(0) as f64
    }

    /// What one point of Diplomatic Favor is worth to `pid` in Gold by this
    /// engine's own book: the live seat's floor for a favor sale, which used
    /// to be a flat Gold a point.
    pub fn favor_gold_value(&self, pid: usize) -> f64 {
        self.favor_unit_value(pid)
    }

    /// What passage through another empire's territory is worth to
    /// `receiver` in Gold by this engine's book, read as if it were not yet
    /// open — a mirrored board can carry the very passage the live seat is
    /// about to buy. The live seat's ceiling for a passage purchase, which
    /// used to be whatever the treasury held.
    pub fn passage_gold_value(&self, receiver: usize) -> f64 {
        let tourism = (self.players[receiver].tourism_lifetime
            / self.turn.saturating_sub(1).max(1) as f64)
            .min(80.0);
        28.0 + tourism * 0.35
    }

    pub(super) fn open_borders_receive_value(&self, receiver: usize, grantor: usize) -> f64 {
        if self.has_open_borders(receiver, grantor) {
            return 0.0;
        }
        let tourism = (self.players[receiver].tourism_lifetime
            / self.turn.saturating_sub(1).max(1) as f64)
            .min(80.0);
        28.0 + tourism * 0.35
    }

    pub(super) fn open_borders_give_cost(&self, grantor: usize, receiver: usize) -> f64 {
        if self.has_open_borders(receiver, grantor) {
            return 0.0;
        }
        let own_power = self.military_power(grantor).max(1.0);
        let threat = (self.military_power(receiver) / own_power).clamp(0.0, 3.0);
        9.0 + 8.0 * threat
    }

    pub(super) fn tradable_great_work_kind(kind: &str) -> bool {
        matches!(
            kind,
            "writing" | "art" | "religious_art" | "artifact" | "music" | "relic"
        )
    }

    pub(super) fn great_work_inventory(&self, pid: usize, kind: &str) -> i32 {
        self.players[pid]
            .counters
            .get(&format!("great_work:{kind}"))
            .copied()
            .unwrap_or(0)
            .max(0) as i32
    }

    pub(super) fn housed_great_work_count(&self, pid: usize, kind: &str) -> i32 {
        self.housed_great_works(pid)
            .values()
            .map(|works| works.get(kind).copied().unwrap_or(0))
            .sum::<usize>() as i32
    }

    pub(super) fn great_work_receive_value(&self, pid: usize, kind: &str) -> f64 {
        let tourism = self.great_work_tourism(pid, kind);
        let (culture, faith) = match kind {
            "writing" => (2.0, 0.0),
            "art" | "religious_art" | "artifact" => (3.0, 0.0),
            "music" => (4.0, 0.0),
            "relic" => (0.0, 4.0),
            _ => return 0.0,
        };
        // Permanent culture assets are valued over a substantial share of a
        // Standard-speed game. Player-specific Tourism modifiers (notably
        // Printing) make mutually useful transfers possible without assigning
        // every civilization the same fixed price.
        60.0 + 32.0 * tourism + 18.0 * culture + 12.0 * faith
    }

    pub(super) fn great_work_give_cost(&self, pid: usize, kind: &str) -> f64 {
        let scarcity = if self.great_work_inventory(pid, kind) <= 1 {
            1.10
        } else {
            0.90
        };
        self.great_work_receive_value(pid, kind) * scarcity
    }

    pub(super) fn captured_spy_receive_value(&self, pid: usize, spy_id: u32) -> f64 {
        self.spies.get(&spy_id).map_or(0.0, |spy| {
            if spy.owner != pid || spy.captured_by.is_none() {
                0.0
            } else {
                // Recovery turns an occupied but unusable capacity slot back
                // into a productive operative.
                325.0 + 70.0 * spy.level.max(0) as f64 + 35.0 * spy.promotions.len() as f64
            }
        })
    }

    pub(super) fn captured_spy_give_cost(&self, captor: usize, spy_id: u32) -> f64 {
        self.spies.get(&spy_id).map_or(f64::INFINITY, |spy| {
            if spy.captured_by != Some(captor) {
                f64::INFINITY
            } else {
                // A captive has bargaining value but cannot work for the
                // captor, so release is substantially cheaper than recovery.
                90.0 + 30.0 * spy.level.max(0) as f64 + 15.0 * spy.promotions.len() as f64
            }
        })
    }

    /// Stable equivalent-Gold value for a permanent city transfer. The
    /// valuation deliberately follows live infrastructure and yields rather
    /// than population alone, so a developed small city is not priced like a
    /// fresh settlement and multi-city terms compose additively.
    pub(super) fn city_trade_base_value(&self, city_id: u32) -> f64 {
        let Some(city) = self.cities.get(&city_id) else {
            return 0.0;
        };
        let yields = self.city_yields(city_id);
        let recurring = yields.food
            + yields.production * 1.5
            + yields.gold
            + yields.science * 1.4
            + yields.culture * 1.4
            + yields.faith * 0.8;
        450.0
            + city.pop.max(1) as f64 * 105.0
            + city.owned_tiles.len() as f64 * 8.0
            + city.districts.len() as f64 * 150.0
            + city.buildings.len() as f64 * 65.0
            + city.wonders.len() as f64 * 350.0
            + city.products.len() as f64 * 100.0
            + recurring * 18.0
    }

    pub(super) fn city_trade_receive_value(&self, receiver: usize, city_id: u32) -> f64 {
        let Some(city) = self.cities.get(&city_id) else {
            return 0.0;
        };
        let nearest = self
            .player_city_ids(receiver)
            .into_iter()
            .map(|owned| self.wdist(self.cities[&owned].pos, city.pos))
            .min()
            .unwrap_or(12);
        let distance_factor = (1.08 - nearest.min(24) as f64 * 0.006).clamp(0.90, 1.05);
        self.city_trade_base_value(city_id) * distance_factor
    }

    pub(super) fn city_trade_give_cost(&self, city_id: u32) -> f64 {
        // A modest liquidity discount leaves room for a genuinely beneficial
        // purchase without making automatic city dumping attractive.
        self.city_trade_base_value(city_id) * 0.88
    }

    /// Whether the receiver can house the post-trade inventory. Typed slots
    /// are exclusive except that Art and Religious Art share museum slots;
    /// universal slots absorb only the remaining overflow.
    pub(super) fn great_work_receipts_fit(
        &self,
        receiver: usize,
        incoming: &DealItems,
        outgoing: &DealItems,
    ) -> bool {
        let adjusted = |kind: &str| {
            self.great_work_inventory(receiver, kind)
                + incoming.great_works.get(kind).copied().unwrap_or(0)
                - outgoing.great_works.get(kind).copied().unwrap_or(0)
        };
        let mut typed = BTreeMap::<String, i32>::new();
        let mut any = 0;
        for (_, slot) in self.great_work_slots(receiver) {
            if slot == "any" {
                any += 1;
            } else {
                *typed.entry(slot).or_insert(0) += 1;
            }
        }
        let art = adjusted("art") + adjusted("religious_art");
        if art < 0 {
            return false;
        }
        let art_slots = typed.get("art").copied().unwrap_or(0)
            + typed.get("religious_art").copied().unwrap_or(0);
        let mut overflow = (art - art_slots).max(0);
        for kind in ["writing", "artifact", "music", "relic"] {
            let count = adjusted(kind);
            if count < 0 {
                return false;
            }
            overflow += (count - typed.get(kind).copied().unwrap_or(0)).max(0);
        }
        overflow <= any
    }

    pub(super) fn receive_items_value(&self, pid: usize, other: usize, items: &DealItems) -> f64 {
        let resources = items
            .resources
            .iter()
            .map(|(resource, amount)| self.resource_receive_value(pid, resource, *amount))
            .sum::<f64>();
        let great_works = items
            .great_works
            .iter()
            .map(|(kind, amount)| self.great_work_receive_value(pid, kind) * *amount as f64)
            .sum::<f64>();
        let captured_spies = items
            .captured_spies
            .iter()
            .map(|spy| self.captured_spy_receive_value(pid, *spy))
            .sum::<f64>();
        let cities = items
            .cities
            .iter()
            .map(|city| self.city_trade_receive_value(pid, *city))
            .sum::<f64>();
        items.gold
            + 25.0 * items.gold_per_turn
            + self.favor_unit_value(pid) * items.diplomatic_favor
            + resources
            + great_works
            + captured_spies
            + cities
            + if items.open_borders {
                self.open_borders_receive_value(pid, other)
            } else {
                0.0
            }
    }

    pub(super) fn give_items_cost(&self, pid: usize, other: usize, items: &DealItems) -> f64 {
        let resources = items
            .resources
            .iter()
            .map(|(resource, amount)| self.resource_give_cost(pid, resource, *amount))
            .sum::<f64>();
        let great_works = items
            .great_works
            .iter()
            .map(|(kind, amount)| self.great_work_give_cost(pid, kind) * *amount as f64)
            .sum::<f64>();
        let captured_spies = items
            .captured_spies
            .iter()
            .map(|spy| self.captured_spy_give_cost(pid, *spy))
            .sum::<f64>();
        let cities = items
            .cities
            .iter()
            .map(|city| self.city_trade_give_cost(*city))
            .sum::<f64>();
        items.gold
            + 25.0 * items.gold_per_turn
            + self.favor_unit_value(pid) * items.diplomatic_favor
            + resources
            + great_works
            + captured_spies
            + cities
            + if items.open_borders {
                self.open_borders_give_cost(pid, other)
            } else {
                0.0
            }
    }

    /// Net equivalent-Gold utility to `(from, to)`. Every executable trade
    /// must remain strictly favorable to both parties at the moment it closes.
    pub fn trade_utilities(
        &self,
        from: usize,
        to: usize,
        offer: &DealItems,
        request: &DealItems,
    ) -> (f64, f64) {
        (
            self.receive_items_value(from, to, request) - self.give_items_cost(from, to, offer),
            self.receive_items_value(to, from, offer) - self.give_items_cost(to, from, request),
        )
    }

    pub(super) fn items_are_valid(&self, owner: usize, items: &DealItems) -> bool {
        let finite_nonnegative = |value: f64| value.is_finite() && value >= 0.0;
        if !finite_nonnegative(items.gold)
            || !finite_nonnegative(items.gold_per_turn)
            || !finite_nonnegative(items.diplomatic_favor)
            || items.gold + items.gold_per_turn > self.players[owner].gold
            || items.gold_per_turn > self.empire_gold_per_turn(owner).max(0.0)
            || items.diplomatic_favor > self.players[owner].diplomatic_favor
        {
            return false;
        }
        let unique_spies = items
            .captured_spies
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let unique_cities = items.cities.iter().copied().collect::<BTreeSet<_>>();
        items.resources.iter().all(|(resource, amount)| {
            *amount > 0
                && self.rules.resources.get(resource).is_some_and(|spec| {
                    matches!(spec.class.as_str(), "luxury" | "strategic")
                        && self.resource_visible_to(owner, resource)
                        && self.resource_access_count(owner, resource) >= *amount
                })
        }) && items.great_works.iter().all(|(kind, amount)| {
            *amount > 0
                && Self::tradable_great_work_kind(kind)
                && self.great_work_inventory(owner, kind) >= *amount
                && self.housed_great_work_count(owner, kind) >= *amount
        }) && unique_spies.len() == items.captured_spies.len()
            && items.captured_spies.iter().all(|spy| {
                self.spies
                    .get(spy)
                    .is_some_and(|agent| agent.captured_by == Some(owner))
            })
            && unique_cities.len() == items.cities.len()
            && (items.cities.is_empty()
                || (items.cities.len() < self.player_city_ids(owner).len()
                    && items.cities.iter().all(|city_id| {
                        self.cities.get(city_id).is_some_and(|city| {
                            city.owner == owner
                                && !city.is_capital
                                && !self.city_has_palace(city)
                                && city.captured_from.is_none()
                                && !self.active_emergencies.iter().any(|emergency| {
                                    emergency.ends > self.turn && emergency.city == *city_id
                                })
                        })
                    })))
    }

    pub(super) fn strategic_receipts_fit(&self, receiver: usize, items: &DealItems) -> bool {
        let capacity = self.strategic_stockpile_capacity(receiver);
        items.resources.iter().all(|(resource, amount)| {
            self.rules.resources.get(resource).is_none_or(|spec| {
                spec.class != "strategic"
                    || self.strategic_stockpile(receiver, Name::new(resource)) + *amount as f64
                        <= capacity + f64::EPSILON
            })
        })
    }

    pub(super) fn validate_trade(
        &self,
        from: usize,
        to: usize,
        offer: &DealItems,
        request: &DealItems,
    ) -> Result<(f64, f64), String> {
        if from == to
            || to >= self.players.len()
            || !self.players[from].alive
            || !self.players[to].alive
            || !self.has_met(from, to)
            || self.players[from].is_minor
            || self.players[to].is_minor
            || self.players[from].is_barbarian
            || self.players[to].is_barbarian
            || self.is_at_war(from, to)
            || (offer.is_empty() && request.is_empty())
            // A one-sided deal that only TAKES is a demand, and a demand is
            // `Action::DemandGold`: refusable, with a grievance. It never
            // executes as a trade.
            || (offer.is_empty() && !request.is_empty())
            || !self.items_are_valid(from, offer)
            || !self.items_are_valid(to, request)
            || offer
                .captured_spies
                .iter()
                .any(|spy| self.spies.get(spy).is_none_or(|agent| agent.owner != to))
            || request
                .captured_spies
                .iter()
                .any(|spy| self.spies.get(spy).is_none_or(|agent| agent.owner != from))
            || !self.strategic_receipts_fit(to, offer)
            || !self.strategic_receipts_fit(from, request)
            || !self.great_work_receipts_fit(to, offer, request)
            || !self.great_work_receipts_fit(from, request, offer)
            || (offer.gold > 0.0 && request.gold > 0.0)
            || (offer.gold_per_turn > 0.0 && request.gold_per_turn > 0.0)
            || (offer.diplomatic_favor > 0.0 && request.diplomatic_favor > 0.0)
            || offer
                .resources
                .keys()
                .any(|resource| request.resources.contains_key(resource))
            || offer
                .great_works
                .keys()
                .any(|kind| request.great_works.contains_key(kind))
            || offer
                .captured_spies
                .iter()
                .any(|spy| request.captured_spies.contains(spy))
            || offer
                .cities
                .iter()
                .any(|city| request.cities.contains(city))
            // Great Works housed in a traded city already follow that city.
            // Keeping explicit Great Work terms separate prevents the same
            // work from being counted once as housed cargo and again by kind.
            || ((!offer.cities.is_empty() || !request.cities.is_empty())
                && (!offer.great_works.is_empty() || !request.great_works.is_empty()))
        {
            return Err("invalid trade terms".into());
        }
        if (offer.open_borders || request.open_borders)
            && (self.tree_effect(from, "open_borders") <= 0.0
                || self.tree_effect(to, "open_borders") <= 0.0)
        {
            return Err("Open Borders requires Early Empire for both civilizations".into());
        }
        let utilities = self.trade_utilities(from, to, offer, request);
        // ★ A GIFT IS LEGAL, AND IT BUYS NOTHING — Civilization VI's rule, and
        // since 2026-08-24 this engine's. A one-sided deal that only GIVES
        // proposes and the recipient accepts; the game's own database carries
        // no diplomatic modifier for a gift (a delegation and a demand, yes),
        // so `relationship_opinion` has none either. Until now the engine
        // refused the gift outright, which was stricter than the game it
        // mirrors and hid the question the live seat actually faces. An
        // exchange must still pay both sides; the AI never gives without
        // receiving (`gifts_given` is the counter that says so).
        let gift = request.is_empty();
        if utilities.1 <= 0.25 || (!gift && utilities.0 <= 0.25) {
            return Err("both civilizations must benefit from the trade".into());
        }
        Ok(utilities)
    }

    pub(super) fn transfer_gold(&mut self, payer: usize, receiver: usize, amount: f64) {
        let paid = amount.min(self.players[payer].gold).max(0.0);
        self.players[payer].gold -= paid;
        self.players[receiver].gold += paid;
    }

    pub(super) fn transfer_strategic_items(
        &mut self,
        payer: usize,
        receiver: usize,
        items: &DealItems,
    ) {
        for (resource, amount) in &items.resources {
            if self.rules.resources[resource].class != "strategic" {
                continue;
            }
            let quantity = *amount as f64;
            let payer_stock = self.strategic_stockpile(payer, Name::new(resource));
            let receiver_stock = self.strategic_stockpile(receiver, Name::new(resource));
            self.players[payer]
                .strategic_resources
                .insert(Name::new(resource), payer_stock - quantity);
            self.players[receiver]
                .strategic_resources
                .insert(Name::new(resource), receiver_stock + quantity);
        }
    }

    pub(super) fn transfer_great_work_items(
        &mut self,
        payer: usize,
        receiver: usize,
        items: &DealItems,
    ) {
        for (kind, amount) in &items.great_works {
            for _ in 0..*amount {
                self.move_great_work(payer, receiver, kind);
            }
        }
    }

    pub(super) fn transfer_captured_spies(
        &mut self,
        captor: usize,
        owner: usize,
        items: &DealItems,
    ) {
        let home = self.spy_home_city(owner);
        for spy_id in &items.captured_spies {
            let Some(spy) = self.spies.get_mut(spy_id) else {
                continue;
            };
            if spy.captured_by != Some(captor) || spy.owner != owner {
                continue;
            }
            spy.captured_by = None;
            spy.city = home;
            spy.ready_turn = self.turn;
            spy.mission = None;
            spy.sources_city = None;
            spy.sources_until = 0;
        }
    }

    pub(super) fn peaceful_city_transfer_destination(
        &self,
        unit_id: u32,
        transferred_city: u32,
    ) -> Option<Pos> {
        let unit = self.units.get(&unit_id)?;
        let spec = &self.rules.units[unit.kind];
        if spec.domain.as_deref() == Some("air") {
            return self
                .map
                .tiles
                .keys()
                .copied()
                .filter(|position| {
                    self.map.tiles[position].owner_city != Some(transferred_city)
                        && self.can_air_base_at(unit.owner, *position, Some(unit_id))
                })
                .min_by_key(|position| (self.wdist(unit.pos, *position), *position));
        }

        let want_sea = spec.domain.as_deref() == Some("sea");
        self.map
            .tiles
            .iter()
            .filter(|(position, tile)| {
                if tile.owner_city == Some(transferred_city)
                    || !self.rules.is_passable(tile)
                    || self.rules.is_water(tile) != want_sea
                {
                    return false;
                }
                let territory_owner = tile
                    .owner_city
                    .and_then(|city| self.cities.get(&city))
                    .map(|city| city.owner);
                if territory_owner.is_some_and(|owner| owner != unit.owner) {
                    return false;
                }
                self.unit_ids_at(**position).iter().all(|other_id| {
                    *other_id == unit_id
                        || (self.units[other_id].owner == unit.owner
                            && self.rules.units[self.units[other_id].kind].class != spec.class)
                })
            })
            .map(|(position, tile)| {
                let friendly = tile
                    .owner_city
                    .and_then(|city| self.cities.get(&city))
                    .is_some_and(|city| city.owner == unit.owner);
                (
                    (!friendly) as u8,
                    self.wdist(unit.pos, *position),
                    *position,
                )
            })
            .min()
            .map(|(_, _, position)| position)
    }

    pub(super) fn evacuate_peaceful_city_transfer(&mut self, city_id: u32, owner: usize) {
        let Some(city) = self.cities.get(&city_id) else {
            return;
        };
        let city_tiles = city.owned_tiles.iter().copied().collect::<BTreeSet<_>>();
        let mut units: Vec<u32> = self
            .units
            .values()
            .filter(|unit| unit.owner == owner && city_tiles.contains(&unit.pos))
            .map(|unit| unit.id)
            .collect();
        // Move carriers and other bases before their aircraft so the latter
        // can select the newly relocated carrier as a valid destination.
        units.sort_by_key(|unit| {
            (
                self.rules.units[self.units[unit].kind].domain.as_deref() == Some("air"),
                *unit,
            )
        });
        for unit_id in units {
            if let Some(destination) = self.peaceful_city_transfer_destination(unit_id, city_id) {
                self.relocate(unit_id, destination);
            } else {
                // A malformed scenario with no legal remaining base must not
                // hand the seller's unit to the buyer.
                self.remove_unit(unit_id);
            }
        }
    }

    pub(super) fn transfer_city_items(&mut self, owner: usize, receiver: usize, items: &DealItems) {
        for city_id in &items.cities {
            let returned_grievance = self.cities.get(city_id).and_then(|city| {
                (city.owner == owner && city.occupied_from == Some(receiver)).then(|| {
                    city.occupation_grievance
                        .unwrap_or_else(|| self.city_capture_base_grievances(*city_id))
                })
            });
            if self
                .cities
                .get(city_id)
                .is_some_and(|city| city.owner == owner)
            {
                self.evacuate_peaceful_city_transfer(*city_id, owner);
                self.transfer_city(*city_id, receiver, false);
                // Returning an occupied city is distinct from liberation:
                // it removes the capture grievance with its former owner,
                // rather than granting the global goodwill of a liberation.
                if let Some(grievance) = returned_grievance {
                    self.relieve_direct_grievances(receiver, owner, grievance);
                }
            }
        }
    }

    pub(super) fn do_trade(
        &mut self,
        from: usize,
        to: usize,
        offer: &DealItems,
        request: &DealItems,
    ) -> Result<(), String> {
        self.validate_trade(from, to, offer, request)?;

        self.transfer_gold(from, to, offer.gold + offer.gold_per_turn);
        self.transfer_gold(to, from, request.gold + request.gold_per_turn);
        self.players[from].diplomatic_favor -= offer.diplomatic_favor;
        self.players[to].diplomatic_favor += offer.diplomatic_favor;
        self.players[to].diplomatic_favor -= request.diplomatic_favor;
        self.players[from].diplomatic_favor += request.diplomatic_favor;

        self.transfer_strategic_items(from, to, offer);
        self.transfer_strategic_items(to, from, request);
        self.transfer_great_work_items(from, to, offer);
        self.transfer_great_work_items(to, from, request);
        self.transfer_captured_spies(from, to, offer);
        self.transfer_captured_spies(to, from, request);
        self.transfer_city_items(from, to, offer);
        self.transfer_city_items(to, from, request);

        let mut timed_offer = offer.clone();
        timed_offer
            .resources
            .retain(|resource, _| self.rules.resources[resource].class == "luxury");
        timed_offer.great_works.clear();
        timed_offer.captured_spies.clear();
        timed_offer.cities.clear();
        let mut timed_request = request.clone();
        timed_request
            .resources
            .retain(|resource, _| self.rules.resources[resource].class == "luxury");
        timed_request.great_works.clear();
        timed_request.captured_spies.clear();
        timed_request.cities.clear();

        let timed = timed_offer.gold_per_turn > 0.0
            || timed_request.gold_per_turn > 0.0
            || !timed_offer.resources.is_empty()
            || !timed_request.resources.is_empty()
            || offer.open_borders
            || request.open_borders;
        if timed {
            let id = self.next_deal_id;
            self.next_deal_id = self.next_deal_id.saturating_add(1);
            self.active_trade_deals.push(ActiveTradeDeal {
                id,
                from,
                to,
                offer: timed_offer,
                request: timed_request,
                started: self.turn,
                ends: self.turn + self.standard_duration(STANDARD_DEAL_TURNS),
            });
        }
        *self.players[from]
            .counters
            .entry("trades_completed".to_string())
            .or_insert(0) += 1;
        *self.players[to]
            .counters
            .entry("trades_completed".to_string())
            .or_insert(0) += 1;
        if request.is_empty() {
            self.record_gift(from, to);
        }
        Ok(())
    }

    /// The gift ledger: what a seat gave for nothing, and what it was given.
    /// A controller that reads `gifts_given` above zero on its own seat has
    /// done something no controller here is meant to do.
    pub(super) fn record_gift(&mut self, from: usize, to: usize) {
        bump(&mut self.players[from], "gifts_given");
        bump(&mut self.players[to], "gifts_received");
    }

    /// A diplomatic deal that only hands over Gold: legal as a gift, worth
    /// nothing to the relationship, counted on the ledger.
    pub(super) fn diplomatic_deal_is_gift(deal: &DiplomaticDeal) -> bool {
        deal.give_gold > 0.0
            && deal.request_gold <= 0.0
            && !deal.open_borders
            && !deal.friendship
            && !deal.peace
            && deal.alliance.is_none()
            && !deal.defensive_pact
            && deal.joint_war_target.is_none()
            && deal.promise.is_none()
            && !deal.demand
    }

    pub(super) fn quoted_payment(&self, payer: usize, price: f64) -> Option<DealItems> {
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

    pub(super) fn quote_asset_trade(
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
            let payment = self.quoted_payment(partner, (minimum + maximum) / 2.0)?;
            (asset, payment)
        } else {
            let minimum = self.give_items_cost(partner, viewer, &asset);
            let maximum = self.receive_items_value(viewer, partner, &asset);
            if maximum <= minimum + 1.0 {
                return None;
            }
            let payment = self.quoted_payment(viewer, (minimum + maximum) / 2.0)?;
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

    /// Gold per turn this empire has already promised away in live contracts.
    /// `empire_gold_per_turn` is a deliberately cheap liquidity proxy that does
    /// not know about them, so a quote that ignored this could sell the same
    /// income twice over.
    pub(super) fn committed_gold_per_turn(&self, pid: usize) -> f64 {
        self.active_trade_deals
            .iter()
            .filter(|deal| deal.ends > self.turn)
            .map(|deal| {
                if deal.from == pid {
                    deal.offer.gold_per_turn
                } else if deal.to == pid {
                    deal.request.gold_per_turn
                } else {
                    0.0
                }
            })
            .sum()
    }

    /// Price one cancellable asset at the buyer's own walk-away, settled
    /// entirely in lump Gold.
    ///
    /// `quoted_payment` splits a price down the middle and pays part of it as
    /// Gold per turn, which is the right shape for a contract both sides mean
    /// to keep. Neither half is right on the eve of a declaration: the
    /// instalments would be cancelled along with everything else, and the
    /// seller is parting with something the war hands straight back. The only
    /// question left is how much lump Gold the buyer will pay before its own
    /// valuation says no.
    ///
    /// The Gold-per-turn rider covers a buyer richer than the asset is worth to
    /// it. Twenty-five Gold of promised income lifts the buyer's ceiling by 25
    /// and the seller's stated cost by the same 25, so it buys no free margin;
    /// what it does is reach a treasury the asset alone could not, for the
    /// price of the single instalment `do_trade` settles at signing.
    pub(super) fn war_eve_quote(
        &self,
        viewer: usize,
        target: usize,
        category: &str,
        item: &str,
        asset: DealItems,
        rider_cap: f64,
    ) -> Option<QuickDeal> {
        let cost = self.give_items_cost(viewer, target, &asset);
        let value = self.receive_items_value(target, viewer, &asset);
        // Two Gold of headroom, not one: the price is floored to a whole Gold
        // below the buyer's ceiling, and a margin thinner than the rounding
        // cannot survive it.
        if !cost.is_finite() || value <= cost + 2.0 {
            return None;
        }
        let reserve = (self.players[target].gold * 0.30).min(40.0);
        let spendable = (self.players[target].gold - reserve).max(0.0).floor();
        if spendable <= cost + 2.0 {
            return None;
        }
        // Round the rider *down* to the tenth `quoted_payment` already quotes
        // in. Rounding up would ask for a ceiling above the buyer's treasury
        // and cost more in stated value than the extra reach is worth; rounding
        // down leaves at most 2.5 Gold on the table and can never eat a margin
        // the checks above have already proved.
        let reach = if spendable > value - 1.0 {
            (spendable - value + 1.0) / 25.0
        } else {
            0.0
        };
        let rider = ((reach.min(rider_cap) * 10.0).floor() / 10.0).max(0.0);
        let mut offer = asset;
        offer.gold_per_turn = rider;
        let price = spendable.min(value + 25.0 * rider - 1.0).floor();
        if price <= cost + 25.0 * rider {
            return None;
        }
        let request = DealItems {
            gold: price,
            ..DealItems::default()
        };
        let (my_value, partner_value) =
            self.validate_trade(viewer, target, &offer, &request).ok()?;
        Some(QuickDeal {
            partner: target,
            category: category.to_string(),
            item: item.to_string(),
            direction: "sell".to_string(),
            offer,
            request,
            my_value,
            partner_value,
        })
    }

    /// Every quote that hands `target` only a commitment the coming
    /// declaration voids, and takes back only value that has already settled
    /// when it lands.
    ///
    /// A luxury copy, Open Borders, and Gold per turn all run for
    /// `STANDARD_DEAL_TURNS`, and `end_bilateral_relations_for_war` cancels
    /// every live contract between two civilizations that go to war — the
    /// seller's resource access is restored on the spot and the remaining
    /// instalments are never paid. Lump Gold, by contrast, `do_trade` pays into
    /// the treasury immediately and the war cannot reach it. Selling the first
    /// for the second on the turn a war opens is an ordinary Civ VI line, and
    /// the part worth stating is that it is not a gamble: `validate_trade`
    /// still requires both empires to profit at the price quoted here, so a
    /// declaration that never comes leaves a plain good trade behind.
    ///
    /// Strategic resources, Great Works, captured Spies, cities, and Favor are
    /// deliberately absent from the offer side. `do_trade` transfers all of
    /// them the moment the contract closes, so a war would not bring them back.
    ///
    /// Quotes are independent of one another and every one of them assumes the
    /// buyer's whole spendable treasury and this empire's whole uncommitted
    /// income. Execute one, then ask again.
    pub fn war_eve_deals(&self, viewer: usize, target: usize) -> Vec<QuickDeal> {
        if viewer >= self.players.len()
            || target >= self.players.len()
            || viewer == target
            || !self.players[viewer].alive
            || !self.players[target].alive
            || self.players[viewer].is_minor
            || self.players[viewer].is_barbarian
            || self.players[target].is_minor
            || self.players[target].is_barbarian
            || !self.has_met(viewer, target)
            || self.is_at_war(viewer, target)
        {
            return Vec::new();
        }
        // A rider is only sellable income this empire both earns and has not
        // already promised elsewhere, and `items_are_valid` also requires the
        // treasury to cover the instalment paid at signing.
        let rider_cap = (self.empire_gold_per_turn(viewer) - self.committed_gold_per_turn(viewer))
            .max(0.0)
            .min(self.players[viewer].gold.max(0.0));
        let mut deals = Vec::new();
        // `resource_access_count` walks every tile of every city, and sweeping
        // the whole luxury table with it is the exact pattern
        // `connected_resource_census` exists to avoid. `empire_luxury_names` is
        // that one pass, memoized, and it is already the set of Luxuries this
        // empire has of its own — including a Suzerain's and Amani's, and
        // excluding copies it merely leases in, which price out as a last copy
        // anyway.
        for resource in self.empire_luxury_names(viewer) {
            if !self.resource_visible_to(viewer, &resource)
                || !self.resource_visible_to(target, &resource)
            {
                continue;
            }
            let mut asset = DealItems::default();
            asset.resources.insert(resource.to_string(), 1);
            if let Some(deal) =
                self.war_eve_quote(viewer, target, "luxury", &resource, asset, rider_cap)
            {
                deals.push(deal);
            }
        }
        if self.tree_effect(viewer, "open_borders") > 0.0
            && self.tree_effect(target, "open_borders") > 0.0
            && !self.has_open_borders(target, viewer)
        {
            let borders = DealItems {
                open_borders: true,
                ..DealItems::default()
            };
            if let Some(deal) = self.war_eve_quote(
                viewer,
                target,
                "agreement",
                "open_borders",
                borders,
                rider_cap,
            ) {
                deals.push(deal);
            }
        }
        deals.sort_by(|left, right| {
            Self::war_eve_net_gold(right)
                .partial_cmp(&Self::war_eve_net_gold(left))
                .unwrap()
                .then_with(|| left.item.cmp(&right.item))
        });
        deals
    }

    /// What a war-eve quote is actually worth once the declaration lands: the
    /// lump payment, less the one Gold-per-turn instalment `do_trade` settles
    /// before the war cancels the rest.
    pub fn war_eve_net_gold(deal: &QuickDeal) -> f64 {
        deal.request.gold - deal.offer.gold_per_turn
    }

    pub(super) fn process_trade_deals(&mut self, pid: usize) {
        self.active_trade_deals.retain(|deal| deal.ends > self.turn);
        let payments: Vec<(usize, usize, f64)> = self
            .active_trade_deals
            .iter()
            .filter(|deal| deal.started < self.turn)
            .flat_map(|deal| {
                [
                    (deal.from, deal.to, deal.offer.gold_per_turn),
                    (deal.to, deal.from, deal.request.gold_per_turn),
                ]
            })
            .filter(|(payer, _, amount)| *payer == pid && *amount > 0.0)
            .collect();
        for (payer, receiver, amount) in payments {
            self.transfer_gold(payer, receiver, amount);
        }
    }

    pub(super) fn carbon_favor_penalty(&self, pid: usize) -> f64 {
        let contributors: Vec<f64> = self
            .players
            .iter()
            .filter(|player| player.alive && !player.is_minor && !player.is_barbarian)
            .map(|player| player.co2_emissions)
            .collect();
        if contributors.is_empty() {
            return 0.0;
        }
        let average = contributors.iter().sum::<f64>() / contributors.len() as f64;
        let pollution_points_above_average = (self.players[pid].co2_emissions - average) / 1_000.0;
        (pollution_points_above_average / 3.0)
            .floor()
            .clamp(0.0, 20.0)
    }

    pub(super) fn process_diplomacy(&mut self, pid: usize) {
        let turn = self.turn;
        self.decay_war_weariness(pid);
        self.pending_deals.retain(|deal| deal.expires >= turn);
        self.players[pid]
            .denounced_until
            .retain(|_, until| *until > turn);
        let active_denouncements: BTreeSet<usize> =
            self.players[pid].denounced_until.keys().copied().collect();
        self.players[pid]
            .denounced_since
            .retain(|other, _| active_denouncements.contains(other));
        self.players[pid]
            .friends_until
            .retain(|_, until| *until > turn);
        self.players[pid]
            .open_borders_until
            .retain(|_, until| *until > turn);
        self.players[pid]
            .alliances
            .retain(|_, alliance| alliance.ends > turn);
        let allied_pact_partners: BTreeSet<usize> = self.players[pid]
            .defensive_pacts
            .keys()
            .copied()
            .filter(|partner| self.are_allied(pid, *partner))
            .collect();
        self.players[pid]
            .defensive_pacts
            .retain(|partner, until| *until > turn && allied_pact_partners.contains(partner));
        for promises in self.players[pid].promises.values_mut() {
            promises.retain(|_, until| *until > turn);
        }
        self.players[pid]
            .promises
            .retain(|_, promises| !promises.is_empty());
        self.players[pid]
            .broken_promises_until
            .retain(|_, until| *until > turn);
        let offenders: Vec<usize> = self.players[pid].grievances.keys().copied().collect();
        for offender in offenders {
            if self.is_at_war(pid, offender) {
                continue;
            }
            // Cyber Warfare's DISABLE_GRIEVANCE_DECAY: what others hold
            // against its bearer never wears off.
            if self.policy_effect(offender, "no_grievance_decay") > 0.0 {
                continue;
            }
            let occupation_modifier = |founder: usize, occupier: usize| {
                let occupied = self
                    .cities
                    .values()
                    .filter(|city| city.original_owner == founder && city.owner == occupier)
                    .collect::<Vec<_>>();
                if occupied.iter().any(|city| city.is_capital) {
                    3.0
                } else if occupied.is_empty() {
                    0.0
                } else {
                    1.0
                }
            };
            // Base decay runs from 10 in Ancient to 2 in Future. Holding the
            // victim's city slows decay; the victim holding the offender's
            // city accelerates it, with original Capitals using +/-3.
            let decay = (10.0 - self.world_era.min(8) as f64 - occupation_modifier(pid, offender)
                + occupation_modifier(offender, pid))
            .max(0.0);
            if let Some(amount) = self.players[pid].grievances.get_mut(&offender) {
                *amount = (*amount - decay).max(0.0);
            }
        }
        self.players[pid]
            .grievances
            .retain(|_, amount| *amount > 0.0);

        // Retaining a city founded by another civilization continues to
        // create Grievances every turn; it is not only a one-off capture
        // penalty. A city returned to its founder stops this immediately.
        let mut occupied_founders: BTreeMap<usize, f64> = BTreeMap::new();
        for city in self
            .cities
            .values()
            .filter(|city| city.owner == pid && city.original_owner != pid)
            .filter(|city| {
                self.players
                    .get(city.original_owner)
                    .is_some_and(|player| player.alive && !player.is_barbarian)
            })
            .filter(|city| !self.is_at_war(pid, city.original_owner))
        {
            occupied_founders
                .entry(city.original_owner)
                .and_modify(|amount| {
                    *amount = (*amount).max(if city.is_capital { 3.0 } else { 1.0 })
                })
                .or_insert(if city.is_capital { 3.0 } else { 1.0 });
        }
        for (founder, amount) in occupied_founders {
            self.add_grievances(founder, pid, amount);
        }

        let partners: Vec<usize> = self.players[pid]
            .alliances
            .iter()
            .filter(|(_, alliance)| alliance.ends > turn)
            .map(|(partner, _)| *partner)
            .filter(|partner| pid < *partner)
            .collect();
        for partner in partners {
            let outgoing_route = self.routes.iter().any(|route| {
                route.ends > turn
                    && route.owner == pid
                    && self.cities.get(&route.dest).map(|city| city.owner) == Some(partner)
            });
            let incoming_route = self.routes.iter().any(|route| {
                route.ends > turn
                    && route.owner == partner
                    && self.cities.get(&route.dest).map(|city| city.owner) == Some(pid)
            });
            let Some(mut alliance) = self.players[pid].alliances.get(&partner).cloned() else {
                continue;
            };
            let partnership_bonus = |member: usize| {
                if self.players[member].government.as_deref() == Some("democracy")
                    || self.has_policy(member, "wisselbanken")
                {
                    0.25
                } else {
                    0.0
                }
            };
            let same_society = self.players[pid].secret_society.is_some()
                && self.players[pid].secret_society == self.players[partner].secret_society;
            let common_enemy = self.players.iter().any(|other| {
                other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && self.is_at_war(pid, other.id)
                    && self.is_at_war(partner, other.id)
            });
            let sumerian_joint_war = common_enemy
                && (self.players[pid].civ == "Sumeria" || self.players[partner].civ == "Sumeria");
            alliance.points += 1.0
                + if outgoing_route { 0.25 } else { 0.0 }
                + if incoming_route { 0.25 } else { 0.0 }
                + partnership_bonus(pid)
                + partnership_bonus(partner)
                + if same_society { 0.5 } else { 0.0 }
                + if sumerian_joint_war { 0.5 } else { 0.0 };
            alliance.level = if alliance.points >= 240.0 {
                3
            } else if alliance.points >= 80.0 {
                2
            } else {
                1
            };
            self.players[pid]
                .alliances
                .insert(partner, alliance.clone());
            self.players[partner]
                .alliances
                .insert(pid, alliance.clone());
            let first_key = Self::alliance_points_key(partner, &alliance.kind);
            let second_key = Self::alliance_points_key(pid, &alliance.kind);
            let stored_points = (alliance.points * 4.0).round() as i64;
            self.players[pid].counters.insert(first_key, stored_points);
            self.players[partner]
                .counters
                .insert(second_key, stored_points);
            if alliance.kind == "research"
                && alliance.level >= 2
                && turn > 0
                && turn.is_multiple_of(self.standard_duration(30))
            {
                self.share_research_alliance_boosts(pid, partner);
            }
            if alliance.kind == "military" && alliance.level >= 2 {
                self.share_visibility_memories(&[pid, partner]);
            }
        }

        if self.players[pid].is_minor || self.players[pid].is_barbarian {
            return;
        }
        let government_favor = self.players[pid]
            .government
            .as_deref()
            .and_then(|government| self.rules.governments.get(government))
            .map(|government| government.diplomatic_favor_per_turn)
            .unwrap_or(0.0);
        let suzerains = self
            .players
            .iter()
            .filter(|minor| minor.is_minor && !minor.is_barbarian && minor.alive)
            .filter(|minor| self.suzerain_of(minor.id) == Some(pid))
            .count() as f64;
        let alliance_favor = self.players[pid]
            .alliances
            .values()
            .filter(|alliance| alliance.ends > turn)
            .map(|alliance| alliance.level as f64)
            .sum::<f64>();
        let suzerain_multiplier = self.suzerain_diplomatic_favor_per_turn(pid);
        let buildings = self.empire_building_sum(pid, |building| {
            building
                .effects
                .get("diplomatic_favor")
                .copied()
                .unwrap_or(0.0)
        });
        let world_grievances = self
            .players
            .iter()
            .filter(|observer| observer.id != pid)
            .map(|observer| observer.grievances.get(&pid).copied().unwrap_or(0.0))
            .sum::<f64>();
        let occupied_original_capitals = self
            .cities
            .values()
            .filter(|city| city.owner == pid && city.is_capital && city.original_owner != pid)
            .count() as f64;
        let diplomatic_penalty = world_grievances / 100.0
            + 5.0 * occupied_original_capitals
            + self.carbon_favor_penalty(pid);
        // MONARCHY_STARFORT_FAVOR is a PLAYER_CITIES modifier gated on
        // REQUIREMENT_CITY_HAS_BUILDING BUILDING_STAR_FORT, so it pays per
        // qualifying city rather than once. Star Fort is Renaissance Walls.
        let walled_favor = self.gov_effects(pid).walled_city_diplomatic_favor;
        let walled_cities = if walled_favor == 0.0 {
            0.0
        } else {
            self.cities
                .values()
                .filter(|city| city.owner == pid)
                .filter(|city| {
                    self.city_has_active_building_family(city, crate::name!("renaissance_walls"))
                })
                .count() as f64
        };
        let favor = government_favor
            + walled_favor * walled_cities
            + suzerains * suzerain_multiplier
            + alliance_favor
            + buildings
            + self.policy_effect(pid, "diplomatic_favor_per_turn")
            + self.policy_effect(pid, "favor_per_broadcast_center")
                * self
                    .cities
                    .values()
                    .filter(|city| city.owner == pid)
                    .filter(|city| {
                        self.city_has_active_building_family(city, crate::name!("broadcast_center"))
                    })
                    .count() as f64
            - diplomatic_penalty;
        self.players[pid].diplomatic_favor = (self.players[pid].diplomatic_favor + favor).max(0.0);
        *self.players[pid]
            .counters
            .entry("diplomatic_favor".to_string())
            .or_insert(0) += favor.max(0.0).floor() as i64;
        self.score_favor_competition(pid, favor.max(0.0));
    }

    pub(super) fn process_influence(&mut self, pid: usize) {
        if self.players[pid].is_minor || self.players[pid].is_barbarian {
            return;
        }
        let Some(government) = self.players[pid]
            .government
            .as_deref()
            .and_then(|government| self.rules.governments.get(government))
        else {
            return;
        };
        let base_influence = government.influence_per_turn;
        let threshold = government.influence_threshold;
        let envoys_per_threshold = government.envoys_per_threshold;
        let building_effect = |effect: &str| {
            self.cities
                .values()
                .filter(|city| city.owner == pid)
                .map(|city| self.city_building_effect(city, effect))
                .sum::<f64>()
        };
        let economic_alliance_influence = self
            .alliance_partner(pid, "economic", 2)
            .map(|partner| {
                self.players
                    .iter()
                    .filter(|minor| minor.alive && minor.is_minor && !minor.is_barbarian)
                    .filter(|minor| self.suzerain_of(minor.id) == Some(partner))
                    .count() as f64
            })
            .unwrap_or(0.0);
        let mut influence = (base_influence
            + economic_alliance_influence
            + self.policy_effect(pid, "influence_per_turn")
            + building_effect("influence_points"))
            * (1.0 + self.gov_effects(pid).influence_pct / 100.0);
        // Rogue State: ROGUESTATE_DISABLE_INFLUENCE — no points, no Envoys.
        if self.policy_effect(pid, "no_influence") > 0.0 {
            influence = 0.0;
        }
        let player = &mut self.players[pid];
        player.influence += influence;
        while threshold > 0.0
            && envoys_per_threshold > 0
            && player.influence + f64::EPSILON >= threshold
        {
            player.influence -= threshold;
            player.envoys_free += envoys_per_threshold;
        }
    }

    pub(super) fn cancel_trade_deals_with(&mut self, first: usize, second: usize) {
        self.active_trade_deals.retain(|deal| {
            !((deal.from == first && deal.to == second)
                || (deal.from == second && deal.to == first))
        });
        self.pending_deals.retain(|deal| {
            !((deal.from == first && deal.to == second)
                || (deal.from == second && deal.to == first))
        });
    }

    /// Whether `pid`'s territory is closed to foreign units at all. Early
    /// Empire is what turns a border on — the shipped civic "unlocks the
    /// abilities to enforce borders and grant Open Borders" — and before it
    /// anyone may walk through. A live mirror answers from the host's own
    /// civic list ([`Player::borders_enforced`]) because it does not model a
    /// rival's civics; a native game reads the seat's tree.
    pub fn enforces_borders(&self, pid: usize) -> bool {
        self.players
            .get(pid)
            .and_then(|player| player.borders_enforced)
            .unwrap_or_else(|| self.tree_effect(pid, "open_borders") > 0.0)
    }

    /// Whether `mover` may enter `territory_owner`'s peaceful territory.
    /// Before Early Empire borders are open; Alliances and directional trade
    /// grants also qualify.
    pub fn has_open_borders(&self, mover: usize, territory_owner: usize) -> bool {
        if mover == territory_owner
            || self.same_team(mover, territory_owner)
            || !self.enforces_borders(territory_owner)
            || (self.players[territory_owner].is_minor
                && (self.suzerain_of(territory_owner) == Some(mover)
                    || self.policy_effect(mover, "open_city_state_borders") > 0.0
                    // João III's Porta do Cerco is folded into Portugal's
                    // modeled Casa da Índia ability record.
                    || self.has_ability(mover, "casa_da_india")))
            || self.players[mover]
                .alliances
                .get(&territory_owner)
                .is_some_and(|alliance| alliance.ends > self.turn)
            || self.players[territory_owner]
                .open_borders_until
                .get(&mover)
                .is_some_and(|until| *until > self.turn)
        {
            return true;
        }
        self.active_trade_deals
            .iter()
            .filter(|deal| deal.ends > self.turn)
            .any(|deal| {
                (deal.from == territory_owner && deal.to == mover && deal.offer.open_borders)
                    || (deal.to == territory_owner
                        && deal.from == mover
                        && deal.request.open_borders)
            })
    }

    pub(super) fn do_make_peace(&mut self, pid: usize, other: usize) -> Result<(), String> {
        let first_side = self.team_members(pid);
        let second_side = self.team_members(other);
        if first_side.iter().any(|first| {
            second_side
                .iter()
                .any(|second| self.emergency_war_pair(*first, *second))
        }) {
            return Err("active Emergency members cannot make peace with its target".into());
        }
        if !self.is_at_war(pid, other) {
            return Err("not at war".into());
        }
        // `is_at_war` also includes a city-state's derived participation in
        // its Suzerain's wars.  A bilateral peace with that city-state cannot
        // remove the principals' declared relation, so accepting it would do
        // nothing except emit another "made peace" event every turn.  Only a
        // pair that belongs to the explicit war may conclude it; derived
        // participants follow their controller back to peace automatically.
        let declared_war = first_side.iter().any(|first| {
            second_side
                .iter()
                .any(|second| self.at_war.contains(&pair(*first, *second)))
        });
        if !declared_war {
            return Err("a city-state's derived war must be settled by its Suzerain".into());
        }
        if let Some(until) = self.peace_available_at(pid, other) {
            return Err(format!("this war cannot be settled before turn {until}"));
        }
        self.conclude_peace(pid, other, Vec::new());
        Ok(())
    }

    /// A peace agreement in this compact diplomacy model accepts the current
    /// borders: cities held by either signatory are ceded, ending Occupation's
    /// ungarrisoned -5 Loyalty pressure. Civ VI charges the population- and
    /// casus-adjusted capture value once when the city falls and once again
    /// when a peace treaty recognizes that possession.
    pub(super) fn conclude_peace(&mut self, first: usize, second: usize, mut terms: Vec<String>) {
        let first_side = self.team_members(first);
        let second_side = self.team_members(second);
        // A peace treaty binds for the shipped ten turns, and it binds every
        // pair the treaty covers. Without it the two sides can re-declare the
        // turn after signing, which is not a war — it is the same war, and it
        // reads in the log as a dozen of them.
        let holds_until = self.turn + self.standard_duration(PEACE_TREATY_TURNS);
        // WAR_WEARINESS_DECAY_PEACE_DECLARED: signing forgives a large block
        // of weariness outright, for both sides of the treaty.
        for side in first_side.iter().chain(second_side.iter()) {
            self.add_war_weariness(*side, -2000.0);
        }
        terms.push("Current borders recognized".to_string());
        terms.push(format!("Peace treaty through Turn {holds_until}"));
        let settlement = WarPeace {
            turn: self.turn,
            first,
            second,
            terms,
        };
        for war in self.wars.values_mut().filter(|war| {
            (first_side.contains(&war.aggressor) && second_side.contains(&war.defender))
                || (second_side.contains(&war.aggressor) && first_side.contains(&war.defender))
        }) {
            war.peace_terms.push(settlement.clone());
        }
        for first_member in &first_side {
            for second_member in &second_side {
                self.at_war.remove(&pair(*first_member, *second_member));
                self.peace_treaties
                    .insert(pair(*first_member, *second_member), holds_until);
            }
        }
        let (a, b) = (self.civ_name(first), self.civ_name(second));
        let message = format!("{a} made peace with {b}");
        for participant in first_side
            .iter()
            .copied()
            .chain(second_side.iter().copied())
        {
            self.note(participant, "Diplomacy", message.clone(), None);
        }
        let cessions: Vec<(usize, usize, f64)> = self
            .cities
            .values()
            .filter_map(|city| {
                let cross_team_occupation = (first_side.contains(&city.owner)
                    && city
                        .occupied_from
                        .is_some_and(|former| second_side.contains(&former)))
                    || (second_side.contains(&city.owner)
                        && city
                            .occupied_from
                            .is_some_and(|former| first_side.contains(&former)));
                if !cross_team_occupation {
                    return None;
                }
                let former = city.occupied_from?;
                let grievance = city
                    .occupation_grievance
                    // Saves made before split capture/cession accounting can
                    // still settle cleanly. Newer saves retain the exact
                    // casus-adjusted amount recorded at capture time.
                    .unwrap_or_else(|| self.city_capture_base_grievances(city.id));
                Some((former, city.owner, grievance))
            })
            .collect();
        for (former, holder, grievance) in cessions {
            if self.players.get(former).is_some_and(|player| {
                player.alive && !player.is_minor && !player.is_barbarian && !player.is_free_city
            }) {
                self.add_grievances(former, holder, grievance);
            }
        }
        for city in self.cities.values_mut() {
            let cross_team_occupation = (first_side.contains(&city.owner)
                && city
                    .occupied_from
                    .is_some_and(|former| second_side.contains(&former)))
                || (second_side.contains(&city.owner)
                    && city
                        .occupied_from
                        .is_some_and(|former| first_side.contains(&former)));
            if cross_team_occupation {
                city.occupied_from = None;
                city.occupation_grievance = None;
            }
        }
        self.sync_war_log();
    }
}
