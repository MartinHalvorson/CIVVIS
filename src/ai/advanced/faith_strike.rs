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
//! `faith-strike-at-a-quiet-parity` moves one gate. The 0.7 edge is the faith
//! war's, set for a war that may run for a hundred turns; a Deity seat rarely
//! has it. In the 57 games of the Russia religion lane from 2026-11-09 14:00Z
//! on, bare-distance reach found two or more spreaders worth a strike, with a
//! legal declaration, on 255 rival-turns in 25 games. The edge left 27 of
//! them, in 5 games; a rival no stronger than us leaves 86, in 12. Power is
//! empire-wide, and it is a poor read of a short war: on the first such turn
//! for each rival, the median rival had none of its land units within six
//! tiles of our cities. So a rival over the edge may still be struck when its
//! steady power is at most [`FAITH_STRIKE_PARITY`] of ours and it has no
//! soldier or ship within [`FAITH_STRIKE_QUIET_TILES`] tiles of a city of
//! ours. Recon units do not count. Those same turns show the rival was at
//! war with us within 30 turns only about one time in five, so this war is
//! a new one, which is why parity stays a bar and every other gate holds.
//!
//! ## `faith-strike-spares-the-stages`
//!
//! **Why (live Deity Online Pericles, 2026-11-10).** A Culture seat wins with
//! Rock Bands, and a band cannot play in the land of a civ we are at war
//! with. Three strikes came in the band phase and each war ran about thirty
//! turns: civvis-20261110T232642Z struck India at t127, three turns after
//! Cold War (29 turns of war; 113 of the game's 208 band-turns fell inside
//! it, while India held 15-23 legal concert plots 5-10 tiles from our cities
//! whenever we were at peace, and the first eight bands walked 10+ turns to
//! the Mapuche overseas); 220038Z struck France at t133 (32+ turns, 51
//! band-turns, our cities 8 to 5); 204138Z struck Egypt at t130 (31 turns,
//! one city lost). The first already knew Cold War; the other two had the
//! band bank open (`culture_band_bank`, Nationalism known at t128 in both).
//!
//! **What the gene does.** On a seat whose live target is Culture, once Cold
//! War is known or the band bank is open, the strike is held ("Holding the
//! heretic strike on X | faith-strike-spares-the-stages: ..."), and a war the
//! strike opened is ended as soon as the host's minimum term allows: the
//! faith war's exit offers peace without waiting for the spreaders to leave,
//! and it no longer refuses peace while a city of ours is being converted.
//! A war the plan targets, an urgent victory threat, and every other war and
//! gate are unchanged. In those three games the seat already offered peace
//! from three to six turns into the war on other grounds and the host did not
//! sign for 29 turns or more, so holding the declaration is what saves the
//! stages; the exit only fills the turns when no other clause offers.
//!
//! ## `faith-strike-fears-the-tech-lead`
//!
//! **Why (live Deity Online Culture seats, 25 games from 2026-11-10 20:00Z).**
//! The only wars the seat declared were eight heretic strikes, at 1.04 to
//! 2.55 times the target's raw power; three of the seven targets declared
//! on us later and two of those wars were fatal. civvis-20261110T201752Z
//! struck Sumeria at t95 with Sumeria at 42 techs to our 19 on the board
//! (denounced t119, war t134, three cities lost including the capital);
//! 214026Z struck the Aztecs at t115 at 45 to our 29 (the Aztecs took our
//! last city at t161). The gates above read power only.
//!
//! **What the gene does.** On a seat whose live target is Culture, in every
//! phase of the game, the strike is held on a rival that leads us by
//! [`FAITH_STRIKE_TECH_LEAD`] techs or more ("Holding the heretic strike on
//! X | faith-strike-fears-the-tech-lead: it leads us by N techs"). Each side
//! is counted from the host's public count where it crossed (a live board
//! redacts a rival's tree), else from the tree on its seat.
//!
//! Not built: refusing a rival that declared war on us earlier in the game.
//! The live board rebuilds every frame and keeps no record of a war's
//! declarer once it ends, and the seat's own record
//! (`unchosen_war_since`) drops a war at peace, so it would need new state
//! that survives across frames.
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

/// `faith-strike-at-a-quiet-parity`: a rival over the faith war's edge may
/// still be struck at this share of our power, if its army is away.
pub(crate) const FAITH_STRIKE_PARITY: f64 = 1.0;

/// `faith-strike-at-a-quiet-parity`: "away" means no soldier or ship of the
/// rival within this many tiles of a city of ours.
const FAITH_STRIKE_QUIET_TILES: i32 = 6;

/// `faith-strike-fears-the-tech-lead`: a Culture seat strikes no rival that
/// leads it by this many techs or more.
pub(crate) const FAITH_STRIKE_TECH_LEAD: usize = 10;

/// A major's researched techs: the host's public count on a live board, which
/// redacts a rival's tree, else the tree on its seat.
fn faith_strike_techs(g: &Game, pid: usize) -> usize {
    g.observed_public_empire_stats
        .get(&pid)
        .and_then(|stats| stats.techs)
        .unwrap_or(g.players[pid].techs.len())
}

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

    /// `faith-strike-at-a-quiet-parity`: whether `rival` is no stronger than
    /// us and has no soldier or ship near our cities. Recon units roam
    /// everywhere and cannot take a city, so they do not break the quiet.
    fn faith_strike_quiet_parity(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
        steady: f64,
        ours: f64,
    ) -> bool {
        if !self.faith_strike_at_a_quiet_parity || steady > FAITH_STRIKE_PARITY * ours {
            return false;
        }
        let homes: Vec<Pos> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .collect();
        !g.units.values().any(|unit| {
            let spec = &g.rules.units[unit.kind];
            unit.owner == rival
                && spec.class == "military"
                && spec.promotion_class != "recon"
                && spec.domain.as_deref() != Some("air")
                && homes
                    .iter()
                    .any(|home| g.wdist(*home, unit.pos) <= FAITH_STRIKE_QUIET_TILES)
        })
    }

    /// `faith-strike-spares-the-stages`: whether a live Culture target's band
    /// economy has begun: Cold War is known, or the band bank is open
    /// (`culture_band_bank`). `false` with the gene off.
    fn faith_strike_stages_live(&self, g: &Game, pid: usize) -> bool {
        self.faith_strike_spares_the_stages
            && !self.lane_lost
            && self.active_victory_target(g) == Some(VictoryTarget::Culture)
            && (g.players[pid].civics.contains(&crate::name!("cold_war"))
                || self.culture_band_bank(g, pid) > 0.0)
    }

    /// `faith-strike-spares-the-stages`: whether the faith war on record with
    /// `rival` is a heretic strike's war that now ends as soon as the host
    /// signs: the strike opened it and the band economy has begun. `false`
    /// with the gene off.
    pub(crate) fn faith_strike_war_spares_the_stages(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> bool {
        self.faith_strike_spares_the_stages
            && self.faith_war_before_the_flip_war.is_some_and(|war| {
                war.rival == rival && self.faith_strike_last.get(&rival) == Some(&war.opened)
            })
            && self.faith_strike_stages_live(g, pid)
    }

    /// `faith-strike-fears-the-tech-lead`: the techs `rival` leads a live
    /// Culture target by, when that is [`FAITH_STRIKE_TECH_LEAD`] or more, with
    /// both counts. `None` with the gene off.
    fn faith_strike_tech_lead(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> Option<(usize, usize, usize)> {
        if !self.faith_strike_fears_the_tech_lead
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
        {
            return None;
        }
        let (theirs, ours) = (faith_strike_techs(g, rival), faith_strike_techs(g, pid));
        let lead = theirs.saturating_sub(ours);
        (lead >= FAITH_STRIKE_TECH_LEAD).then_some((lead, theirs, ours))
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
        if self.faith_strike_stages_live(g, pid) {
            let live = if g.players[pid].civics.contains(&crate::name!("cold_war")) {
                "Cold War is known"
            } else {
                "the band bank is open"
            };
            return Some((
                "stages",
                format!("{live}, and its land holds the stages our Rock Bands play"),
            ));
        }
        if let Some((lead, theirs, ours)) = self.faith_strike_tech_lead(g, pid, rival) {
            return Some((
                "tech_lead",
                format!("it leads us by {lead} techs ({theirs} to our {ours})"),
            ));
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
        if steady > faith_war_before_the_flip::FAITH_WAR_EDGE * ours
            && !self.faith_strike_quiet_parity(g, pid, rival, steady, ours)
        {
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
        let tag = match gate {
            "stages" => "faith-strike-spares-the-stages",
            "tech_lead" => "faith-strike-fears-the-tech-lead",
            _ => "faith-strike-wipes-the-spreaders",
        };
        think!(self.journal(), Military, Detail,
               "Holding the heretic strike on {}", g.players[rival].civ;
               "{tag}: {reason}");
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
            // `score-seat-holds-its-faith-strikes`: a Score seat strikes only
            // a faith at match point. `false` with the gene off. See
            // `advanced/score_seat_holds_its_faith_strikes.rs`.
            if self.score_seat_holds_the_strike(g, pid, rival) {
                continue;
            }
            // `faithless-seat-holds-its-faith-strikes`: a seat that founded
            // no religion strikes only a faith at match point. `false` with
            // the gene off. See
            // `advanced/faithless_seat_holds_its_faith_strikes.rs`.
            if self.faithless_seat_holds_the_strike(g, pid, rival) {
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
