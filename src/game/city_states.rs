//! City-states: envoys, suzerainty, levies, and what a city-state grants.
//!
//! A city-state's type and the bonuses its envoys and suzerain earn
//! (`cs_type`, `cs_bonus`, `envoy_yields`, the per-type yields and the
//! next envoy tier, the district-production and Kilwa type bonuses, unique
//! suzerain bonuses); counting envoys and finding the suzerain
//! (`envoys_at`, `suzerain_of`); sending envoys and where an envoy is worth
//! sending (`can_send_envoy`, `do_send_envoy`, `envoy_investment_options`,
//! Amani's terms); and levying a city-state's military (`levy_cost`,
//! `do_levy_military`, `process_levies`).
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! and `pub(crate)` methods are unchanged; the code is otherwise unchanged.

use super::*;

impl Game {
    /// The shipped `MinorCivBonuses` type a city-state pays its 1/3/6 Envoy
    /// thresholds in. Every seat the roster names carries one; a name the
    /// roster does not know (a mod's own city-state) falls back to Trade,
    /// which is the only type whose bonus needs no matching building tier.
    pub fn cs_type<'a>(&'a self, civ: &str) -> &'a str {
        self.rules
            .city_states
            .roster
            .iter()
            .find(|seat| seat.name == civ)
            .map_or("trade", |seat| seat.kind.as_str())
    }

    /// The seven `ADJUST_DISTRICT_PRODUCTION` rows every city-state carries:
    /// all of them construct Harbors 500% faster, and each type constructs
    /// its specialty district 500% faster.  These are city-state-owned
    /// modifiers, not Envoy bonuses a major civilization receives.
    pub(super) fn city_state_district_production_pct(&self, pid: usize, family: &str) -> f64 {
        let Some(player) = self
            .players
            .get(pid)
            .filter(|player| player.is_minor && !player.is_barbarian)
        else {
            return 0.0;
        };
        if family == "harbor" {
            return 500.0;
        }
        let specialty = match self.cs_type(&player.civ) {
            "scientific" => "campus",
            "cultural" => "theater_square",
            "religious" => "holy_site",
            "militaristic" => "encampment",
            "industrial" => "industrial_zone",
            "trade" => "commercial_hub",
            _ => return 0.0,
        };
        if family == specialty {
            500.0
        } else {
            0.0
        }
    }

    /// The bespoke Suzerain bonus key a city-state carries, if the engine
    /// implements one. A declared-but-unimplemented bonus reads as `None` so
    /// no code path can act on a bonus that does not exist yet.
    pub fn cs_bonus<'a>(&'a self, civ: &str) -> Option<&'a str> {
        self.rules
            .city_states
            .roster
            .iter()
            .find(|seat| seat.name == civ)
            .filter(|seat| seat.implemented)
            .and_then(|seat| seat.bonus.as_deref())
    }

    fn envoy_tier_building_count(&self, city: &City, kind: &str, tier: i32) -> usize {
        if kind == "religious" && tier == 3 {
            return city
                .buildings
                .iter()
                .filter(|building| {
                    !city.pillaged_buildings.contains(*building)
                        && self.building_district_is_active(city, building)
                })
                .filter(|building| self.rules.buildings[building].worship_belief.is_some())
                .count();
        }
        let families: &[&str] = match (kind, tier) {
            ("scientific", 1) => &["library"],
            ("scientific", 2) => &["university"],
            ("scientific", 3) => &["research_lab"],
            ("cultural", 1) => &["amphitheater"],
            ("cultural", 2) => &["art_museum", "archaeological_museum"],
            ("cultural", 3) => &["broadcast_center"],
            ("religious", 1) => &["shrine"],
            ("religious", 2) => &["temple"],
            ("militaristic", 1) => &["barracks", "stable"],
            ("militaristic", 2) => &["armory"],
            ("militaristic", 3) => &["military_academy"],
            ("industrial", 1) => &["workshop"],
            ("industrial", 2) => &["factory"],
            ("industrial", 3) => &["coal_power_plant", "oil_power_plant", "nuclear_power_plant"],
            ("trade", 1) => &["market", "lighthouse"],
            ("trade", 2) => &["bank", "shipyard"],
            ("trade", 3) => &["stock_exchange", "seaport"],
            _ => &[],
        };
        families
            .iter()
            .filter(|family| self.city_has_active_building_family(city, Name::new(family)))
            .count()
    }

    fn envoy_production_applies(kind: &str, item: Option<&Item>) -> bool {
        match kind {
            "militaristic" => matches!(item, Some(Item::Unit { .. } | Item::Formation { .. })),
            "industrial" => matches!(
                item,
                Some(Item::Building { .. } | Item::District { .. } | Item::Wonder { .. })
            ),
            _ => true,
        }
    }

    fn suzerain_type_count(&self, pid: usize, kind: &str) -> usize {
        self.players
            .iter()
            .filter(|minor| {
                minor.alive
                    && minor.is_minor
                    && !minor.is_barbarian
                    && self.cs_type(&minor.civ) == kind
                    && self.suzerain_of(minor.id) == Some(pid)
            })
            .count()
    }

    /// Kilwa grants 15% in its own city for one Suzerain of a type and a
    /// further empire-wide 15% once the civilization controls two of that
    /// same type. Production types retain their unit versus infrastructure
    /// restrictions through the caller.
    pub(super) fn kilwa_type_bonus_pct(&self, pid: usize, city: &City, kind: &str) -> f64 {
        let count = self.suzerain_type_count(pid, kind);
        if count == 0 {
            return 0.0;
        }
        let local = if city.wonders.contains_key(&crate::name!("kilwa_kisiwani")) {
            self.rules.wonders["kilwa_kisiwani"]
                .effects
                .get("suzerain_type_city_bonus_pct")
                .copied()
                .unwrap_or(0.0)
        } else {
            0.0
        };
        let empire = if count >= 2 {
            self.empire_wonder_effect(pid, "suzerain_type_empire_bonus_pct")
        } else {
            0.0
        };
        local + empire
    }

    pub(super) fn raw_envoys_at(&self, pid: usize, minor: usize) -> i64 {
        self.players[pid]
            .envoys
            .iter()
            .find(|(m, _)| *m == minor)
            .map(|(_, n)| *n)
            .unwrap_or(0)
    }

    /// Effective city-state representation includes an established Amani.
    /// Messenger contributes two Envoys and Puppeteer doubles the complete
    /// effective delegation, while the stored envoy balance remains unchanged.
    pub fn envoys_at(&self, pid: usize, minor: usize) -> i64 {
        let raw = self.raw_envoys_at(pid, minor);
        let (messenger, multiplier) = self.amani_envoy_terms(pid, minor);
        if messenger == 0.0 && multiplier == 1.0 {
            return raw;
        }
        ((raw as f64 + messenger) * multiplier).round() as i64
    }

    /// What an established Amani adds to `pid`'s delegation at `minor`, as
    /// `(messenger, multiplier)` with `envoys_at = round((raw + messenger) *
    /// multiplier)`; `(0.0, 1.0)` when she is not established in one of that
    /// city-state's cities. Public to the crate so a mirrored board can store
    /// the host's count NET of these terms — the host's `GetTokensReceived`
    /// already includes her, and storing it raw counted her twice.
    pub(crate) fn amani_envoy_terms(&self, pid: usize, minor: usize) -> (f64, f64) {
        let none = (0.0, 1.0);
        if self.players[pid].governor_roster.is_empty() {
            return none;
        }
        let Some(city) = self.established_governor_city(pid, "amani") else {
            return none;
        };
        if self.cities.get(&city).map(|city| city.owner) != Some(minor) {
            return none;
        }
        let messenger = self.governor_effect(pid, city, "city_state_envoys");
        let multiplier = self
            .governor_effect(pid, city, "city_state_envoys_multiplier")
            .max(1.0);
        (messenger, multiplier)
    }

    /// Suzerain: at least 3 envoys and strictly more than every other major.
    pub fn suzerain_of(&self, minor: usize) -> Option<usize> {
        if let Some(memo) = self.query_memo.suzerain.borrow().as_ref() {
            if let Some(suzerain) = memo.get(&minor) {
                return *suzerain;
            }
        }
        let suzerain = self.suzerain_of_stable(minor);
        if let Some(memo) = self.query_memo.suzerain.borrow_mut().as_mut() {
            memo.insert(minor, suzerain);
        }
        suzerain
    }

    /// `suzerain_of_uncached` kept for as long as nothing it reads can have
    /// moved: envoys and governors live on players, an Amani's post on a city,
    /// and her establishment on the clock. The stamp belongs to this `Game`
    /// alone; a clone starts with an empty cache and its own counters.
    fn suzerain_of_stable(&self, minor: usize) -> Option<usize> {
        let stamp: SuzerainStamp = (self.players.epoch(), self.cities.generation(), self.turn);
        {
            let stable = self.query_memo.suzerain_stable.borrow();
            if let Some((held, answers)) = stable.as_ref() {
                if *held == stamp {
                    if let Some(suzerain) = answers.get(&minor) {
                        return *suzerain;
                    }
                }
            }
        }
        let suzerain = self.suzerain_of_uncached(minor);
        let mut stable = self.query_memo.suzerain_stable.borrow_mut();
        match stable.as_mut() {
            Some((held, answers)) if *held == stamp => {
                answers.insert(minor, suzerain);
            }
            _ => *stable = Some((stamp, BTreeMap::from([(minor, suzerain)]))),
        }
        suzerain
    }

    pub(super) fn suzerain_of_uncached(&self, minor: usize) -> Option<usize> {
        let mut best: Option<(i64, usize)> = None;
        let mut tied = false;
        for p in self.players.iter().filter(|p| !p.is_minor && p.alive) {
            let n = self.envoys_at(p.id, minor);
            match best {
                Some((bn, _)) if n == bn => tied = true,
                Some((bn, _)) if n > bn => {
                    best = Some((n, p.id));
                    tied = false;
                }
                None => {
                    best = Some((n, p.id));
                    tied = false;
                }
                _ => {}
            }
        }
        match best {
            Some((n, pid)) if n >= 3 && !tied => Some(pid),
            _ => None,
        }
    }

    /// A civilization receives a city-state's unique bonus either by being
    /// its direct Suzerain or through a Level 3 Economic Alliance with that
    /// direct Suzerain. Sharing is deliberately non-transitive: a partner's
    /// own Economic Alliance cannot relay the bonus through a third empire.
    pub(super) fn grants_city_state_unique_bonus(&self, pid: usize, city_state: &str) -> bool {
        if self.cs_bonus(city_state).is_none() {
            return false;
        }
        // The Economic Alliance walk is only needed by the second arm, and the
        // first arm -- we are the suzerain ourselves -- answers most calls. It
        // is a pure `find` over this player's alliances, so deferring it is the
        // same answer; computing it eagerly paid for it on every call that
        // never asked.
        let mut economic_partner: Option<Option<usize>> = None;
        for minor in self.players.iter().filter(|minor| {
            minor.alive && minor.is_minor && !minor.is_barbarian && minor.civ == city_state
        }) {
            if self.congress_effect_active("sovereignty", "B", self.cs_type(&minor.civ)) {
                continue;
            }
            let suzerain = self.suzerain_of(minor.id);
            if suzerain == Some(pid) {
                return true;
            }
            let partner =
                *economic_partner.get_or_insert_with(|| self.alliance_partner(pid, "economic", 3));
            if partner.is_some_and(|partner| suzerain == Some(partner)) {
                return true;
            }
        }
        false
    }

    /// Whether this civilization can currently send one of its Envoys to a
    /// city-state. Contact is part of legality, not merely presentation: an
    /// unmet city-state's identity and influence table are hidden information.
    pub(crate) fn can_send_envoy(&self, pid: usize, minor: usize) -> bool {
        self.players
            .get(pid)
            .is_some_and(|player| player.envoys_free > 0)
            && self
                .players
                .get(minor)
                .is_some_and(|player| player.is_minor && !player.is_barbarian && player.alive)
            && self.has_met(pid, minor)
            && self
                .host_envoy_permissions
                .get(&(pid, minor))
                .filter(|(observed, _)| *observed == self.turn)
                .map(|(_, allowed)| *allowed)
                .unwrap_or_else(|| !self.is_at_war(pid, minor))
    }

    pub(super) fn do_send_envoy(&mut self, pid: usize, minor: usize) -> Result<(), String> {
        if self
            .players
            .get(pid)
            .is_none_or(|player| player.envoys_free <= 0)
        {
            return Err("no envoys to send".into());
        }
        if !self.can_send_envoy(pid, minor) {
            return Err("invalid city-state".into());
        }
        let old_suzerain = self.suzerain_of_uncached(minor);
        let canceled_levy = old_suzerain.is_some_and(|owner| {
            owner != pid
                && self
                    .units
                    .values()
                    .any(|unit| unit.levied_from == Some(minor))
        });
        // Diplomatic League reacts to the first envoy actually sent; Amani's
        // virtual Envoys do not consume that one-time trigger.
        let existing = self.raw_envoys_at(pid, minor);
        let first_bonus = if existing == 0 {
            self.policy_effect(pid, "first_envoy_bonus") as i64
        } else {
            0
        };
        let different_government_bonus = self
            .suzerain_of_uncached(minor)
            .filter(|leader| *leader != pid)
            .filter(|leader| self.players[*leader].government != self.players[pid].government)
            .map(|_| self.policy_effect(pid, "different_government_envoy_bonus") as i64)
            .unwrap_or(0);
        // Tamar: `ADJUST_DUPLICATE_INFLUENCE_TOKEN_WHEN_SAME_RELIGION` 1. An
        // Envoy counts double at a city-state following your religion.
        let same_religion_bonus = self.players[pid]
            .religion
            .as_deref()
            .filter(|religion| {
                self.player_city_ids(minor)
                    .first()
                    .is_some_and(|city| self.city_religion(&self.cities[city]) == Some(*religion))
            })
            .map(|_| self.civ_effect(pid, "same_religion_envoy_bonus") as i64)
            .unwrap_or(0);
        let amount = 1 + first_bonus + different_government_bonus + same_religion_bonus;
        let papal_religion = self.players[pid].religion.clone();
        let papal_pressure = papal_religion
            .as_deref()
            .map(|religion| self.religion_belief_effect(religion, "envoy_religious_pressure"))
            .unwrap_or(0.0);
        let p = &mut self.players[pid];
        p.envoys_free -= 1;
        match p.envoys.iter_mut().find(|(m, _)| *m == minor) {
            Some(e) => e.1 += amount,
            None => p.envoys.push((minor, amount)),
        }
        if papal_pressure > 0.0 {
            if let Some(city) = self.player_city_ids(minor).first().copied() {
                if let Some(religion) = papal_religion {
                    *self
                        .cities
                        .get_mut(&city)
                        .unwrap()
                        .pressure
                        .entry(religion)
                        .or_insert(0.0) += papal_pressure;
                }
            }
        }
        // Actual final-patch behavior is narrower than the Civilopedia's
        // summary: the first manually sent Envoy and sends that merely tie the
        // lead do not grow borders. Policy, quest, Governor, and other free
        // Envoys also never add extra tiles. A later deliberate send grows one
        // tile when it leaves this civilization as the unique influence lead.
        let leads = self.envoys_at(pid, minor)
            > self
                .players
                .iter()
                .filter(|player| !player.is_minor && !player.is_barbarian && player.id != pid)
                .map(|player| self.envoys_at(player.id, minor))
                .max()
                .unwrap_or(0);
        if existing > 0 && leads {
            if let Some(city) = self.player_city_ids(minor).first().copied() {
                self.expand_borders(city);
            }
        }
        let new_suzerain = self.suzerain_of_uncached(minor);
        if new_suzerain == Some(pid) && old_suzerain != Some(pid) {
            self.first_historic_moment(
                pid,
                &format!("first_suzerain:{minor}"),
                None,
                Some("MOMENT_PLAYER_GAVE_ENVOY_BECAME_SUZERAIN_FIRST_IN_WORLD"),
            );
            if canceled_levy {
                self.add_historic_moment(pid, "MOMENT_PLAYER_GAVE_ENVOY_CANCELED_LEVY");
            }
            if old_suzerain.is_some_and(|owner| self.is_at_war(pid, owner)) {
                self.add_historic_moment(
                    pid,
                    "MOMENT_PLAYER_GAVE_ENVOY_CANCELED_SUZERAIN_DURING_WAR",
                );
            }
        }
        Ok(())
    }

    /// Gold required to control all currently available military units of a
    /// city-state for the standard 30-turn levy. Civ VI prices a levy at the
    /// units' combined Production costs; Foreign Ministry halves that price.
    pub fn levy_cost(&self, pid: usize, minor: usize) -> Option<f64> {
        if !self
            .players
            .get(minor)
            .is_some_and(|player| player.is_minor && !player.is_barbarian && player.alive)
            || self.suzerain_of(minor) != Some(pid)
            || self.is_at_war(pid, minor)
        {
            return None;
        }
        let units: Vec<&Unit> = self
            .units
            .values()
            .filter(|unit| unit.owner == minor && unit.levied_from.is_none())
            .filter(|unit| self.rules.units[unit.kind].class == "military")
            .collect();
        if units.is_empty() {
            return None;
        }
        // `LEVY_MILITARY_PERCENT_OF_UNIT_PURCHASE_COST` is 25 of a purchase
        // price of four times the unit's cost, which is that cost at this
        // game's speed.
        let base = units
            .iter()
            .map(|unit| self.item_cost_for(minor, &Item::Unit { unit: unit.kind }))
            .sum::<f64>();
        let discount = self.empire_building_sum(pid, |building| {
            building
                .effects
                .get("levy_discount_pct")
                .copied()
                .unwrap_or(0.0)
        }) + self.civ_effect(pid, "levy_discount_pct");
        Some(base * (1.0 - discount / 100.0).max(0.0))
    }

    pub(super) fn do_levy_military(&mut self, pid: usize, minor: usize) -> Result<(), String> {
        let cost = self
            .levy_cost(pid, minor)
            .ok_or_else(|| "city-state military cannot be levied".to_string())?;
        if self.players[pid].gold + f64::EPSILON < cost {
            return Err("not enough gold to levy this military".into());
        }
        let units: Vec<u32> = self
            .units
            .values()
            .filter(|unit| unit.owner == minor && unit.levied_from.is_none())
            .filter(|unit| self.rules.units[unit.kind].class == "military")
            .map(|unit| unit.id)
            .collect();
        self.players[pid].gold -= cost;
        let levied_until = self.turn + self.standard_duration(STANDARD_DEAL_TURNS);
        for unit_id in units {
            self.transfer_unit_owner(unit_id, pid);
            let unit = self.units.get_mut(&unit_id).unwrap();
            unit.levied_from = Some(minor);
            unit.levied_until = levied_until;
            unit.moves_left = 0.0;
            unit.acted = true;
        }
        let near_enemy = self.player_city_ids(minor).into_iter().any(|city_id| {
            let city = &self.cities[&city_id];
            self.cities.values().any(|enemy| {
                enemy.owner != pid
                    && self.players[enemy.owner].alive
                    && !self.players[enemy.owner].is_minor
                    && !self.players[enemy.owner].is_barbarian
                    && self.is_at_war(pid, enemy.owner)
                    && self.wdist(city.pos, enemy.pos) <= 6
            })
        });
        self.add_historic_moment(
            pid,
            if near_enemy {
                "MOMENT_PLAYER_LEVIED_MILITARY_NEAR_ENEMY_CITY"
            } else {
                "MOMENT_PLAYER_LEVIED_MILITARY"
            },
        );
        Ok(())
    }

    pub(super) fn process_levies(&mut self, pid: usize) {
        let expired: Vec<(u32, usize)> = self
            .units
            .values()
            .filter(|unit| unit.owner == pid)
            .filter_map(|unit| unit.levied_from.map(|minor| (unit, minor)))
            .filter(|(unit, minor)| {
                self.turn >= unit.levied_until
                    || self.suzerain_of(*minor) != Some(pid)
                    || self.is_at_war(pid, *minor)
            })
            .map(|(unit, minor)| (unit.id, minor))
            .collect();
        for (unit_id, minor) in expired {
            // A levy outlives the city-state that raised it: the unit belongs
            // to the Suzerain while it serves, so elimination -- which disbands
            // that civilization's army -- never sees it. Handing it back to a
            // player who no longer exists strands it on the map, because
            // nobody takes turns for the dead: it would hold its tile and its
            // zone of control for the rest of the game. The rest of the
            // city-state's army went with the city-state, and so does this.
            if !self.players[minor].alive {
                self.remove_unit(unit_id);
                continue;
            }
            self.transfer_unit_owner(unit_id, minor);
            let (class, current_pos) = {
                let unit = self.units.get_mut(&unit_id).unwrap();
                unit.levied_from = None;
                unit.levied_until = 0;
                unit.moves_left = 0.0;
                unit.acted = true;
                (self.rules.units[unit.kind].class.clone(), unit.pos)
            };
            let occupied = self.unit_ids_at(current_pos).iter().any(|other| {
                *other != unit_id
                    && self.units[other].owner == minor
                    && self.rules.units[self.units[other].kind].class == class
            });
            if occupied {
                // Look outward from where the operative actually stands before
                // falling back to the city-state's first city: a levy can
                // expire far from home, and searching only around that city
                // can miss every reachable tile near the unit.
                let origin = self
                    .player_city_ids(minor)
                    .first()
                    .map(|city| self.cities[city].pos)
                    .unwrap_or(current_pos);
                let mut candidates = self.wdisk(current_pos, 4);
                candidates.extend(self.wdisk(origin, 4));
                candidates.dedup();
                let free = candidates.into_iter().find(|position| {
                    self.unit_can_traverse(unit_id, *position)
                        && !self.unit_ids_at(*position).iter().any(|other| {
                            self.units[other].owner == minor
                                && self.rules.units[self.units[other].kind].class == class
                        })
                });
                match free {
                    Some(position) => self.relocate(unit_id, position),
                    // One unit of a class per tile is an invariant the rest of
                    // the engine relies on -- zone of control, combat and
                    // stacking all assume it. A returning levy with nowhere
                    // legal to stand used to simply stay where it was, leaving
                    // two of the same class on one tile for the rest of the
                    // match (seen in a finished game: two Coursers of Valletta
                    // sharing its own city centre). A unit that cannot be
                    // placed does not exist, which is what `place_new_unit`
                    // already decides for the same situation.
                    None => self.remove_unit(unit_id),
                }
            }
        }
    }

    /// Final-patch city-state type bonuses: the 1/3/6 thresholds grant
    /// +1/+2/+3 in the Capital or tier-1/2/3 matching buildings, with the
    /// latter two tiers also applying in the Consulate/Chancery. Trade
    /// city-states grant twice those Gold amounts. Militaristic and Industrial
    /// Production only applies to their respective queue categories.
    pub(super) fn envoy_type_yields_for_count(&self, city: &City, kind: &str, n: i64) -> Yields {
        if n <= 0 || !Self::envoy_production_applies(kind, city.queue.first()) {
            return Yields::default();
        }
        let scale = if kind == "trade" { 2.0 } else { 1.0 };
        let mut amount = 0.0;
        if n >= 1 {
            if self.city_has_palace(city) {
                amount += scale;
            }
            amount += scale * self.envoy_tier_building_count(city, kind, 1) as f64;
        }
        if n >= 3 {
            amount += 2.0 * scale * self.envoy_tier_building_count(city, kind, 2) as f64;
            if self.city_has_active_building_family(city, crate::name!("consulate")) {
                amount += 2.0 * scale;
            }
        }
        if n >= 6 {
            amount += 3.0 * scale * self.envoy_tier_building_count(city, kind, 3) as f64;
            if self.city_has_active_building_family(city, crate::name!("chancery")) {
                amount += 3.0 * scale;
            }
        }
        let mut yields = Yields::default();
        match kind {
            "scientific" => yields.science = amount,
            "cultural" => yields.culture = amount,
            "religious" => yields.faith = amount,
            "militaristic" | "industrial" => yields.production = amount,
            _ => yields.gold = amount,
        }
        yields
    }

    /// Return the number of additional Envoys and the empire-wide yield gain
    /// at the next type-bonus threshold. This gives strategic AI an exact
    /// marginal value for the civilization's active buildings and queues.
    pub(crate) fn next_envoy_type_bonus(&self, pid: usize, minor: usize) -> Option<(i64, Yields)> {
        let state = self
            .players
            .get(minor)
            .filter(|state| state.alive && state.is_minor && !state.is_barbarian)?;
        let current = self.envoys_at(pid, minor);
        let threshold = [1, 3, 6]
            .into_iter()
            .find(|threshold| *threshold > current)?;
        let kind = self.cs_type(&state.civ);
        let mut gain = Yields::default();
        for city in self.cities.values().filter(|city| city.owner == pid) {
            let before = self.envoy_type_yields_for_count(city, kind, current);
            let mut after = self.envoy_type_yields_for_count(city, kind, threshold);
            after.food -= before.food;
            after.production -= before.production;
            after.gold -= before.gold;
            after.science -= before.science;
            after.culture -= before.culture;
            after.faith -= before.faith;
            gain.add(after);
        }
        Some((threshold - current, gain))
    }

    /// Affordable envoy packages, priced with the same active buildings as
    /// turn income. Each row is (placements, effective delegation, yield gain).
    /// Stop at the last paying tier or a one-envoy suzerainty defense margin.
    pub(crate) fn envoy_investment_options(
        &self,
        pid: usize,
        minor: usize,
    ) -> Vec<(i64, i64, Yields)> {
        if !self.can_send_envoy(pid, minor) {
            return Vec::new();
        }
        let current = self.envoys_at(pid, minor);
        let rival = self
            .players
            .iter()
            .filter(|p| !p.is_minor && !p.is_barbarian && p.id != pid)
            .map(|p| self.envoys_at(p.id, minor))
            .max()
            .unwrap_or(0);
        let capture = 3.max(rival + 1);
        let defend = self.suzerain_of(minor) == Some(pid) && current == rival + 1;
        let target = 6.max(if defend { current + 1 } else { capture });
        let kind = self.cs_type(&self.players[minor].civ);
        let (messenger, multiplier) = self.amani_envoy_terms(pid, minor);
        let foreign_government = self
            .suzerain_of(minor)
            .filter(|leader| *leader != pid)
            .is_some_and(|leader| self.players[leader].government != self.players[pid].government);
        let mut raw = self.raw_envoys_at(pid, minor);
        let mut previous = current;
        let mut options = Vec::new();
        for spent in 1..=self.players[pid].envoys_free.min((target - current).max(0)) {
            // Match do_send_envoy: the first *raw* placement gets League;
            // Containment stops when we tie or displace the foreign Suzerain.
            let first = if raw == 0 {
                self.policy_effect(pid, "first_envoy_bonus") as i64
            } else {
                0
            };
            let foreign = if foreign_government && previous < rival {
                self.policy_effect(pid, "different_government_envoy_bonus") as i64
            } else {
                0
            };
            raw += 1 + first + foreign;
            let count = ((raw as f64 + messenger) * multiplier).round() as i64;
            let pays = [1, 3, 6]
                .into_iter()
                .any(|tier| previous < tier && count >= tier);
            let captures = previous < capture && count >= capture;
            if pays || captures || (defend && spent == 1) {
                let mut gain = Yields::default();
                for city in self.cities.values().filter(|city| city.owner == pid) {
                    let before = self.envoy_type_yields_for_count(city, kind, current);
                    let mut after = self.envoy_type_yields_for_count(city, kind, count);
                    after.food -= before.food;
                    after.production -= before.production;
                    after.gold -= before.gold;
                    after.science -= before.science;
                    after.culture -= before.culture;
                    after.faith -= before.faith;
                    gain.add(after);
                }
                options.push((spent, count, gain));
            }
            previous = count;
            if count >= target {
                break;
            }
        }
        options
    }

    pub(super) fn envoy_yields(&self, pid: usize, city: &City) -> Yields {
        let mut yields = Yields::default();
        // A city-state at war with the seat suspends its type bonuses for the
        // war's length: Ostia's "+2 from Consulate" Culture (Caguana, three
        // Envoys) went to nothing on the turn Caguana's new Suzerain brought it
        // into a war against us (run civvis-20260816T223457Z t90) and came
        // back the turn peace was made (t98); Rome's capital point from the
        // same city-state went and returned with it.
        for state in self
            .players
            .iter()
            .filter(|state| state.is_minor && !state.is_barbarian && state.alive)
            .filter(|state| !self.is_at_war(pid, state.id))
        {
            yields.add(self.envoy_type_yields_for_count(
                city,
                self.cs_type(&state.civ),
                self.envoys_at(pid, state.id),
            ));
        }
        yields
    }
}
