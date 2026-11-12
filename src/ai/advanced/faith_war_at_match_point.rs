//! `faith-war-at-match-point`: the defensive faith war on a founder whose
//! faith stands at match point against a seat with no faith of its own opens
//! whatever the power and army gates read.
//!
//! **Why (live Deity Online, 4-player Tiny Pangaea, Gran Colombia, no faith
//! founded, 2026-11-12).** Three of seven games ended on a rival Religious
//! Victory at turns 76, 88 and 90, and each journal held the only war the
//! seat had against the faith on power: civvis-20261112T091845Z read
//! "Holding off war with Canada | their faith is close to winning, but a war
//! at under 70% of their power cannot stop it; 116 power against their 448"
//! on turns 61-69, after "Holding the faith war on Canada |
//! faith-war-before-the-flip-2: ... our 132 power against their steady 450
//! (needs 0.7x)" at 54 and "faith-war-on-a-small-world: ... a city of ours is
//! threatened" at 58; civvis-20261112T094511Z read the same counter-floor
//! line on Russia at 239 against 367 on turns 67-68; and in
//! civvis-20261112T083948Z the small-world faith war opened only at 76. A
//! Religious Victory needs the faith to hold every living major. Once it
//! holds every other one, our own flip ends the game: a war that lets our
//! soldiers condemn its spreaders at our cities is the one defence a seat
//! with no faith has, and refusing it is a loss, at any power.
//!
//! **What the gene does.** Riding `faith-war-before-the-flip-2`, for a seat
//! that founded no religion, when a founder's faith stands at match point
//! against us (`faith_war_match_point`: our majority does not follow it and
//! every other living major does, by `we_are_the_last_holdout`, or the
//! religion lane reads its match point, `faith_at_match_point`) and a city of
//! ours follows it or is being converted to it (`our_cities_on_the_faith`),
//! the faith war on that founder opens on any victory lane, without the
//! family's power floor, edge, rout ratio, threatened-city and falling-city
//! holds, major-war limit, treasury reserve and income gate, and without the
//! counter war's power floor or the urgent denial's edge. Each waived hold is
//! journalled. It keeps three holds: a target the host would refuse
//! (`campaign_target_legal`; a declared friend or ally is named with the turn
//! its pact ends, since neither can be ended early), version 2's cooldown
//! after a peace with the founder (`faith_war_cooldown`), and a war already
//! running. When the host allows no declaration on the founder yet (Canada
//! takes no Surprise War, and a Formal War needs a denouncement five turns
//! old), the seat denounces it so the Formal War can follow. The war is the
//! family's own record (`faith_war_before_the_flip_war`): no siege row is
//! written for it and `faith-war-is-no-front` keeps it off the fronts.
//!
//! The war is kept (`faith_war_keeps_the_war`) while the faith stands at
//! match point and a spreader of it stood at our cities within
//! `FAITH_WAR_CLEAR_TURNS` standard turns, a city of ours falling included;
//! after that the family's exit (`faith_war_exit_peace`) ends it. The
//! declaration desk's counter-war floor (`counter_floor_holds`) no longer
//! holds a declaration on such a founder.
//!
//! Replay note: in the three games above the seat was never the faith's last
//! holdout while it was at peace with the founder. In 091845Z our majority
//! followed Catholicism from turn 61 (three of five cities) while the fourth
//! major still held out; in 094511Z every city of ours followed Orthodoxy
//! from turn 50; in 083948Z we held out alone from turn 86, at war with the
//! founder since 77. The gene answers the case it names, not those games.
//!
//! Off: unchanged.

use super::faith_war_before_the_flip::{FaithWar, FAITH_WAR_CLEAR_TURNS, FAITH_WAR_NOTE_TURNS};
use super::*;

/// `faith-war-at-match-point`: a founder's faith at match point against us.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MatchPoint {
    /// Living majors, us included.
    pub(crate) majors: usize,
    /// Living majors following the founder's faith, as a percentage.
    pub(crate) race: usize,
    /// Our cities following the faith or being converted to it.
    pub(crate) leaning: usize,
    /// Our cities.
    pub(crate) cities: usize,
}

impl AdvancedAi {
    /// `faith-war-at-match-point`: `rival`'s founded faith at match point
    /// against us. The gene and version 2 are on, a Religious Victory is in
    /// play, we founded no faith, our majority does not follow `rival`'s,
    /// every other living major does (or the religion lane reads its match
    /// point), and a city of ours follows it or is being converted to it.
    /// `None` otherwise, and always with the gene off.
    pub(crate) fn faith_war_match_point(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> Option<MatchPoint> {
        if !self.faith_war_at_match_point
            || !self.faith_war_before_the_flip_2
            || !g.victory_conditions.religious
            || rival == pid
        {
            return None;
        }
        // A seat with a faith of its own answers with it.
        if g.players.get(pid)?.religion.is_some() {
            return None;
        }
        let faith = Self::faith_war_faith(g, rival)?;
        if g.civ_follows_religion(pid, faith) {
            return None;
        }
        if !Self::we_are_the_last_holdout(g, pid, rival) && !self.faith_at_match_point(g, rival) {
            return None;
        }
        let leaning = Self::our_cities_on_the_faith(g, pid, faith);
        if leaning == 0 {
            return None;
        }
        let (converted, majors) = self.religious_conversion_tally(g, rival);
        Some(MatchPoint {
            majors,
            race: 100 * converted / majors.max(1),
            leaning,
            cities: g.player_city_ids(pid).len(),
        })
    }

    /// The reading, on a met founder we are not at war with, and the
    /// spreaders of its faith within six tiles of our cities.
    fn match_point_trigger(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> Option<(MatchPoint, usize)> {
        if g.is_at_war(pid, rival) || !g.has_met(pid, rival) {
            return None;
        }
        let reading = self.faith_war_match_point(g, pid, rival)?;
        let faith = Self::faith_war_faith(g, rival)?;
        let spreaders =
            super::religious_interception::visible_home_spreaders(g, pid, rival, faith).len();
        Some((reading, spreaders))
    }

    /// `faith-war-at-match-point`: whether the counter war's power floor
    /// (`counter_war_hopeless`) holds the declaration desk's war on `target`
    /// on an `urgent_denial` clock. Never on a founder whose faith stands at
    /// match point against us under the gene; unchanged with it off.
    pub(super) fn counter_floor_holds(
        &self,
        g: &Game,
        pid: usize,
        target: usize,
        urgent_denial: bool,
    ) -> bool {
        urgent_denial
            && self.counter_war_hopeless(g, pid, target)
            && self.faith_war_match_point(g, pid, target).is_none()
    }

    /// `faith-war-at-match-point`: the reading that keeps the faith war on
    /// record with `rival`, and the standard turns since a spreader of its
    /// faith last stood at our cities, while that is under
    /// [`FAITH_WAR_CLEAR_TURNS`]. `None` otherwise, and always with the gene
    /// off.
    pub(crate) fn faith_war_match_point_keeps(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> Option<(MatchPoint, u32)> {
        let war = self
            .faith_war_before_the_flip_war
            .filter(|war| war.rival == rival)?;
        let reading = self.faith_war_match_point(g, pid, rival)?;
        let since = g.turn.saturating_sub(war.spreader_seen);
        (since < g.standard_duration(FAITH_WAR_CLEAR_TURNS).max(1)).then_some((reading, since))
    }

    /// The holds the war on `rival` opens past: every family gate after
    /// legality (`faith_war_holds`), the counter war's power floor and the
    /// urgent denial's edge, each as a key and its reason.
    pub(super) fn match_point_waived_holds(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> Vec<(&'static str, String)> {
        let mut waived = self.faith_war_holds(g, pid, rival, false);
        let ours = g.military_power(pid);
        let theirs = g.military_power(rival);
        if self.counter_war_hopeless(g, pid, rival) {
            waived.push((
                "counter-floor",
                format!(
                    "the counter war's floor: our {ours:.0} power is under {:.1} times their {theirs:.0}",
                    one_war::COUNTER_WAR_POWER_FLOOR
                ),
            ));
        }
        if self.urgent_victory_threat(g, rival) && !self.urgent_denial_has_the_edge(g, pid, rival) {
            waived.push((
                "urgent-edge",
                format!(
                    "the urgent denial's edge: our {ours:.0} power is under {:.1} times their steady {:.0}",
                    one_war::DECLARATION_EDGE_RATIO,
                    self.steady_rival_power(g, rival)
                ),
            ));
        }
        waived
    }

    /// Why `campaign_target_legal` refuses `rival`: a declared friendship or
    /// an alliance, with the turn it ends (neither can be ended early, so
    /// the war waits for it), or the plain refusal.
    fn match_point_illegal_reason(&self, g: &Game, pid: usize, rival: usize) -> String {
        if let Some(alliance) = g.alliance_with(pid, rival) {
            return format!(
                "an ally until turn {}: the host refuses the war while the alliance runs, and we cannot end it early",
                alliance.ends
            );
        }
        if g.are_friends(pid, rival) {
            if let Some(until) = g.players[pid].friends_until.get(&rival) {
                return format!(
                    "a declared friend until turn {until}: the host refuses the war while the friendship runs, and we cannot end it early, only leave it unrenewed"
                );
            }
        }
        "not a legal target".to_string()
    }

    /// Journal a held match-point war, at most once per founder every
    /// `FAITH_WAR_NOTE_TURNS` standard turns for the same gate.
    fn journal_match_point_held(
        &mut self,
        g: &Game,
        rival: usize,
        gate: &'static str,
        reading: MatchPoint,
        reason: &str,
    ) {
        if !self.journal().wants(crate::reasoning::Level::Detail) {
            return;
        }
        let window = g.standard_duration(FAITH_WAR_NOTE_TURNS).max(1);
        if self
            .faith_war_held_noted
            .get(&rival)
            .is_some_and(|(turn, noted)| *noted == gate && g.turn.saturating_sub(*turn) < window)
        {
            return;
        }
        self.faith_war_held_noted.insert(rival, (g.turn, gate));
        think!(self.journal(), Military, Detail,
               "Holding the faith war on {}", g.players[rival].civ;
               "faith-war-at-match-point: its faith stands at match point ({}% of {} living majors; only our flip is missing) and {} of our {} cities lean to it, but {reason}",
               reading.race, reading.majors, reading.leaning, reading.cities);
    }

    /// When the host allows no war on `rival` yet, denounce it, so a Formal
    /// War is legal once the denouncement's five turns have passed. `true`
    /// when the denouncement was made.
    fn match_point_denounce(
        &mut self,
        g: &mut Game,
        pid: usize,
        rival: usize,
        reading: MatchPoint,
    ) -> bool {
        let Some(denounce) = g
            .legal_actions_within(pid, ActionFamilies::DIPLOMACY)
            .into_iter()
            .find(|action| matches!(action, Action::Denounce { player } if *player == rival))
        else {
            return false;
        };
        if g.apply(pid, &denounce).is_err() {
            return false;
        }
        think!(self.journal(), Military, Strategy,
               "Denouncing {} for the faith war", g.players[rival].civ;
               "faith-war-at-match-point: its faith stands at match point ({}% of {} living majors) and {} of our {} cities lean to it, but the host allows no war on it yet; a Formal War opens once the denouncement's five turns have passed",
               reading.race, reading.majors, reading.leaning, reading.cities);
        true
    }

    /// `faith-war-at-match-point`: open the defensive war on a founder whose
    /// faith stands at match point against us. The turn's one declaration (a
    /// denouncement when the host allows no war yet); `false` with the gene
    /// off or when no founder qualifies.
    pub(super) fn faith_war_at_match_point_opening(&mut self, g: &mut Game, pid: usize) -> bool {
        if !self.faith_war_at_match_point {
            return false;
        }
        let mut candidates: Vec<(usize, MatchPoint, usize)> = g
            .players
            .iter()
            .map(|player| player.id)
            .filter_map(|rival| {
                self.match_point_trigger(g, pid, rival)
                    .map(|(reading, spreaders)| (rival, reading, spreaders))
            })
            .collect();
        // Most cities of ours on the faith first, then most spreaders at our
        // cities; the lower seat breaks a tie.
        candidates.sort_by(|a, b| {
            b.1.leaning
                .cmp(&a.1.leaning)
                .then(b.2.cmp(&a.2))
                .then(a.0.cmp(&b.0))
        });
        for (rival, reading, spreaders) in candidates {
            if !self.campaign_target_legal(g, pid, rival) {
                let reason = self.match_point_illegal_reason(g, pid, rival);
                self.journal_match_point_held(g, rival, "match-point-legal", reading, &reason);
                continue;
            }
            if let Some(ago) = self.faith_war_cooldown(g, pid, rival) {
                let reason = format!(
                    "the cooldown after a peace: peace {ago} turn(s) ago and no city of ours converting"
                );
                self.journal_match_point_held(g, rival, "match-point-cooldown", reading, &reason);
                continue;
            }
            let waived = self.match_point_waived_holds(g, pid, rival);
            let Some(opening) = self.faith_war_opening(g, pid, rival) else {
                if self.match_point_denounce(g, pid, rival, reading) {
                    return true;
                }
                self.journal_match_point_held(
                    g,
                    rival,
                    "match-point-opening",
                    reading,
                    "the host allows no declaration on it yet",
                );
                continue;
            };
            if g.apply(pid, &opening).is_err() {
                continue;
            }
            self.faith_war_before_the_flip_war = Some(FaithWar {
                rival,
                opened: g.turn,
                spreader_seen: g.turn,
            });
            let civ = g.players[rival].civ.clone();
            let ours = g.military_power(pid);
            let steady = self.steady_rival_power(g, rival);
            think!(self.journal(), Military, Strategy,
                   "Opening the faith war on {civ}";
                   "faith-war-at-match-point: its faith stands at match point ({}% of {} living majors; only our flip is missing), {} of our {} cities lean to it and {spreaders} spreader(s) stand at our cities; {} hold(s) waived; defensive, no siege: our soldiers condemn its spreaders; {ours:.0} power against their steady {steady:.0}",
                   reading.race, reading.majors, reading.leaning, reading.cities, waived.len());
            for (gate, reason) in &waived {
                think!(self.journal(), Military, Detail,
                       "Waiving the {gate} hold on the faith war on {civ}";
                       "faith-war-at-match-point: {reason}; waived because our flip ends the game and the war asks no siege, only soldiers at home");
            }
            return true;
        }
        false
    }

    /// The journal line for a withheld peace offer or a refused one while
    /// [`Self::faith_war_match_point_keeps`] holds; `false` when it does not.
    pub(crate) fn journal_match_point_keeps(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
        refused: bool,
    ) -> bool {
        let Some((reading, since)) = self.faith_war_match_point_keeps(g, pid, rival) else {
            return false;
        };
        if !self.journal().wants(crate::reasoning::Level::Decision) {
            return true;
        }
        let what = if refused {
            "Refusing peace with"
        } else {
            "Keeping the faith war with"
        };
        think!(self.journal(), Diplomacy, Decision,
               "{what} {}", g.players[rival].civ;
               "faith-war-at-match-point: its faith stands at match point ({}% of {} living majors; only our flip is missing) and {} of our {} cities lean to it; a spreader of its faith stood at our cities {since} turn(s) ago; at war our soldiers condemn its spreaders",
               reading.race, reading.majors, reading.leaning, reading.cities);
        true
    }
}

#[cfg(test)]
mod tests;
