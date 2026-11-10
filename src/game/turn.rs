//! The turn: what happens to every player and city as a turn begins.
//!
//! `begin_turn`, which advances each player through the new turn;
//! `process_city` and `process_city_with_upkeep`, which run one city's
//! turn (growth, production, culture, borders) and the upkeep it owes;
//! modernizing a city's queued units when a better unit unlocks
//! (`modernize_unit_queue`); and the damage nuclear fallout does to units
//! standing in it (`irradiate_units`). The helpers `begin_turn` calls for
//! arena mode, exoplanets, tourism, spies and religious pressure stay with
//! their own families.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

impl Game {
    /// Bleed every one of `pid`'s units that is standing in fallout.
    ///
    /// A blast that only zeroed a tile's yields would deny nothing: the crater
    /// is the last place a defender would want to stand and exactly the place
    /// it could stand for free. Charged at the start of the owner's turn, so a
    /// unit that walks in and out inside one turn pays nothing and one that
    /// holds the ground pays every turn it holds it. Fatalities go through the
    /// same removal path as any other death, so a unit killed by fallout is
    /// counted, logged and unlinked like one killed in battle.
    pub(super) fn irradiate_units(&mut self, pid: usize) {
        for uid in self.player_unit_ids(pid) {
            let Some(unit) = self.units.get(&uid) else {
                continue;
            };
            if !self.fallout_at(unit.pos) {
                continue;
            }
            let position = unit.pos;
            let hp = unit.hp - Self::FALLOUT_UNIT_DAMAGE;
            if hp <= 0 {
                let kind = pretty(&self.units[&uid].kind);
                self.remove_unit(uid);
                self.note_important(
                    pid,
                    "Nuclear",
                    format!("lost a {kind} to nuclear fallout"),
                    Some(position),
                );
            } else {
                self.units.get_mut(&uid).unwrap().hp = hp;
            }
        }
    }

    pub(super) fn begin_turn(&mut self, pid: usize) {
        self.process_anarchy(pid);
        self.process_levies(pid);
        self.process_spies(pid);
        self.advance_exoplanet(pid);
        if self.is_finished() {
            return;
        }
        self.check_boosts(pid);
        // Before the yields below are paid, so the arena's Science lands on
        // a technology rather than in overflow.
        if self.is_arena() {
            self.arena_auto_research(pid);
        }
        self.process_trade_deals(pid);
        // Before the ordinary accumulation, so a turn's cargo is subject to
        // the same stockpile ceiling as a turn's mining rather than sitting
        // outside it.
        self.process_mass_drivers(pid);
        self.process_strategic_resources(pid);
        self.process_reactors(pid);
        self.process_diplomacy(pid);
        self.reconcile_closed_border_units(Some(pid));
        self.process_routes(pid);
        self.process_great_people(pid);
        // Districts and buildings score a competition for being *maintained*,
        // so they pay once a turn like the yields above.
        self.score_competition_holdings(pid);
        self.process_pressure(pid);
        self.process_loyalty(pid);
        self.record_emergency_presence(pid);
        self.process_influence(pid);
        self.check_city_state_quests(pid);
        self.irradiate_units(pid);
        let turn_unit_ids = self.player_unit_ids(pid);
        for uid in turn_unit_ids.iter().copied() {
            let (kind, hp, acted, attacks_left, air_patrol) = {
                let u = &self.units[&uid];
                (u.kind, u.hp, u.acted, u.attacks_left, u.air_patrol)
            };
            let start_owner = self.map.tiles[&self.units[&uid].pos]
                .owner_city
                .and_then(|city| self.cities.get(&city))
                .map(|city| city.owner);
            let territory_bonus = if start_owner == Some(pid) {
                self.policy_effect(pid, "friendly_start_movement")
            } else if start_owner.is_some_and(|owner| owner != pid) {
                self.policy_effect(pid, "enemy_start_movement")
            } else {
                0.0
            };
            let moves = self.unit_max_moves(uid) + territory_bonus;
            let attacks = self.unit_max_attacks(uid);
            let spec = &self.rules.units[kind];
            let embarked = self.is_embarked(&self.units[&uid]);
            let improvement_fortification = self.improvement_fortification_at(self.units[&uid].pos);
            let heal = if hp < 100
                && (!acted
                    || (attacks_left < attacks
                        && self.promotion_effect(&self.units[&uid], "heal_after_attack") > 0.0)
                    || (air_patrol
                        && self.promotion_effect(&self.units[&uid], "heal_on_patrol") > 0.0))
            {
                self.unit_heal_rate(uid)
            } else {
                0
            };
            let u = self.units.get_mut(&uid).unwrap();
            u.moves_left = moves;
            u.attacks_left = attacks;
            u.moved = false;
            u.zoc_stopped = false;
            u.hp = (u.hp + heal).min(100);
            if spec.class == "military"
                && spec.domain.as_deref() != Some("sea")
                && !embarked
                && (u.fortified || !acted)
            {
                u.fortify_turns = (u.fortify_turns + 1).min(2);
            } else if acted {
                u.fortify_turns = 0;
            }
            u.fortify_turns = u.fortify_turns.max(improvement_fortification);
            u.acted = false;
        }
        // Snapshot ZOC after every per-turn stop flag has been cleared. This
        // distinguishes beginning a turn in ZOC from entering one mid-turn.
        for uid in turn_unit_ids.iter().copied() {
            let started_in_zoc =
                !self.unit_ignores_zoc(uid) && self.in_enemy_zoc_for(uid, self.units[&uid].pos);
            self.units.get_mut(&uid).unwrap().started_turn_in_zoc = started_in_zoc;
        }
        let mut sci = 0.0;
        let mut cul = 0.0;
        let mut gold = 0.0;
        let mut faith = 0.0;
        // City processing already has the complete local state needed to
        // price infrastructure. Keep the derived per-city answers in a
        // stable ledger so upkeep is reduced once here rather than by a
        // second empire-wide district/building scan. A later wonder can add a
        // free building to a city that has already been processed; the
        // returned correction below keeps that observable order exact.
        let mut infrastructure_upkeep = BTreeMap::new();
        let turn_city_ids = self.player_city_ids(pid);
        for cid in turn_city_ids.iter().copied() {
            let (ys, city_upkeep, upkeep_updates) = self.process_city_with_upkeep(pid, cid);
            sci += ys.science;
            cul += ys.culture;
            gold += ys.gold;
            faith += ys.faith;
            infrastructure_upkeep.insert(cid, city_upkeep);
            for (changed_city, upkeep) in upkeep_updates {
                // A city not reached yet will derive its post-wonder answer
                // when its own turn runs. Only an earlier city needs a
                // correction in the reduction ledger.
                if infrastructure_upkeep.contains_key(&changed_city) {
                    infrastructure_upkeep.insert(changed_city, upkeep);
                }
            }
        }
        let founder_yields = self.founder_belief_yields(pid);
        sci += founder_yields.science;
        cul += founder_yields.culture;
        gold += founder_yields.gold;
        faith += founder_yields.faith;
        // Policy income paid to the player, not to a city — see
        // `player_policy_yields`.
        let policy_yields = self.player_policy_yields(pid);
        sci += policy_yields.science;
        cul += policy_yields.culture;
        gold += policy_yields.gold;
        faith += policy_yields.faith;
        let cultural_alliance_partner = self.alliance_partner(pid, "cultural", 3);
        if let Some(partner) = cultural_alliance_partner {
            cul += self
                .player_city_ids(partner)
                .into_iter()
                .map(|city| self.city_yields(city).culture)
                .sum::<f64>()
                * 0.10;
        }
        // ALLIANCE_SCIENCE_SHARING_FROM_ALLY is the Research tier's twin of the
        // Cultural one above, at the same Level 3 and the same 10 percent.
        if let Some(partner) = self.alliance_partner(pid, "research", 3) {
            sci += self
                .player_city_ids(partner)
                .into_iter()
                .map(|city| self.city_yields(city).science)
                .sum::<f64>()
                * 0.10;
        }
        // An arena is granted its economy rather than earning it. Both sides
        // receive the same flat Gold and the same Science pace whatever they
        // hold on the field, so a result is attributable to the fighting and
        // not to a better start — and a side with no city still techs and
        // still upgrades, which is what makes the no-city setting a real
        // option rather than a mute one. Culture is deliberately zero: with
        // no civics tree there is nothing for it to buy, and a policy landing
        // mid-battle would be exactly the asymmetry the mode excludes.
        if self.is_arena() {
            let granted = self.arena_side_yields(pid);
            sci = granted.science;
            gold = granted.gold;
            cul = 0.0;
            faith = 0.0;
        }
        // Anarchy: an empire between governments collects no Science,
        // Culture, Gold or Faith. Its upkeep still falls due below.
        let anarchy = self.in_anarchy(pid);
        if anarchy {
            sci = 0.0;
            cul = 0.0;
            gold = 0.0;
            faith = 0.0;
        }
        if !self.players[pid].is_minor {
            let (mut tourism, film_studio_tourism) = self.tourism_components_per_turn(pid);
            if let Some(partner) = cultural_alliance_partner {
                tourism += self.tourism_components_per_turn(partner).0 * 0.20;
            }
            let (religious_tourism, film_studio_religious) =
                self.religious_tourism_components_per_turn(pid);
            self.players[pid].culture_lifetime += cul;
            self.add_passive_tourism(
                pid,
                tourism,
                religious_tourism,
                film_studio_tourism,
                film_studio_religious,
            );
        }
        if !anarchy {
            gold += self.monopoly_bonuses(pid).0;
        }
        gold -= self.unit_gold_maintenance(pid);
        gold -= infrastructure_upkeep.values().copied().sum::<f64>();
        gold -= self.nuclear_gold_maintenance(pid);
        self.settle_gold_budget(pid, gold);
        self.players[pid].faith += faith;
        let research = self.players[pid].research.clone();
        let mut completed_tech = None;
        if let Some(tech) = research {
            let cost = self.tech_cost(tech.as_str());
            let p = &mut self.players[pid];
            p.research_progress += sci;
            if p.research_progress >= cost {
                let first = p.techs.insert(Name::new(&tech));
                p.research_overflow = p.research_progress - cost;
                p.research = None;
                p.research_progress = 0.0;
                completed_tech = Some((tech, first));
            }
        } else {
            self.players[pid].research_overflow += sci;
        }
        if let Some((node, first)) = completed_tech {
            self.note(
                pid,
                "Science",
                format!("researched {}", pretty(&node)),
                None,
            );
            self.apply_tree_completion(pid, true, &node, first);
            if first {
                self.share_team_technology_boost(pid, &node);
            }
            for city in turn_city_ids.iter().copied() {
                self.modernize_unit_queue(pid, city);
            }
            self.drop_obsolete_production(pid);
        }
        let civic = self.players[pid].civic.clone();
        let mut completed_civic = None;
        if let Some(cv) = civic {
            let cost = self.civic_cost(cv.as_str());
            let p = &mut self.players[pid];
            p.civic_progress += cul;
            if p.civic_progress >= cost {
                let first = p.civics.insert(Name::new(&cv));
                p.civic_overflow = p.civic_progress - cost;
                p.civic = None;
                p.civic_progress = 0.0;
                completed_civic = Some((cv, first));
            }
        } else {
            self.players[pid].civic_overflow += cul;
        }
        if let Some((node, first)) = completed_civic {
            self.note(pid, "Culture", format!("adopted {}", pretty(&node)), None);
            self.apply_tree_completion(pid, false, &node, first);
        }
    }

    /// Replace an obsolete queued unit with the newest consecutively
    /// available successor while preserving all invested Production. Any
    /// strategic material committed to the old order is refunded before the
    /// replacement's material is committed, so automatic modernization is
    /// atomic and never creates a stockpile deficit.
    pub(super) fn modernize_unit_queue(&mut self, pid: usize, cid: u32) {
        let mut seen = BTreeSet::new();
        loop {
            let Some(head) = self.cities[&cid].queue.first().cloned() else {
                return;
            };
            let (source, formation) = match &head {
                Item::Unit { unit } => (unit.as_str(), None),
                Item::Formation { unit, formation } => (unit.as_str(), Some(*formation)),
                _ => return,
            };
            if !seen.insert(source.to_string()) {
                return;
            }
            if !self.unit_is_obsolete(pid, Name::new(source)) {
                return;
            }
            let Some(target) = self.unit_upgrade_target(pid, Name::new(source)) else {
                return;
            };
            let target_item = formation.map_or_else(
                || Item::Unit {
                    unit: Name::new(&target),
                },
                |formation| Item::Formation {
                    unit: Name::new(&target),
                    formation,
                },
            );
            let old_key = Self::item_progress_key(&head);
            let target_key = Self::item_progress_key(&target_item);
            let old_commitment = self.cities[&cid]
                .strategic_resource_commitments
                .get(&old_key)
                .copied()
                .unwrap_or(0.0);
            let old_resource = self.rules.units[&source].requires_resource;
            let target_resource = self.rules.units[&target].requires_resource;
            let refundable_credit = if old_resource == target_resource {
                old_commitment
            } else {
                0.0
            };
            if !self.can_produce_unit(pid, cid, target, false, refundable_credit) {
                return;
            }
            let target_cost = self.unit_resource_cost(cid, &target_item);
            let target_already_committed = self.cities[&cid]
                .strategic_resource_commitments
                .get(&target_key)
                .is_some_and(|amount| *amount + f64::EPSILON >= target_cost);
            if let Some(resource) = target_resource.as_deref() {
                let effective_stock = self.strategic_stockpile(pid, Name::new(resource))
                    + if old_resource.as_deref() == Some(resource) {
                        old_commitment
                    } else {
                        0.0
                    };
                if !target_already_committed && effective_stock + f64::EPSILON < target_cost {
                    return;
                }
            }

            if old_commitment > 0.0 {
                self.cities
                    .get_mut(&cid)
                    .unwrap()
                    .strategic_resource_commitments
                    .remove(&old_key);
                if let Some(resource) = old_resource.as_deref() {
                    let stock = self.strategic_stockpile(pid, Name::new(resource));
                    self.players[pid]
                        .strategic_resources
                        .insert(Name::new(resource), stock + old_commitment);
                }
            }
            if target_cost > 0.0 && !target_already_committed {
                let resource = target_resource
                    .as_deref()
                    .expect("resource cost has a resource");
                let stock = self.strategic_stockpile(pid, Name::new(resource));
                self.players[pid]
                    .strategic_resources
                    .insert(Name::new(resource), stock - target_cost);
                self.cities
                    .get_mut(&cid)
                    .unwrap()
                    .strategic_resource_commitments
                    .insert(target_key.clone(), target_cost);
            }
            let resumed = self
                .cities
                .get_mut(&cid)
                .unwrap()
                .production_progress
                .remove(&target_key)
                .unwrap_or(0.0);
            let city = self.cities.get_mut(&cid).unwrap();
            city.queue[0] = target_item;
            city.production += resumed;
        }
    }

    // Kept as the compact test-facing API; the authoritative turn path uses
    // the richer result so it can reduce upkeep without a second scan.
    #[cfg(test)]
    pub(super) fn process_city(&mut self, pid: usize, cid: u32) -> Yields {
        self.process_city_with_upkeep(pid, cid).0
    }

    /// Advance one city and derive the infrastructure upkeep that belongs to
    /// its post-processing state. The third return value contains corrected
    /// answers for cities a wonder changed while this city completed.
    pub(super) fn process_city_with_upkeep(
        &mut self,
        pid: usize,
        cid: u32,
    ) -> (Yields, f64, Vec<(u32, f64)>) {
        let mut upkeep_updates = Vec::new();
        self.modernize_unit_queue(pid, cid);
        // Upgrade old saves lazily: legacy queued districts predate explicit
        // foundations, so establish and lock them before producing this turn.
        if let Some(Item::District { district, pos }) = self.cities[&cid].queue.first().cloned() {
            self.place_district_foundation(pid, cid, &district, pos);
        }
        let city = self.cities.get_mut(&cid).unwrap();
        city.struck = false;
        city.extra_strikes_used = 0;
        city.encampment_struck = false;
        city.encampment_extra_strikes_used = 0;
        for state in &mut city.defending_districts {
            state.struck = false;
            state.extra_strikes_used = 0;
        }
        // Yield assignment asks for housing and amenities as part of its
        // citizen strategy, then turn processing asks for both again to apply
        // growth. Keep those immutable answers in one memo scope before any
        // city state changes make them stale.
        let (ys, housing, am) = {
            let _memo = self.query_memo();
            (
                self.city_yields(cid),
                self.city_housing(&self.cities[&cid]),
                self.city_amenity_surplus(&self.cities[&cid]),
            )
        };
        let repair_project = matches!(
            self.cities[&cid].queue.first(),
            Some(Item::Project { project }) if project == "repair_outer_defenses"
        );
        let production_before = self.cities[&cid].production;
        let base_produced = ys.production;
        let production_multiplier = {
            let city = &self.cities[&cid];
            self.item_prod_mult(pid, cid, city.queue.first())
        };
        // A running district project's per-turn conversion is part of
        // `city_yields` now (it is a city yield the host lists in the city's
        // own ledger), so nothing is added here.
        let district_project_stalled = matches!(
            self.cities[&cid].queue.first(),
            Some(Item::Project { project })
                if !self.project_has_active_district(
                    &self.cities[&cid],
                    &self.rules.projects[project]
                )
        );
        let surplus = self.city_growth_surplus(pid, cid, ys.food, housing, am);
        let mut grew = false;
        let mut grew_to: Option<i64> = None;
        {
            let city = self.cities.get_mut(&cid).unwrap();
            city.food += surplus;
            let need = self.game_speed.scale(growth_threshold(city.pop));
            if city.food >= need {
                city.pop += 1;
                city.food -= need;
                grew = true;
                grew_to = Some(city.pop as i64);
            } else if city.food < 0.0 {
                city.pop = (city.pop - 1).max(1);
                city.food = 0.0;
            }
            let produced = base_produced * production_multiplier;
            if repair_project {
                let can_repair = {
                    let city = &self.cities[&cid];
                    let max = self.city_max_wall_hp(city);
                    max > 0
                        && city.wall_hp < max
                        && self.turn.saturating_sub(city.last_attacked) >= 3
                };
                if can_repair {
                    let max = self.city_max_wall_hp(&self.cities[&cid]);
                    let city = self.cities.get_mut(&cid).unwrap();
                    city.production += produced;
                    let repaired = (city.production.floor() as i32).min(max - city.wall_hp);
                    city.wall_hp += repaired;
                    city.production -= repaired as f64;
                    if city.wall_hp == max {
                        city.queue.remove(0);
                    }
                }
            } else if !district_project_stalled && !self.cities[&cid].queue.is_empty() {
                self.cities.get_mut(&cid).unwrap().production += produced;
            }
        }
        if grew {
            self.apply_growth_pressure(cid);
        }
        if let Some(pop) = grew_to {
            self.note_city_size_moment(pid, pop);
        }
        let queue_head = self.cities[&cid].queue.first().cloned();
        if let Some(item) = queue_head {
            if matches!(&item, Item::Project { project } if project == "repair_outer_defenses") {
                // Repair applies Production directly to wall HP above.
            } else {
                let cost = self.item_cost_for_city(pid, cid, &item);
                let stalled = matches!(&item, Item::Unit { unit } if unit == "settler")
                    && self.cities[&cid].pop < 2;
                if !stalled && self.cities[&cid].production >= cost {
                    let (completed, changed_upkeep) =
                        self.complete_item_and_collect_upkeep(pid, cid, &item);
                    upkeep_updates.extend(changed_upkeep);
                    if completed {
                        // Gathering Storm strips item-specific Production
                        // bonuses from overflow. Only unspent base Production
                        // and previously banked base progress carry forward.
                        let overflow = if production_before >= cost {
                            production_before - cost + base_produced
                        } else {
                            let remaining = cost - production_before;
                            base_produced - remaining / production_multiplier.max(f64::EPSILON)
                        }
                        .max(0.0);
                        let city = self.cities.get_mut(&cid).unwrap();
                        city.queue.remove(0);
                        city.production = overflow;
                        if let Some(next) = city.queue.first() {
                            let key = Self::item_progress_key(next);
                            city.production += city.production_progress.remove(&key).unwrap_or(0.0);
                        }
                    } else if self.cities[&cid].queue.first() == Some(&item) {
                        // A completed unit waiting for an open placement tile
                        // does not bank more Production every turn.
                        self.cities.get_mut(&cid).unwrap().production = cost;
                    }
                }
            }
        }
        if self.annexes_tiles_with_own_yields(pid)
            && !self.congress_effect_active("border_control_treaty", "B", &pid.to_string())
        {
            let owned = self.cities[&cid].owned_tiles.len() as i32;
            let border_mult = 1.0
                + (self.pantheon_effect(pid, "border_growth_pct")
                    + self.governor_effect(pid, cid, "border_growth_pct"))
                    / 100.0;
            let city = self.cities.get_mut(&cid).unwrap();
            // Borders grow on the city's Culture, and the next plot costs the
            // shipped curve: 10 + 6 * plots^1.3.
            city.border_culture += ys.culture * border_mult;
            let plots = (owned - 7).max(0) as f64;
            let need_b = (10.0 + 6.0 * plots.powf(1.3)).trunc();
            if city.border_culture >= need_b {
                city.border_culture -= need_b;
                self.expand_borders(cid);
            }
        }
        let besieged = self.city_under_siege(cid);
        let final_war_no_heal =
            self.plays_to_the_last_civ() && crate::lcs_knobs::get("noheal") > 0.0;
        let encampment =
            self.city_district_family_position(&self.cities[&cid], crate::name!("encampment"));
        let encampment_besieged =
            encampment.is_some_and(|position| self.district_under_siege(pid, position));
        let city = self.cities.get_mut(&cid).unwrap();
        if !besieged && !final_war_no_heal {
            city.hp = (city.hp + 20).min(200); // Civ 6 heal rate
        }
        if encampment.is_some() && !city.encampment_pillaged && !encampment_besieged {
            city.encampment_hp = (city.encampment_hp + 20).min(100);
        }
        let other_forts: Vec<_> = self.cities[&cid].defending_districts.clone();
        for mut state in other_forts {
            if !state.pillaged && !self.district_under_siege(pid, state.pos) {
                state.hp = (state.hp + 20).min(100);
                self.set_defending_district_state(cid, state);
            }
        }
        let city_upkeep = self.city_infrastructure_gold_maintenance(&self.cities[&cid]);
        (ys, city_upkeep, upkeep_updates)
    }
}
