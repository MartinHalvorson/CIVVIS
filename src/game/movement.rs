//! Movement: what a step costs, how far a unit moves, and where it may go
//! and stop.
//!
//! Step costs: terrain and roads (`step_cost`, `unit_step_cost`), crossing
//! rivers and cliffs. Movement points: a unit's base and capped maximum
//! moves (`unit_max_moves`, `unit_max_moves_at`). Traversal: a unit's
//! traversal class and whether that class can cross a tile
//! (`unit_can_traverse`, `class_can_traverse`), the passage improvements
//! and enemy air patrols it reads, and the land and naval move terms.
//! Legality: whether a step can be paid for and taken (`can_pay_step`,
//! `can_move_step`), entering, passing and stopping (`can_enter`,
//! `can_pass`, `can_stop`, `entry_at`), stacking layers and linked escort
//! pairs. Borders: territory access, closed borders, and displacing units
//! that a closing border leaves inside (`reconcile_closed_border_units`).
//! `docs/MOVEMENT.md` describes the rules these implement.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! and `pub(crate)` methods are unchanged; the code is otherwise unchanged.

use super::*;

impl Game {
    /// Static terrain/domain portion of unit movement. Occupancy and
    /// diplomacy are checked separately by `can_enter`.
    pub fn unit_can_traverse(&self, uid: u32, pos: Pos) -> bool {
        let Some(unit) = self.units.get(&uid) else {
            return false;
        };
        let Some(tile) = self.map.get(pos) else {
            return false;
        };
        let spec = &self.rules.units[unit.kind];
        if spec.domain.as_deref() == Some("air") {
            return false; // aircraft use rebase/strike missions, never tile steps
        }
        // Delegating to the class predicate keeps this answer and the cached
        // connectivity regions incapable of disagreeing.
        self.class_can_traverse(self.traversal_class(uid), tile)
    }

    /// The [`TraversalClass`] a unit moves as, folding its kind and its
    /// owner's unlocks into the flags the tile predicate reads.
    pub(crate) fn traversal_class(&self, uid: u32) -> TraversalClass {
        if let Some(memo) = self.query_memo.traversal.borrow().as_ref() {
            if let Some(class) = memo.get(&uid) {
                return *class;
            }
        }
        let class = self.traversal_class_uncached(uid);
        if let Some(memo) = self.query_memo.traversal.borrow_mut().as_mut() {
            memo.insert(uid, class);
        }
        class
    }

    /// Every unit flying a patrol right now, as `(owner, tile)`.
    ///
    /// The open-coded scan this replaces sat in `can_enter_past`, which
    /// `flow_past` calls once per neighbour of every tile it expands — so one
    /// unit's movement flood walked `units` hundreds of times to learn a fact
    /// about the whole board. Inside a `&self` query the board cannot change,
    /// so the answer is computed once per memo scope and shared.
    ///
    /// Outside a memo scope there is nothing to hold it against, so it is
    /// rebuilt per call — exactly the work the open-coded scan did, never
    /// more. `query_memo.yields` is the discriminator for "a scope is live",
    /// the same one `query_memo()` itself uses.
    pub(super) fn air_patrols(&self) -> AirPatrols {
        if let Some(cached) = self.query_memo.air_patrols.borrow().as_ref() {
            return Arc::clone(cached);
        }
        let roster = self.units.vision_epoch();
        let stable = self.query_memo.air_patrols_stable.borrow();
        let reusable = stable
            .as_ref()
            .filter(|(epoch, _)| *epoch == roster)
            .map(|(_, patrols)| Arc::clone(patrols));
        drop(stable);
        let patrols: AirPatrols = reusable.unwrap_or_else(|| {
            let built: AirPatrols = Arc::new(
                self.units
                    .values()
                    .filter(|unit| unit.air_patrol)
                    .filter_map(|unit| unit.air_patrol_pos.map(|pos| (unit.owner, pos)))
                    .collect(),
            );
            *self.query_memo.air_patrols_stable.borrow_mut() = Some((roster, Arc::clone(&built)));
            built
        });
        if self.query_memo.yields.borrow().is_some() {
            *self.query_memo.air_patrols.borrow_mut() = Some(Arc::clone(&patrols));
        }
        patrols
    }

    /// `true` at `Name::id` for every improvement whose `passage` effect is
    /// positive — the test `class_can_traverse` makes per tile.
    ///
    /// Sized to the largest improvement id, so an id past the end simply names
    /// something that is not an improvement and reads `false`, which is what
    /// the `BTreeMap` miss it replaces returned.
    pub(super) fn passage_improvements(&self) -> PassageTable {
        if let Some(cached) = self.query_memo.passage_improvements.borrow().as_ref() {
            return Arc::clone(cached);
        }
        let span = self
            .rules
            .improvements
            .keys()
            .map(|name| name.id() as usize + 1)
            .max()
            .unwrap_or(0);
        let mut table = vec![false; span];
        for (name, spec) in self.rules.improvements.iter() {
            table[name.id() as usize] = spec.effects.get("passage").copied().unwrap_or(0.0) > 0.0;
        }
        let table = Arc::new(table);
        if self.query_memo.yields.borrow().is_some() {
            *self.query_memo.passage_improvements.borrow_mut() = Some(Arc::clone(&table));
        }
        table
    }

    /// The unit's half of a step cost, from the memo when a scope is live.
    ///
    /// `flow_past` calls `unit_step_cost` once per neighbour of every tile it
    /// expands, and three of that function's five tests name only the unit —
    /// each one a `BTreeMap<String, f64>` descent, a `memcmp` per level, for
    /// an answer the flood cannot change. So, exactly as
    /// [`Game::traversal_class`] already does for the same reason, they are
    /// derived once per unit per memo scope.
    ///
    /// Outside a memo scope there is nothing to hold the answer against, so it
    /// is rebuilt per call — precisely the work the open-coded version did,
    /// never more. That "never more" is why the three groups of
    /// [`MovementProfile`] are separate: see its documentation.
    pub(super) fn step_terms(&self, unit: &Unit) -> StepTerms {
        if let Some(terms) = self
            .query_memo
            .movement
            .borrow()
            .as_ref()
            .and_then(|memo| memo.get(&unit.id))
            .and_then(|profile| profile.step)
        {
            return terms;
        }
        let spec = &self.rules.units[unit.kind];
        let terms = StepTerms {
            religious_flat_movement: spec.class == "religious"
                && unit.religion.as_deref().is_some_and(|religion| {
                    self.religion_belief_effect(religion, "religious_flat_movement") > 0.0
                }),
            woods_move_cost: self.unit_effect(unit, "woods_move_cost"),
            hills_move_cost: self.unit_effect(unit, "hills_move_cost"),
            amphibious: self.promotion_effect(unit, "amphibious"),
        };
        if let Some(memo) = self.query_memo.movement.borrow_mut().as_mut() {
            memo.entry(unit.id).or_default().step = Some(terms);
        }
        terms
    }

    /// The tile-independent terms of a unit's movement allowance.
    ///
    /// `flow_past` re-derives the allowance at every neighbour it relaxes, and
    /// the support aura alone sweeps the occupancy of seven tiles to find out
    /// whether anything is standing next to a unit that has not moved.
    pub(super) fn move_terms(&self, unit: &Unit) -> MoveTerms {
        if let Some(terms) = self
            .query_memo
            .movement
            .borrow()
            .as_ref()
            .and_then(|memo| memo.get(&unit.id))
            .and_then(|profile| profile.moves)
        {
            return terms;
        }
        let spec = &self.rules.units[unit.kind];
        let terms = MoveTerms {
            promotion_movement: self.promotion_effect(unit, "movement"),
            civilization_movement: self.civ_effect(unit.owner, "unit_movement"),
            guru_adjacent: spec.class == "religious"
                && self.wdisk(unit.pos, 1).into_iter().any(|position| {
                    self.unit_ids_at(position).iter().any(|other| {
                        self.units[other].owner == unit.owner
                            && self.units[other].kind == "guru"
                            && self.units[other].religion == unit.religion
                    })
                }),
            monumentality_builder: self.dedication_active(unit.owner, "monumentality")
                && unit.kind == "builder",
            exodus_religious: self.dedication_active(unit.owner, "exodus_of_the_evangelists")
                && spec.class == "religious",
            adjacent_movement: self.adjacent_support_effect(unit, "adjacent_movement"),
        };
        if let Some(memo) = self.query_memo.movement.borrow_mut().as_mut() {
            memo.entry(unit.id).or_default().moves = Some(terms);
        }
        terms
    }

    /// The movement unlocks a naval or embarked unit adds. Derived only from
    /// inside that branch, so a land unit on land never pays for it.
    pub(super) fn naval_move_terms(&self, unit: &Unit) -> NavalMoveTerms {
        if let Some(terms) = self
            .query_memo
            .movement
            .borrow()
            .as_ref()
            .and_then(|memo| memo.get(&unit.id))
            .and_then(|profile| profile.naval)
        {
            return terms;
        }
        let terms = NavalMoveTerms {
            naval_movement_tree: self.tree_effect(unit.owner, "naval_movement"),
            embarked_movement_tree: self.tree_effect(unit.owner, "embarked_movement"),
            mana: self.has_ability(unit.owner, "mana"),
            naval_movement_wonder: self.empire_wonder_effect(unit.owner, "naval_movement"),
        };
        if let Some(memo) = self.query_memo.movement.borrow_mut().as_mut() {
            memo.entry(unit.id).or_default().naval = Some(terms);
        }
        terms
    }

    /// A unit's traversal class costs a ruleset effect sum over every tech
    /// and civic it owns plus two improvement unlocks, and a route search
    /// asks for it once per tile it examines — hence the memo above.
    fn traversal_class_uncached(&self, uid: u32) -> TraversalClass {
        let unit = &self.units[&uid];
        let spec = &self.rules.units[unit.kind];
        let sea = spec.domain.as_deref() == Some("sea");
        let wading_giant = unit.kind == "giant_death_robot";
        // Cartography is the stock gate for every naval and embarked unit.
        // Knarr substitutes Shipbuilding for Norway; Mana lets Māori cross
        // Ocean from the opening turn.
        let ocean = wading_giant
            || self.has_ability(unit.owner, "mana")
            || self.tree_effect(unit.owner, "ocean_navigation") > 0.0
            || (sea
                && self.has_ability(unit.owner, "knarr")
                && self.players[unit.owner]
                    .techs
                    .contains(&crate::name!("shipbuilding")));
        TraversalClass {
            sea,
            embark: !sea && (wading_giant || self.has_embarkation(unit.owner, &unit.kind)),
            ocean,
            mountain: (unit.kind == "builder"
                && !self.observed_builders_avoid_mountains
                && self.unlocked(
                    unit.owner,
                    &self.rules.improvements["ski_resort"].tech,
                    &self.rules.improvements["ski_resort"].civic,
                ))
                // Qhapaq Nan is built by a Builder *on* a mountain. The
                // ordinary mountain passage comes from the finished route,
                // but an Incan Builder must be able to reach the first site
                // when Foreign Trade unlocks it.
                || (unit.kind == "builder"
                    && self.players[unit.owner].civ == "Inca"
                    && self.unlocked(
                        unit.owner,
                        &self.rules.improvements["qhapaq_nan"].tech,
                        &self.rules.improvements["qhapaq_nan"].civic,
                    ))
                || (unit.kind == "military_engineer"
                    && self.unlocked(
                        unit.owner,
                        &self.rules.improvements["mountain_tunnel"].tech,
                        &self.rules.improvements["mountain_tunnel"].civic,
                    )),
        }
    }

    /// Whether a unit moving as `class` could ever stand on `tile`. The
    /// static half of [`Game::unit_can_traverse`], and the predicate the
    /// cached connectivity regions are flooded from.
    pub(super) fn class_can_traverse(&self, class: TraversalClass, tile: &Tile) -> bool {
        // A mirror frontier is an invitation to discover the tile, not a claim
        // that it is land or water. Until the host reveals it, either movement
        // domain may plan toward it; the next authoritative sync replaces the
        // unknown with its real terrain before subsequent orders are chosen.
        //
        // The land prior (`assumed_traversable`) is grown from revealed land
        // and read by every domain — it is what `is_passable` answers for an
        // unknown tile; the sea prior (`assumed_navigable`) is grown from
        // revealed water and read by ships alone, here, ahead of the
        // passability test that would otherwise refuse it. Without the second
        // one a coast revealed to the horizon walls the fleet in: the fog
        // beyond it is reached from no land tile, so no ship can plan toward
        // it, and the empire's naval eye has nowhere to look. See
        // `mirror::grow_frontier`.
        if self.rules.is_unknown(tile) {
            return tile.assumed_traversable || (class.sea && tile.assumed_navigable);
        }
        if !self.rules.is_passable(tile) && !(tile.terrain == "mountain" && class.mountain) {
            // Passage matters only when terrain would otherwise block the
            // mover. Ordinary improved tiles need no table lookup or Arc clone.
            let improvement_passage = !tile.pillaged
                && tile.improvement.is_some_and(|improvement| {
                    self.passage_improvements()
                        .get(improvement.id() as usize)
                        .copied()
                        .unwrap_or(false)
                });
            if !improvement_passage {
                return false;
            }
        }
        let water = self.rules.is_water(tile);
        if water && tile.terrain == "ocean" && !class.ocean {
            return false;
        }
        if class.sea {
            // Naval units may use water, City Centers, and Canals. This also
            // lets naval melee units attack and capture coastal cities.
            water
                || self.city_at(tile.pos).is_some()
                || (!tile.pillaged
                    && tile.district.is_some_and(|district| {
                        self.rules.districts[district]
                            .effects
                            .get("naval_passage")
                            .copied()
                            .unwrap_or(0.0)
                            > 0.0
                    }))
                || tile.wonder.as_deref().is_some_and(|wonder| {
                    self.rules.wonders[wonder]
                        .effects
                        .get("naval_passage")
                        .copied()
                        .unwrap_or(0.0)
                        > 0.0
                })
        } else {
            !water || tile.wonder.as_deref() == Some("golden_gate_bridge") || class.embark
        }
    }

    /// Terrain MP to step between adjacent tiles. The unit-aware wrapper
    /// applies bridge technology and promotion exceptions.
    pub fn step_cost(&self, from: Pos, to: Pos) -> f64 {
        let t = &self.map.tiles[&to];
        let mut c = self.rules.move_cost(t);
        if self.crosses_river(from, to) {
            c += 2.0;
        }
        c
    }

    pub(super) fn unit_step_cost(&self, uid: u32, from: Pos, to: Pos) -> f64 {
        let unit = &self.units[&uid];
        let tile = &self.map.tiles[&to];
        // Three of the tests below name the unit, not the tile, and a flood
        // asks all three once per neighbour of every tile it expands.
        let terms = self.step_terms(unit);
        // Medieval and later routes bridge rivers (SupportsBridges); Ancient
        // roads leave the crossing penalty in place. Bridging has to withhold
        // the surcharge rather than refund it afterwards: the route ladder
        // below caps the step at the route's own cost, which already discards
        // the surcharge, so a refund taken after that cancels nothing and
        // drives the step negative instead.
        let bridged =
            self.crosses_river(from, to) && self.map.tiles[&from].road >= 2 && tile.road >= 2;
        // Waive the river penalty before pricing the route. Applying
        // Amphibious afterwards would overwrite a valid road discount.
        let mut cost = if bridged || terms.amphibious > 0.0 && self.crosses_river(from, to) {
            self.rules.move_cost(tile)
        } else {
            self.step_cost(from, to)
        };
        if self.map.tiles[&from].road > 0 && tile.road > 0 {
            let modern_bridge = self.map.tiles[&from].wonder.as_deref()
                == Some("golden_gate_bridge")
                || tile.wonder.as_deref() == Some("golden_gate_bridge");
            // The shipped route ladder, per tile: Ancient and Medieval roads
            // cost 1 MP, Industrial 0.75, Modern 0.5, Railroads 0.25.
            let mut route: f64 = match tile.road {
                5 => 0.25,
                4 => 0.5,
                3 => 0.75,
                _ => 1.0,
            };
            if modern_bridge {
                route = route.min(0.5);
            }
            cost = cost.min(route);
        }
        if terms.religious_flat_movement {
            cost = 1.0;
        }
        if terms.woods_move_cost > 0.0
            && matches!(tile.feature.as_deref(), Some("forest" | "jungle"))
        {
            cost = 1.0;
        }
        if terms.hills_move_cost > 0.0 && tile.hills {
            cost = 1.0;
        }
        let changes_embarkation =
            self.unit_is_embarked_at(unit, from) != self.unit_is_embarked_at(unit, to);
        if changes_embarkation
            && terms.amphibious <= 0.0
            && !self.has_ability(unit.owner, "knarr")
            && !self.embarkation_facility_at(from)
            && !self.embarkation_facility_at(to)
        {
            // Civ VI adds a two-point embark/disembark surcharge to the
            // destination terrain cost. A full-movement unit can still take
            // one otherwise-unaffordable step, as with other rough terrain.
            cost += 2.0;
        }
        // A step never grants movement. `flow` and `path_to` terminate because
        // each relaxation leaves strictly less movement than it started with;
        // a negative cost restores movement instead, so a two-tile route loop
        // regains MP on every crossing and the search stops being bounded by
        // the unit's budget. That is a memory fault, not a movement quirk, so
        // the invariant is kept here rather than assumed of every rule above.
        debug_assert!(
            cost >= 0.0,
            "step {from:?} -> {to:?} costs {cost}, which would grant movement"
        );
        cost.max(0.0)
    }

    pub(super) fn crosses_river(&self, from: Pos, to: Pos) -> bool {
        let Some(a) = self.map.get(from) else {
            return false;
        };
        let Some(b) = self.map.get(to) else {
            return false;
        };
        !self.rules.is_water(a) && !self.rules.is_water(b) && self.map.has_river_edge(from, to)
    }

    pub(super) fn crosses_cliff(&self, from: Pos, to: Pos) -> bool {
        let Some(a) = self.map.get(from) else {
            return false;
        };
        let Some(b) = self.map.get(to) else {
            return false;
        };
        self.rules.is_water(a) != self.rules.is_water(b) && self.map.has_cliff_edge(from, to)
    }

    pub(super) fn unit_can_cross_cliff(&self, uid: u32, from: Pos, to: Pos) -> bool {
        if !self.crosses_cliff(from, to) {
            return true;
        }
        let unit = &self.units[&uid];
        let water_side = if self.rules.is_water(&self.map.tiles[&from]) {
            from
        } else {
            to
        };
        self.map.tiles[&from].wonder.as_deref() == Some("golden_gate_bridge")
            || self.map.tiles[&to].wonder.as_deref() == Some("golden_gate_bridge")
            || self.promotion_effect(unit, "scale_cliffs") > 0.0
            || self.embarkation_facility_at(water_side)
    }

    pub(super) fn unit_base_max_moves_at(&self, uid: u32, pos: Pos) -> f64 {
        let u = &self.units[&uid];
        let spec = &self.rules.units[u.kind];
        let tile = &self.map.tiles[&pos];
        // Only the terrain terms and the emergency below read `pos`; the rest
        // is a property of the unit, and `flow_past` re-derives the allowance
        // at every neighbour it relaxes.
        let terms = self.move_terms(u);
        let embarked = self.unit_is_embarked_at(u, pos);
        let mut moves = if embarked {
            2.0
        } else if matches!(u.kind.as_str(), "war_cart" | "maryannu_chariot_archer")
            && !tile.hills
            && !matches!(tile.feature.as_deref(), Some("forest" | "jungle"))
        {
            4.0
        } else {
            spec.moves
        };
        moves += terms.promotion_movement + u.bonus_moves;
        moves += terms.civilization_movement;
        // The three naval unlocks are added in the order they always were —
        // `embarked` implies this branch, so nesting changes which terms are
        // summed for no unit and their sequence for none either.
        if let Some(naval) =
            (spec.domain.as_deref() == Some("sea") || embarked).then(|| self.naval_move_terms(u))
        {
            moves += naval.naval_movement_tree;
            if embarked {
                moves += naval.embarked_movement_tree;
                if naval.mana {
                    moves += 2.0;
                }
            }
            moves += naval.naval_movement_wonder;
        }
        if terms.guru_adjacent {
            moves += self.empire_wonder_effect(u.owner, "guru_adjacent_religious_movement");
        }
        if u.kind == "giant_death_robot" {
            moves += self.tree_effect(u.owner, "gdr_movement");
        }
        if spec.clear_terrain_start_movement > 0.0
            && self.map.get(u.pos).is_some_and(|tile| {
                !tile.hills
                    && !matches!(tile.feature.as_deref(), Some("forest" | "jungle"))
                    && self.rules.is_passable(tile)
                    && !self.rules.is_water(tile)
            })
        {
            moves += spec.clear_terrain_start_movement;
        }
        if terms.monumentality_builder {
            moves += 2.0;
        }
        if terms.exodus_religious {
            moves += 2.0;
        }
        let territory_owner = tile
            .owner_city
            .and_then(|city| self.cities.get(&city))
            .map(|city| city.owner);
        if self.active_emergencies.iter().any(|emergency| {
            emergency.ends > self.turn
                && emergency.members.contains(&u.owner)
                && territory_owner == Some(emergency.target)
        }) {
            moves += 1.0;
        }
        moves += terms.adjacent_movement;
        moves
    }

    #[cfg(test)]
    pub(super) fn unit_base_max_moves(&self, uid: u32) -> f64 {
        self.unit_base_max_moves_at(uid, self.units[&uid].pos)
    }

    /// Whose allowance [`Game::unit_max_moves_at`] reads on a tile, decided
    /// without naming one — a linked pair, an escort's mobility promotion and
    /// the unit's own class are all facts about the units, so a flood settles
    /// this once and then only reads tiles. See [`MovesCap`].
    pub(super) fn unit_moves_cap(&self, uid: u32) -> MovesCap {
        let unit = &self.units[&uid];
        let Some(linked) = unit.linked_to.and_then(|id| self.units.get(&id)) else {
            return MovesCap::Own;
        };
        let spec = &self.rules.units[unit.kind];
        if spec.class != "military" && self.unit_shares_escort_movement(linked) {
            MovesCap::Peer(linked.id)
        } else if spec.class != "military" || self.unit_shares_escort_movement(unit) {
            MovesCap::Own
        } else {
            MovesCap::Min(linked.id)
        }
    }

    /// The tile half of [`Game::unit_max_moves_at`], once the unit half is
    /// settled.
    pub(super) fn capped_moves_at(&self, uid: u32, cap: &MovesCap, pos: Pos) -> f64 {
        match *cap {
            MovesCap::Own => self.unit_base_max_moves_at(uid, pos),
            MovesCap::Peer(peer) => self.unit_base_max_moves_at(peer, pos),
            MovesCap::Min(peer) => self
                .unit_base_max_moves_at(uid, pos)
                .min(self.unit_base_max_moves_at(peer, pos)),
        }
    }

    pub(super) fn unit_max_moves_at(&self, uid: u32, pos: Pos) -> f64 {
        self.capped_moves_at(uid, &self.unit_moves_cap(uid), pos)
    }

    /// Full movement allowance for a fresh turn, including embarkation, tech,
    /// policy, wonder, road, formation, and support effects. The live mirror
    /// uses this after rebuilding a board because Civilization VI's exported
    /// movement field is a remaining-movement observation, not a fresh-turn
    /// allowance.
    pub(crate) fn unit_max_moves(&self, uid: u32) -> f64 {
        // `Unit:GetMaxMoves()` (`UnitPanel.lua:2242`) when the host exported
        // it: the allowance with every road, promotion, Great General, policy
        // and embarkation effect the host applies, in place of the board's
        // model of them. Zero or less is no reading, not an immobile unit.
        if let Some(moves) = self
            .host_unit_facts
            .get(&uid)
            .and_then(|facts| facts.max_moves)
            .filter(|moves| *moves > 0.0)
        {
            return moves;
        }
        self.unit_max_moves_at(uid, self.units[&uid].pos)
    }

    /// Whether `uid` could finish one observed host step from `from` at `to`.
    ///
    /// A live-state export can be older than an input event, so a conformance
    /// audit cannot assume that the unit still stands at the position in its
    /// latest mirror frame.  This creates an isolated branch, places the
    /// moving formation at the host-reported source, and grants it a fresh
    /// movement allowance.  The answer therefore covers structural movement
    /// legality (terrain, borders, stacking, hostile units, and zone of
    /// control) without treating previously spent movement as a disagreement.
    ///
    /// `None` means the observation cannot be represented by this board: the
    /// unit or either endpoint is absent, or the event is not one map edge.
    /// The authoritative game is never changed.
    pub fn can_move_observed_step(&self, uid: u32, from: Pos, to: Pos) -> Option<bool> {
        if !self.units.contains_key(&uid)
            || !self.map.tiles.contains_key(&from)
            || !self.map.tiles.contains_key(&to)
            || self.wdist(from, to) != 1
        {
            return None;
        }

        let mut probe = self.clone();
        // A linked support unit shares its leader's tile and movement test.
        // Rebase both halves so a state frame from before the formation moved
        // does not manufacture a false formation mismatch.
        let mut movers = vec![uid];
        if let Some(peer) = probe.units[&uid]
            .linked_to
            .filter(|peer| probe.units.contains_key(peer))
        {
            movers.push(peer);
        }
        for mover in movers {
            probe.relocate(mover, from);
            let attacks = probe.unit_max_attacks(mover);
            let unit = probe.units.get_mut(&mover).expect("checked above");
            unit.moves_left = 1_000.0;
            unit.attacks_left = attacks;
            unit.moved = false;
            unit.acted = false;
            unit.zoc_stopped = false;
            unit.fortified = false;
            unit.fortify_turns = 0;
            // This host reading is tied to its old coordinate.  At a replayed
            // source, let the board's terrain rule decide embarkation instead.
            unit.host_embarked = None;
            unit.started_turn_in_zoc = false;
        }
        Some(probe.can_move(uid, to))
    }

    /// `can_move` for a hex in the middle of a longer walk, where the unit is
    /// crossing rather than arriving.
    ///
    /// `stopping` is the whole difference and it asks exactly one rule: may
    /// this unit be left standing here. See [`Game::entry_at`] — Civilization
    /// VI checks the stacking layer at the end of a move, so a friendly unit
    /// blocks an arrival and not a crossing. Only [`Game::do_move_to`], which
    /// executes a whole path in one action, passes `false`.
    pub(super) fn can_move_step(&self, uid: u32, pos: Pos, stopping: bool) -> bool {
        let admits = |g: &Self, id: u32, from: Pos, to: Pos| {
            if stopping {
                g.can_enter(id, from, to)
            } else {
                g.can_pass(id, from, to)
            }
        };
        let u = &self.units[&uid];
        if u.linked_to.is_some() && !self.is_linked_leader(uid) {
            return false;
        }
        if self.formation_movement_locked_by_zoc(uid) {
            return false;
        }
        if u.attacks_left < self.unit_max_attacks(uid)
            && self.promotion_effect(u, "move_after_attack") == 0.0
        {
            return false;
        }
        if !self.can_pay_step(uid, u.pos, pos) {
            return false;
        }
        if !admits(self, uid, self.units[&uid].pos, pos) {
            return false;
        }
        if self.is_linked_leader(uid) {
            let peer = self.units[&uid].linked_to.unwrap();
            return self.can_pay_step(peer, self.units[&peer].pos, pos)
                && admits(self, peer, self.units[&peer].pos, pos);
        }
        true
    }

    /// MP is paid before entering (Civ VI): a unit needs the full step cost,
    /// except that one untouched unit may always take one step.
    pub(super) fn can_pay_step(&self, uid: u32, from: Pos, to: Pos) -> bool {
        let Some(unit) = self.units.get(&uid) else {
            return false;
        };
        self.map.tiles.contains_key(&to)
            && (unit.moves_left >= self.unit_max_moves(uid)
                || unit.moves_left >= self.unit_step_cost(uid, from, to))
    }

    pub(super) fn territory_owner_at(&self, pos: Pos) -> Option<usize> {
        self.map
            .get(pos)
            .and_then(|tile| tile.owner_city)
            .and_then(|city| self.cities.get(&city))
            .map(|city| city.owner)
    }

    /// Units which Gathering Storm lets cross an enforced border without a
    /// diplomatic grant. Traders and religious units always ignore closed
    /// borders; Rock Bands use Music Censorship instead; Terracotta Army gives
    /// the same exception to Archaeologists.
    fn unit_ignores_closed_borders(&self, unit: &Unit) -> bool {
        let spec = &self.rules.units[unit.kind];
        spec.class == "religious"
            || unit.kind == "trader"
            || unit.kind == "rock_band"
            || (unit.kind == "archaeologist"
                && self.empire_wonder_effect(unit.owner, "archaeologist_open_borders") > 0.0)
    }

    /// The complete territory gate shared by immediate movement, future route
    /// segments, and displacement when a temporary grant ends.
    ///
    /// A Rock Band is shut out of land closed to bands before anything else
    /// is asked: Music Censorship holds at war and under Open Borders alike.
    /// It used to be checked only on the step itself, so a route planned
    /// straight through a censoring rival and the band froze at its border.
    pub(super) fn unit_has_territory_access(&self, unit: &Unit, territory_owner: usize) -> bool {
        if territory_owner == unit.owner {
            return true;
        }
        if unit.kind == "rock_band" && self.bars_foreign_rock_bands(territory_owner, unit.owner) {
            return false;
        }
        self.unit_ignores_closed_borders(unit)
            || self.is_at_war(unit.owner, territory_owner)
            || self.has_open_borders(unit.owner, territory_owner)
    }

    /// Whether `host` keeps `owner`'s Rock Bands out of its land: its own
    /// Music Censorship, or on a live mirror the host's word for it
    /// ([`Game::rock_band_barred_hosts`]), since the export never shows a
    /// rival's policies.
    pub(crate) fn bars_foreign_rock_bands(&self, host: usize, owner: usize) -> bool {
        host != owner
            && (self.policy_effect(host, "block_foreign_rock_bands") > 0.0
                || self.rock_band_barred_hosts.contains(&(owner, host)))
    }

    /// Whether this unit may take a one-hex step onto `pos` **and finish
    /// there** — the question a single `Move` action asks.
    pub(super) fn can_enter(&self, uid: u32, from: Pos, pos: Pos) -> bool {
        self.entry_at(uid, from, pos, false) == Entry::Stop
    }

    /// [`Self::can_enter`] for a destination returned directly by
    /// [`Self::nbrs`] for `from`.
    pub(super) fn can_enter_neighbor(&self, uid: u32, from: Pos, pos: Pos) -> bool {
        self.entry_at_neighbor(uid, from, pos, false) == Entry::Stop
    }

    /// Whether this unit may cross `pos` on its way somewhere else.
    ///
    /// Weaker than [`Game::can_enter`] by exactly one rule: a tile held by one
    /// of this unit's own units on the same stacking layer, or a peaceful
    /// foreign unit, may be crossed without permitting an occupied arrival.
    pub(crate) fn can_pass(&self, uid: u32, from: Pos, pos: Pos) -> bool {
        self.entry_at(uid, from, pos, false) != Entry::Blocked
    }

    /// [`Self::can_pass`] for a destination returned directly by
    /// [`Self::nbrs`] for `from`.
    pub(super) fn can_pass_neighbor(&self, uid: u32, from: Pos, pos: Pos) -> bool {
        self.entry_at_neighbor(uid, from, pos, false) != Entry::Blocked
    }

    /// Whether two units belonging to one player contend for a single
    /// stacking layer, and so may not end a turn on the same tile.
    pub(super) fn shares_stacking_layer(
        &self,
        spec: &crate::rules::UnitSpec,
        ospec: &crate::rules::UnitSpec,
    ) -> bool {
        // Based aircraft occupy a slot, not the land/naval stacking layer.
        if ospec.domain.as_deref() == Some("air") {
            return false;
        }
        if ospec.class != spec.class {
            return false;
        }
        // Land and naval military units use separate stacking layers,
        // including when the land unit is embarked.
        let separate_military_layers = spec.class == "military"
            && (spec.domain.as_deref() == Some("sea")) != (ospec.domain.as_deref() == Some("sea"));
        !separate_military_layers
    }

    /// Whether two units on the board would contest one stacking layer if they
    /// stood on the same tile. False for an unknown id.
    pub fn units_share_stacking_layer(&self, unit: u32, other: u32) -> bool {
        let (Some(a), Some(b)) = (self.units.get(&unit), self.units.get(&other)) else {
            return false;
        };
        self.shares_stacking_layer(&self.rules.units[a.kind], &self.rules.units[b.kind])
    }

    /// Whether this unit may **end its movement** on `pos`.
    ///
    /// The stacking layer is the only rule that separates finishing a move
    /// from crossing a tile, and it is a fact about the destination alone:
    /// nothing here depends on which neighbour the unit arrives from, which is
    /// why a flood can decide it once per tile. Everything a step must satisfy
    /// on the way — terrain, cliffs, borders, zone of control, foreign units —
    /// belongs to [`Game::entry_at`].
    pub(crate) fn can_stop(&self, uid: u32, pos: Pos) -> bool {
        let Some(u) = self.units.get(&uid) else {
            return false;
        };
        let spec = &self.rules.units[u.kind];
        // A Great Person the host has on this plot but CIVVIS has no unit for
        // holds the civilian layer for its owner. See `great_person_plots`.
        if self
            .great_person_plots
            .get(&pos)
            .is_some_and(|owner| *owner == u.owner && spec.class == "civilian")
        {
            return false;
        }
        for oid in self.unit_ids_at(pos) {
            if *oid == uid {
                continue; // where it already stands is somewhere it may stand
            }
            let o = &self.units[oid];
            if o.owner != u.owner {
                let ospec = &self.rules.units[o.kind];
                // Peace permits crossing, not ending on the foreign unit.
                // Preserve the independent aircraft and religious layers.
                if !self.is_at_war(u.owner, o.owner)
                    && ospec.domain.as_deref() != Some("air")
                    && (spec.class == "religious") == (ospec.class == "religious")
                {
                    return false;
                }
                continue;
            }
            if self.shares_stacking_layer(spec, &self.rules.units[o.kind]) {
                return false;
            }
        }
        true
    }

    /// What a one-hex step onto `pos` is allowed to do.
    ///
    /// ★★★★★ CIVILIZATION VI CHECKS THE STACKING LAYER AT THE END OF A MOVE,
    /// NOT AT EVERY STEP. A unit may walk **through** a tile held by its own
    /// unit of the same layer as long as it has the Movement to leave again,
    /// and may not finish its move there. The shipped game's own end-turn
    /// blocker `ENDTURN_BLOCKING_STACKED_UNITS` — which this repository's
    /// Civilization VI bridge already handles, see
    /// `docs/CIV6_COMPUTER_CONTROL.md` — exists precisely because units are
    /// allowed to be stacked in the middle of a turn. Reference basis: the
    /// in-game Civilopedia "Movement" entry,
    /// <https://www.civilopedia.net/en-US/standard-rules/concepts/movement_3/>.
    /// The rule is pinned by `src/game/movement_rule_tests.rs`; the rest of it
    /// is written down in `docs/MOVEMENT.md`.
    ///
    /// ⚠ This was one boolean for both questions until 2026-08-26, and the
    /// whole engine consequently read a friendly unit as a wall: `flow`,
    /// `path_to` and the multi-turn `route_step` all refused to expand across
    /// one, so no column ever filed through a defile and no front line ever
    /// rotated. It was found by a person playing the Tactics arena, not by any
    /// test, screen or ladder — every A/B in this repository plays both arms
    /// under the same rules, so a rule error is invisible to all of them.
    ///
    /// Everything else a step has to satisfy binds identically at every hex of
    /// a path — terrain, cliffs, rivers, borders, zone of control, hostile
    /// cities, and hostile units. Peaceful foreign occupancy permits transit
    /// but still blocks arrival; see the official Civ VI manual, page 72, and
    /// the recorded neutral-warrior passage in `movement_rule_tests`.
    ///
    /// `through_units` drops occupancy altogether and nothing the rules decide
    /// sets it. It exists for the question somebody watching asks about a unit
    /// that is not theirs: how far can that thing reach, given that whatever
    /// is parked in front of it can walk away or die before it matters.
    pub(super) fn entry_at(&self, uid: u32, from: Pos, pos: Pos, through_units: bool) -> Entry {
        if self.wdist(from, pos) != 1 {
            return Entry::Blocked;
        }
        self.entry_at_neighbor(uid, from, pos, through_units)
    }

    /// [`Self::entry_at`] after the caller has obtained `pos` directly from
    /// [`Self::nbrs`] for `from`.
    ///
    /// The one-step check in the general entry gate is vital for ordinary
    /// callers, but repeating it inside a movement flood is redundant: each
    /// candidate is already an in-map neighbor, including across a wrap seam
    /// or around the globe. Keep this helper private so that precondition
    /// remains local to the few loops that establish it.
    pub(super) fn entry_at_neighbor(
        &self,
        uid: u32,
        from: Pos,
        pos: Pos,
        through_units: bool,
    ) -> Entry {
        let u = &self.units[&uid];
        if !self.unit_can_traverse(uid, pos) {
            return Entry::Blocked;
        }
        // Only a fighter actually flying a patrol over this very tile can
        // block the step, and deciding a war is far from free — a city-state's
        // belligerence follows its Suzerain, which is derived from every
        // major's envoys. This used to walk every unit in the world here, once
        // per neighbour a route search considers; `air_patrols` hoists that
        // whole-board scan to once per memo scope, and the list is empty in
        // every game that never fields a fighter.
        let patrols = self.air_patrols();
        if patrols
            .iter()
            .any(|&(owner, at)| at == pos && owner != u.owner && self.is_at_war(u.owner, owner))
        {
            return Entry::Blocked;
        }
        if !self.unit_can_cross_cliff(uid, from, pos) {
            return Entry::Blocked;
        }
        let spec = &self.rules.units[u.kind];
        // Music Censorship is part of the territory gate below, so the route
        // planner and this step can no longer disagree about it.
        match self.territory_owner_at(pos) {
            // The owner is on the board, so ordinary diplomacy is authoritative.
            Some(owner) => {
                if !self.unit_has_territory_access(u, owner) {
                    return Entry::Blocked;
                }
            }
            // Nobody on this board holds it — but a live host may still say
            // somebody does, and a border whose owning city has never been seen
            // resolves to `None` here. See [`Game::closed_borders`]: without this
            // arm a scout re-sends the same one-hex step for fifty turns while
            // the empire goes blind. Empty in an ordinary CIVVIS game.
            None => {
                if !self.closed_borders.is_empty()
                    && self.closed_borders.contains(&pos)
                    && !self.unit_ignores_closed_borders(u)
                {
                    return Entry::Blocked;
                }
            }
        }
        if let Some(cid) = self.city_at(pos) {
            if self.cities[&cid].owner != u.owner {
                return Entry::Blocked;
            }
        }
        if let Some(cid) = self.defending_district_at(pos) {
            if self.cities[&cid].owner != u.owner {
                return Entry::Blocked;
            }
        }
        if through_units {
            return Entry::Stop;
        }
        // One of our own on the layer we would finish on: the step is legal
        // and the arrival is not.
        let mut stacked = false;
        // A Great Person the host has on this plot but CIVVIS has no unit for,
        // read from the plot map. A foreign one blocks every unit, at war or
        // not: Firaxis captures no Great Person. Live King
        // civvis-20261004T160213Z (game 65) ordered a helicopter MOVE_TO
        // (44,20), a Babylonian Great Scientist's plot beside Kar-Shamash, on
        // turns 193-195 at war with Babylon: the host left it where it stood
        // each turn and its ATTACK on the city struck nothing. One of ours
        // holds the civilian layer against arrival. See `great_person_plots`.
        if let Some(owner) = self.great_person_plots.get(&pos) {
            if *owner != u.owner {
                return Entry::Blocked;
            } else if spec.class == "civilian" {
                stacked = true;
            }
        }
        // Read the occupancy list in place: `units_at` copies it into a fresh
        // `Vec`, and `flow_past` asks this once per neighbour of every tile.
        for oid in self.unit_ids_at(pos) {
            let o = &self.units[oid];
            let ospec = &self.rules.units[o.kind];
            if o.owner != u.owner {
                // Based aircraft occupy a slot, not the land/naval stacking
                // layer. A hostile ground unit may enter the base and
                // subsequently pillage it, which triggers forced aircraft
                // scattering.
                if ospec.domain.as_deref() == Some("air") {
                    continue;
                }
                // Religious units occupy their own layer and may share a tile
                // with any non-religious unit, regardless of diplomacy.
                if (spec.class == "religious") != (ospec.class == "religious") {
                    continue;
                }
                if !self.is_at_war(u.owner, o.owner) {
                    // Official Civ VI manual p72: passage is legal with enough
                    // movement to leave. Native run 20260909T055408Z t9/t12
                    // crosses a neutral city-state warrior in exactly this way.
                    // Keep scanning: a hostile occupant can still block transit.
                    stacked = true;
                    continue;
                }
                if ospec.class == "military" || spec.class != "military" {
                    return Entry::Blocked;
                }
            } else if self.shares_stacking_layer(spec, ospec) {
                // ⚠ Keep scanning. A tile holding one of ours can also hold a
                // foreign religious unit, which blocks the step outright.
                stacked = true;
            }
        }
        if stacked {
            Entry::Pass
        } else {
            Entry::Stop
        }
    }

    /// Whether one unit can be displaced to `pos` without capturing anything,
    /// entering a hostile district, or violating the ordinary stacking layer.
    /// Border expulsion is a teleport, not a move or attack, so enemy units are
    /// never resolved as casualties along the way.
    fn legal_border_displacement(
        &self,
        uid: u32,
        pos: Pos,
        moving: &BTreeSet<u32>,
        forbidden_owner: Option<usize>,
    ) -> bool {
        let unit = &self.units[&uid];
        let spec = &self.rules.units[unit.kind];
        if spec.domain.as_deref() == Some("air") || !self.unit_can_traverse(uid, pos) {
            return false;
        }
        if let Some(owner) = self.territory_owner_at(pos) {
            if Some(owner) == forbidden_owner
                || (owner != unit.owner
                    && (self.is_at_war(unit.owner, owner)
                        || !self.unit_has_territory_access(unit, owner)))
            {
                return false;
            }
        }
        if self
            .city_at(pos)
            .is_some_and(|city| self.cities[&city].owner != unit.owner)
            || self
                .defending_district_at(pos)
                .is_some_and(|city| self.cities[&city].owner != unit.owner)
        {
            return false;
        }
        self.unit_ids_at(pos).iter().all(|other_id| {
            if moving.contains(other_id) {
                return true;
            }
            let other = &self.units[other_id];
            let other_spec = &self.rules.units[other.kind];
            if other_spec.domain.as_deref() == Some("air") {
                return true;
            }
            if other.owner != unit.owner {
                return false;
            }
            if other_spec.class != spec.class {
                return true;
            }
            spec.class == "military"
                && (spec.domain.as_deref() == Some("sea"))
                    != (other_spec.domain.as_deref() == Some("sea"))
        })
    }

    fn border_displacement_destination(
        &self,
        uid: u32,
        forbidden_owner: Option<usize>,
    ) -> Option<Pos> {
        let unit = self.units.get(&uid)?;
        let mut moving = BTreeSet::from([uid]);
        if let Some(peer) = unit.linked_to {
            moving.insert(peer);
        }
        self.map
            .tiles
            .keys()
            .copied()
            .filter(|position| {
                moving.iter().all(|member| {
                    self.legal_border_displacement(*member, *position, &moving, forbidden_owner)
                })
            })
            .map(|position| {
                let rank = match self.territory_owner_at(position) {
                    Some(owner)
                        if owner == unit.owner
                            || self.same_team(unit.owner, owner)
                            || (self.players[owner].is_minor
                                && self.suzerain_of(owner) == Some(unit.owner)) =>
                    {
                        0
                    }
                    None => 1,
                    Some(_) => 2,
                };
                (rank, self.wdist(unit.pos, position), position)
            })
            .min()
            .map(|(_, _, position)| position)
    }

    fn displace_unit_from_closed_border(&mut self, uid: u32, forbidden_owner: Option<usize>) {
        let Some(unit) = self.units.get(&uid) else {
            return;
        };
        if unit.linked_to.is_some() && !self.is_linked_leader(uid) {
            return;
        }
        let Some(destination) = self.border_displacement_destination(uid, forbidden_owner) else {
            return;
        };
        let peer = self.units[&uid].linked_to;
        let carried_aircraft: Vec<u32> = if self.units[&uid].kind == "aircraft_carrier" {
            self.unit_ids_at(self.units[&uid].pos)
                .iter()
                .filter(|other| {
                    **other != uid
                        && self.units[other].owner == self.units[&uid].owner
                        && self.rules.units[self.units[other].kind].domain.as_deref() == Some("air")
                })
                .copied()
                .collect()
        } else {
            Vec::new()
        };
        self.relocate(uid, destination);
        if let Some(peer) = peer {
            self.relocate(peer, destination);
            let maximum = self.unit_max_moves(peer);
            self.units.get_mut(&peer).unwrap().moves_left =
                self.units[&peer].moves_left.min(maximum);
        }
        for aircraft in carried_aircraft {
            self.relocate(aircraft, destination);
        }
        let maximum = self.unit_max_moves(uid);
        self.units.get_mut(&uid).unwrap().moves_left = self.units[&uid].moves_left.min(maximum);
    }

    /// Civ VI expels units when an access grant lapses. Reconcile one mover at
    /// the start of its turn, and all movers when Early Empire first changes a
    /// territory owner's borders from universally open to enforced.
    pub(super) fn reconcile_closed_border_units(&mut self, mover: Option<usize>) {
        let mut displaced: Vec<(u32, usize)> = self
            .units
            .values()
            .filter(|unit| mover.is_none_or(|owner| unit.owner == owner))
            .filter(|unit| self.rules.units[unit.kind].domain.as_deref() != Some("air"))
            .filter_map(|unit| {
                let territory_owner = self.territory_owner_at(unit.pos)?;
                (!self.unit_has_territory_access(unit, territory_owner))
                    .then_some((unit.id, territory_owner))
            })
            .collect();
        displaced.sort_unstable();
        for (uid, territory_owner) in displaced {
            self.displace_unit_from_closed_border(uid, Some(territory_owner));
        }
    }

    pub(super) fn is_linked_leader(&self, uid: u32) -> bool {
        let Some(unit) = self.units.get(&uid) else {
            return false;
        };
        let Some(peer) = unit.linked_to.and_then(|id| self.units.get(&id)) else {
            return false;
        };
        if peer.owner != unit.owner || peer.pos != unit.pos || peer.linked_to != Some(uid) {
            return false;
        }
        let spec = &self.rules.units[unit.kind];
        let peer_spec = &self.rules.units[peer.kind];
        spec.class == "military"
            && (peer_spec.class != "military"
                || (spec.domain.as_deref() == Some("sea")
                    && peer_spec.domain.as_deref() != Some("sea")))
    }
}
