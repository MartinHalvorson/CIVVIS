//! Diplomacy actions: declaring war, making peace, denouncing, and deals.
//!
//! `start_war` and the declaration actions, with the casus belli a
//! declaration may use (`casus_belli_available`), the grievances a
//! city-state attack incurs, and the bilateral relations a war ends.
//! Peace (`do_make_peace`, `conclude_peace`) and denouncing. Deals: proposing,
//! accepting and rejecting, whether a deal is a gift, research-alliance
//! boosts, cancelling trade deals. And the turn's `process_diplomacy`, with
//! the carbon favor penalty.
//!
//! Moved verbatim out of the `impl Game` block in `game/actions.rs`
//! (2026-11-05). Methods the rest of the action layer calls are
//! `pub(super)`; methods the rest of `game` calls keep that reach as
//! `pub(in crate::game)`; public ones stay public. The code is otherwise
//! unchanged.

use super::*;

impl Game {
    pub(in crate::game) fn denounced_long_enough(&self, denouncer: usize, target: usize) -> bool {
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

    pub(in crate::game) fn alliance_points_key(partner: usize, kind: &str) -> String {
        format!("alliance_points:{partner}:{kind}")
    }

    pub(in crate::game) fn research_alliance_boost_candidate(
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

    pub(in crate::game) fn share_research_alliance_boosts(&mut self, first: usize, second: usize) {
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

    /// Declaring on a city-state has its own witnesses in Gathering Storm:
    /// the Suzerain receives 100 grievances, while every other major with an
    /// Envoy there receives 50. These are explicitly enumerated rather than
    /// relationship spillover, so a third party is charged once even when it
    /// is also friends with the Suzerain.
    pub(in crate::game) fn city_state_declaration_grievances(
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
        // Nothing settles the war a world plays out to its last civilization.
        if self.plays_to_the_last_civ() && self.major_diplomatic_counterpart(a, b) {
            return Some(u32::MAX);
        }
        let (first, second) = self.war_sides(a, b)?;
        let war = self.wars.get(&pair(first, second))?;
        let earliest = (war.started + self.standard_duration(WAR_MIN_TURNS))
            .max(war.joint_war_until.unwrap_or(0));
        (self.turn < earliest).then_some(earliest)
    }

    pub(in crate::game) fn player_tech_era(&self, pid: usize) -> usize {
        if let Some(era) = self
            .observed_public_empire_stats
            .get(&pid)
            .and_then(|stats| stats.tech_era)
        {
            return era;
        }
        self.players[pid]
            .techs
            .iter()
            .filter_map(|name| self.rules.techs.get(name).map(|spec| spec.era))
            .max()
            .unwrap_or(self.start_era)
            .max(self.start_era)
    }

    pub(in crate::game) fn government_tier(&self, pid: usize) -> Option<u8> {
        match self.players.get(pid)?.government.as_deref()? {
            "autocracy" | "oligarchy" | "classical_republic" => Some(1),
            "monarchy" | "merchant_republic" | "theocracy" => Some(2),
            "communism" | "democracy" | "fascism" => Some(3),
            "corporate_libertarianism" | "digital_democracy" | "synthetic_technocracy" => Some(4),
            _ => None,
        }
    }

    pub(in crate::game) fn territorial_war_available(&self, pid: usize, other: usize) -> bool {
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

    pub(in crate::game) fn casus_belli_available(
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
        // Surprise declarations and joint-war agreements have separate paths.
        if !matches!(profile.id, "surprise_war" | "joint_war") {
            if let Some(allowed) = self.host_war_type_permission(pid, other, profile.id) {
                return allowed;
            }
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

    pub(in crate::game) fn end_bilateral_relations_for_war(&mut self, first: usize, second: usize) {
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

    pub(in crate::game) fn start_war(
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

    pub(crate) fn host_blocks_war(&self, pid: usize, other: usize) -> bool {
        self.host_war_blocks.contains(&(pid, other, self.turn))
    }

    pub(crate) fn host_war_type_permission(
        &self,
        pid: usize,
        other: usize,
        casus_belli: &str,
    ) -> Option<bool> {
        let statement = native_war_statement(casus_belli)?;
        self.host_war_type_permissions
            .get(&(pid, other))
            .filter(|observed| observed.turn == self.turn)
            .map(|observed| {
                observed
                    .permissions
                    .get(statement)
                    .copied()
                    .unwrap_or(false)
            })
    }

    pub(in crate::game) fn do_declare_war(
        &mut self,
        pid: usize,
        other: usize,
    ) -> Result<(), String> {
        if self.host_blocks_war(pid, other)
            || self.host_war_type_permission(pid, other, "surprise_war") == Some(false)
        {
            return Err("host forbids declaring war this turn".into());
        }
        self.start_war(
            pid,
            other,
            casus_belli_profile("surprise_war").unwrap(),
            None,
        )
    }

    pub(in crate::game) fn do_declare_war_with_casus_belli(
        &mut self,
        pid: usize,
        other: usize,
        casus_belli: &str,
    ) -> Result<(), String> {
        if self.host_blocks_war(pid, other) {
            return Err("host forbids declaring war this turn".into());
        }
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

    pub(in crate::game) fn do_denounce(&mut self, pid: usize, other: usize) -> Result<(), String> {
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
    pub(in crate::game) fn do_propose_deal(
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
        // A proposal that only gives Gold is a gift (legal, and priced at
        // nothing here — see `validate_trade`); one that only asks is a demand
        // and goes through `Action::DemandGold`; an exchange goes through the trade
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

    pub(in crate::game) fn do_accept_deal(
        &mut self,
        pid: usize,
        deal_id: u32,
    ) -> Result<(), String> {
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
            self.score_aid_gold_gift(deal.from, deal.to, deal.give_gold);
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

    pub(in crate::game) fn do_reject_deal(
        &mut self,
        pid: usize,
        deal_id: u32,
    ) -> Result<(), String> {
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

    pub(in crate::game) fn break_promises_on_city_state_attack(
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

    /// A diplomatic deal that only hands over Gold: legal as a gift, worth
    /// nothing to the relationship, counted on the ledger.
    pub(in crate::game) fn diplomatic_deal_is_gift(deal: &DiplomaticDeal) -> bool {
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

    pub(in crate::game) fn carbon_favor_penalty(&self, pid: usize) -> f64 {
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

    pub(in crate::game) fn process_diplomacy(&mut self, pid: usize) {
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

    pub(in crate::game) fn cancel_trade_deals_with(&mut self, first: usize, second: usize) {
        self.active_trade_deals.retain(|deal| {
            !((deal.from == first && deal.to == second)
                || (deal.from == second && deal.to == first))
        });
        self.pending_deals.retain(|deal| {
            !((deal.from == first && deal.to == second)
                || (deal.from == second && deal.to == first))
        });
    }

    pub(in crate::game) fn do_make_peace(
        &mut self,
        pid: usize,
        other: usize,
    ) -> Result<(), String> {
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
    pub(in crate::game) fn conclude_peace(
        &mut self,
        first: usize,
        second: usize,
        mut terms: Vec<String>,
    ) {
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
