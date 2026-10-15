//! `faith-war-before-the-flip`: a defensive war on a founding rival whose
//! spreaders are working our cities while our majority still holds out.
//!
//! The religion counter is named when the rival's race reading reaches 50%,
//! and the reading reaches 50% when *we* convert. Across the twelve Emperor
//! religious losses of 2026-10-14 (07:25Z on) the counter appeared on the
//! very turn our majority flipped to the racer's faith in five, eight to
//! thirty-three turns after it in four more, and earlier in only two; on 366
//! of the 408 peace turns after it our majority already followed the faith.
//! The window that mattered came first: from the racer's first spreader
//! within six tiles of our cities to our flip, 258 turns over the twelve
//! games (3 to 51 each, opening at turns 38-54), with our army at 0.7 times
//! the racer's steady power or better on 226 of them. The shipped answers
//! all wait for that flip: the religious interception for the racer's match
//! point, `match-point-interception-ignores-power` and
//! `holdout-keeps-the-faith-war` for us to be its last holdout, and the
//! faith counter for a staged siege at 1.5 times its power. At war, the
//! founder's spreaders reached our cities at 0.26 a turn against 1.47 at
//! peace (`advanced/holdout_faith_war.rs`), because only a war lets our
//! soldiers condemn them (`condemn_step`).
//!
//! Under the gene, when a founding rival's spreaders stand within six tiles
//! of our cities (`visible_home_spreaders`), our majority does not follow its
//! faith, and a city of ours already follows it or is being converted to it
//! (`our_cities_on_the_faith`), the seat opens a war on that founder with no
//! siege row, staging or edge: the war exists so the units at home condemn
//! the spreaders. Its gates are its own:
//!
//! - our power at [`FAITH_WAR_POWER_FLOOR`] times the founder's steady power;
//! - a rout guard: the founder under [`FAITH_WAR_ROUT_RATIO`] times our
//!   current power, and no city of ours threatened or falling;
//! - solvency: no negative income, and the treasury reserve
//!   (`war_is_affordable`, the "Holding the war on X | the treasury reserve"
//!   line) clear;
//! - fewer than [`FAITH_WAR_MAX_ENEMIES`] major wars already running.
//!
//! While the war runs, peace with the founder is neither offered nor accepted
//! as long as a city of ours is still being converted to its faith (unless a
//! city of ours is falling). Once no spreader of the faith has stood at our
//! cities for [`FAITH_WAR_CLEAR_TURNS`] standard turns and no city of ours is
//! converting, the seat offers it peace.
//!
//! `faith-war-before-the-flip-2` is the same war with two changes, after its
//! first three live fires (235353Z t44 Russia, 000225Z t109 Georgia, 001113Z
//! t134 Norway) cost a 107-turn war and three simultaneous wars, and two of
//! them came where we had founded Buddhism and our majority followed it --
//! the case the census did not measure (founders lost to Religion in 2 of 39
//! games): when we founded a faith and our majority follows it, the war fires
//! only once [`FAITH_WAR_2_FOUNDER_LEANING`] of our cities follow or are
//! converting to the rival's faith, or our own faith is one city from losing
//! its majority (`faith_war_founder_holds`). And after any peace with a
//! founder, whatever made it, the war is not reopened on that founder for
//! [`FAITH_WAR_2_COOLDOWN_TURNS`] standard turns unless a city of ours is
//! actually being converted to its faith (`faith_war_cooldown`): live
//! 011139Z opened on the Aztecs at 63, the "freeing the Domination army"
//! peace ended it at 111, and the faith war reopened at 120, nine turns
//! later. The non-founder trigger, and every other gate, is version 1's:
//! [`FAITH_WAR_2_MAX_WARS`] counts the war it would open and equals version
//! 1's limit, so the faith war makes two major wars at most.
//!
//! Off: unchanged.

use super::*;

/// The war opens at this share of the founder's steady power or better. The
/// last-holdout interception's floor
/// (`match_point_last_holdout::LAST_HOLDOUT_INTERCEPTION_FLOOR`): the war
/// asks no siege, only soldiers at home.
pub(crate) const FAITH_WAR_POWER_FLOOR: f64 = 0.7;

/// Rout guard: no faith war on a founder at this multiple of our current
/// power.
pub(crate) const FAITH_WAR_ROUT_RATIO: f64 = 1.5;

/// No faith war beside this many major wars already running.
pub(crate) const FAITH_WAR_MAX_ENEMIES: usize = 2;

/// `faith-war-before-the-flip-2`: with our own founded faith holding our
/// majority, this many cities of ours following or converting to the
/// rival's faith open the war.
pub(crate) const FAITH_WAR_2_FOUNDER_LEANING: usize = 2;

/// `faith-war-before-the-flip-2`: the gate holds once our major wars,
/// counting the one it would open, reach this many: version 1's limit
/// ([`FAITH_WAR_MAX_ENEMIES`] running, plus this one). A domination seat is
/// nearly always at war; a peace-only gate would mute the war in the
/// non-founder games it exists for (002058Z's non-founder fire at t48 took
/// Rouen and Guangzhou in that war).
pub(crate) const FAITH_WAR_2_MAX_WARS: usize = FAITH_WAR_MAX_ENEMIES + 1;

/// `faith-war-before-the-flip-2`: standard turns after any peace with a
/// founder before the faith war may reopen on it, unless a city of ours is
/// being converted to its faith.
pub(crate) const FAITH_WAR_2_COOLDOWN_TURNS: u32 = 20;

/// Standard turns with no spreader of the faith at our cities before the
/// seat offers the founder peace.
pub(crate) const FAITH_WAR_CLEAR_TURNS: u32 = 5;

/// Standard turns between two "held" lines about one founder for the same
/// gate.
const FAITH_WAR_NOTE_TURNS: u32 = 5;

/// The faith war the gene opened: the founder, the turn it opened, and the
/// last turn a spreader of its faith stood within six tiles of our cities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FaithWar {
    pub(crate) rival: usize,
    pub(crate) opened: u32,
    pub(crate) spreader_seen: u32,
}

impl AdvancedAi {
    /// Whether either version of the gene is on.
    fn faith_war_active(&self) -> bool {
        self.faith_war_before_the_flip || self.faith_war_before_the_flip_2
    }

    /// The journal's name for the version that is on.
    fn faith_war_tag(&self) -> &'static str {
        if self.faith_war_before_the_flip_2 {
            "faith-war-before-the-flip-2"
        } else {
            "faith-war-before-the-flip"
        }
    }

    /// `faith-war-before-the-flip-2`: whether our own founded faith holds our
    /// majority firmly enough to leave the war unopened: our majority follows
    /// the faith we founded, fewer than [`FAITH_WAR_2_FOUNDER_LEANING`] cities
    /// of ours follow or are converting to `rival`'s faith, and our faith
    /// would keep its majority after losing one more city. Our cities on our
    /// faith and our cities, or `None` when the war may open. Always `None`
    /// with the version off.
    pub(crate) fn faith_war_founder_holds(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> Option<(usize, usize)> {
        if !self.faith_war_before_the_flip_2 {
            return None;
        }
        let ours = g.players.get(pid)?.religion.as_deref()?;
        if !g.civ_follows_religion(pid, ours) {
            return None;
        }
        let theirs = Self::faith_war_faith(g, rival)?;
        if theirs == ours {
            return None;
        }
        let cities = g.player_city_ids(pid).len();
        let on_ours = g
            .player_city_ids(pid)
            .iter()
            .filter_map(|cid| g.cities.get(cid))
            .filter(|city| g.city_religion(city) == Some(ours))
            .count();
        let leaning = Self::our_cities_on_the_faith(g, pid, theirs);
        let one_from_flipping = on_ours.saturating_sub(1) * 2 <= cities;
        (leaning < FAITH_WAR_2_FOUNDER_LEANING && !one_from_flipping).then_some((on_ours, cities))
    }

    /// The faith `rival` founded, when it is a living major with one.
    fn faith_war_faith(g: &Game, rival: usize) -> Option<&str> {
        let player = g.players.get(rival)?;
        if !player.alive || player.is_minor || player.is_barbarian {
            return None;
        }
        player.religion.as_deref()
    }

    /// The trigger: `rival` founded a faith, our majority does not follow it,
    /// a city of ours follows it or is being converted to it, and a spreader
    /// of it stands within six tiles of our cities. The spreaders and our
    /// cities on the faith, or `None`.
    fn faith_war_trigger(&self, g: &Game, pid: usize, rival: usize) -> Option<(usize, usize)> {
        if rival == pid || g.is_at_war(pid, rival) || !g.has_met(pid, rival) {
            return None;
        }
        let faith = Self::faith_war_faith(g, rival)?;
        if g.civ_follows_religion(pid, faith) {
            return None;
        }
        let on_it = Self::our_cities_on_the_faith(g, pid, faith);
        if on_it == 0 {
            return None;
        }
        let spreaders = super::religious_interception::visible_home_spreaders(g, pid, rival, faith);
        if spreaders.is_empty() {
            return None;
        }
        Some((spreaders.len(), on_it))
    }

    /// The first gate that holds the faith war on `rival`, as a gate key and
    /// its journal reason; `None` when every gate is clear.
    fn faith_war_gate(&self, g: &Game, pid: usize, rival: usize) -> Option<(&'static str, String)> {
        if !self.campaign_target_legal(g, pid, rival) {
            return Some(("legal", "not a legal target".to_string()));
        }
        let enemies = self.one_war_enemies(g, pid).len();
        if self.faith_war_before_the_flip_2 && enemies + 1 >= FAITH_WAR_2_MAX_WARS {
            return Some((
                "wars",
                format!(
                    "we would be at war with {} majors counting this one",
                    enemies + 1
                ),
            ));
        }
        if enemies >= FAITH_WAR_MAX_ENEMIES {
            return Some((
                "enemies",
                format!("we are already at war with {enemies} majors"),
            ));
        }
        let ours = g.military_power(pid);
        let steady = self.steady_rival_power(g, rival).max(1.0);
        if ours < FAITH_WAR_POWER_FLOOR * steady {
            return Some((
                "power",
                format!(
                    "our {ours:.0} power against their steady {steady:.0} (needs {FAITH_WAR_POWER_FLOOR:.1}x)"
                ),
            ));
        }
        let theirs = g.military_power(rival);
        if theirs >= FAITH_WAR_ROUT_RATIO * ours {
            return Some((
                "rout",
                format!(
                    "their {theirs:.0} power is {FAITH_WAR_ROUT_RATIO:.1} times our {ours:.0} or more"
                ),
            ));
        }
        if self.threatened_city(g, pid).is_some() {
            return Some(("threatened", "a city of ours is threatened".to_string()));
        }
        if Self::our_cities_falling(g, pid) {
            return Some(("falling", "a city of ours is falling".to_string()));
        }
        if !self.war_is_affordable(g, pid) {
            let reserve = 100.0 + 25.0 * g.player_city_ids(pid).len() as f64;
            return Some((
                "treasury",
                format!(
                    "the treasury reserve: {:.0} gold of {reserve:.0} needed",
                    g.players[pid].gold
                ),
            ));
        }
        let per_turn = g.players[pid].gold_per_turn;
        if per_turn < 0.0 {
            return Some(("income", format!("our income is {per_turn:.1} gold a turn")));
        }
        None
    }

    /// The declaration the faith war opens with: the shipped opening when it
    /// is a war (a casus belli first, as `preferred_war_opening` ranks them),
    /// else a surprise war. The shipped opening answers a non-urgent rival
    /// with a denouncement and five turns of preparation; the window before
    /// the flip was 3 to 51 turns, and in a quarter of the losses no more
    /// than 7.
    fn faith_war_opening(&self, g: &Game, pid: usize, rival: usize) -> Option<Action> {
        if let Some(
            opening @ (Action::DeclareWar { .. } | Action::DeclareWarWithCasusBelli { .. }),
        ) = self.preferred_war_opening(g, pid, rival)
        {
            return Some(opening);
        }
        g.legal_actions_within(pid, ActionFamilies::DIPLOMACY)
            .into_iter()
            .find(|action| matches!(action, Action::DeclareWar { player } if *player == rival))
    }

    /// Journal a held faith war, at most once per founder every
    /// [`FAITH_WAR_NOTE_TURNS`] standard turns for the same gate.
    fn journal_faith_war_held(
        &mut self,
        g: &Game,
        rival: usize,
        gate: &'static str,
        reason: &str,
        trigger: (usize, usize),
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
        let (spreaders, on_it) = trigger;
        let tag = self.faith_war_tag();
        if gate == "founder" || gate == "cooldown" {
            think!(self.journal(), Military, Detail,
                   "Holding the faith war on {}", g.players[rival].civ;
                   "{reason}; {tag}");
            return;
        }
        think!(self.journal(), Military, Detail,
               "Holding the faith war on {}", g.players[rival].civ;
               "{tag}: {spreaders} spreader(s) at our cities and {on_it} of ours lean to its faith, but {reason}");
    }

    /// `faith-war-before-the-flip`: open the defensive war on the founder
    /// whose spreaders are working our cities. The turn's one declaration;
    /// `false` with the gene off or when no founder qualifies.
    pub(super) fn faith_war_before_the_flip_opening(&mut self, g: &mut Game, pid: usize) -> bool {
        if !self.faith_war_active()
            || !g.victory_conditions.religious
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || self.faith_war_before_the_flip_war.is_some()
        {
            return false;
        }
        let mut candidates: Vec<(usize, (usize, usize))> = g
            .players
            .iter()
            .map(|player| player.id)
            .filter_map(|rival| self.faith_war_trigger(g, pid, rival).map(|t| (rival, t)))
            .collect();
        // Most spreaders at our cities first, then most cities of ours on the
        // faith; the lower seat breaks a tie.
        candidates.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        for (rival, trigger) in candidates {
            if let Some(ago) = self.faith_war_cooldown(g, pid, rival) {
                let reason = format!("cooldown: peace {ago} turns ago");
                self.journal_faith_war_held(g, rival, "cooldown", &reason, trigger);
                continue;
            }
            if let Some((on_ours, cities)) = self.faith_war_founder_holds(g, pid, rival) {
                let (spreaders, on_it) = trigger;
                let reason = format!(
                    "founder majority holds ({on_ours} of {cities} ours); {on_it} of ours lean to its faith, {spreaders} spreader(s) at our cities"
                );
                self.journal_faith_war_held(g, rival, "founder", &reason, trigger);
                continue;
            }
            if let Some((gate, reason)) = self.faith_war_gate(g, pid, rival) {
                self.journal_faith_war_held(g, rival, gate, &reason, trigger);
                continue;
            }
            let Some(opening) = self.faith_war_opening(g, pid, rival) else {
                self.journal_faith_war_held(g, rival, "opening", "no legal declaration", trigger);
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
            let (spreaders, on_it) = trigger;
            let tag = self.faith_war_tag();
            think!(self.journal(), Military, Strategy,
                   "Opening a faith war on {}", g.players[rival].civ;
                   "{tag}: its spreaders stand at our cities ({spreaders} within 6 tiles) and {on_it} of ours lean to its faith while our majority holds out; defensive, no siege: our soldiers condemn its spreaders; {:.0} power against their steady {:.0}",
                   g.military_power(pid), self.steady_rival_power(g, rival));
            return true;
        }
        false
    }

    /// Keep the record of the gene's war current: drop it once the war is
    /// over, and note the turn a spreader of the founder's faith last stood
    /// at our cities.
    pub(super) fn faith_war_track(&mut self, g: &Game, pid: usize) {
        // `faith-war-before-the-flip-2`: the turn each major was last seen at
        // war with us, and the first turn of the peace that followed, however
        // the peace was made.
        if self.faith_war_before_the_flip_2 {
            for player in &g.players {
                if player.id == pid || !player.alive || player.is_minor || player.is_barbarian {
                    continue;
                }
                let rival = player.id;
                if g.is_at_war(pid, rival) {
                    self.faith_war_war_seen.insert(rival, g.turn);
                } else if let Some(&war_seen) = self.faith_war_war_seen.get(&rival) {
                    if self
                        .faith_war_peace_at
                        .get(&rival)
                        .is_none_or(|&peace| peace <= war_seen)
                    {
                        self.faith_war_peace_at.insert(rival, g.turn);
                    }
                }
            }
        }
        let Some(mut war) = self.faith_war_before_the_flip_war else {
            return;
        };
        if !g.is_at_war(pid, war.rival) {
            self.faith_war_before_the_flip_war = None;
            return;
        }
        if let Some(faith) = Self::faith_war_faith(g, war.rival) {
            if !super::religious_interception::visible_home_spreaders(g, pid, war.rival, faith)
                .is_empty()
            {
                war.spreader_seen = g.turn;
            }
        }
        self.faith_war_before_the_flip_war = Some(war);
    }

    /// `faith-war-before-the-flip-2`: the standard turns since the last peace
    /// with `rival`, while that is under [`FAITH_WAR_2_COOLDOWN_TURNS`] and no
    /// city of ours is being converted to its faith (a city already
    /// following it, or under a minor pressure, does not lift it). `None`
    /// when the war may reopen, and always with the version off.
    pub(crate) fn faith_war_cooldown(&self, g: &Game, pid: usize, rival: usize) -> Option<u32> {
        if !self.faith_war_before_the_flip_2 {
            return None;
        }
        let peace = *self.faith_war_peace_at.get(&rival)?;
        let ago = g.turn.saturating_sub(peace);
        (ago < g.standard_duration(FAITH_WAR_2_COOLDOWN_TURNS).max(1)
            && Self::faith_war_converting(g, pid, rival) == 0)
            .then_some(ago)
    }

    /// Cities of ours the founder's faith is converting that do not follow it
    /// yet.
    fn faith_war_converting(g: &Game, pid: usize, rival: usize) -> usize {
        let Some(faith) = Self::faith_war_faith(g, rival) else {
            return 0;
        };
        let following = g
            .player_city_ids(pid)
            .iter()
            .filter_map(|cid| g.cities.get(cid))
            .filter(|city| g.city_religion(city) == Some(faith))
            .count();
        Self::our_cities_on_the_faith(g, pid, faith).saturating_sub(following)
    }

    /// Whether the gene keeps the war with `rival`: it is the gene's war, a
    /// city of ours is still being converted to the founder's faith, and no
    /// city of ours is falling. `false` with the gene off.
    pub(crate) fn faith_war_keeps_the_war(&self, g: &Game, pid: usize, rival: usize) -> bool {
        self.faith_war_active()
            && self
                .faith_war_before_the_flip_war
                .is_some_and(|war| war.rival == rival)
            && g.is_at_war(pid, rival)
            && Self::faith_war_converting(g, pid, rival) > 0
            && !Self::our_cities_falling(g, pid)
    }

    /// Whether the gene's war with `rival` has done its work: no spreader of
    /// the faith at our cities for [`FAITH_WAR_CLEAR_TURNS`] standard turns,
    /// no city of ours converting, the host's minimum war term over, and the
    /// rival neither the plan's target nor an urgent victory threat.
    pub(crate) fn faith_war_exit_peace(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
        plan: &StrategicPlan,
    ) -> bool {
        let Some(war) = self.faith_war_before_the_flip_war else {
            return false;
        };
        self.faith_war_active()
            && war.rival == rival
            && g.is_at_war(pid, rival)
            && g.turn.saturating_sub(war.spreader_seen)
                >= g.standard_duration(FAITH_WAR_CLEAR_TURNS).max(1)
            && Self::faith_war_converting(g, pid, rival) == 0
            && g.peace_available_at(pid, rival).is_none()
            && plan.target_player != Some(rival)
            && !self.urgent_victory_threat(g, rival)
    }

    /// The journal line for a withheld peace offer or a refused one.
    pub(crate) fn journal_faith_war_keeps(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
        refused: bool,
    ) {
        if !self.journal().wants(crate::reasoning::Level::Decision) {
            return;
        }
        let converting = Self::faith_war_converting(g, pid, rival);
        let what = if refused {
            "Refusing peace with"
        } else {
            "Keeping the faith war with"
        };
        think!(self.journal(), Diplomacy, Decision,
               "{what} {}", g.players[rival].civ;
               "{}: {converting} of our cities are still being converted to its faith; at war our soldiers condemn its spreaders",
               self.faith_war_tag());
    }
}

#[cfg(test)]
mod tests;
