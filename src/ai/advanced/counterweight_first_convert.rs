//! `counterweight-from-the-first-convert`: a faithless Domination seat starts
//! its counterweight at the first of its cities a threatening faith takes,
//! not at the majority, and builds a counterweight source when it has none.
//!
//! The shipped counterweight waits for the majority: `adopted_faith_threat`
//! names a faith once it holds half our cities (or every other major), and
//! `counterweight_need` is zero below the majority, so the cap stays at the
//! shipped two and `counterweight_faith_reserve` holds nothing. Over the 17
//! Religious defeats of the October 6-7 Emperor runs, 12 of them faithless,
//! our first city followed the winner's faith at a median turn 68 and the
//! majority fell at a median turn 85 (4 to 49 turns later); a counterweight
//! source (a Shrine city of ours on another faith) existed at any point in
//! only 6 of the 12, and the Faith banked at the end was a median ~400
//! (6 to 1,206), unspent. Under the gene:
//! - a faith founded by a living rival is a live threat from the first city
//!   of ours it holds once it stands at the religious early-warning bar or
//!   holds all but one of the other majors (`first_convert_threat`);
//! - against it the counterweight need is the cities it holds, so the cap
//!   and the Faith reserve rise from that first convert
//!   (`counterweight_need_for`, read by `counterweight_cap` and
//!   `counterweight_faith_reserve`);
//! - the Holy Site and Shrine sanctuary (`adopted_faith_sanctuary_choice`)
//!   starts on that threat too, and with no city on a safe counterfaith it
//!   builds in a city that follows no religion yet.
//!
//! Off: unchanged. A founder is untouched.

use super::{AdvancedAi, VictoryTarget};
use crate::game::Game;

impl AdvancedAi {
    fn first_convert_scope(&self, g: &Game, pid: usize) -> bool {
        self.counterweight_from_the_first_convert
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && g.victory_conditions.religious
            && g.players[pid].religion.is_none()
    }

    /// Our cities whose majority follows `faith`.
    fn cities_following(g: &Game, pid: usize, faith: &str) -> usize {
        g.player_city_ids(pid)
            .into_iter()
            .filter(|cid| g.city_religion(&g.cities[cid]) == Some(faith))
            .count()
    }

    /// Whether `faith`, founded by a living rival, holds one of our cities and
    /// stands at the religious early-warning bar or holds all but one of the
    /// other majors (never fewer than one). The founder and we are not
    /// counted among the others.
    pub(super) fn first_convert_live(&self, g: &Game, pid: usize, faith: &str) -> bool {
        if !self.first_convert_scope(g, pid) || Self::cities_following(g, pid, faith) == 0 {
            return false;
        }
        let Some(founder) = g.players.iter().find(|p| {
            p.id != pid
                && p.alive
                && !p.is_minor
                && !p.is_barbarian
                && p.religion.as_deref() == Some(faith)
        }) else {
            return false;
        };
        let living = g
            .players
            .iter()
            .filter(|p| p.alive && !p.is_minor && !p.is_barbarian)
            .count() as i32;
        let others: Vec<usize> = g
            .players
            .iter()
            .filter(|p| {
                p.alive && !p.is_minor && !p.is_barbarian && p.id != pid && p.id != founder.id
            })
            .map(|p| p.id)
            .collect();
        let dominated = others
            .iter()
            .filter(|other| g.civ_follows_religion(**other, faith))
            .count();
        let holds_all_but_one = dominated >= others.len().saturating_sub(1).max(1);
        let match_point = 100 * (living - 1) / living.max(1);
        let early_warning = (100 * (living - 2) / living.max(1))
            .max(50)
            .min(match_point);
        holds_all_but_one || self.lane_progress_table(g, founder.id)[2] >= early_warning
    }

    /// The live first-convert threat: the faith holding the most other
    /// majors, then the most of our cities. `None` with the gene off. Under
    /// `counterweight-faith-is-no-threat` a stronger faith present in our
    /// cities is the threat instead, as `adopted_faith_threat` reads it.
    pub(super) fn first_convert_threat(&self, g: &Game, pid: usize) -> Option<String> {
        if !self.first_convert_scope(g, pid) {
            return None;
        }
        let threat = g
            .players
            .iter()
            .filter(|p| p.id != pid && p.alive && !p.is_minor && !p.is_barbarian)
            .filter_map(|p| p.religion.as_deref())
            .filter(|faith| self.first_convert_live(g, pid, faith))
            .map(|faith| {
                let dominated = g
                    .players
                    .iter()
                    .filter(|p| {
                        p.id != pid
                            && p.alive
                            && !p.is_minor
                            && !p.is_barbarian
                            && p.religion.as_deref() != Some(faith)
                            && g.civ_follows_religion(p.id, faith)
                    })
                    .count();
                (dominated, Self::cities_following(g, pid, faith), faith)
            })
            .max_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(b.2.cmp(a.2)))
            .map(|(_, _, faith)| faith.to_owned());
        if self.counterweight_faith_is_no_threat {
            if let Some(stronger) = threat
                .as_deref()
                .and_then(|faith| Self::stronger_faith_than(g, pid, faith))
            {
                return Some(stronger);
            }
        }
        threat
    }

    /// `counterweight_need` under the gene: against a live first-convert
    /// threat, every city of ours it holds. The shipped majority reading
    /// otherwise, and never less than it.
    pub(super) fn counterweight_need_for(&self, g: &Game, pid: usize, threat: &str) -> usize {
        let shipped = Self::counterweight_need(g, pid, threat);
        if self.first_convert_live(g, pid, threat) {
            shipped.max(Self::cities_following(g, pid, threat))
        } else {
            shipped
        }
    }

    /// Whether the sanctuary may fall back to a city of ours that follows no
    /// religion: the gene on and its threat live.
    pub(super) fn first_convert_sanctuary_fallback(&self, g: &Game, pid: usize) -> bool {
        self.first_convert_threat(g, pid).is_some()
    }
}

#[cfg(test)]
mod tests;
