//! Modifiers: government and policy effects, and modifiers attached to a
//! player or to a player type.
//!
//! Reading effects: the government's (`gov_effects`), a slotted policy's
//! (`has_policy`, `policy_effect`, `policy_effect_for_unit`), a city's and a
//! building's (`city_modifier_effect`, `building_modifier_yields`), the
//! purchase discount and the abilities a modifier grants a unit. Modifiers
//! attached at runtime: to one player or to every player of a type
//! (`attach_modifier_to_player*`, `detach_modifier_from_player*`), the
//! context and player-type matching they read through, and the
//! policy-read trace (`trace_policy_reads`) that records which effects a
//! caller consulted.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! and `pub(crate)` methods are unchanged; the code is otherwise unchanged.

use super::*;

impl Game {
    pub fn gov_effects(&self, pid: usize) -> crate::rules::GovEffects {
        match &self.players[pid].government {
            Some(g) => self
                .rules
                .governments
                .get(g)
                .map(|s| s.effects)
                .unwrap_or_default(),
            None => Default::default(),
        }
    }

    pub fn has_policy(&self, pid: usize, name: &str) -> bool {
        let card = Name::new(name);
        self.note_policy_card_read(card);
        !self.in_anarchy(pid) && self.players[pid].policies.contains(&card)
    }

    /// Record every question the next read-only sweep asks `pid`'s policy
    /// deck, so a caller that runs the same sweep once per candidate card can
    /// tell which candidates it already holds the answer for. See
    /// [`PolicyReadSet`].
    ///
    /// One trace covers one sweep; opening a second while the first is alive
    /// restarts the recording rather than nesting.
    pub fn trace_policy_reads(&self, pid: usize) -> PolicyReadTrace<'_> {
        *self.query_memo.policy_reads.borrow_mut() = PolicyReadSet {
            // A runtime attachment is matched against the deck by card name,
            // not by effect key, so a seat carrying one is not classifiable.
            opaque: !self.has_no_attachments(pid),
            ..PolicyReadSet::default()
        };
        self.query_memo.policy_reads_on.set(true);
        PolicyReadTrace { game: self }
    }

    /// A traced sweep must preserve its questions as well as its answer.
    #[inline]
    pub(crate) fn is_tracing_policy_reads(&self) -> bool {
        self.query_memo.policy_reads_on.get()
    }

    #[inline]
    fn note_policy_effect_read(&self, effect: &str) {
        if !self.query_memo.policy_reads_on.get() {
            return;
        }
        // `insert` would build the `String` before discovering it is already
        // there, and a sweep asks the same few dozen keys once per city.
        let mut seen = self.query_memo.policy_reads.borrow_mut();
        if !seen.effects.contains(effect) {
            seen.effects.insert(effect.to_string());
        }
    }

    #[inline]
    fn note_policy_card_read(&self, card: Name) {
        if self.query_memo.policy_reads_on.get() {
            self.query_memo.policy_reads.borrow_mut().cards.insert(card);
        }
    }

    /// Sum a reusable modifier bundle attached directly to one player.
    /// Whether a seat carries no runtime modifier bundle.
    ///
    /// [`crate::rules::EffectIndex`] speaks for the ruleset, and the ruleset
    /// is fixed once a game begins — but `Rules::modifiers` is swapped in
    /// wholesale when a resolution installs a bundle, so an indexed answer
    /// about it could be stale. Every collection path that consults the index
    /// pairs it with this, and a seat carrying an attachment simply takes the
    /// unoptimized route. The overwhelming majority of seats carry none.
    #[inline]
    fn has_no_attachments(&self, pid: usize) -> bool {
        self.players
            .get(pid)
            .is_none_or(|player| player.attached_modifiers.is_empty())
    }

    fn modifier_player_type(player: &Player) -> &'static str {
        if player.is_free_city {
            "free_cities"
        } else if player.is_barbarian {
            "barbarian"
        } else if player.is_minor {
            "minor"
        } else {
            "major"
        }
    }

    pub(super) fn modifier_context(&self, pid: usize) -> ModifierContext<'_> {
        let player = &self.players[pid];
        ModifierContext {
            player_type: Some(Self::modifier_player_type(player)),
            civilization: Some(player.civ.as_str()),
            government: player.government.as_deref(),
            religion: player.religion.as_deref(),
            pantheon: player.pantheon.as_deref(),
            secret_society: player.secret_society.as_deref(),
            age: Some(player.age.as_str()),
            policies: Some(&player.policies),
            technologies: Some(&player.techs),
            civics: Some(&player.civics),
        }
    }

    pub fn player_modifier_effect(&self, pid: usize, effect: &str) -> f64 {
        self.player_modifier_effect_in_collections(pid, effect, &[ModifierCollection::Player])
    }

    fn player_modifier_effect_in_collections(
        &self,
        pid: usize,
        effect: &str,
        collections: &[ModifierCollection],
    ) -> f64 {
        let context = self.modifier_context(pid);
        self.players
            .get(pid)
            .into_iter()
            .flat_map(|player| player.attached_modifiers.iter())
            .filter_map(|modifier| self.rules.modifiers.get(modifier))
            .filter(|modifier| collections.contains(&modifier.collection))
            .filter(|modifier| modifier.requirements.matches(&context))
            .filter_map(|modifier| modifier.effects.get(effect))
            .sum()
    }

    /// Attach one named modifier to one player. Repeating the same attachment
    /// is idempotent, matching Civ VI's stable modifier identity.
    pub fn attach_modifier_to_player(
        &mut self,
        pid: usize,
        modifier: &str,
    ) -> Result<bool, String> {
        if !self.rules.modifiers.contains_key(modifier) {
            return Err(format!("unknown modifier {modifier}"));
        }
        let player = self
            .players
            .get_mut(pid)
            .ok_or_else(|| format!("unknown player {pid}"))?;
        Ok(player.attached_modifiers.insert(modifier.to_string()))
    }

    /// Remove one named runtime attachment from one player.
    pub fn detach_modifier_from_player(&mut self, pid: usize, modifier: &str) -> bool {
        self.players
            .get_mut(pid)
            .is_some_and(|player| player.attached_modifiers.remove(modifier))
    }

    /// Sum a numeric primitive across all currently slotted policy cards and
    /// runtime player attachments. Policy rules remain data-driven while
    /// callers provide the game context (unit class, district family, city
    /// state, and so on). Anarchy suppresses cards but not external modifiers.
    pub fn policy_effect(&self, pid: usize, effect: &str) -> f64 {
        // A runtime attachment can carry any key at all, and the table it
        // comes from is swapped in after the ruleset is indexed, so the
        // index only settles this for a seat carrying no attachments.
        let granted = self.rules.effect_index.policies(effect);
        if granted {
            // A key no card in the ruleset grants cannot be a card's key, so
            // a trace loses nothing by recording only the ones that are.
            self.note_policy_effect_read(effect);
        }
        if !granted && self.has_no_attachments(pid) {
            return 0.0;
        }
        // The generic empire/city path collects player-wide and city-wide
        // bundles together. Unit-wide bundles are collected by
        // `policy_effect_for_unit`, where the subject is known.
        let attached = self.player_modifier_effect_in_collections(
            pid,
            effect,
            &[ModifierCollection::Player, ModifierCollection::PlayerCities],
        );
        if self.in_anarchy(pid) {
            return attached; // the cards come out; external modifiers remain
        }
        attached
            + self.players[pid]
                .policies
                .iter()
                .filter_map(|name| self.rules.policies.get_interned(*name)?.effects.get(effect))
                .sum::<f64>()
    }

    /// A numeric modifier whose subject is this city. This is the common
    /// collection path for typed selector effects: runtime attachments,
    /// policies, researched nodes and civilization traits are empire-wide;
    /// infrastructure, beliefs and Governors contribute only where active.
    pub(super) fn city_modifier_effect(&self, city: &City, effect: &str) -> f64 {
        // Nine sweeps follow, over the player's policies, trees, traits,
        // buildings, districts, wonders, beliefs, pantheon and Governors.
        // Every one of them ends in `spec.effects.get(effect)`, so if nothing
        // in the ruleset grants this key they all add up to the same nothing
        // — unless the owner carries a runtime attachment, which the index
        // cannot speak for.
        if !self.rules.effect_index.any(effect) && self.has_no_attachments(city.owner) {
            return 0.0;
        }
        let pid = city.owner;
        self.policy_effect(pid, effect)
            + self.tree_effect(pid, effect)
            + self.civ_effect(pid, effect)
            + self.city_building_effect(city, effect)
            + self.city_district_effect(city, effect)
            + self.empire_wonder_effect(pid, effect)
            + self.city_religion_belief_effect(city, effect)
            + self.pantheon_effect(pid, effect)
            + self.governor_effect(pid, city.id, effect)
    }

    pub(super) fn building_modifier_yields(&self, city: &City, building: &str) -> Yields {
        let mut selectors = vec![building, "*"];
        if let Some(replaced) = self.rules.buildings[building].replaces {
            selectors.push(replaced.as_str());
        }
        // Each surviving selector costs six assembled keys and six collection
        // sweeps. A selector no modifier anywhere names contributes zero to
        // all six, so it is dropped before a key is built for it — but only
        // where the index speaks for every source, which excludes a seat
        // carrying runtime attachments.
        if self.has_no_attachments(city.owner) {
            selectors.retain(|selector| self.rules.effect_index.modifies_building_yields(selector));
            if selectors.is_empty() {
                return Yields::default();
            }
        }
        let effect = |yield_type: &str| {
            selectors
                .iter()
                .map(|selector| {
                    self.city_modifier_effect(
                        city,
                        &building_yield_effect_key(selector, yield_type),
                    )
                })
                .sum()
        };
        Yields {
            food: effect("food"),
            production: effect("production"),
            gold: effect("gold"),
            science: effect("science"),
            culture: effect("culture"),
            faith: effect("faith"),
        }
    }

    pub(super) fn unit_purchase_modifier_discount(&self, city: &City, unit: &str) -> f64 {
        let mut selectors = vec![unit, "*"];
        if let Some(replaced) = self.rules.units[unit].replaces {
            selectors.push(replaced.as_str());
        }
        if self.has_no_attachments(city.owner) {
            selectors.retain(|selector| self.rules.effect_index.discounts_unit_purchase(selector));
        }
        selectors
            .into_iter()
            .map(|selector| {
                self.city_modifier_effect(city, &unit_purchase_discount_effect_key(selector))
            })
            .sum()
    }

    pub(super) fn modifier_grants_ability(&self, pid: usize, ability: &str) -> bool {
        // Assembling the key allocates, and this is asked behind every
        // `has_ability` call, so rule the question out on the borrowed name.
        if !self.rules.effect_index.grants_ability(ability) && self.has_no_attachments(pid) {
            return false;
        }
        let effect = grant_ability_effect_key(ability);
        let religion = self
            .players
            .get(pid)
            .and_then(|player| player.religion.as_deref());
        self.player_modifier_effect(pid, &effect)
            + self.tree_effect(pid, &effect)
            + self.civ_effect(pid, &effect)
            + self.empire_wonder_effect(pid, &effect)
            + self.empire_building_sum(pid, |building| {
                building.effects.get(&effect).copied().unwrap_or(0.0)
            })
            + religion.map_or(0.0, |religion| {
                self.religion_belief_effect(religion, &effect)
            })
            > 0.0
    }

    /// Sums a policy effect over the cards whose era window admits `era`.
    pub(super) fn policy_effect_for_unit(&self, pid: usize, effect: &str, unit: &str) -> f64 {
        self.note_policy_effect_read(effect);
        let attached = self.player_modifier_effect_in_collections(
            pid,
            effect,
            &[ModifierCollection::Player, ModifierCollection::PlayerUnits],
        );
        if self.in_anarchy(pid) {
            return attached;
        }
        let era = self.unit_era(unit);
        let class = self
            .rules
            .units
            .get(unit)
            .map_or("", |unit| unit.promotion_class.as_str());
        attached
            + self.players[pid]
                .policies
                .iter()
                .filter_map(|name| self.rules.policies.get_interned(*name))
                .filter(|spec| spec.unit_eras.is_empty() || spec.unit_eras.contains(&era))
                .filter(|spec| {
                    !spec
                        .unit_era_gaps
                        .get(class)
                        .is_some_and(|eras| eras.contains(&era))
                })
                .filter_map(|spec| spec.effects.get(effect))
                .sum::<f64>()
    }
}
