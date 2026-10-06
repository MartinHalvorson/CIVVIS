//! `capital-taken-moves-on`: once a rival's original capital is ours, the
//! Domination army moves on to the next capital instead of mopping up the
//! rival's towns.
//!
//! Domination needs only the original capitals. `one_war_peace` offers the
//! beaten rival "the required capital is secure" peace and
//! `one_war_second_front` names the next capital's owner, but a refused
//! peace left the army on the remnant: the declaration desk asks a staged
//! siege of the next capital, and the Board writes no Siege row for a rival
//! at peace, so nothing ever staged there. Live King civvis-20261005T051413Z
//! offered Canada that peace 57 times from turn 88 while "Holding off war
//! with Sumeria | the Siege row for Uruk asks 489 strength and 0 is staged"
//! and only reached Uruk's siege at 146; civvis-20261005T212727Z (game 165)
//! offered France it 28 times after Paris fell at 195 and spent until 244
//! eliminating France; Indonesia won on Diplomacy at 266. Over the 10-04..06
//! control runs, captures whose game another rival won held their army on the
//! beaten rival's ordinary towns a median 13 siege-turns (11 of 21 at 13 or
//! more), against 7 in the wins.
//!
//! Under the gene:
//!
//! 1. After [`CAPITAL_MOVES_ON_TURNS`] standard turns of a refused
//!    CapitalSecured peace, with the beaten rival neither the Diplomatic
//!    Victory contender the seat must eliminate nor the rival with the
//!    shortest victory clock, the next capital's owner is the second front
//!    and the desk declares on it without a staged siege (it must pass the
//!    version-2 edge), the way `blocker-becomes-the-target` opens a road. The
//!    remnant war stays at war on the defensive; the front then moves to the
//!    new war (`one_war_choose_front`).
//! 2. The elimination front (`diplomatic_contender_to_eliminate`) yields when
//!    another rival's concrete victory clock (Science Victory points, the
//!    engine's culture clock) runs out [`CLOCK_MARGIN_TURNS`] standard turns
//!    before the contender can reach the Diplomatic Victory, so the army goes
//!    to the clock that ends the game first. Live King
//!    civvis-20261006T004226Z (game 177) read "Eliminating Egypt" from turn
//!    207 to 231 at 14-15 points and took seven more Egyptian towns; Ethiopia
//!    won on Science at 248.

use super::one_war::OneWarPeace;
use super::{AdvancedAi, VictoryTarget};
use crate::game::{Game, DIPLOMATIC_VICTORY_POINTS};

/// Standard turns of a refused CapitalSecured peace before the army moves
/// on: `one_war::ONE_WAR_SECOND_FRONT_PATIENCE`, the same patience the second
/// front already gives an offered peace.
pub(crate) const CAPITAL_MOVES_ON_TURNS: u32 = 3;

/// Diplomatic Victory points a contender is credited per Congress session:
/// the leader's +2 for the winning resolution and +1 for voting with it
/// (memory: "leader gets +3/session").
const DVP_PER_SESSION: i64 = 3;

/// A Congress session's standard length (`mirror::apply_host_congress`).
const CONGRESS_SESSION_TURNS: u32 = 30;

/// How many standard turns sooner another clock must run out than the
/// contender's before the elimination front yields to it: a reading that is
/// only just shorter is not worth abandoning a crushed contender for.
const CLOCK_MARGIN_TURNS: u32 = 5;

/// The horizon inside which a rival's clock makes it "the shortest-clock
/// threat" that keeps the front: two Congress sessions.
const THREAT_HORIZON_TURNS: u32 = 60;

impl AdvancedAi {
    /// The turns before `rival` can reach the Diplomatic Victory if it takes
    /// [`DVP_PER_SESSION`] points at every Congress, the next session read
    /// from the active resolutions' expiry (now, when none is in force): the
    /// short end of the estimate, so the elimination front yields only to a
    /// clearly shorter clock.
    pub(crate) fn moves_on_dvp_clock(&self, g: &Game, rival: usize) -> f64 {
        let gap = DIPLOMATIC_VICTORY_POINTS - g.players[rival].dvp;
        if gap <= 0 {
            return 0.0;
        }
        let sessions = (gap + DVP_PER_SESSION - 1) / DVP_PER_SESSION;
        let session = g.standard_duration(CONGRESS_SESSION_TURNS);
        let next = g
            .active_congress_effects
            .iter()
            .map(|effect| effect.expires.saturating_sub(1).saturating_sub(g.turn))
            .min()
            .unwrap_or(0);
        f64::from(next) + (sessions - 1).max(0) as f64 * f64::from(session)
    }

    /// `rival`'s shortest concrete Science or Culture clock in turns: the
    /// host's Science Victory points against their target at the current
    /// rate, and the engine's own culture clock once
    /// `denial_nearest_finish::ENGINE_CLOCK_MIN_TOURISTS` visitors back it.
    /// `None` when neither reading exists.
    pub(crate) fn moves_on_race_clock(&self, g: &Game, rival: usize) -> Option<f64> {
        let science = g.observed_public_empire_stats.get(&rival).and_then(|stats| {
            let (points, needed, rate) = (
                stats.science_victory_points?,
                stats.science_victory_points_needed?,
                stats.science_victory_points_per_turn?,
            );
            (rate > 0.0 && needed > 0.0).then(|| (needed - points).max(0.0) / rate)
        });
        let culture = g.culture_turns_to_victory(rival).filter(|turns| {
            *turns >= 0.0
                && g.foreign_tourists(rival) >= super::denial_nearest_finish::ENGINE_CLOCK_MIN_TOURISTS
        });
        match (science, culture) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// The living major with the shortest victory clock inside
    /// [`THREAT_HORIZON_TURNS`] standard turns, Science, Culture or
    /// Diplomacy; the lowest id on a tie.
    fn moves_on_shortest_clock(&self, g: &Game, pid: usize) -> Option<usize> {
        let horizon = f64::from(g.standard_duration(THREAT_HORIZON_TURNS));
        g.players
            .iter()
            .filter(|rival| rival.id != pid && rival.alive && !rival.is_minor && !rival.is_barbarian)
            .filter_map(|rival| {
                let dvp = self.moves_on_dvp_clock(g, rival.id);
                let clock = self
                    .moves_on_race_clock(g, rival.id)
                    .map_or(dvp, |race| race.min(dvp));
                (clock <= horizon).then_some((rival.id, clock))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
            .map(|(rival, _)| rival)
    }

    /// Part 2: whether the elimination front on `contender` yields, under
    /// the gene: we hold its original capital, and another living major's
    /// concrete Science or Culture clock runs out [`CLOCK_MARGIN_TURNS`]
    /// standard turns before the contender can reach 20 points.
    pub(crate) fn elimination_yields_to_a_shorter_clock(
        &self,
        g: &Game,
        pid: usize,
        contender: usize,
    ) -> bool {
        if !self.capital_taken_moves_on || !self.holds_original_capital_of(g, pid, contender) {
            return false;
        }
        let contender_clock = self.moves_on_dvp_clock(g, contender);
        let margin = f64::from(g.standard_duration(CLOCK_MARGIN_TURNS));
        g.players
            .iter()
            .filter(|rival| {
                rival.id != pid
                    && rival.id != contender
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
            })
            .filter_map(|rival| self.moves_on_race_clock(g, rival.id))
            .any(|clock| clock + margin < contender_clock)
    }

    /// Whether `pid` holds `rival`'s original capital.
    fn holds_original_capital_of(&self, g: &Game, pid: usize, rival: usize) -> bool {
        g.cities
            .values()
            .any(|city| city.owner == pid && city.is_capital && city.original_owner == rival)
    }

    /// The rival the army moves on to from the current front, whether or not
    /// we are at war with it yet; `None` with the gene off or while the
    /// front still has a reason to hold the army. See the module docs.
    pub(crate) fn capital_moves_on_next(&self, g: &Game, pid: usize) -> Option<usize> {
        if !self.capital_taken_moves_on
            || !self.one_war_at_a_time
            || self.forced_target_player.is_some()
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
        {
            return None;
        }
        let front_state = self.one_war.as_ref()?;
        let front = front_state.target;
        if !g.is_at_war(pid, front) || !self.holds_original_capital_of(g, pid, front) {
            return None;
        }
        // Part 2: the yielding contender hands the army to the shortest
        // clock that holds a capital, else to the cheapest next capital.
        if self.diplomatic_contender_base(g, pid) == Some(front)
            && self.elimination_yields_to_a_shorter_clock(g, pid, front)
        {
            return self.moves_on_capital_holder(g, pid, front);
        }
        // Part 1: a refused CapitalSecured peace.
        let refused = self.one_war_peace(g, pid, front) == Some(OneWarPeace::CapitalSecured)
            && front_state.closure_wanted_since.is_some_and(|since| {
                g.turn.saturating_sub(since) >= g.standard_duration(CAPITAL_MOVES_ON_TURNS).max(1)
            });
        if !refused
            || self.diplomatic_contender_to_eliminate(g, pid) == Some(front)
            || self.moves_on_shortest_clock(g, pid) == Some(front)
        {
            return None;
        }
        self.domination_followup_target(g, pid, Some(front))
    }

    /// The next capital's owner after `beaten`: the shortest-clock rival when
    /// it holds an original capital we need, else the owner of the cheapest
    /// such capital (`campaign_city_value`, as `domination_followup_target`
    /// ranks them).
    fn moves_on_capital_holder(&self, g: &Game, pid: usize, beaten: usize) -> Option<usize> {
        let holds_needed_capital = |owner: usize| {
            g.cities.values().any(|city| {
                city.owner == owner
                    && city.is_capital
                    && !g.players[city.original_owner].is_minor
                    && !g.players[city.original_owner].is_barbarian
                    && city.original_owner != pid
                    && !g.same_team(pid, city.original_owner)
            })
        };
        let usable = |owner: usize| {
            owner != pid
                && owner != beaten
                && !g.same_team(pid, owner)
                && g.players
                    .get(owner)
                    .is_some_and(|player| player.alive && !player.is_minor && !player.is_barbarian)
                && holds_needed_capital(owner)
                && self.campaign_target_legal(g, pid, owner)
        };
        if let Some(shortest) = self.moves_on_shortest_clock(g, pid).filter(|rival| usable(*rival)) {
            return Some(shortest);
        }
        g.cities
            .values()
            .filter(|city| {
                city.is_capital
                    && usable(city.owner)
                    && !g.players[city.original_owner].is_minor
                    && !g.players[city.original_owner].is_barbarian
            })
            .min_by(|left, right| {
                self.campaign_city_value(g, pid, left, super::GrandStrategy::Conquest)
                    .total_cmp(&self.campaign_city_value(g, pid, right, super::GrandStrategy::Conquest))
                    .then(left.id.cmp(&right.id))
            })
            .map(|city| city.owner)
    }

    /// Part 1's second front: the next capital's owner, still at peace,
    /// legal, and passing the version-2 declaration edge.
    pub(crate) fn capital_moves_on_second_front(&self, g: &Game, pid: usize) -> Option<usize> {
        let front = self.one_war_front()?;
        self.capital_moves_on_next(g, pid).filter(|next| {
            *next != front
                && !g.is_at_war(pid, *next)
                && self.campaign_target_legal(g, pid, *next)
                && self.declaration_edge_2(g, pid, *next).passes()
        })
    }
}

#[cfg(test)]
mod tests;
