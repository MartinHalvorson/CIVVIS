//! `faith-strike-wipes-the-spreaders`: hold the war on a rival faith until
//! the turn it can condemn several of its spreaders at once, then declare and
//! condemn them on that same turn.
//!
//! Condemning takes a war with the spreader's owner, a land soldier on its
//! tile and a move left, so the declaration decides how much the war buys
//! before the rival can react. The wars we already open for a faith do not aim
//! for that turn: `faith-war-before-the-flip` opens on the first spreader
//! within six tiles and condemns only what later wanders beside a soldier,
//! the religious interception condemns one spreader and only for a Domination
//! seat, and a Religion seat gets neither (`faith_war_lane`).
//!
//! **Live read, the 70 newest games of 2026-11-08/09.** At peace, a rival's
//! visible spreaders stood within a bare distance reach of distinct land
//! soldiers of ours, one soldier each, on 1,490 rival-turns for two or more
//! kills (50 of the logs) and 644 for three or more (41). Distance flatters
//! it. Asked at the first frame of the 80 replayable turns that also cleared
//! the value and power bars below and that the host would let us declare on,
//! the decider with this gene struck on 34, in 7 games, and condemned 90
//! units. The others held because too few spreaders could really be reached
//! (41: a third civ's closed borders, or a soldier arriving with no move
//! left to condemn with), or for power, legality, treasury or a third war
//! (28). A tool for the right moment rather than a standing war.
//!
//! Under the gene the seat scores each rival at peace whose founded faith has
//! spreaders within six tiles of our cities, and matches our soldiers to them
//! on a copy of the board with the war already declared, so a walk the
//! declaration blocks or a tile a soldier cannot safely stand on is not
//! counted. It strikes when the condemnations it can carry out this turn are
//! worth at least [`FAITH_STRIKE_MIN_VALUE`] and number at least
//! [`FAITH_STRIKE_MIN_KILLS`]; an Apostle is worth two, since it carries the
//! heaviest spread and the theological combat. Anything less waits, which is
//! the point: the same spreaders usually walk closer.
//!
//! The war is also not free, so the strike needs a reason and an edge. The
//! faith must matter (a city of ours follows or is being converted to it, the
//! seat is racing for a Religious Victory itself, or the faith holds half the
//! living majors) and the gates are the faith war's, always on here: at most
//! two major wars counting this one, the founder's steady power at most
//! [`faith_war_before_the_flip::FAITH_WAR_EDGE`] of ours, no city of ours
//! threatened or falling, the treasury reserve clear and income not negative.
//! A rival struck once is not struck again for
//! [`FAITH_STRIKE_COOLDOWN_TURNS`] standard turns.
//!
//! The war then runs as the gene's faith war (`faith_war_keeps_the_war`,
//! `faith_war_exit_peace`): it is no campaign front, and the seat offers
//! peace once the founder's spreaders have left and the host's term is over.
//!
//! Off: unchanged.

use super::*;
use std::collections::{BTreeMap, BTreeSet};

/// The condemnations a strike must be able to carry out this turn are worth
/// at least this. Three Missionaries, or an Apostle and a Missionary.
pub(crate) const FAITH_STRIKE_MIN_VALUE: u32 = 3;

/// A strike condemns at least this many units: one Apostle alone is not "a
/// few", however much it is worth.
pub(crate) const FAITH_STRIKE_MIN_KILLS: usize = 2;

/// Standard turns before the same rival may be struck again.
pub(crate) const FAITH_STRIKE_COOLDOWN_TURNS: u32 = 30;

/// The living majors a faith holds, as a percentage, at which it matters
/// whatever it is doing to us.
pub(crate) const FAITH_STRIKE_RACE: usize = 50;

/// A soldier keeps at least this much health to take part (`heretic_hunt`).
const FAITH_STRIKE_MIN_HP: i32 = 30;

/// Standard turns between two "held" lines about one rival for one gate.
const FAITH_STRIKE_NOTE_TURNS: u32 = 5;

/// A strike ready to carry out.
struct FaithStrike {
    rival: usize,
    opening: Action,
    /// The soldier, the spreader, and the walk and condemnation between them.
    kills: Vec<(u32, u32, Vec<Action>)>,
    value: u32,
}

/// What condemning a unit of this kind is worth to the strike.
fn spreader_value(kind: &str) -> u32 {
    if kind == "apostle" {
        2
    } else {
        1
    }
}

/// Kuhn's augmenting step: give spreader `target` a soldier, moving soldiers
/// off earlier spreaders only when those can be given another. An earlier
/// spreader never loses its soldier outright, so processing the Apostles first
/// keeps them matched.
fn assign_soldier(
    target: usize,
    reach: &[Vec<u32>],
    seen: &mut BTreeSet<u32>,
    holder: &mut BTreeMap<u32, usize>,
) -> bool {
    for soldier in &reach[target] {
        if !seen.insert(*soldier) {
            continue;
        }
        let free = match holder.get(soldier).copied() {
            None => true,
            Some(other) => assign_soldier(other, reach, seen, holder),
        };
        if free {
            holder.insert(*soldier, target);
            return true;
        }
    }
    false
}

impl AdvancedAi {
    /// `rival`'s spreaders of `faith` within six tiles of our cities that the
    /// seat may condemn, the most valuable first.
    fn faith_strike_targets(&self, g: &Game, pid: usize, rival: usize, faith: &str) -> Vec<u32> {
        let mut targets: Vec<u32> =
            super::religious_interception::visible_home_spreaders(g, pid, rival, faith)
                .into_iter()
                .filter(|uid| !self.preserve_competing_faith(g, pid, &g.units[uid]))
                .collect();
        targets.sort_by_key(|uid| {
            (
                std::cmp::Reverse(spreader_value(g.units[uid].kind.as_str())),
                *uid,
            )
        });
        targets
    }

    /// Whether `faith` is worth a war: a city of ours follows or is being
    /// converted to it, we are racing for a Religious Victory, or it holds
    /// [`FAITH_STRIKE_RACE`] percent of the living majors.
    fn faith_strike_matters(&self, g: &Game, pid: usize, rival: usize, faith: &str) -> bool {
        if Self::our_cities_on_the_faith(g, pid, faith) > 0
            || self.active_victory_target(g) == Some(VictoryTarget::Religion)
        {
            return true;
        }
        let (converted, living) = self.religious_conversion_tally(g, rival);
        100 * converted / living.max(1) >= FAITH_STRIKE_RACE
    }

    /// The first gate that holds a strike on `rival`, as a gate key and its
    /// journal reason; `None` when every gate is clear.
    fn faith_strike_gate(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> Option<(&'static str, String)> {
        if !self.campaign_target_legal(g, pid, rival) {
            return Some(("legal", "not a legal target".to_string()));
        }
        if let Some(last) = self.faith_strike_last.get(&rival) {
            let ago = g.turn.saturating_sub(*last);
            if ago < g.standard_duration(FAITH_STRIKE_COOLDOWN_TURNS).max(1) {
                return Some(("cooldown", format!("we struck it {ago} turns ago")));
            }
        }
        let wars = self.one_war_enemies(g, pid).len() + 1;
        if wars >= faith_war_before_the_flip::FAITH_WAR_2_MAX_WARS {
            return Some((
                "wars",
                format!("we would be at war with {wars} majors counting this one"),
            ));
        }
        let ours = g.military_power(pid);
        let steady = self.steady_rival_power(g, rival).max(1.0);
        if steady > faith_war_before_the_flip::FAITH_WAR_EDGE * ours {
            return Some((
                "edge",
                format!(
                    "their steady {steady:.0} power is over {:.1} times our {ours:.0}",
                    faith_war_before_the_flip::FAITH_WAR_EDGE
                ),
            ));
        }
        if self.threatened_city(g, pid).is_some() {
            return Some(("threatened", "a city of ours is threatened".to_string()));
        }
        if Self::our_cities_falling(g, pid) {
            return Some(("falling", "a city of ours is falling".to_string()));
        }
        if !self.treasury_can_carry_a_war(g, pid) {
            return Some(("treasury", "the treasury cannot carry a war".to_string()));
        }
        let per_turn = g.players[pid].gold_per_turn;
        if per_turn < 0.0 {
            return Some(("income", format!("our income is {per_turn:.1} gold a turn")));
        }
        None
    }

    /// The best strike on `rival` this turn, played out on a copy of the
    /// board with `opening` applied: one soldier per spreader, the Apostles
    /// matched first, every walk and condemnation replayed in order. `None`
    /// when what can actually be carried out falls short of the bars.
    fn faith_strike_plan(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
        opening: Action,
        targets: &[u32],
    ) -> Option<FaithStrike> {
        let mut declared = g.speculative_clone();
        declared.apply(pid, &opening).ok()?;
        let soldiers: Vec<u32> = declared
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let unit = &declared.units[uid];
                let spec = &declared.rules.units[unit.kind];
                spec.class == "military"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && unit.hp >= FAITH_STRIKE_MIN_HP
                    && unit.moves_left > 0.0
                    && !unit.acted
                    && unit.linked_to.is_none()
                    && !declared.is_embarked(unit)
            })
            .collect();
        let reach: Vec<Vec<u32>> = targets
            .iter()
            .map(|target| {
                let at = declared.units[target].pos;
                soldiers
                    .iter()
                    .copied()
                    .filter(|uid| {
                        let unit = &declared.units[uid];
                        declared.wdist(unit.pos, at)
                            <= declared.rules.units[unit.kind].moves.max(1.0) as i32
                            && Self::hunt_actions(&declared, pid, *uid, *target, at).is_some()
                    })
                    .collect()
            })
            .collect();
        let mut holder: BTreeMap<u32, usize> = BTreeMap::new();
        for target in 0..targets.len() {
            assign_soldier(target, &reach, &mut BTreeSet::new(), &mut holder);
        }
        let mut pairs: Vec<(usize, u32)> = holder.into_iter().map(|(s, t)| (t, s)).collect();
        pairs.sort_unstable();
        let mut kills = Vec::new();
        let mut value = 0;
        for (index, soldier) in pairs {
            let target = targets[index];
            let (Some(unit), true) = (
                declared.units.get(&target),
                declared.units.contains_key(&soldier),
            ) else {
                continue;
            };
            let (at, worth) = (unit.pos, spreader_value(unit.kind.as_str()));
            let Some(actions) = Self::hunt_actions(&declared, pid, soldier, target, at) else {
                continue;
            };
            if actions
                .iter()
                .all(|action| declared.apply(pid, action).is_ok())
            {
                value += worth;
                kills.push((soldier, target, actions));
            }
        }
        (kills.len() >= FAITH_STRIKE_MIN_KILLS && value >= FAITH_STRIKE_MIN_VALUE).then_some(
            FaithStrike {
                rival,
                opening,
                kills,
                value,
            },
        )
    }

    /// Journal a strike that is worth making but held, at most once per rival
    /// every [`FAITH_STRIKE_NOTE_TURNS`] standard turns for one gate.
    fn journal_faith_strike_held(
        &mut self,
        g: &Game,
        rival: usize,
        gate: &'static str,
        reason: &str,
    ) {
        if !self.journal().wants(crate::reasoning::Level::Detail) {
            return;
        }
        let window = g.standard_duration(FAITH_STRIKE_NOTE_TURNS).max(1);
        if self
            .faith_war_held_noted
            .get(&rival)
            .is_some_and(|(turn, noted)| *noted == gate && g.turn.saturating_sub(*turn) < window)
        {
            return;
        }
        self.faith_war_held_noted.insert(rival, (g.turn, gate));
        think!(self.journal(), Military, Detail,
               "Holding the heretic strike on {}", g.players[rival].civ;
               "faith-strike-wipes-the-spreaders: {reason}");
    }

    /// `faith-strike-wipes-the-spreaders`: declare on the rival whose
    /// spreaders we can condemn most profitably this turn and condemn them.
    /// The turn's one declaration; `false` with the gene off or no strike
    /// ready.
    pub(super) fn faith_strike_opening(&mut self, g: &mut Game, pid: usize) -> bool {
        if !self.faith_strike_wipes_the_spreaders
            || !g.victory_conditions.religious
            || self.faith_war_before_the_flip_war.is_some()
        {
            return false;
        }
        let rivals: Vec<usize> = g.players.iter().map(|player| player.id).collect();
        let mut best: Option<FaithStrike> = None;
        for rival in rivals {
            if rival == pid || g.is_at_war(pid, rival) || !g.has_met(pid, rival) {
                continue;
            }
            let Some(faith) = Self::faith_war_faith(g, rival).map(str::to_owned) else {
                continue;
            };
            let targets = self.faith_strike_targets(g, pid, rival, &faith);
            let ceiling: u32 = targets
                .iter()
                .map(|uid| spreader_value(g.units[uid].kind.as_str()))
                .sum();
            if targets.len() < FAITH_STRIKE_MIN_KILLS
                || ceiling < FAITH_STRIKE_MIN_VALUE
                || !self.faith_strike_matters(g, pid, rival, &faith)
            {
                continue;
            }
            if let Some((gate, reason)) = self.faith_strike_gate(g, pid, rival) {
                self.journal_faith_strike_held(g, rival, gate, &reason);
                continue;
            }
            let Some(opening) = self.faith_war_opening(g, pid, rival) else {
                self.journal_faith_strike_held(g, rival, "opening", "no legal declaration");
                continue;
            };
            let Some(strike) = self.faith_strike_plan(g, pid, rival, opening, &targets) else {
                let reason = format!(
                    "{} spreader(s) stand near our cities but too few can be reached and condemned this turn",
                    targets.len()
                );
                self.journal_faith_strike_held(g, rival, "reach", &reason);
                continue;
            };
            if best.as_ref().is_none_or(|held| {
                (strike.value, strike.kills.len()) > (held.value, held.kills.len())
            }) {
                best = Some(strike);
            }
        }
        let Some(strike) = best else {
            return false;
        };
        if g.apply(pid, &strike.opening).is_err() {
            return false;
        }
        let rival = strike.rival;
        self.faith_war_before_the_flip_war = Some(faith_war_before_the_flip::FaithWar {
            rival,
            opened: g.turn,
            spreader_seen: g.turn,
        });
        self.faith_strike_last.insert(rival, g.turn);
        let planned = strike.kills.len();
        let mut condemned = 0;
        for (_, _, actions) in &strike.kills {
            if actions.iter().all(|action| g.apply(pid, action).is_ok()) {
                condemned += 1;
            }
        }
        *g.players[pid]
            .counters
            .entry("faith_strike:opened".to_string())
            .or_insert(0) += 1;
        *g.players[pid]
            .counters
            .entry("faith_strike:condemned".to_string())
            .or_insert(0) += condemned;
        think!(self.journal(), Military, Strategy,
               "Striking {} to condemn its spreaders", g.players[rival].civ;
               "faith-strike-wipes-the-spreaders: war declared and {condemned} of {planned} spreaders condemned this turn (value {}); {:.0} power against their steady {:.0}; no siege, peace once its spreaders have left",
               strike.value, g.military_power(pid), self.steady_rival_power(g, rival));
        true
    }
}

#[cfg(test)]
mod tests;
