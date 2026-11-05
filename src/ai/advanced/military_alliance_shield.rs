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
//! 2. **The border.** Off the Domination lane, a major with a city within
//!    [`SHIELD_NEIGHBOUR_REACH`] of one of ours that fields at least
//!    [`SHIELD_THREAT_FLOOR`] of our power scores [`SHIELD_THREAT_WEIGHT`]
//!    per unit of that ratio (capped at [`SHIELD_THREAT_RATIO_CAP`]) plus
//!    [`SHIELD_THREAT_PER_TILE`] per tile nearer than the reach. Allied, it
//!    cannot declare on us, and its pact stands beside us against anyone
//!    else who does. Every [`SHIELD_CADENCE`] turns, staggered by seat.
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
//! ([`AdvancedAi::military_alliance_shield_partner`]) on every lane, and the
//! agent's `CivvisAllianceStep` takes it under `MilitaryAllies`, which
//! civ6_play.py sets when the tag is armed.
//!
//! Counters: `military_alliance_shield:asked`,
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

impl AdvancedAi {
    /// The partner `military-alliance-shield` asked on the last decided turn,
    /// for the live orders layer's lane filter. `None` with the gene off or
    /// when nobody was asked.
    pub fn military_alliance_shield_partner(&self) -> Option<usize> {
        self.military_alliance_shield_ask
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
        if !self.military_alliance_shield
            || !g.players[pid]
                .civics
                .contains(&crate::name!("civil_service"))
        {
            return false;
        }
        let turn = g.turn;
        let military_taken = |who: usize| {
            g.players[who]
                .alliances
                .values()
                .any(|alliance| alliance.ends > turn && alliance.kind == "military")
        };
        // One alliance ask in flight at a time (the coalition's, say): the
        // live host would hold a second (`hold_deals_beside_an_alliance`),
        // yet the refusal memory would remember it as asked.
        if military_taken(pid)
            || g.pending_deals
                .iter()
                .any(|deal| deal.from == pid && deal.alliance.is_some() && deal.expires >= turn)
        {
            return false;
        }
        let enemies = Self::shield_enemies(g, pid);
        let domination = plan.strategy == GrandStrategy::Conquest
            || self.active_victory_target(g) == Some(VictoryTarget::Domination);
        let border_turn = !domination && turn % SHIELD_CADENCE == pid as u32 % SHIELD_CADENCE;
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
        let pending_with = |partner: usize| {
            g.pending_deals.iter().any(|deal| {
                deal.expires >= turn
                    && ((deal.from == pid && deal.to == partner)
                        || (deal.from == partner && deal.to == pid))
            })
        };
        let mut best: Option<(f64, usize, usize)> = None;
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
                || military_taken(id)
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
            let common = enemies
                .iter()
                // `rivals_at_war`: on the live seat the host's report of a
                // war between two rivals (`Game::observed_rival_wars`).
                .filter(|enemy| **enemy != id && g.rivals_at_war(id, **enemy))
                .count();
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
            if best.is_none_or(|(top, _, _)| score > top) {
                best = Some((score, id, common));
            }
        }
        let Some((_, partner, common)) = best else {
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
            alliance: Some("military".to_string()),
        };
        if g.apply(pid, &proposal).is_err() {
            return false;
        }
        self.military_alliance_shield_ask = Some(partner);
        self.alliance_note_asked(g, pid, partner);
        let arm = if common > 0 {
            "military_alliance_shield:asked_common_enemy"
        } else {
            "military_alliance_shield:asked_border"
        };
        for counter in ["military_alliance_shield:asked", arm] {
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
            } else {
                think!(self.journal(), Diplomacy, Decision,
                       "Asking {} for a Military Alliance", g.players[partner].civ;
                       "they are the strongest empire at our border: allied, they cannot declare \
                        on us, and their pact stands beside us against anyone who does");
            }
        }
        true
    }
}

#[cfg(test)]
mod tests;
