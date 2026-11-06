//! `military-alliance-shield`: hold a Military Alliance beside the lane's own
//! alliance — with the empire already fighting our enemy, or with the
//! neighbour whose war on us we most need to prevent.
//!
//! What the alliance gives on the live ruleset (Gathering Storm):
//!
//! - **+5 combat strength** for our units against the units of a player at
//!   war with both us and the ally, from level 1: `ALLIANCE_ADJUST_COMBAT_STRENGTH`
//!   (Expansion1_Alliances.xml:146), `MODIFIER_ALLIANCE_COMBATS_UNIT_STRENGTHS`
//!   with `ALLIES_AT_WAR_WITH_TARGET_REQUIREMENTS` (:186-188), Amount 5
//!   (:299-301); "+5 Combat Strength against units of players at war with you
//!   and your ally" (`LOC_ALLIANCE_LV1_MILITARY_EFFECT_1`). From level 2,
//!   +15% Production toward military units while either is at war (:159)
//!   and shared visibility (:158); at level 3 units are trained promoted
//!   (:163). The engine models all of them (`Game::vs_bonus`,
//!   `Game::item_prod_mult`, the shared-vision viewer set).
//! - **No war between the pair** while it stands — "Civilizations in an
//!   Alliance cannot declare war on each other" — **and a Defensive Pact**:
//!   "If two leaders are in an Alliance, they will automatically have a
//!   Defensive Pact between them", and "If either member of the Defensive
//!   Pact is targeted, the other member will also declare war against the
//!   aggressor" (Expansion1_Civilopedia_Text.xml,
//!   `LOC_PEDIA_CONCEPTS_PAGE_DIPLO_12_CHAPTER_CONTENT_PARA_2` and
//!   `..._DIPLO_11_CHAPTER_CONTENT_PARA_1`). So whoever declares on us is at
//!   war with the ally too, and the +5 holds against them. ⚠ The engine
//!   models the pact as a separate deal item only (`Game::start_war`,
//!   `defensive_pact_until`), so a native screen under-reads this half of the
//!   gene; the live host plays it.
//! - Any alliance needs declared friends and Civil Service on both sides
//!   (Expansion1_Alliances.xml:33; "This relationship state is required to
//!   make an Alliance", DIPLO_12 above), one alliance per pair.
//!
//! ★★ NO DESK ASKS FOR THE SHIELD. `propose_strategic_alliance` asks for one
//! alliance, of the kind the grand strategy names (Research on a Science
//! seat, Military only on a Conquest or Recovery plan), on a twelve-turn
//! cadence, and ranks partners by friendship, trade routes and complement —
//! never by who threatens us or who is fighting our enemy. The coalition
//! asks only before a war of our own choosing. And the live host drops every
//! alliance order off the Diplomatic lane unless a seat gene keeps its lane's
//! own kind (`drop_off_lane_alliances` in `civvis_orders`), so a Domination or
//! Science seat has never sent `ALLIANCE_MILITARY`. Meanwhile wars come to
//! us: of 24 live Immortal seats of 2026-10-31, 9 friends whose friendship
//! lapsed later declared war on us (`friendship-keeps-the-peace`).
//!
//! What the gene does, all of it inert while the flag is off. Once Civil
//! Service is ours and no Military Alliance of ours stands, one partner is
//! asked for a Military Alliance, friendship bundled as every desk does,
//! ahead of the stock ask:
//!
//! 1. **The common enemy.** While we are at war with a major, a partner at
//!    war with the same major scores [`SHIELD_COMMON_ENEMY`] per shared
//!    enemy: the +5 holds from the turn the alliance is signed. Every turn
//!    of the war, on every lane. On the live seat the rival's war is the
//!    host's own report (`Game::rivals_at_war`, #4452).
//! 2. **The border.** Off the Domination lane, and once the lane's own
//!    alliance is not waiting to be won ([`AdvancedAi::shield_lane_alliance_waits`]),
//!    a major with a city within
//!    [`SHIELD_NEIGHBOUR_REACH`] of one of ours that fields at least
//!    [`SHIELD_THREAT_FLOOR`] of our power scores [`SHIELD_THREAT_WEIGHT`]
//!    per unit of that ratio (capped at [`SHIELD_THREAT_RATIO_CAP`]) plus
//!    [`SHIELD_THREAT_PER_TILE`] per tile nearer than the reach. Allied, it
//!    cannot declare on us, and its pact stands beside us against anyone
//!    else who does. Every [`SHIELD_CADENCE`] turns, staggered by seat.
//!
//! ★ The lane's own alliance comes first. Any alliance with a neighbour
//! forbids its war on us and carries the Defensive Pact, and a pair holds one
//! alliance, so a Military ask to the very partner the stock desk courts for
//! the lane's kind trades that kind's yield for a +5 that pays only in a
//! shared war. On the live Deity Korea Science board (civvis-20261104T213449Z)
//! a first cut did exactly that: every border turn t108-t141 asked player 3
//! for a Military Alliance where the stock desk asked it for the Research
//! Alliance. So while the lane's kind (Research on a Science seat, Cultural on
//! Culture, Religious on Religion, Economic on Expansion and Diplomacy) is
//! unheld, legal on our tree, and its ask reaches the host, the border arm
//! waits; the Military Alliance then goes to a second neighbour. The common
//! enemy does not wait: a war is being fought.
//!
//! A declared friend scores [`SHIELD_FRIEND`] more: the live host asks it the
//! alliance at once, while anyone else is asked the friendship first. Every
//! partner filter of the stock ask applies (the plan's target, the
//! Domination prey and counter target, the culture and science racers, the
//! seat-ally bars, the refusal memory, race pressure), plus
//! [`SHIELD_GRIEVANCE_CEILING`] and no denunciation either way. A partner
//! neither fighting our enemy nor at our border is never asked. On the
//! Domination lane the border arm is off: an ally there is a capital
//! deferred for the alliance's term.
//!
//! On the live seat the order crosses as `ALLIANCE_MILITARY`
//! (`civ6_alliance_type`). `drop_off_lane_alliances` keeps the gene's own ask
//! ([`AdvancedAi::military_alliance_shield_ask`]) on every lane, and the
//! agent's `CivvisAllianceStep` takes it under `MilitaryAllies`, which
//! civ6_play.py sets when the tag is armed.
//!
//! **Version two (`military-alliance-shield-2`, 2026-11-05).** The first armed
//! live Deity Korea games read two faults. Three Military asks to declared
//! friends were refused (152431Z t96 Byzantium, 154934Z t108 Poland, 155735Z
//! t108), while Research and Economic asks are often accepted. And asks to
//! rivals the host read as Neutral or Unfriendly (154934Z t87, 155735Z t102
//! and t105) never left the agent, which declares a friendship only from
//! Friendly, yet the shared refusal memory barred those rivals from every
//! desk for thirty turns. Version two:
//!
//! - asks only a rival the host reads as Friendly or a declared friend
//!   (`Player::observed_diplomatic_state`; absent on a native board);
//! - asks the first of [`SHIELD_KINDS`] free on both sides and not refused
//!   lately: Military first, then Economic, Cultural, Religious (never
//!   Religious for a seat aiming at Religion). Any alliance forbids the
//!   partner's war on us and carries the pact; the common enemy still asks
//!   only for Military, the +5 being its point;
//! - keeps its own memory per partner and kind (thirty turns, or the
//!   five-turn friendship step) and waits [`SHIELD_RETRY_TURNS`] between asks
//!   to one partner, leaving the shared memory to the stock desk, so a
//!   refused Military ask bars neither the lane's own ask nor this desk's
//!   next kind;
//! - binds a further neighbour once our Military Alliance stands.
//!
//! On the live seat its non-Military asks cross under `ShieldAllies`.
//!
//! Counters: `military_alliance_shield:asked_other_kind` (version two) and
//! `military_alliance_shield:asked`,
//! `military_alliance_shield:asked_common_enemy`,
//! `military_alliance_shield:asked_border`.

use super::*;

/// A major is at our border when one of its cities is within this many
/// tiles of one of ours: the reach at which the city campaign calls a rival
/// attackable, read from the other side.
pub(super) const SHIELD_NEIGHBOUR_REACH: i32 = super::city_campaign::CAMPAIGN_REACH;
/// Score per major we are at war with that the partner is at war with too.
/// Above any border score: the +5 is worth most in the war being fought.
pub(super) const SHIELD_COMMON_ENEMY: f64 = 400.0;
/// A neighbour under this share of our military power is no threat worth
/// the Military slot.
pub(super) const SHIELD_THREAT_FLOOR: f64 = 0.6;
/// The power ratio past which a neighbour scores no higher.
pub(super) const SHIELD_THREAT_RATIO_CAP: f64 = 2.0;
/// Border score per unit of the neighbour's power over ours.
pub(super) const SHIELD_THREAT_WEIGHT: f64 = 150.0;
/// Border score per tile the neighbour's nearest city is inside the reach.
pub(super) const SHIELD_THREAT_PER_TILE: f64 = 10.0;
/// A declared friend is asked the alliance itself; the stock ask's weight.
pub(super) const SHIELD_FRIEND: f64 = 180.0;
/// Our grievances against a partner at or above this and it is not asked
/// (the coalition's active-front ceiling).
pub(super) const SHIELD_GRIEVANCE_CEILING: f64 = 50.0;
/// The border arm asks every this many turns, staggered by seat. The common
/// enemy is asked every turn of a war.
pub(super) const SHIELD_CADENCE: u32 = 3;
/// Version two: the kinds a partner is asked for, in order. Military first
/// (the +5); then the kinds whose level-one yields are a few Gold, Culture or
/// Faith on trade routes, so binding a neighbour with them costs no lane its
/// own alliance. Research is the Science lane's own kind and is left to the
/// stock desk.
pub(super) const SHIELD_KINDS: [&str; 4] = ["military", "economic", "cultural", "religious"];
/// Version two: turns after any ask before the same partner is asked again.
/// The agent refuses a second ask to one rival inside its own retry floor
/// (`AllianceRetryTurns`, 5) without reaching the rival.
pub(super) const SHIELD_RETRY_TURNS: u32 =
    super::alliance_remembers_the_refusal::ALLIANCE_FRIENDSHIP_STEP_TURNS;

impl AdvancedAi {
    /// The partner and the alliance kind the shield asked on the last decided
    /// turn (always `military` under version one), for the live orders
    /// layer's lane filter. `None` with the gene off or when nobody was asked.
    pub fn military_alliance_shield_ask(&self) -> Option<(usize, &'static str)> {
        self.military_alliance_shield_ask
    }

    /// Version two's own refusal memory, per partner and kind: an ask that
    /// did not ally waits [`super::alliance_remembers_the_refusal::ALLIANCE_REFUSAL_TURNS`],
    /// or only [`SHIELD_RETRY_TURNS`] when it was the friendship step (the
    /// partner was not a declared friend then and is one now). Kept apart from
    /// the shared memory, so a refused Military ask does not bar the stock
    /// desk's lane ask, nor this desk's next kind, to the same partner.
    fn shield_kind_barred(&self, g: &Game, pid: usize, partner: usize, kind: &str) -> bool {
        if g.alliance_with(pid, partner).is_some() {
            return false;
        }
        let Some(&(asked, friend_when_asked)) = self
            .military_alliance_shield_asks
            .iter()
            .find(|((who, asked_kind), _)| *who == partner && *asked_kind == kind)
            .map(|(_, value)| value)
        else {
            return false;
        };
        let bar = if !friend_when_asked && g.are_friends(pid, partner) {
            SHIELD_RETRY_TURNS
        } else {
            super::alliance_remembers_the_refusal::ALLIANCE_REFUSAL_TURNS
        };
        g.turn < asked.saturating_add(bar)
    }

    /// Version two: whether `partner` was asked for any kind inside the
    /// agent's retry floor.
    fn shield_asked_recently(&self, g: &Game, partner: usize) -> bool {
        self.military_alliance_shield_asks
            .iter()
            .any(|((who, _), (asked, _))| {
                *who == partner && g.turn < asked.saturating_add(SHIELD_RETRY_TURNS)
            })
    }

    /// Whether the border arm waits for the lane's own alliance: the kind
    /// `propose_strategic_alliance` asks for on this plan is not Military, we
    /// hold none of it, it is legal on our tree, and its ask reaches the live
    /// host (the Diplomatic lane always; the Culture and Science lanes under
    /// their seat genes; an adaptive native seat has no host filter).
    pub(super) fn shield_lane_alliance_waits(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        let kind = if self.research_alliance_lane(g, pid)
            == super::research_alliance::ResearchAllianceLane::Research
        {
            super::research_alliance::ALLY_RESEARCH_KIND
        } else {
            match plan.strategy {
                GrandStrategy::Science => "research",
                GrandStrategy::Culture => "cultural",
                GrandStrategy::Religion => "religious",
                GrandStrategy::Conquest | GrandStrategy::Recovery => "military",
                GrandStrategy::Expansion | GrandStrategy::Diplomacy => "economic",
            }
        };
        if kind == "military"
            || (kind == "research" && g.tree_effect(pid, "research_agreements") <= 0.0)
            || g.players[pid]
                .alliances
                .values()
                .any(|alliance| alliance.ends > g.turn && alliance.kind == kind)
        {
            return false;
        }
        match self.active_victory_target(g) {
            None | Some(VictoryTarget::Diplomacy) => true,
            Some(VictoryTarget::Culture) => self.culture_seat_allies,
            Some(VictoryTarget::Science) => self.science_seat_allies,
            Some(_) => false,
        }
    }

    /// The living majors at war with `pid`.
    fn shield_enemies(g: &Game, pid: usize) -> Vec<usize> {
        g.players
            .iter()
            .filter(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && g.is_at_war(pid, other.id)
            })
            .map(|other| other.id)
            .collect()
    }

    /// The border arm's score for `other` against our cities `ours`: zero
    /// beyond [`SHIELD_NEIGHBOUR_REACH`] or under [`SHIELD_THREAT_FLOOR`].
    pub(super) fn shield_border_score(g: &Game, pid: usize, ours: &[Pos], other: usize) -> f64 {
        let Some(nearest) = g
            .player_city_ids(other)
            .into_iter()
            .filter_map(|cid| g.cities.get(&cid).map(|city| city.pos))
            .flat_map(|theirs| ours.iter().map(move |mine| g.wdist(*mine, theirs)))
            .min()
        else {
            return 0.0;
        };
        if nearest > SHIELD_NEIGHBOUR_REACH {
            return 0.0;
        }
        let ratio = g.military_power(other) / g.military_power(pid).max(1.0);
        if !ratio.is_finite() || ratio < SHIELD_THREAT_FLOOR {
            return 0.0;
        }
        SHIELD_THREAT_WEIGHT * ratio.min(SHIELD_THREAT_RATIO_CAP)
            + f64::from(SHIELD_NEIGHBOUR_REACH - nearest) * SHIELD_THREAT_PER_TILE
    }

    /// Ask one partner for a Military Alliance: the empire fighting our
    /// enemy, or the strongest neighbour at our border. Returns whether an
    /// ask went out. Nothing with the gene off.
    pub(super) fn military_alliance_shield_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
        denied_partner: Option<usize>,
    ) -> bool {
        self.military_alliance_shield_ask = None;
        let v2 = self.military_alliance_shield_2;
        if !(self.military_alliance_shield || v2)
            || !g.players[pid]
                .civics
                .contains(&crate::name!("civil_service"))
        {
            return false;
        }
        let turn = g.turn;
        let kind_taken = |who: usize, kind: &str| {
            g.players[who]
                .alliances
                .values()
                .any(|alliance| alliance.ends > turn && alliance.kind == kind)
        };
        let military_taken = |who: usize| kind_taken(who, "military");
        // One alliance ask in flight at a time (the coalition's, say): the
        // live host would hold a second (`hold_deals_beside_an_alliance`),
        // yet the refusal memory would remember it as asked.
        // Version two binds a second neighbour with another kind once our
        // Military Alliance stands.
        if (!v2 && military_taken(pid))
            || g.pending_deals
                .iter()
                .any(|deal| deal.from == pid && deal.alliance.is_some() && deal.expires >= turn)
        {
            return false;
        }
        let enemies = Self::shield_enemies(g, pid);
        let domination = plan.strategy == GrandStrategy::Conquest
            || self.active_victory_target(g) == Some(VictoryTarget::Domination);
        let border_turn = !domination
            && turn % SHIELD_CADENCE == pid as u32 % SHIELD_CADENCE
            && !self.shield_lane_alliance_waits(g, pid, plan);
        if enemies.is_empty() && !border_turn {
            return false;
        }
        let ours: Vec<Pos> = g
            .player_city_ids(pid)
            .into_iter()
            .filter_map(|cid| g.cities.get(&cid).map(|city| city.pos))
            .collect();
        let science_threats = self.science_threat_seats(g, pid);
        let grievance = |holder: usize, against: usize| {
            g.players[holder]
                .grievances
                .get(&against)
                .copied()
                .unwrap_or(0.0)
        };
        let denounced = |holder: usize, against: usize| {
            g.players[holder]
                .denounced_until
                .get(&against)
                .is_some_and(|until| *until > turn)
        };
        let religion_lane = plan.strategy == GrandStrategy::Religion
            || self.active_victory_target(g) == Some(VictoryTarget::Religion);
        let pending_with = |partner: usize| {
            g.pending_deals.iter().any(|deal| {
                deal.expires >= turn
                    && ((deal.from == pid && deal.to == partner)
                        || (deal.from == partner && deal.to == pid))
            })
        };
        let mut best: Option<(f64, usize, usize, &'static str)> = None;
        for other in g.players.iter() {
            let id = other.id;
            if id == pid
                || !other.alive
                || other.is_minor
                || other.is_barbarian
                || !g.has_met(pid, id)
                || g.is_at_war(pid, id)
                || Some(id) == denied_partner
                || !other.civics.contains(&crate::name!("civil_service"))
                || g.alliance_with(pid, id).is_some()
                || (!v2 && military_taken(id))
                || pending_with(id)
                || denounced(pid, id)
                || denounced(id, pid)
                || grievance(pid, id) >= SHIELD_GRIEVANCE_CEILING
                || self.rival_victory_pressure(g, id).progress >= 82
                || self.domination_counter_target(g, pid, id)
                || self.domination_capital_prey(g, pid, id)
                || self.nearest_finish_culture_clock(g, id).is_some()
                || self.science_denial_refuses_alliance(&science_threats, id)
                || self.culture_seat_ally_barred(g, pid, id)
                || self.alliance_refusal_barred(g, pid, id)
            {
                continue;
            }
            // Version two asks only a rival the host reads as Friendly or a
            // declared friend: the agent asks nobody colder (it declares a
            // friendship only from Friendly), yet the ask would still be
            // remembered as refused. Absent on a native board.
            if v2
                && (other
                    .observed_diplomatic_state
                    .get(&pid)
                    .is_some_and(|state| !matches!(state.as_str(), "FRIENDLY" | "DECLARED_FRIEND"))
                    || self.shield_asked_recently(g, id))
            {
                continue;
            }
            // The kind: Military under version one; under version two the
            // first of `SHIELD_KINDS` free on both sides and not refused
            // lately (never Religious on a Religion lane: allies take no
            // religious pressure from each other).
            let kind_ok = |kind: &str| {
                !kind_taken(pid, kind)
                    && !kind_taken(id, kind)
                    && !(v2 && self.shield_kind_barred(g, pid, id, kind))
                    && !(kind == "religious" && religion_lane)
            };
            let military_ok = kind_ok("military");
            let common = if !military_ok {
                0
            } else {
                enemies
                    .iter()
                    // `rivals_at_war`: on the live seat the host's report of a
                    // war between two rivals (`Game::observed_rival_wars`).
                    .filter(|enemy| **enemy != id && g.rivals_at_war(id, **enemy))
                    .count()
            };
            // The common enemy asks for Military; the border for the first
            // free kind (Military only, under version one).
            let kind = if common > 0 {
                Some("military")
            } else if v2 {
                SHIELD_KINDS.iter().copied().find(|kind| kind_ok(kind))
            } else {
                military_ok.then_some("military")
            };
            let Some(kind) = kind else {
                continue;
            };
            let border = if border_turn {
                Self::shield_border_score(g, pid, &ours, id)
            } else {
                0.0
            };
            if common == 0 && border <= 0.0 {
                continue;
            }
            let friend = if g.are_friends(pid, id) {
                SHIELD_FRIEND
            } else {
                0.0
            };
            let score = common as f64 * SHIELD_COMMON_ENEMY + border + friend - grievance(pid, id);
            if best.is_none_or(|(top, _, _, _)| score > top) {
                best = Some((score, id, common, kind));
            }
        }
        let Some((_, partner, common, kind)) = best else {
            return false;
        };
        let proposal = Action::ProposeDeal {
            player: partner,
            give_gold: 0.0,
            request_gold: 0.0,
            // `no_free_passage`: passage is sold, not bundled.
            open_borders: !self.base.no_free_passage
                && g.players[pid]
                    .civics
                    .contains(&crate::name!("early_empire")),
            friendship: true,
            peace: false,
            alliance: Some(kind.to_string()),
        };
        if g.apply(pid, &proposal).is_err() {
            return false;
        }
        self.military_alliance_shield_ask = Some((partner, kind));
        if v2 {
            // Its own memory only: the shared one would bar the stock desk's
            // lane ask to this partner, and this desk's next kind.
            let friend = g.are_friends(pid, partner);
            self.military_alliance_shield_asks
                .insert((partner, kind), (turn, friend));
        } else {
            self.alliance_note_asked(g, pid, partner);
        }
        let arm = if common > 0 {
            "military_alliance_shield:asked_common_enemy"
        } else {
            "military_alliance_shield:asked_border"
        };
        let other_kind =
            (kind != "military").then_some("military_alliance_shield:asked_other_kind");
        for counter in ["military_alliance_shield:asked", arm]
            .into_iter()
            .chain(other_kind)
        {
            *g.players[pid]
                .counters
                .entry(counter.to_string())
                .or_insert(0) += 1;
        }
        if self.journal().wants(crate::reasoning::Level::Decision) {
            if common > 0 {
                think!(self.journal(), Diplomacy, Decision,
                       "Asking {} for a Military Alliance", g.players[partner].civ;
                       "they are at war with {common} empire{} we are fighting: allied, our units \
                        and theirs take +5 combat strength against that common enemy",
                       if common == 1 { "" } else { "s" });
            } else if kind == "military" {
                think!(self.journal(), Diplomacy, Decision,
                       "Asking {} for a Military Alliance", g.players[partner].civ;
                       "they are the strongest empire at our border: allied, they cannot declare \
                        on us, and their pact stands beside us against anyone who does");
            } else {
                think!(self.journal(), Diplomacy, Decision,
                       "Asking {} for a {kind} alliance", g.players[partner].civ;
                       "they are the strongest empire at our border and the Military kind is \
                        taken or was refused: any alliance forbids their war on us and carries \
                        the pact");
            }
        }
        true
    }
}

#[cfg(test)]
mod tests;
