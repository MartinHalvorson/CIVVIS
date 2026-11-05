//! The war log: one record per war, kept up to date as it is fought.
//!
//! Allocating a conflict id and repairing logs saved before the current
//! layout; opening a war record and its fronts, and closing it at peace or
//! elimination; keeping each war's sides and participants in step with the
//! diplomatic state (`sync_war_log`, `war_sides`); and the ledger entries a
//! war accumulates: unit participation and losses, city garrisons and city
//! losses, and war moments. `set_war_ledger` switches mid-turn ledger
//! syncing on or off.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

impl Game {
    pub(super) fn allocate_conflict_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Saves written before conflict ids and participant snapshots still load
    /// as readable two-party wars. Their historical military score cannot be
    /// recovered, so zero is retained as an explicit "not recorded" value.
    pub(super) fn repair_legacy_war_log(&mut self) {
        let active_keys: Vec<(usize, usize)> = self.wars.keys().copied().collect();
        for key in &active_keys {
            let legacy = self.wars[key].conflict == 0;
            if legacy {
                let conflict = self.allocate_conflict_id();
                let war = self.wars.get_mut(key).unwrap();
                war.conflict = conflict;
                war.declarer = war.aggressor;
                war.target = war.defender;
            }
        }
        for index in 0..self.concluded_wars.len() {
            if self.concluded_wars[index].conflict != 0 {
                continue;
            }
            let conflict = self.allocate_conflict_id();
            let war = &mut self.concluded_wars[index];
            war.conflict = conflict;
            war.declarer = war.aggressor;
            war.target = war.defender;
            if war.participants.is_empty() {
                let exited = war.ended;
                war.participants = vec![
                    WarParticipation {
                        player: war.aggressor,
                        declarer_side: true,
                        suzerain: None,
                        entered: war.started,
                        exited,
                        strength: 0,
                        peak_strength: None,
                        saw_action_units: BTreeMap::new(),
                    },
                    WarParticipation {
                        player: war.defender,
                        declarer_side: false,
                        suzerain: None,
                        entered: war.started,
                        exited,
                        strength: 0,
                        peak_strength: None,
                        saw_action_units: BTreeMap::new(),
                    },
                ];
            }
        }
        for key in active_keys {
            self.sync_war_participants_for(key);
        }
    }

    /// Open an otherwise unclassified front. Direct declarations and
    /// Emergency coalitions use `open_war_front` below so every related pair
    /// receives the same conflict id and named principals.
    pub(super) fn open_war_record(&mut self, aggressor: usize, defender: usize) {
        if self.wars.contains_key(&pair(aggressor, defender)) {
            return;
        }
        let conflict = self.allocate_conflict_id();
        self.open_war_front(
            aggressor,
            defender,
            WarDeclaration {
                conflict,
                declarer: aggressor,
                target: defender,
                declared_front: true,
                casus_belli: None,
                joint_war_until: None,
            },
        );
    }

    pub(super) fn open_war_front(
        &mut self,
        aggressor: usize,
        defender: usize,
        declaration: WarDeclaration,
    ) {
        if aggressor == defender
            || self.players[aggressor].is_barbarian
            || self.players[defender].is_barbarian
        {
            return;
        }
        let key = pair(aggressor, defender);
        if self.wars.contains_key(&key) {
            return;
        }
        // Before the first armies meet, the named target's capital is the
        // intelligible theater of the declaration.  Only the declared front
        // contributes this seed; alliance fronts will add their own real
        // action sites if and when fighting reaches them.
        let opening_site = if declaration.declared_front {
            self.cities
                .values()
                .find(|city| city.owner == declaration.target && self.city_capital_flag(city))
                .or_else(|| {
                    self.cities
                        .values()
                        .find(|city| city.owner == declaration.target)
                })
                .map(|city| city.pos)
                .or_else(|| {
                    self.units
                        .values()
                        .find(|unit| unit.owner == declaration.target)
                        .map(|unit| unit.pos)
                })
        } else {
            None
        };
        let mut record = WarRecord::new(aggressor, defender, self.turn, declaration);
        if let Some(pos) = opening_site {
            record.record_theater_site(self.turn, pos);
        }
        self.wars.insert(key, record);
        self.sync_war_participants_for(key);
    }

    /// Reconcile the actual civilizations fighting one front, including the
    /// city-states whose participation follows current suzerainty. Every
    /// entry/exit is retained as an interval so a later reload can still say
    /// exactly when a late ally entered or peaced out.
    fn sync_war_participants_for(&mut self, key: (usize, usize)) {
        let city_states = self
            .players
            .iter()
            .filter(|player| player.alive && player.is_minor && !player.is_barbarian)
            .filter_map(|player| {
                self.suzerain_of(player.id)
                    .map(|suzerain| (player.id, suzerain))
            })
            .collect::<Vec<_>>();
        self.sync_war_participants_from(key, &city_states);
    }

    fn sync_war_participants_from(&mut self, key: (usize, usize), city_states: &[(usize, usize)]) {
        let Some(front) = self.wars.get(&key) else {
            return;
        };
        let (aggressor, defender) = (front.aggressor, front.defender);
        let mut desired = vec![(aggressor, true, None), (defender, false, None)];
        for (principal, declarer_side) in [(aggressor, true), (defender, false)] {
            desired.extend(
                city_states
                    .iter()
                    .filter(|(_, suzerain)| *suzerain == principal)
                    .map(|(player, _)| (*player, declarer_side, Some(principal))),
            );
        }
        let desired_players: BTreeSet<usize> =
            desired.iter().map(|(player, _, _)| *player).collect();
        let active_strengths = self.wars[&key]
            .participants
            .iter()
            .filter(|entry| entry.exited.is_none())
            .map(|entry| {
                (
                    (entry.player, entry.entered),
                    self.military_power(entry.player).round() as i64,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let entering: Vec<(usize, bool, Option<usize>, i64)> = desired
            .into_iter()
            .filter(|(player, _, _)| {
                !self.wars[&key]
                    .participants
                    .iter()
                    .any(|entry| entry.player == *player && entry.exited.is_none())
            })
            .map(|(player, side, suzerain)| {
                (
                    player,
                    side,
                    suzerain,
                    self.military_power(player).round() as i64,
                )
            })
            .collect();
        let turn = self.turn;
        let war = self.wars.get_mut(&key).unwrap();
        for participant in war
            .participants
            .iter_mut()
            .filter(|entry| entry.exited.is_none())
        {
            let total = active_strengths[&(participant.player, participant.entered)];
            participant.peak_strength = Some(
                participant
                    .peak_strength
                    .unwrap_or(participant.strength)
                    .max(total),
            );
            if !desired_players.contains(&participant.player) {
                participant.exited = Some(turn);
            }
        }
        war.participants.extend(entering.into_iter().map(
            |(player, declarer_side, suzerain, strength)| WarParticipation {
                player,
                declarer_side,
                suzerain,
                entered: turn,
                exited: None,
                strength,
                peak_strength: Some(strength),
                saw_action_units: BTreeMap::new(),
            },
        ));
    }

    /// Count one unit once in the war where it saw action. If the same unit
    /// fights again after healing or upgrading, retain its greatest observed
    /// unit-only military value rather than turning activity into double count.
    pub(super) fn record_war_unit_participation(&mut self, unit: &Unit, enemy: usize) {
        let owner = unit.owner;
        let Some((first, second)) = self.war_sides(owner, enemy) else {
            return;
        };
        let value =
            (self.rules.units[unit.kind].strength * unit.hp.max(0) as f64 / 100.0).round() as i64;
        let current_strength = self.military_power(owner).round() as i64;
        let key = pair(first, second);
        let turn = self.turn;
        self.open_war_record(first, second);
        let Some(war) = self.wars.get_mut(&key) else {
            return;
        };
        war.record_theater_site(turn, unit.pos);
        if let Some(participant) = war
            .participants
            .iter_mut()
            .find(|participant| participant.player == owner && participant.exited.is_none())
        {
            participant.peak_strength = Some(
                participant
                    .peak_strength
                    .unwrap_or(participant.strength)
                    .max(current_strength),
            );
            participant
                .saw_action_units
                .entry(unit.id)
                .and_modify(|known| *known = (*known).max(value))
                .or_insert(value);
        }
    }

    /// A City Center's strongest military garrison is the unit that actually
    /// supplies its garrison defense. Count that unit, while still excluding
    /// the city's own defense value from every war-log military measure.
    pub(super) fn record_war_city_garrison_participation(&mut self, city: u32, enemy: usize) {
        let owner = self.cities[&city].owner;
        let position = self.cities[&city].pos;
        let garrison = self
            .unit_ids_at(position)
            .iter()
            .filter_map(|unit| {
                let candidate = &self.units[unit];
                (candidate.owner == owner && self.rules.units[candidate.kind].class == "military")
                    .then_some(candidate)
            })
            .max_by(|first, second| {
                self.unit_unembarked_strength(first)
                    .total_cmp(&self.unit_unembarked_strength(second))
            })
            .cloned();
        if let Some(garrison) = garrison {
            self.record_war_unit_participation(&garrison, enemy);
        }
    }

    /// Retire one declared war with either the state-derived result or a
    /// caller-supplied exceptional result. Emergency coalitions compel their
    /// members to stop fighting one another; that is neither negotiated peace
    /// nor conquest and must remain distinguishable in the chronicle.
    pub(super) fn close_war_record(
        &mut self,
        key: (usize, usize),
        outcome: Option<(&str, usize, usize)>,
    ) {
        let final_strengths = self
            .wars
            .get(&key)
            .map(|war| {
                war.participants
                    .iter()
                    .map(|participant| {
                        participant
                            .exited
                            .is_none()
                            .then(|| self.military_power(participant.player).round() as i64)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let Some(mut war) = self.wars.remove(&key) else {
            return;
        };
        war.ended = Some(self.turn);
        for (index, participant) in war
            .participants
            .iter_mut()
            .enumerate()
            .filter(|(_, participant)| participant.exited.is_none())
        {
            let total = final_strengths[index].unwrap();
            participant.peak_strength = Some(
                participant
                    .peak_strength
                    .unwrap_or(participant.strength)
                    .max(total),
            );
            participant.exited = Some(self.turn);
        }
        let (kind, actor, subject) = outcome.unwrap_or_else(|| {
            match (
                self.players[war.aggressor].alive,
                self.players[war.defender].alive,
            ) {
                (true, false) => ("conquest", war.aggressor, war.defender),
                (false, true) => ("conquest", war.defender, war.aggressor),
                _ => ("peace", war.aggressor, war.defender),
            }
        });
        war.highlights.push(WarHighlight {
            turn: self.turn,
            kind: kind.into(),
            actor,
            subject,
            city: None,
        });
        self.concluded_wars.push(war);
        const CONCLUDED_CONFLICTS_KEPT: usize = 24;
        let conflicts = self
            .concluded_wars
            .iter()
            .map(|war| war.conflict)
            .collect::<BTreeSet<_>>();
        if conflicts.len() > CONCLUDED_CONFLICTS_KEPT {
            let oldest = self.concluded_wars[0].conflict;
            self.concluded_wars.retain(|war| war.conflict != oldest);
        }
    }

    /// Reconcile the ledger with the diplomatic state: open a record for any
    /// war being fought without one, and retire the records whose war is over.
    ///
    /// One war is one record. The ledger follows the *declared* relations in
    /// `at_war` and not the derived ones: a Suzerain that goes to war brings
    /// every city-state it patronizes with it, and logging each of those as a
    /// war of its own turned a single declaration into a dozen entries that
    /// buried it. What those city-states lose is scored inside the war that
    /// dragged them in.
    pub fn sync_war_log(&mut self) {
        let mut active: BTreeSet<(usize, usize)> = BTreeSet::new();
        for (a, b) in self.at_war.clone() {
            // A war whose loser no longer exists is over however the relation
            // set reads: nobody signs a peace with a civilization that has
            // been wiped off the map, so without this the ledger would show
            // that war as ongoing forever.
            let live = |player: usize| {
                self.players
                    .get(player)
                    .is_some_and(|seat| seat.alive && !seat.is_barbarian)
            };
            if live(a) && live(b) {
                active.insert(pair(a, b));
            }
        }
        for (a, b) in &active {
            self.open_war_record(*a, *b);
        }
        let city_states = self
            .players
            .iter()
            .filter(|player| player.alive && player.is_minor && !player.is_barbarian)
            .filter_map(|player| {
                self.suzerain_of(player.id)
                    .map(|suzerain| (player.id, suzerain))
            })
            .collect::<Vec<_>>();
        for key in active.iter().copied() {
            self.sync_war_participants_from(key, &city_states);
        }
        let ended: Vec<(usize, usize)> = self
            .wars
            .keys()
            .filter(|key| !active.contains(*key))
            .copied()
            .collect();
        for key in ended {
            self.close_war_record(key, None);
        }
    }

    /// The war a blow between these two belongs to, as the pair of
    /// belligerents whose declaration is being fought out. A levied city-state
    /// answers for its Suzerain: what it loses is a cost of that war, not of a
    /// separate one nobody declared.
    pub(super) fn war_sides(&self, actor: usize, victim: usize) -> Option<(usize, usize)> {
        let principals = |player: usize| {
            let mut seats = vec![player];
            if self.players[player].is_minor {
                seats.extend(self.suzerain_of(player));
            }
            seats
        };
        principals(actor).into_iter().find_map(|first| {
            principals(victim)
                .into_iter()
                .find(|second| first != *second && self.at_war.contains(&pair(first, *second)))
                .map(|second| (first, second))
        })
    }

    /// A war record's mutable half, opened on demand: a unit can die on the
    /// first turn of a war the ledger has not reconciled yet. Returns the
    /// record with the two sides the blow scores against.
    fn war_ledger(
        &mut self,
        actor: usize,
        victim: usize,
    ) -> Option<(&mut WarRecord, usize, usize)> {
        if actor == victim || self.players[actor].is_barbarian || self.players[victim].is_barbarian
        {
            return None;
        }
        let (first, second) = self.war_sides(actor, victim)?;
        self.open_war_record(first, second);
        self.wars
            .get_mut(&pair(first, second))
            .map(|war| (war, first, second))
    }

    /// A military unit lost in a war it belongs to. Barbarian skirmishes are
    /// deliberately outside the ledger: that war never starts and never ends,
    /// so counting it would swamp the wars that were chosen.
    /// Record one unit lost, and charge its owner for it.
    /// `WAR_WEARINESS_PER_UNIT_KILLED` is 3 -- the largest per-event source
    /// after a WMD launch, and the reason a grinding war turns a people
    /// against it even when the fighting goes well. The weariness lands on the
    /// side that lost the unit, and rides the same policy and government
    /// reductions every other source does.
    pub(super) fn record_war_unit_loss(
        &mut self,
        killer: usize,
        victim_owner: usize,
        kind: &str,
        formation: u8,
        at: Pos,
    ) {
        let naval = self
            .rules
            .units
            .get(kind)
            .is_some_and(|spec| spec.domain.as_deref() == Some("sea"));
        let label = format!(
            "{}{}",
            kind,
            match (naval, formation) {
                (false, 1) => " corps",
                (false, 2) => " army",
                (true, 1) => " fleet",
                (true, 2) => " armada",
                _ => "",
            }
        );
        if self.at_war_with_any_civilization(victim_owner) {
            let home = self
                .map
                .get(at)
                .and_then(|tile| tile.owner_city)
                .and_then(|cid| self.cities.get(&cid))
                .is_some_and(|city| city.owner == victim_owner);
            let multiplier = self.war_weariness_multiplier(victim_owner, home);
            self.add_war_weariness(victim_owner, 3.0 * multiplier);
        }
        let turn = self.turn;
        if let Some((war, _, _)) = self.war_ledger(killer, victim_owner) {
            war.record_theater_site(turn, at);
            let losses = war.losses.entry(victim_owner).or_default();
            losses.units += 1;
            *losses.unit_kinds.entry(label).or_insert(0) += 1;
        }
    }

    /// A moment worth naming in the war it belongs to. Silently dropped when
    /// the two are not at war — a Barbarian raid or a strike in peacetime has
    /// no war to be part of.
    pub(super) fn record_war_moment(
        &mut self,
        actor: usize,
        subject: usize,
        kind: &str,
        city: Option<String>,
        at: Option<Pos>,
    ) {
        let turn = self.turn;
        let kind = kind.to_string();
        if let Some((war, _, _)) = self.war_ledger(actor, subject) {
            if let Some(pos) = at {
                war.record_theater_site(turn, pos);
            }
            war.highlights.push(WarHighlight {
                turn,
                kind,
                actor,
                subject,
                city,
            });
        }
    }

    /// A city changing hands under arms — the one event a war log should
    /// never summarize away. A capital is called out by name: losing one is
    /// the moment a war turns.
    pub(super) fn record_war_city_loss(
        &mut self,
        taker: usize,
        loser: usize,
        city: &str,
        capital: bool,
        pop: i32,
        at: Pos,
    ) {
        let turn = self.turn;
        if let Some((war, _, _)) = self.war_ledger(taker, loser) {
            let losses = war.losses.entry(loser).or_default();
            losses.cities += 1;
            losses.city_names.push(city.to_string());
            losses.city_losses.push(WarCityLoss {
                name: city.to_string(),
                turn,
                pop,
                capital,
            });
        }
        let kind = if capital {
            "capital_captured"
        } else {
            "city_captured"
        };
        self.record_war_moment(taker, loser, kind, Some(city.to_string()), Some(at));
    }

    /// Stop (or resume) re-syncing the narrated war ledger after every
    /// action. Declarations, peaces, and turn boundaries still sync it
    /// unconditionally, so the ledger every observer and report reads is
    /// identical at every turn boundary; only its mid-turn freshness — a
    /// spectator-infobox nicety — is given up. Outcomes are unchanged either
    /// way: nothing in the rules or the built-in agents reads the ledger.
    pub fn set_war_ledger(&mut self, track: bool) {
        self.track_war_ledger = track;
    }
}
