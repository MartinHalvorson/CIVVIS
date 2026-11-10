//! `spreader-steps-out-of-the-apostle`: a religious unit of ours never ends a
//! step where a rival Apostle or Inquisitor of another faith can reach it and
//! win the theological fight.
//!
//! **Why (10-09/10, 109 Emperor runs with events on disk).** Rival Apostles
//! attacked our religious units 1,039 times and killed 395 of them:
//! Missionaries 872 of the targets, Inquisitors 101, Gurus 61. 576 of the
//! fights (55%) were one or two tiles from one of our cities: the Missionary
//! had walked to `stop_range` 1 of its target and spread from beside it, and
//! a spread ends the unit's turn where it stands. Every kill hands the
//! winner's faith 250 pressure across a ten-tile radius
//! (`religious_combat_pressure`; the host's `RELIGION_SPREAD_COMBAT_VICTORY`
//! is 250), so a founder's bloc beside the kill drops: G231456Z (Buddhism)
//! lost Missionaries at t90, t92 and twice at t103, went from five Buddhist
//! cities to none between t96 and t104, and lost the game at t106. Nothing
//! read the rival corps: `missionary_evades_raiders` reads Barbarians only,
//! and `theology-fights-only-to-win` only stops the fights we start.
//!
//! **What it does.** A rival religious unit is a threat to one of ours when
//! it is visible, of another faith, can initiate (an Apostle or an
//! Inquisitor), and its mean blow would kill our unit or out-trade it on the
//! tile in question (`Game::theological_exchange_preview_with_defender_at`:
//! our strength moves with us — home territory, a Holy Site of our faith
//! beside us, our units around us — so the same rival loses the fight on one
//! tile and wins it on the next; G213038Z t124 died stepping off such a
//! tile). Its reach is every tile beside the tile it stands on or could
//! stand on after a full turn's movement (`Game::threat_reach`). At the start
//! of each activation a threatened unit:
//! 1. spends a last charge on the city beside it, which removes the unit
//!    before any fight can (`last_charge_spends_here`);
//! 2. otherwise steps to the neighbour outside every threat's reach, nearest
//!    our cities first, or along the route to the nearest such tile this
//!    turn's movement still reaches;
//! 3. with none, spends a charge on the city beside it when it can, else
//!    steps to the neighbour the fewest threats reach.
//! The march (`religious_step_toward_range`) never steps into a threat's
//! reach either: with a dangerous neighbour it takes the route's own next
//! tile when that is safe, else a safe neighbour that still closes on the
//! target, else holds; with none it walks as it always did.
//!
//! Exactly the shipped behaviour with the gene off or no threat in sight.

use super::AdvancedAi;
use crate::game::{Action, Game};
use crate::Pos;

/// How far off a rival is read at all: its full turn's reach plus ours, with
/// slack for roads.
const THREAT_HORIZON: i32 = 12;

/// A rival religious unit able to open a theological fight: every tile it can
/// strike from on a fresh turn, its own tile included. Whether it wins is read
/// per tile, since our strength moves with us (home territory, a Holy Site of
/// our faith beside us, our units around us).
pub(super) struct TheologicalThreat {
    rival: u32,
    strike_from: Vec<Pos>,
    /// `stalled-spreader-braves-the-march`: only a lethal blow counts.
    lethal_only: bool,
    /// `corps-comes-home-alive`: the blows of every rival in reach are
    /// counted together. See [`AdvancedAi::every_blow_kills`].
    every_blow: bool,
}

/// A rival Apostle's blow at the 75th percentile, over the blow the model
/// reads. Live, five games of 2026-11-10, full-health rival Apostles striking
/// our full-health units: 53 on an Apostle where the model reads 30 (mean 39,
/// 90th percentile 56), 63 on a Missionary where it reads 45. The host adds
/// difficulty and Intel strength, and the model does not see a rival's
/// Debater.
const BLOW_AT_THE_QUARTILE: f64 = 1.75;

/// How many blows on one unit of ours in one turn the corps plans for. Two
/// such blows kill a full-health Apostle, so a tile two rival Apostles reach
/// is unsafe for every unit of ours. The games played with this rule relaxed
/// to the mean blow (2026-11-10 07:30Z to 08:30Z) lost 11, 12 and 16 units
/// and converted 0 to 2 civs of 5; the four before it lost 0, 2, 9 and 13,
/// won once and reached 3 of 5 once, with as many kills.
const BLOWS_IN_A_TURN: usize = 2;

/// How far past the modelled reach a pair of rivals is still counted. Three
/// units traced on 2026-11-10 died at full health on the first tile the
/// model called out of reach: it misses a rival's roads and rivals it has not
/// seen.
const REACH_MARGIN: i32 = 1;

impl AdvancedAi {
    /// The visible rival Apostles and Inquisitors of another faith within
    /// [`THREAT_HORIZON`] of `uid`. Empty with the gene off, for a
    /// non-religious unit, or for a unit without a faith.
    pub(super) fn theological_threats(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
    ) -> Vec<TheologicalThreat> {
        if !self.spreader_steps_out_of_the_apostle {
            return Vec::new();
        }
        let Some(unit) = g.units.get(&uid) else {
            return Vec::new();
        };
        if g.rules.units[unit.kind].class != "religious" {
            return Vec::new();
        }
        let Some(faith) = unit
            .religion
            .clone()
            .or_else(|| g.players[pid].religion.clone())
        else {
            return Vec::new();
        };
        let ours = unit.pos;
        // `stalled-spreader-braves-the-march`: a stalled spreader on a
        // Religion seat yields only to a lethal blow. See
        // `advanced/stalled_spreader.rs`.
        // `religion-seat-presses-the-campaign`: from the start, on the same
        // terms. See `advanced/religion_seat_fields_apostles.rs`.
        let lethal_only =
            self.spreader_braves_the_march(g, uid) || self.religion_seat_presses_on(g, pid);
        g.units
            .values()
            .filter(|rival| {
                rival.owner != pid
                    && rival.hp > 0
                    && matches!(rival.kind.as_str(), "apostle" | "inquisitor")
                    && rival
                        .religion
                        .as_deref()
                        .is_some_and(|religion| religion != faith)
                    && g.wdist(rival.pos, ours) <= THREAT_HORIZON
                    && self.battlefront_unit_visible(g, pid, rival.id)
            })
            .map(|rival| {
                let mut strike_from = g.threat_reach(rival.id);
                strike_from.push(rival.pos);
                TheologicalThreat {
                    rival: rival.id,
                    strike_from,
                    lethal_only,
                    every_blow: self.corps_comes_home_alive,
                }
            })
            .collect()
    }

    /// The rival's mean blow at `ours` standing on `at` kills it, or deals
    /// more than the rival takes back. With `lethal_only`, only the kill.
    fn rival_wins_the_exchange(
        g: &Game,
        rival: u32,
        ours: u32,
        at: Pos,
        lethal_only: bool,
    ) -> bool {
        g.theological_exchange_preview_with_defender_at(rival, ours, at)
            .is_some_and(|(dealt, received, defender_hp)| {
                dealt >= f64::from(defender_hp) || (!lethal_only && dealt > received)
            })
    }

    fn theological_threat_reaches(
        game: &Game,
        ours: u32,
        position: Pos,
        threat: &TheologicalThreat,
    ) -> bool {
        threat
            .strike_from
            .iter()
            .any(|from| *from != position && game.wdist(*from, position) <= 1)
            && Self::rival_wins_the_exchange(game, threat.rival, ours, position, threat.lethal_only)
    }

    fn theological_tile_is_safe(
        game: &Game,
        ours: u32,
        position: Pos,
        threats: &[TheologicalThreat],
    ) -> bool {
        threats
            .iter()
            .all(|threat| !Self::theological_threat_reaches(game, ours, position, threat))
            && Self::every_blow_kills(game, ours, position, threats) == 0
    }

    /// `corps-comes-home-alive`: how many rivals can strike `ours` on `pos`
    /// this turn (within [`REACH_MARGIN`] of their modelled reach), when the
    /// two heaviest blows together would kill it. Zero with the gene off or
    /// when the unit survives them. 20 of 74 kills on our corps in four games
    /// of 2026-11-09/10 took two or three attacks in one turn, none of them
    /// lethal alone. One rival Apostle does not hold a full-health unit; two
    /// hold any.
    fn every_blow_kills(g: &Game, ours: u32, pos: Pos, threats: &[TheologicalThreat]) -> usize {
        if !threats.iter().any(|threat| threat.every_blow) {
            return 0;
        }
        let Some(unit) = g.units.get(&ours) else {
            return 0;
        };
        let mut blows: Vec<f64> = threats
            .iter()
            .filter(|threat| {
                threat
                    .strike_from
                    .iter()
                    .any(|from| *from != pos && g.wdist(*from, pos) <= 1 + REACH_MARGIN)
            })
            .filter_map(|threat| {
                g.theological_exchange_preview_with_defender_at(threat.rival, ours, pos)
            })
            .map(|(dealt, _, _)| dealt * BLOW_AT_THE_QUARTILE)
            .collect();
        blows.sort_by(|a, b| b.total_cmp(a));
        let heaviest: f64 = blows.iter().take(BLOWS_IN_A_TURN).sum();
        if heaviest >= f64::from(unit.hp) {
            blows.len()
        } else {
            0
        }
    }

    /// How many threats can strike `ours` standing on `pos` and win.
    pub(super) fn theological_threats_reaching(
        g: &Game,
        ours: u32,
        pos: Pos,
        threats: &[TheologicalThreat],
    ) -> usize {
        let winning = threats
            .iter()
            .filter(|threat| Self::theological_threat_reaches(g, ours, pos, threat))
            .count();
        if winning > 0 {
            return winning;
        }
        Self::every_blow_kills(g, ours, pos, threats)
    }

    /// `spreader-steps-out-of-the-apostle`, the flee. `None` when the gene is
    /// off or no threat reaches the unit's tile, so the caller goes on with
    /// its turn; `Some(false)` when nothing legal helps, which holds it. One
    /// step per call; the unit loop composes the rest of the turn.
    pub(super) fn religious_unit_evades_apostles(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        let threats = self.theological_threats(g, pid, uid);
        if threats.is_empty() {
            return None;
        }
        let current = g.units.get(&uid)?.pos;
        let current_reaching = Self::theological_threats_reaching(g, uid, current, &threats);
        if current_reaching == 0 {
            return None;
        }
        let kind = g.units[&uid].kind.as_str().to_string();
        // `last-charge-joins-the-army`: a kept charge flees instead.
        // A kept last charge stays kept while the unit can still fight or
        // scout; wounded and in a rival's reach, it is spent like any other.
        let kept = self.keeps_its_last_charge(g, pid, uid)
            && g.units[&uid].hp > super::apostle_army::RETREAT_FLOOR;
        if Self::last_charge_spends_here(g, pid, uid) && !kept {
            crate::think!(self.journal(), Faith, Detail,
                "{kind} {uid} spends its last charge before a rival Apostle reaches it";
                "spreader-steps-out-of-the-apostle: the charge removes the unit, so the \
                 fight and the 250 pressure a kill hands the winner never happen";
                current);
            return Some(g.apply(pid, &Action::Spread { unit: uid }).is_ok());
        }
        let home_distance = |position: Pos| {
            g.player_city_ids(pid)
                .into_iter()
                .map(|city| g.wdist(position, g.cities[&city].pos))
                .min()
                .unwrap_or(0)
        };
        let flight = g
            .nbrs(current)
            .into_iter()
            .filter(|position| g.can_move(uid, *position))
            .map(|position| {
                (
                    Self::theological_threats_reaching(g, uid, position, &threats),
                    home_distance(position),
                    position,
                )
            })
            .min();
        if let Some((0, _, position)) = flight {
            crate::think!(self.journal(), Faith, Detail,
                "{kind} {uid} steps out of a rival Apostle's reach";
                "spreader-steps-out-of-the-apostle: a rival of another faith would win the \
                 fight here, and a kill hands its faith 250 pressure across our cities";
                position);
            return Some(self.base.path_move(g, pid, uid, position));
        }
        // No safe neighbour: a safe tile this turn's movement still reaches,
        // one step along the route to the nearest. The next activation reads
        // the new tile, so the walk composes over the turn.
        let reachable_safe = g
            .reachable(uid)
            .into_iter()
            .filter(|position| Self::theological_tile_is_safe(g, uid, *position, &threats))
            .map(|position| {
                (
                    g.wdist(current, position),
                    home_distance(position),
                    position,
                )
            })
            .min();
        if let Some((_, _, goal)) = reachable_safe {
            if let Some(next) = g
                .route_step(uid, goal, 0)
                .filter(|next| g.can_move(uid, *next))
            {
                crate::think!(self.journal(), Faith, Detail,
                    "{kind} {uid} walks out of a rival Apostle's reach";
                    "spreader-steps-out-of-the-apostle: every neighbour is still reached, \
                     but this turn's movement reaches a tile none is";
                    goal);
                return Some(self.base.path_move(g, pid, uid, next));
            }
        }
        let can_spread = {
            let unit = &g.units[&uid];
            unit.charges > 0
                && unit.moves_left > 0.0
                && g.rules.units[unit.kind].religious_spread > 0.0
        };
        // A kept last charge is spent here too: with every tile in a rival's
        // reach the choice is the spread or the death, and a death hands the
        // rival faith 250 pressure around it.
        let useful = can_spread && Self::spread_helps_here(g, pid, uid);
        if useful {
            crate::think!(self.journal(), Faith, Detail,
                "{kind} {uid} spreads before a rival Apostle reaches it";
                "spreader-steps-out-of-the-apostle: every neighbour is in a rival's reach, \
                 so the charge is spent while the unit still lives";
                current);
            return Some(g.apply(pid, &Action::Spread { unit: uid }).is_ok());
        }
        match flight {
            Some((reaching, _, position)) if reaching < current_reaching => {
                Some(self.base.path_move(g, pid, uid, position))
            }
            // `corps-comes-home-alive`: see `preacher_runs_for_home`.
            _ => Some(self.preacher_runs_for_home(g, pid, uid)),
        }
    }

    /// Whether a charge spent from here lands on a city our faith wants:
    /// one of ours that needs support, or a foreign one at peace that does
    /// not follow it yet.
    fn spread_helps_here(g: &Game, pid: usize, uid: u32) -> bool {
        let unit = &g.units[&uid];
        let Some(religion) = unit
            .religion
            .clone()
            .or_else(|| g.players[pid].religion.clone())
        else {
            return false;
        };
        std::iter::once(unit.pos)
            .chain(g.nbrs(unit.pos))
            .filter_map(|position| g.city_at(position))
            .map(|city| &g.cities[&city])
            .any(|city| {
                Self::city_needs_religious_support(g, pid, city, &religion)
                    || (city.owner != pid
                        && !g.is_at_war(pid, city.owner)
                        && g.city_religion(city) != Some(religion.as_str()))
            })
    }

    /// `spreader-steps-out-of-the-apostle`, the march. `None` when the gene
    /// is off or no threat is in sight, so the caller walks as it always did;
    /// otherwise the route's next tile when no threat reaches it, else a safe
    /// neighbour that still closes on `target`, else a hold.
    pub(super) fn apostle_aware_step(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        target: Pos,
        stop_range: i32,
    ) -> Option<bool> {
        let threats = self.theological_threats(g, pid, uid);
        if threats.is_empty() {
            return None;
        }
        let current = g.units[&uid].pos;
        // The caller takes one step to a neighbour; with none of them in a
        // winning rival's reach it walks exactly as it always did.
        if g.nbrs(current)
            .into_iter()
            .all(|position| Self::theological_tile_is_safe(g, uid, position, &threats))
        {
            return None;
        }
        let here = g.wdist(current, target);
        if here <= stop_range {
            return Some(false);
        }
        let route = g
            .route_step(uid, target, stop_range)
            .filter(|next| g.can_move(uid, *next))
            .filter(|next| Self::theological_tile_is_safe(g, uid, *next, &threats));
        if let Some(next) = route {
            return Some(self.base.path_move(g, pid, uid, next));
        }
        let sidestep = g
            .nbrs(current)
            .into_iter()
            .filter(|position| g.can_move(uid, *position))
            .filter(|position| Self::theological_tile_is_safe(g, uid, *position, &threats))
            .map(|position| (g.wdist(position, target), position))
            .filter(|(distance, _)| *distance < here)
            .min();
        if let Some((_, position)) = sidestep {
            return Some(self.base.path_move(g, pid, uid, position));
        }
        let kind = g.units[&uid].kind.as_str().to_string();
        self.record_spreader_hold(g, uid);
        crate::think!(self.journal(), Faith, Detail,
            "{kind} {uid} holds outside a rival Apostle's reach";
            "spreader-steps-out-of-the-apostle: every step toward its target ends where \
             a rival of another faith would win the fight";
            target);
        Some(false)
    }
}

#[cfg(test)]
mod tests;
