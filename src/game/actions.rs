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

/// Combat actions: attack, ranged, air, WMD and city strikes, pillage. See
/// `game/actions/combat.rs`.
mod combat;
/// Diplomacy actions: war and peace, denouncing, deals, and the turn's
/// diplomacy. See `game/actions/diplomacy.rs`.
mod diplomacy;
/// Legal actions: every action a seat may take, and the deal and payment
/// quotes enumeration needs. See `game/actions/legal.rs`.
mod legal;

/// One band's concert at one venue, as a plan reads it before performing.
struct RockConcertRead {
    host: usize,
    base: f64,
    expected_multiplier: f64,
    survival: f64,
    nearby_factor: f64,
}

/// `(career Tourism multiple, concerts)` for a band of `rock_band_level`
/// 1..=4 (rows) playing venues that add 0..=3 levels (columns), from a Monte
/// Carlo of the stock performance tables (level-ups included, fifteen further
/// concerts at most, no album sales yet). Live, a band that stays on venues
/// worth two levels earns about six times the band that plays any stage.
const ROCK_BAND_STAGE_FUTURE: [[(f64, f64); 4]; 4] = [
    [(8.0, 2.5), (19.9, 3.7), (48.9, 5.9), (82.9, 8.4)],
    [(17.0, 3.5), (40.3, 5.5), (82.9, 8.4), (108.8, 10.2)],
    [(29.7, 4.9), (62.0, 7.3), (108.8, 10.2), (120.7, 11.0)],
    [(38.9, 5.9), (73.4, 8.3), (120.7, 11.0), (120.7, 11.0)],
];

/// What a concert in the band's remaining career pays per multiple. A typical
/// venue is a mix of a thousand-Tourism wonder and 250-750 Tourism buildings.
const ROCK_BAND_STAGE_FUTURE_BASE: f64 = 700.0;

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
        if let Some(cid) = self.defending_district_at(pos) {
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
        spec.domain.as_deref() != Some("air")
            && spec.has_ranged_attack()
            && !self.is_embarked(unit)
            && (!spec.siege || !unit.moved || self.siege_may_attack_after_moving(unit))
            && unit.moves_left > 0.0
            && unit.attacks_left > 0
            && self.wdist(unit.pos, target) <= self.unit_attack_range(uid)
            && self.enemy_ranged_target_at(pid, target)
            && self.combat_target_visible_at(pid, target, visible, viewers)
            && self.unit_has_line_of_sight(uid, target)
            && !self.strike_blocked(uid, target)
    }

    /// Whether the host refused exactly this strike earlier in the current
    /// turn. See `Game::blocked_strikes`: one refusal is enough — the host has
    /// already answered this (unit, target) pair for this turn, and asking it
    /// again in a later frame can only be refused again.
    pub fn strike_blocked(&self, uid: u32, target: Pos) -> bool {
        !self.blocked_strikes.is_empty() && self.blocked_strikes.contains(&(uid, target))
    }

    /// The host's own simulation of `uid` striking `target` this turn, if a
    /// `preview` order asked for it: `(attacker_strength, defender_strength,
    /// damage_to_attacker, damage_to_defender)`. `ranged` picks the
    /// RANGE_ATTACK answer over the melee one. See `Game::host_previews`.
    pub fn host_preview(
        &self,
        uid: u32,
        target: Pos,
        ranged: bool,
    ) -> Option<(f64, f64, i32, i32)> {
        let preview = self.host_previews.get(&(uid, target, ranged))?;
        Some((
            preview.attacker_strength,
            preview.defender_strength,
            preview.damage_to_attacker,
            preview.damage_to_defender,
        ))
    }

    fn ranged_defender_strength(&self, defender: &Unit, target: Pos, attacker_owner: usize) -> f64 {
        let def_base = self.unit_strength(defender, true)
            + self.visibility_combat_bonus(defender.owner, attacker_owner)
            + self.ranged_defense_bonus(defender, false)
            + if self.rules.units[defender.kind].domain.as_deref() == Some("air") {
                0.0
            } else {
                self.tile_defense_bonus(target)
            }
            + self.vs_bonus(defender.owner, attacker_owner);
        effective_strength(def_base, defender.hp)
    }

    /// A memory-policy estimate using a full-health, unpromoted unit type,
    /// not an unseen unit's actual combat state. Known defender bonuses use
    /// the same arithmetic as a real shot. This is not a maximum-damage bound.
    pub(crate) fn nominal_ranged_damage_from_kind(
        &self,
        kind: Name,
        attacker_owner: usize,
        did: u32,
        target: Pos,
    ) -> Option<f64> {
        let spec = &self.rules.units[kind];
        if !spec.has_ranged_attack() {
            return None;
        }
        let mut defender = self.units.get(&did)?.clone();
        if defender.pos != target {
            defender.pos = target;
            defender.fortify_turns = 0;
        }
        let defender_is_sea = self.rules.units[defender.kind].domain.as_deref() == Some("sea");
        let mut attack = spec.ranged_strength.max(spec.bombard_strength)
            + self.vs_bonus(attacker_owner, defender.owner);
        if (spec.bombard_strength > 0.0 && !defender_is_sea)
            || (spec.ranged_strength > 0.0
                && spec.domain.as_deref() != Some("sea")
                && defender_is_sea)
        {
            attack -= 17.0;
        }
        Some(expected_damage(
            attack,
            self.ranged_defender_strength(&defender, target, attacker_owner),
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
            .or_else(|| self.defending_district_at(pos))
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
                | Action::DistrictStrike { .. }
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
            Action::DisbandUnit { unit } => self.do_disband_unit(pid, *unit),
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
            Action::DistrictStrike {
                city,
                source,
                target,
            } => self.do_defending_district_strike(pid, *city, *source, *target),
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
                    | Action::DistrictStrike { .. }
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

    pub(in crate::game) fn visibility_combat_bonus(&self, owner: usize, opponent: usize) -> f64 {
        // Expansion1_Civilizations_Major.xml:186-187,636-643: Ortoo adds
        // another Amount=3, DeltaWithOpponent=true modifier to the major
        // civilization's ordinary +3. It belongs to Mongolia, including
        // Kublai, rather than to Genghis's cavalry-only leader ability.
        let per_level = if self.owns_civ_unique(owner, "Mongolia") {
            6.0
        } else {
            3.0
        };
        per_level
            * (self.diplomatic_visibility(owner, opponent)
                - self.diplomatic_visibility(opponent, owner))
            .max(0.0)
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
            defending_districts: Vec::new(),
            last_attacked: 0,
            pressure: BTreeMap::new(),
            loyalty: 100.0,
            free_city_pressure: BTreeMap::new(),
            captured_from: None,
            religion_next_seen: None,
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
            // The ball itself rather than a filter over the world: on a
            // 57,762-tile globe that filter ran a path search per tile, about
            // ten seconds for every city founded.
            let nearby: Vec<&crate::world::Tile> = self
                .map
                .disk(pos, 2)
                .into_iter()
                .filter_map(|near| self.map.tiles.get(&near))
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
        if self.blocked_improvement_sites.contains(&pos) {
            return vec![];
        }
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
            // The host will not clear or harvest under an improvement. Across
            // 70 live logs (2026-11-10), all 81 orders to chop or clear an
            // improved tile failed (43 of them forests under Lumber Mills),
            // as did 32 of 33 harvests. Each one spent a planned Builder turn.
            || tile.improvement.is_some()
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
        // Clearing uses Action::Improve too. Respect the same host exclusions
        // as ordinary improvements, before a planner spends the unit's turn.
        if let Some(refused) = self.blocked_improvements.get(&pos) {
            operations.retain(|operation| !refused.contains(&Name::new(operation)));
        }
        operations
    }

    /// How far a feature removal or a harvest has grown past its shipped base.
    ///
    /// Measured on the host (Deity, Online speed, 2026-11-09, the owning
    /// city's production progress either side of 48 accepted Forest
    /// clearings): the city received `base × game speed × (1 + 9 × progress)`,
    /// where `progress` is the share of every technology and civic this
    /// player has completed. A Forest paid 14 Production with 7 of them
    /// done, 15 with 8 or 9 and 19 with 14 or 15. The world era is not part of
    /// it, and neither is the farther-advanced single tree that district costs
    /// read ([`Self::game_progress_ratio`]).
    pub(crate) fn builder_harvest_scale(&self, pid: usize) -> f64 {
        let player = &self.players[pid];
        let completed = player
            .techs
            .iter()
            .filter(|technology| self.rules.techs.contains_key(technology))
            .count()
            + player
                .civics
                .iter()
                .filter(|civic| self.rules.civics.contains_key(civic))
                .count();
        let all = (self.rules.techs.len() + self.rules.civics.len()).max(1);
        self.game_speed.scale(1.0) * (1.0 + 9.0 * completed as f64 / all as f64)
    }

    /// What [`Self::do_builder_operation`] would pay for `operation` on `pos`,
    /// as `(yield type, amount)` rows.
    ///
    /// The executor calls this, so an agent that prices a chop before ordering
    /// one reads the number it is actually going to be paid. Keeping the two
    /// on one function is the point: a separate estimate in the controller is
    /// free to drift from the rule, and would drift silently.
    ///
    /// Shipped bases: `Feature_Removes` (Forest 20 Production, Jungle 10
    /// Production + 10 Food, Marsh 20 Food) and `Resource_Harvests`, both read
    /// out of `data/*.json`. They scale by [`Self::builder_harvest_scale`],
    /// Magnus' `harvest_pct` applies to removals and harvests alike, Production
    /// takes the production bonus of the item the owning city is building,
    /// and the host rounds the result down.
    pub(crate) fn builder_operation_payout(
        &self,
        pid: usize,
        pos: Pos,
        operation: &str,
    ) -> Vec<(String, f64)> {
        let Some(tile) = self.map.get(pos) else {
            return Vec::new();
        };
        let Some(cid) = tile.owner_city else {
            return Vec::new();
        };
        let scale = self.builder_harvest_scale(pid)
            * (1.0 + self.governor_effect(pid, cid, "harvest_pct") / 100.0);
        // The host pays a clearing's Production at the production bonus of
        // whatever the owning city is building. Measured on live Deity games,
        // 2026-11-09/10: with no card a Forest paid exactly the plain rule (5
        // of 5), and a Forest cleared into a Settler under Colonization paid
        // 27 to 30 where the plain rule gives 19 to 20. Food and Gold carry no
        // such bonus.
        let item_bonus = self
            .cities
            .get(&cid)
            .map_or(1.0, |city| {
                self.item_prod_mult(pid, cid, city.queue.first())
            })
            .max(0.0);
        let pay = |yield_type: &str, base: f64| {
            let bonus = if yield_type == "production" {
                item_bonus
            } else {
                1.0
            };
            (base * scale * bonus).floor()
        };
        match operation {
            "chop_woods" | "chop_rainforest" | "clear_marsh" => tile
                .feature
                .as_deref()
                .and_then(|feature| self.rules.features.get(feature))
                .map(|spec| {
                    spec.chop
                        .iter()
                        .map(|(yield_type, base)| (yield_type.clone(), pay(yield_type, *base)))
                        .collect()
                })
                .unwrap_or_default(),
            "harvest_resource" => tile
                .resource
                .as_deref()
                .and_then(|resource| self.rules.resources.get(resource))
                .and_then(|spec| spec.harvest.as_ref())
                .map(|harvest| {
                    vec![(
                        harvest.yield_type.clone(),
                        pay(&harvest.yield_type, harvest.amount),
                    )]
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        }
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
        let removed_feature = self.map.tiles[&unit.pos].feature;
        // ⭐ ONE RULE, TWO READERS. The payout is computed by the same
        // function an agent prices the operation with, so a controller that
        // chooses to chop cannot be reading a different number from the one
        // this pays. See `builder_operation_payout`.
        let payouts = self.builder_operation_payout(pid, unit.pos, operation);
        match operation {
            "chop_woods" | "chop_rainforest" | "clear_marsh" => {
                if removed_feature.is_none() {
                    return Err("no feature to clear".into());
                }
                self.map.tiles.get_mut(&unit.pos).unwrap().feature = None;
            }
            "plant_woods" => {
                self.map.tiles.get_mut(&unit.pos).unwrap().feature = Some(crate::name!("forest"));
            }
            "harvest_resource" => {
                // `builder_operations` only offers this on a resource with a
                // shipped `Resource_Harvests` row whose technology is in hand,
                // and the gate above proves this operation came from it.
                self.map.tiles.get_mut(&unit.pos).unwrap().resource = None;
            }
            _ => return Err("unknown builder operation".into()),
        }
        let payouts_total: f64 = payouts.iter().map(|(_, amount)| *amount).sum();
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
            self.players[pid].gold += payouts_total;
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
                tile.improvement_owner = Some(pid);
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
            } else {
                t.improvement = Some(Name::new(imp));
                t.improvement_owner = Some(pid);
                t.pillaged = false;
            }
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
        // A Legion or Toa spends its construction charge and remains a combat
        // unit. Civilian and support builders are consumed when depleted.
        if self.units[&uid].charges <= 0
            && self.rules.units[&self.units[&uid].kind].class != "military"
        {
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
        let native_plots = band
            .and_then(|unit| self.host_unit_facts.get(&unit.id))
            .and_then(|facts| facts.concert_plots.as_ref());
        if native_plots.is_some_and(|plots| !plots.contains(&position)) {
            return None;
        }
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
        // SelectedUnit_Expansion2.lua:65-70 highlights the plots this Native
        // band could use its action on. An exact positive offer is stronger
        // than the model's peace-only assumption; keep that assumption when
        // no host menu exists, including ordinary headless games.
        if city.owner == pid
            || self.players[city.owner].is_minor
            || (native_plots.is_none() && self.is_at_war(pid, city.owner))
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
        // SelectedUnit_Expansion2.lua:65-70 uses the RockBand component's
        // activation highlights. An affirmative native venue remains usable
        // when public foreign-city data omits its buildings. Estimate its
        // unknown tier with the least valuable compatible modeled building;
        // do not invent buildings or discard a known higher-tier value.
        let building_tourism = if building_tourism <= 0.0 && native_plots.is_some() {
            self.rules
                .buildings
                .values()
                .filter(|spec| {
                    spec.district
                        .is_some_and(|required| self.district_is_family(district, required))
                })
                .filter_map(|spec| spec.effects.get("rock_concert_tourism").copied())
                .filter(|value| value.is_finite() && *value > 0.0)
                .min_by(f64::total_cmp)
                .unwrap_or(0.0)
        } else {
            building_tourism
        };
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

    /// Native Bands expose performance level independently of promotion level.
    /// Older exports and headless games retain the existing unit-level rule.
    pub(crate) fn rock_band_performance_level(&self, band: &Unit, venue_levels: i32) -> i32 {
        let level = self
            .host_unit_facts
            .get(&band.id)
            .and_then(|facts| facts.rock_band_level)
            .unwrap_or(band.level);
        level.saturating_add(venue_levels).clamp(1, 6)
    }

    /// The band's concert reading at this venue: venue Tourism, the expected
    /// result and album multiplier, survival odds and the Goes To 11 audience.
    fn rock_concert_read(&self, pid: usize, unit: &Unit, position: Pos) -> Option<RockConcertRead> {
        let (base, venue_levels, host_city) =
            self.rock_concert_venue_details(pid, Some(unit), position)?;
        let effective_level = self.rock_band_performance_level(unit, venue_levels);
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
        // Goes to 11 adds Tourism to each distinct nearby civilization. Price
        // the known audience, as the concert below already does for its actual
        // recipients; unrevealed cities must not inform an AI's venue choice.
        let nearby_pct = self.promotion_effect(unit, "rock_nearby_tourism_pct") / 100.0;
        let host = self.cities[&host_city].owner;
        let nearby = if nearby_pct > 0.0 {
            self.cities
                .values()
                .filter(|city| {
                    city.owner != pid
                        && city.owner != host
                        && self.players[city.owner].alive
                        && !self.players[city.owner].is_minor
                        && !self.players[city.owner].is_barbarian
                        && self.has_met(pid, city.owner)
                        && self.players[pid].explored.contains(&city.pos)
                        && self.wdist(position, city.pos) <= 10
                })
                .map(|city| city.owner)
                .collect::<BTreeSet<_>>()
                .len()
        } else {
            0
        };
        Some(RockConcertRead {
            host,
            base,
            expected_multiplier,
            survival,
            nearby_factor: 1.0 + nearby_pct * nearby as f64,
        })
    }

    pub(crate) fn rock_concert_ai_value(&self, pid: usize, uid: u32, position: Pos) -> Option<f64> {
        let unit = self.units.get(&uid)?;
        if unit.owner != pid || unit.kind != "rock_band" || unit.promotions.is_empty() {
            return None;
        }
        let read = self.rock_concert_read(pid, unit, position)?;
        Some(read.base * (read.expected_multiplier * read.nearby_factor + 2.0 * read.survival))
    }

    /// `band-plays-its-stage`: the concert's expected Tourism plus the rest of
    /// the band's career, which only the survivors get. See
    /// [`rock_band_stage_future`](Self::rock_band_stage_future).
    pub(crate) fn rock_concert_stage_value(
        &self,
        pid: usize,
        uid: u32,
        position: Pos,
    ) -> Option<f64> {
        let unit = self.units.get(&uid)?;
        if unit.owner != pid || unit.kind != "rock_band" || unit.promotions.is_empty() {
            return None;
        }
        let read = self.rock_concert_read(pid, unit, position)?;
        Some(
            (read.base * read.expected_multiplier * read.nearby_factor
                + read.survival * self.rock_band_stage_future(unit))
                * self.rock_concert_race_factor(pid, read.host),
        )
    }

    /// A visitor always adds one to our count. Taken from the civilization
    /// holding the Culture bar, it also comes out of that rival's domestic
    /// tourists, so the same concert closes the gap twice. Live, one game drew
    /// 180 of its 226 visitors from a rival far below the bar.
    fn rock_concert_race_factor(&self, pid: usize, host: usize) -> f64 {
        let bar = self
            .players
            .iter()
            .filter(|rival| {
                rival.id != pid && rival.alive && !rival.is_minor && !rival.is_barbarian
            })
            .map(|rival| self.domestic_tourists(rival.id))
            .max()
            .unwrap_or(0);
        if bar <= 0 {
            return 1.0;
        }
        1.0 + (self.domestic_tourists(host).max(0) as f64 / bar as f64).clamp(0.0, 1.0)
    }

    /// The Tourism this band can still earn. A concert adds the band's album
    /// sales straight onto its result, and Tier 5-6 concerts raise the band's
    /// level, so a surviving band's future is far larger than the concert in
    /// front of it. `ROCK_BAND_STAGE_FUTURE` is a Monte Carlo table of the
    /// stock performance tables, capped at fifteen further concerts.
    pub(crate) fn rock_band_stage_future(&self, unit: &Unit) -> f64 {
        let level = self
            .host_unit_facts
            .get(&unit.id)
            .and_then(|facts| facts.rock_band_level)
            .unwrap_or(unit.level)
            .clamp(1, 4) as usize;
        let bonus = [
            "rock_wonder_levels",
            "rock_entertainment_levels",
            "rock_theater_levels",
            "rock_water_park_levels",
            "rock_nature_levels",
            "rock_space_levels",
            "rock_surf_levels",
        ]
        .into_iter()
        .map(|effect| self.promotion_effect(unit, effect))
        .fold(0.0_f64, f64::max)
        .clamp(0.0, 3.0) as usize;
        let (lifetime, concerts) = Self::rock_band_stage_multiple(level, bonus);
        (lifetime + unit.album_sales as f64 / 100.0 * concerts) * ROCK_BAND_STAGE_FUTURE_BASE
    }

    /// Row `level` (1..=4) and column `bonus` (0..=3) of the career table: the
    /// Tourism multiple of a band that plays such venues, and its concerts.
    pub(crate) fn rock_band_stage_multiple(level: usize, bonus: usize) -> (f64, f64) {
        ROCK_BAND_STAGE_FUTURE[level.clamp(1, 4) - 1][bonus.min(3)]
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
        let effective_level = self.rock_band_performance_level(&band, venue_levels);
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
                    .insert(religion.clone(), competing + 1.0);
                // Followers are whole Citizens, so one point past the rest
                // can still seat fewer than half of them when the rest is
                // split over several rows. Out-press them by the city's size
                // and every split leaves this faith at least half.
                if self.city_religion(&self.cities[&host_city]) != Some(religion.as_str()) {
                    let size = f64::from(self.cities[&host_city].pop.max(1));
                    self.cities
                        .get_mut(&host_city)
                        .unwrap()
                        .pressure
                        .insert(religion, competing * size + 1.0);
                }
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

    /// Strategic bombing additionally requires an unoccupied land target:
    /// shipped Civilopedia_Concepts_Text.xml:1041 says "if no land unit
    /// currently occupies the target location". Unlike ground pillage's
    /// district garrison check, this includes civilians/support, all owners,
    /// and improvements too. Barbarian camps still require entering the tile.
    pub(super) fn air_pillageable_at(&self, pid: usize, pos: Pos) -> bool {
        self.map
            .get(pos)
            .is_some_and(|tile| tile.improvement.as_deref() != Some("barbarian_camp"))
            && self.pillageable_at(pid, pos)
            && !self.unit_ids_at(pos).iter().any(|id| {
                matches!(
                    self.rules.units[self.units[id].kind].domain.as_deref(),
                    None | Some("land")
                )
            })
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

    pub(crate) fn air_capacity_at(&self, pid: usize, pos: Pos) -> i32 {
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
                }
            }
            let owner = match tile.owner_city {
                Some(cid) => self.cities.get(&cid).map(|city| city.owner),
                None => tile.improvement_owner,
            };
            if owner == Some(pid)
                && tile.improvement.as_deref() == Some("airstrip")
                && !tile.pillaged
            {
                capacity += self.rules.improvements["airstrip"]
                    .effects
                    .get("air_slots")
                    .copied()
                    .unwrap_or(3.0) as i32;
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

    /// Warrior Monks are a religious-unit unlock attached to the player's
    /// founded religion, not a generic faith discount for military units.
    /// Keeping this separate from the Theocracy/Masonry faith-purchase gate
    /// prevents the unit from becoming available to every military-faith
    /// buyer once the roster contains it.
    fn player_has_warrior_monks_belief(&self, pid: usize) -> bool {
        self.players
            .get(pid)
            .and_then(|player| player.religion.as_deref())
            .is_some_and(|religion| {
                self.religion_belief_effect(religion, "warrior_monk_purchase") > 0.0
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
        // Rules already own the canonical name. Avoid the global interner lock
        // on every quote, retaining the existing key behavior for unknown units.
        let unit_name = self
            .rules
            .units
            .get_key_value(unit)
            .map_or_else(|| Name::new(unit), |(name, _)| *name);
        let key = (pid, cid, unit_name, formation, currency_is_gold);
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

    /// Gitarja (Indonesia's only leader): every naval military unit may be
    /// bought with Faith. Shipped `TRAIT_LEADER_EXALTED_GODDESS`
    /// ("Naval units can be purchased with Faith"); live Indonesia games
    /// export Faith prices for Galleys, Quadriremes, Caravels, Privateers and
    /// Ironclads that the model used to refuse.
    pub(crate) fn exalted_goddess_naval(player: &Player, spec: &crate::rules::UnitSpec) -> bool {
        player.civ == "Indonesia"
            && spec.class == "military"
            && matches!(spec.domain.as_deref(), Some("sea"))
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
        let (unit_name, spec) = self.rules.units.get_key_value(unit)?;
        if unit == "spy" || formation > 2 || !matches!(currency, "gold" | "faith") {
            return None;
        }
        // ★ The host's own answer first, for the standard formation the menu
        // prices (`StateCity::purchasable`). Corps and Army purchases are not
        // exported and keep the model's arithmetic below.
        if formation == 0 {
            let plain = Item::Unit { unit: *unit_name };
            if let Some(host) = self.host_purchase_price(cid, &plain, currency) {
                return host.filter(|_| !self.purchase_is_blocked(cid, &plain));
            }
        }
        if unit == "settler" && (city.pop < 2 || self.policy_effect(pid, "no_settling") > 0.0) {
            return None;
        }

        let religious = spec.class == "religious";
        let warrior_monk = unit == "warrior_monk";
        let rock_band = unit == "rock_band";
        let naturalist = unit == "naturalist";
        let nihang = unit == "nihang"
            && formation == 0
            && currency == "faith"
            && self.grants_city_state_unique_bonus(pid, "Lahore");
        if formation > 0 && (religious || warrior_monk || rock_band || naturalist) {
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
        } else if warrior_monk {
            if currency != "faith"
                || !self.player_has_warrior_monks_belief(pid)
                || !self.unlocked(pid, &spec.tech, &spec.civic)
                || spec.requires_building.as_ref().is_some_and(|building| {
                    !self.city_has_building_family(city, Name::new(building))
                })
            {
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
                Item::Unit { unit: *unit_name }
            } else {
                Item::Formation {
                    unit: *unit_name,
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
                // Gitarja's Exalted Goddess of the Three Worlds: naval units
                // can be purchased with Faith (`Indonesia_Khmer_GameplayData.xml`,
                // `TRAIT_LEADER_EXALTED_GODDESS`; the live host quotes them).
                let exalted_goddess = Self::exalted_goddess_naval(player, spec);
                if !monumentality && !faith_land_combat && !exalted_goddess {
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
            let item = Item::Unit { unit: *unit_name };
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
        let warrior_monk = unit == "warrior_monk";
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
        if formation > 0 && (rock_band || naturalist || religious || warrior_monk) {
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
        } else if warrior_monk {
            if currency != "faith" {
                return Err("Warrior Monks are bought with faith".into());
            }
            if !self.player_has_warrior_monks_belief(pid) {
                return Err("Warrior Monks require the founded religion's belief".into());
            }
            if !self.unlocked(pid, &spec.tech, &spec.civic) {
                return Err("not unlocked".into());
            }
            if spec.requires_building.as_ref().is_some_and(|building| {
                !self.city_has_building_family(&self.cities[&cid], Name::new(building))
            }) {
                return Err("required religious building is missing".into());
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
                // See `unit_purchase_cost_for_formation_uncached`.
                let exalted_goddess = Self::exalted_goddess_naval(&self.players[pid], spec);
                if !monumentality && !faith_land_combat && !exalted_goddess {
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
        // Native purchases prefer the Holy Site itself; an occupied site
        // falls back to the city center, not to the site's neighboring tiles.
        let religious_site = if religious {
            let city = &self.cities[&cid];
            city.districts.iter().find_map(|(district, position)| {
                (self.district_is_family(district, crate::name!("holy_site"))
                    && self.district_is_active(city, district, *position)
                    && self.map.get(*position).is_some_and(|tile| {
                        self.rules.is_passable(tile) && !self.rules.is_water(tile)
                    })
                    && self.unit_ids_at(*position).iter().all(|uid| {
                        let other = &self.units[uid];
                        other.owner == pid && self.rules.units[other.kind].class != "religious"
                    }))
                .then_some(*position)
            })
        } else {
            None
        };
        let placed = religious_site
            .map(|site| self.spawn_unit(unit, pid, site))
            .or_else(|| self.place_new_unit(unit, pid, pos))
            .ok_or_else(|| "no space to place unit".to_string())?;
        if religious && self.rules.units[unit].religious_spread > 0.0 {
            // spawn_unit applies a building bonus only at a city center.
            // A purchase always takes that bonus from its source city,
            // including when district placement or overflow moves it away.
            let source_bonus = self
                .city_building_effect(&self.cities[&cid], "religious_unit_spread_charges")
                as i32;
            let spawned_bonus = self
                .city_at(self.units[&placed].pos)
                .filter(|city| self.cities[city].owner == pid)
                .map_or(0, |city| {
                    self.city_building_effect(&self.cities[&city], "religious_unit_spread_charges")
                        as i32
                });
            self.units.get_mut(&placed).unwrap().charges += source_bonus - spawned_bonus;
        }
        self.set_unit_formation(placed, formation);
        self.apply_training_district_effects(cid, placed);
        if unit == "builder" {
            self.units.get_mut(&placed).unwrap().charges +=
                self.governor_effect(pid, cid, "builder_charges") as i32;
        }
        if religious || warrior_monk {
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
            building: self
                .rules
                .buildings
                .get_key_value(building)
                .map_or_else(|| Name::new(building), |(name, _)| *name),
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

    /// Remove one of `pid`'s own units, as the host's Delete button does.
    /// The host refuses to disband a wounded unit. Across 70 live logs
    /// (2026-11-10), 82 of 83 orders to delete a wounded unit were refused and
    /// all 28 for a unit at full health went through.
    pub(crate) fn unit_can_be_disbanded(&self, unit: u32) -> bool {
        self.units.get(&unit).is_some_and(|unit| unit.hp >= 100)
    }

    pub(super) fn do_disband_unit(&mut self, pid: usize, unit: u32) -> Result<(), String> {
        if self.units.get(&unit).is_none_or(|u| u.owner != pid) {
            return Err("not your unit".into());
        }
        if !self.unit_can_be_disbanded(unit) {
            return Err("a wounded unit cannot be disbanded".into());
        }
        self.remove_unit(unit);
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
            return self.switch_host_research(pid, tech);
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
        let promotion_keeps_moves = self.civ_effect(pid, "promote_no_finish_moves") > 0.0;
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
            if !promotion_keeps_moves {
                unit.moves_left = 0.0;
                unit.attacks_left = 0;
            }
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
        if self.host_policy_change_blocked(pid)
            && !self.players[pid].policies.contains(&Name::new(policy))
        {
            return Err("host policy change window closed".into());
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
        if self.host_policy_change_blocked(pid)
            && self.players[pid].policies.contains(&Name::new(policy))
        {
            return Err("host policy change window closed".into());
        }
        if !self.players[pid].policies.remove(&Name::new(policy)) {
            return Err("policy not slotted".into());
        }
        Ok(())
    }

    pub(super) fn do_defending_district_strike(
        &mut self,
        pid: usize,
        cid: u32,
        position: Pos,
        target: Pos,
    ) -> Result<(), String> {
        let city = self
            .cities
            .get(&cid)
            .filter(|city| city.owner == pid)
            .ok_or_else(|| "not your city".to_string())?;
        let mut district = self
            .defending_district_state(cid, position)
            .ok_or_else(|| "city has no defending district at source".to_string())?;
        if !self.defending_district_can_strike(city, &district) {
            return Err("defending district cannot strike".into());
        }
        if self.wdist(position, target) > 2 || !self.has_line_of_sight(position, target, true) {
            return Err("target out of range or sight".into());
        }
        let visible = self.player_vision_frame(pid);
        let viewers = self.visibility_viewers(pid);
        if !self.combat_target_visible_at(pid, target, visible.as_ref(), &viewers) {
            return Err("target is not visible".into());
        }
        if self.city_at(target).is_some() || self.defending_district_at(target).is_some() {
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
        if district.struck {
            district.extra_strikes_used += 1;
        }
        district.struck = true;
        self.set_defending_district_state(cid, district);
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
        self.relationship_opinion_reading(observer, subject, &mut |pid, measure| {
            self.agenda_measure(pid, measure)
        })
    }

    fn relationship_opinion_reading(
        &self,
        observer: usize,
        subject: usize,
        read: &mut dyn FnMut(usize, &str) -> f64,
    ) -> f64 {
        if observer == subject || observer >= self.players.len() || subject >= self.players.len() {
            return 0.0;
        }
        let mut opinion = self.agenda_opinion_reading(observer, subject, read);
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
        self.relationship_state_reading(observer, subject, &mut |pid, measure| {
            self.agenda_measure(pid, measure)
        })
    }

    /// `relationship_state` from each of `civs` toward each of them, in order.
    /// Agendas compare a civilization with the whole world, so scoring every
    /// pair directly would be cubic in civilizations; each comparison's raw
    /// measure is worked out once here instead.
    pub fn relationship_states_among(&self, civs: &[usize]) -> Vec<Vec<&'static str>> {
        let mut measured: Vec<(String, Vec<Option<f64>>)> = Vec::new();
        let mut read = |pid: usize, measure: &str| {
            let index = measured
                .iter()
                .position(|(name, _)| name == measure)
                .unwrap_or_else(|| {
                    measured.push((measure.to_string(), vec![None; self.players.len()]));
                    measured.len() - 1
                });
            *measured[index].1[pid].get_or_insert_with(|| self.agenda_measure(pid, measure))
        };
        civs.iter()
            .map(|&observer| {
                civs.iter()
                    .map(|&subject| self.relationship_state_reading(observer, subject, &mut read))
                    .collect()
            })
            .collect()
    }

    fn relationship_state_reading(
        &self,
        observer: usize,
        subject: usize,
        read: &mut dyn FnMut(usize, &str) -> f64,
    ) -> &'static str {
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
        } else {
            let opinion = self.relationship_opinion_reading(observer, subject, read);
            if opinion >= 15.0 {
                "friendly"
            } else if opinion <= -15.0 {
                "unfriendly"
            } else {
                "neutral"
            }
        }
    }

    pub(super) fn denounced_active(&self, denouncer: usize, target: usize) -> bool {
        self.players
            .get(denouncer)
            .and_then(|player| player.denounced_until.get(&target))
            .is_some_and(|until| *until > self.turn)
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

    /// What `amount` of the strategic `resource` (a CIVVIS id, `niter`) is
    /// worth to `receiver` in Gold by this engine's book: the live seat's
    /// ceiling for a strategic purchase (`siege-buys-the-gun-resource`).
    /// `0` for an unknown or non-strategic resource.
    pub fn strategic_gold_value(&self, receiver: usize, resource: &str, amount: u32) -> f64 {
        if self
            .rules
            .resources
            .get(resource)
            .is_none_or(|spec| spec.class != "strategic")
        {
            return 0.0;
        }
        self.resource_receive_value(receiver, resource, amount.min(i32::MAX as u32) as i32)
    }

    /// What `amount` of a luxury or strategic `resource` (a CIVVIS id) is
    /// worth to `receiver` in Gold by this engine's book: the live seat's
    /// reading of what a barter item puts on a rival's side of the scale
    /// (`siege-buys-the-gun-resource`). `0` for any other resource.
    pub fn resource_gold_value(&self, receiver: usize, resource: &str, amount: u32) -> f64 {
        if self
            .rules
            .resources
            .get(resource)
            .is_none_or(|spec| spec.class != "strategic" && spec.class != "luxury")
            || receiver >= self.players.len()
        {
            return 0.0;
        }
        self.resource_receive_value(receiver, resource, amount.min(i32::MAX as u32) as i32)
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
        // ★ A GIFT IS LEGAL, AND THIS ENGINE PRICES IT AT NOTHING. A one-sided
        // deal that only GIVES proposes and the recipient accepts. Civilization
        // VI pays for it: `Leaders.xml` STANDARD_DIPLOMATIC_ONE_SIDED_TRADES
        // gives one opinion point per 10 value of a deal in the AI's favour,
        // up to 20, fading a point every two turns (live 222639Z t115: 80
        // Gold, -3 to +7). `relationship_opinion` has no such term, a known
        // gap, so a screen cannot price `tribute-buys-the-friendship`. Until
        // 2026-08-24 the engine refused the gift outright, which was stricter
        // than the game it mirrors and hid the question the live seat
        // actually faces. An
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

        // A mirrored Trade is a request to the host, not a settled exchange.
        // Keep the action in apply's log for export, but wait for the next
        // observed board before spending its proceeds or using its passage.
        // The simulator still settles its own validated exchanges below.
        if from == MIRRORED_SEAT && !self.host_observed.is_empty() {
            return Ok(());
        }

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
            self.score_aid_gold_gift(from, to, offer.gold);
        }
        Ok(())
    }

    /// The gift ledger: what a seat gave for nothing, and what it was given.
    /// This is not relationship credit. An eligible Aid Request can separately
    /// award competition score for the Gold actually transferred.
    pub(super) fn record_gift(&mut self, from: usize, to: usize) {
        bump(&mut self.players[from], "gifts_given");
        bump(&mut self.players[to], "gifts_received");
    }

    /// Score only a completed gift to the currently running request's target.
    /// Sending an offer, paying for goods, and giving to an unrelated rival do
    /// not satisfy the shipped FromGold source. Host scores remain host-owned.
    fn score_aid_gold_gift(&mut self, from: usize, to: usize, gold: f64) {
        if self
            .competition
            .as_ref()
            .is_some_and(|running| running.target == Some(to))
        {
            self.score_native_competition(from, CompetitionScoreSource::GoldGift, gold);
        }
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
}

#[cfg(test)]
mod air_domain_tests;

#[cfg(test)]
#[path = "air_pillage_refusal_tests.rs"]
mod air_pillage_refusal_tests;

#[cfg(test)]
#[path = "neutral_airstrip_tests.rs"]
mod neutral_airstrip_tests;

#[cfg(test)]
#[path = "gran_colombia_promotions_tests.rs"]
mod gran_colombia_promotions_tests;

#[cfg(test)]
mod ranged_visibility_tests;

#[cfg(test)]
#[path = "builder_operation_refusal_tests.rs"]
mod builder_operation_refusal_tests;
