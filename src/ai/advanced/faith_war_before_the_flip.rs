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
//! ## `faith-war-on-a-small-world`
//!
//! **Why (live Deity Online, 4-player Pangaea, 2026-11-11).** A Religious
//! Victory needs every living major's majority, so on four majors it needs
//! three conversions, and it is the top rival clock there: in the ladder's
//! Tiny Immortal games that reached a victory, a rival won on Religion 80
//! times (median turn 117, earliest 54), ahead of Culture 76 and Science 68.
//! The first live Deity Pangaea game, civvis-20261111T040245Z, read "Countering
//! Netherlands | its religion race reads 50%" from t51, and at t52 "Holding
//! the faith war on Netherlands | faith-war-before-the-flip-2: 2 spreader(s)
//! at our cities and 1 of ours lean to its faith, but our 199 power against
//! their steady 398 (needs 0.7x)". Both our cities followed Protestantism by
//! t60, the Inca fell at t83 and the Netherlands won on Religion; we were
//! never at war with them. The power gates were measured on 6-8 major worlds,
//! where a faith war on a stronger founder cost about two cities a game; on
//! four majors the alternative to that risk is the game ending, because once
//! our majority goes the founder needs one more rival.
//!
//! **What the gene does.** Riding version 2, on a world of at most
//! [`SMALL_WORLD_MAJORS`] living majors (counted, not read off the map
//! size), when the founder's faith already holds the majority of a living
//! major other than us and the founder, or its race (living majors following
//! it, as a percentage) reads [`SMALL_WORLD_RACE`] or more, or half our own
//! cities already follow or are converting to it
//! ([`SMALL_WORLD_OWN_SHARE`]: we are the next major to flip)
//! (`faith_war_small_world`), and the shipped trigger is met, the war opens
//! without the power floor, the edge (`faith-war-needs-the-edge`), the rout
//! ratio, the treasury reserve and the income gate. The legality of the
//! target (a declared friend or ally is never declared on; the hold names
//! it), the threatened-city and falling-city holds, the major-war limit,
//! version 2's founder hold and its cooldown after a peace all stay, and the
//! declaration is the shipped `faith_war_opening`. Then the peace desk keeps
//! the war on record (`faith_war_keeps_the_war`) while that reading holds and
//! a spreader of the faith stood at our cities within
//! [`FAITH_WAR_CLEAR_TURNS`] standard turns, so the shipped exit (that many
//! clear turns and no city converting) still ends it, as does a city of ours
//! falling. Nothing changes on a world of five or more living majors.
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

/// `faith-war-needs-the-edge`: the faith war opens only on a founder whose
/// steady power is at most this share of ours. Over the first faith war of
/// each of 54 retained live games (2026-10-25), a founder under 0.7 times our
/// power cost 0.89 cities a game and no collapse in 9; at 0.7-1.0 times, 2.21
/// and 5 collapses (four or more cities lost) in 19; at 1.0 or more, 2.00 and
/// 6 in 26. Live Immortal civvis-20261025T170120Z opened one on Hungary at
/// turn 102 (328 against 273) beside a war on Gaul at our power and fell to
/// one city; Emperor 055158Z opened one on Greece at 434 against 427 and lost
/// eight cities.
pub(crate) const FAITH_WAR_EDGE: f64 = 0.7;

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

/// `faith-war-on-a-small-world`: a world of at most this many living majors,
/// where a Religious Victory needs three conversions or fewer.
pub(crate) const SMALL_WORLD_MAJORS: usize = 4;

/// `faith-war-on-a-small-world`: the founder's race reading (living majors
/// following its faith, as a percentage) at which the war opens without the
/// edge: on four majors, the founder and one more.
pub(crate) const SMALL_WORLD_RACE: usize = 50;

/// `faith-war-on-a-small-world`: the share of our own cities, as a
/// percentage, following or converting to the founder's faith at which we
/// are the next major to flip and the war opens without the edge although no
/// other major follows the faith yet. Live Deity Gran Colombia
/// `civvis-20261111T181048Z` (four majors): turns 68-69 read "7 spreader(s)
/// at our cities and 7 of ours lean to its faith" of ten cities, the race
/// still 25%, and the war was held on the power floor at 402 against 634;
/// we flipped about turn 70 and Byzantium won on turn 81.
/// `civvis-20261111T182106Z` lost the same way to France on turn 90.
pub(crate) const SMALL_WORLD_OWN_SHARE: usize = 50;

/// `faith-war-on-a-small-world`: the cities we must hold for our own flip to
/// open the war ([`SMALL_WORLD_OWN_SHARE`]), and how many of them must be on
/// the founder's faith. One leaning city of two is half an empire that
/// cannot fight: live Deity Gran Colombia `civvis-20261111T194319Z` opened
/// on Arabia at turn 46 with two cities, one leaning, at 205 power against
/// 489, and held one city at turn 110.
pub(crate) const SMALL_WORLD_OWN_CITIES: usize = 3;

/// See [`SMALL_WORLD_OWN_CITIES`].
pub(crate) const SMALL_WORLD_OWN_LEANING: usize = 2;

/// `faith-war-on-a-small-world`: our power, as a share of the founder's
/// steady power, under which our own flip does not open the war. The
/// Byzantium game that motivated the rule stood at 0.63 (402 against 634);
/// the Arabia game above at 0.42.
pub(crate) const SMALL_WORLD_OWN_POWER: f64 = 0.5;

/// `faith-war-on-a-small-world`: a founder's race on a small world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SmallWorldRace {
    /// Living majors, us included.
    pub(crate) majors: usize,
    /// Living majors following the founder's faith, as a percentage.
    pub(crate) race: usize,
    /// Living majors other than us and the founder following its faith.
    pub(crate) others: usize,
}

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

    /// Whether a faith war on record is run by this module: either version
    /// of the gene, or `faith-strike-wipes-the-spreaders`, whose war is the
    /// same defensive war with a different opening.
    fn faith_war_is_run(&self) -> bool {
        self.faith_war_active() || self.faith_strike_wipes_the_spreaders
    }

    /// The journal's name for the gene running the war.
    fn faith_war_tag(&self) -> &'static str {
        if self.faith_war_before_the_flip_2 {
            "faith-war-before-the-flip-2"
        } else if self.faith_war_before_the_flip {
            "faith-war-before-the-flip"
        } else {
            "faith-strike-wipes-the-spreaders"
        }
    }

    /// The journal's name for the gene deciding the faith war on `rival`:
    /// `faith-war-on-a-small-world` while its reading holds, else
    /// [`Self::faith_war_tag`].
    fn faith_war_tag_for(&self, g: &Game, pid: usize, rival: usize) -> &'static str {
        if self.faith_war_small_world(g, pid, rival).is_some() {
            "faith-war-on-a-small-world"
        } else {
            self.faith_war_tag()
        }
    }

    /// `faith-war-on-a-small-world`: `rival`'s race on a world of at most
    /// [`SMALL_WORLD_MAJORS`] living majors, when its founded faith holds the
    /// majority of a living major other than us and `rival`, its race reads
    /// [`SMALL_WORLD_RACE`] or more, or [`SMALL_WORLD_OWN_SHARE`] percent of
    /// our own cities follow or are converting to it (of at least
    /// [`SMALL_WORLD_OWN_CITIES`] cities, at least
    /// [`SMALL_WORLD_OWN_LEANING`] of them, and at
    /// [`SMALL_WORLD_OWN_POWER`] of its steady power or more). `None` on a
    /// larger world, and always with the gene or version 2 off.
    pub(crate) fn faith_war_small_world(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> Option<SmallWorldRace> {
        if !self.faith_war_on_a_small_world || !self.faith_war_before_the_flip_2 {
            return None;
        }
        let faith = Self::faith_war_faith(g, rival)?;
        let living: Vec<usize> = g
            .players
            .iter()
            .filter(|player| player.alive && !player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .collect();
        let majors = living.len();
        if majors > SMALL_WORLD_MAJORS {
            return None;
        }
        let others = living
            .iter()
            .filter(|other| {
                **other != pid && **other != rival && g.civ_follows_religion(**other, faith)
            })
            .count();
        let (converted, _) = self.religious_conversion_tally(g, rival);
        let race = 100 * converted / majors.max(1);
        // We are the next major to flip: half our cities are on the faith.
        // That reading asks for an empire that can fight the war: see
        // `SMALL_WORLD_OWN_CITIES` and `SMALL_WORLD_OWN_POWER`.
        let cities = g.player_city_ids(pid).len();
        let leaning = Self::our_cities_on_the_faith(g, pid, faith);
        let next = cities >= SMALL_WORLD_OWN_CITIES
            && leaning >= SMALL_WORLD_OWN_LEANING
            && 100 * leaning >= SMALL_WORLD_OWN_SHARE * cities
            && g.military_power(pid)
                >= SMALL_WORLD_OWN_POWER * self.steady_rival_power(g, rival).max(1.0);
        (others > 0 || race >= SMALL_WORLD_RACE || next).then_some(SmallWorldRace {
            majors,
            race,
            others,
        })
    }

    /// `faith-war-on-a-small-world`: the race reading that keeps the faith war
    /// on record with `rival`, and the standard turns since a spreader of its
    /// faith last stood at our cities, while that is under
    /// [`FAITH_WAR_CLEAR_TURNS`] (the shipped exit's clock). `None` otherwise,
    /// and always with the gene off.
    fn faith_war_small_world_keeps(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> Option<(SmallWorldRace, u32)> {
        let war = self
            .faith_war_before_the_flip_war
            .filter(|war| war.rival == rival)?;
        let reading = self.faith_war_small_world(g, pid, rival)?;
        let since = g.turn.saturating_sub(war.spreader_seen);
        (since < g.standard_duration(FAITH_WAR_CLEAR_TURNS).max(1)).then_some((reading, since))
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
    pub(super) fn faith_war_faith(g: &Game, rival: usize) -> Option<&str> {
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
    ///
    /// `faith-war-on-a-small-world` lifts the power floor, the edge, the
    /// rout ratio, the treasury reserve and the income gate while its
    /// reading holds (`faith_war_small_world`), and names a declared
    /// friendship or alliance as the legality that holds the war.
    fn faith_war_gate(&self, g: &Game, pid: usize, rival: usize) -> Option<(&'static str, String)> {
        let small_world = self.faith_war_small_world(g, pid, rival).is_some();
        if !self.campaign_target_legal(g, pid, rival) {
            if small_world && (g.are_friends(pid, rival) || g.alliance_with(pid, rival).is_some()) {
                return Some((
                    "legal",
                    "a declared friend or ally: no war on it while the pact runs".to_string(),
                ));
            }
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
        if !small_world && ours < FAITH_WAR_POWER_FLOOR * steady {
            return Some((
                "power",
                format!(
                    "our {ours:.0} power against their steady {steady:.0} (needs {FAITH_WAR_POWER_FLOOR:.1}x)"
                ),
            ));
        }
        // `faith-war-needs-the-edge`: see `FAITH_WAR_EDGE`.
        if !small_world && self.faith_war_needs_the_edge && steady > FAITH_WAR_EDGE * ours {
            return Some((
                "edge",
                format!(
                    "their steady {steady:.0} power is over {FAITH_WAR_EDGE:.1} times our {ours:.0}"
                ),
            ));
        }
        let theirs = g.military_power(rival);
        if !small_world && theirs >= FAITH_WAR_ROUT_RATIO * ours {
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
        if !small_world && !self.war_is_affordable(g, pid) {
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
        if !small_world && per_turn < 0.0 {
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
    pub(super) fn faith_war_opening(&self, g: &Game, pid: usize, rival: usize) -> Option<Action> {
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
        pid: usize,
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
        let tag = self.faith_war_tag_for(g, pid, rival);
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
            // `faith-war-defends-the-diplomat` opens the Diplomacy lane.
            || !self.faith_war_lane(g)
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
                self.journal_faith_war_held(g, pid, rival, "cooldown", &reason, trigger);
                continue;
            }
            if let Some((on_ours, cities)) = self.faith_war_founder_holds(g, pid, rival) {
                let (spreaders, on_it) = trigger;
                let reason = format!(
                    "founder majority holds ({on_ours} of {cities} ours); {on_it} of ours lean to its faith, {spreaders} spreader(s) at our cities"
                );
                self.journal_faith_war_held(g, pid, rival, "founder", &reason, trigger);
                continue;
            }
            if let Some((gate, reason)) = self.faith_war_gate(g, pid, rival) {
                self.journal_faith_war_held(g, pid, rival, gate, &reason, trigger);
                continue;
            }
            let Some(opening) = self.faith_war_opening(g, pid, rival) else {
                self.journal_faith_war_held(
                    g,
                    pid,
                    rival,
                    "opening",
                    "no legal declaration",
                    trigger,
                );
                continue;
            };
            let small_world = self.faith_war_small_world(g, pid, rival);
            if g.apply(pid, &opening).is_err() {
                continue;
            }
            self.faith_war_before_the_flip_war = Some(FaithWar {
                rival,
                opened: g.turn,
                spreader_seen: g.turn,
            });
            let (spreaders, on_it) = trigger;
            if let Some(reading) = small_world {
                think!(self.journal(), Military, Strategy,
                       "Opening the faith war on {}", g.players[rival].civ;
                       "faith-war-on-a-small-world: {} living majors, its race reads {}% ({} other major(s) on its faith), {spreaders} spreader(s) at our cities and {on_it} of ours lean to its faith; the edge is not asked (ours {:.0} against their steady {:.0}); defensive, no siege: our soldiers condemn its spreaders",
                       reading.majors, reading.race, reading.others,
                       g.military_power(pid), self.steady_rival_power(g, rival));
                return true;
            }
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
    /// city of ours is falling. `false` with the gene off, and for a heretic
    /// strike's war that `faith-strike-spares-the-stages` ends.
    ///
    /// `faith-war-on-a-small-world` also keeps it with no city converting
    /// while its race reading holds and a spreader of the faith stood at our
    /// cities within the exit's clear turns (`faith_war_small_world_keeps`).
    pub(crate) fn faith_war_keeps_the_war(&self, g: &Game, pid: usize, rival: usize) -> bool {
        self.faith_war_is_run()
            && self
                .faith_war_before_the_flip_war
                .is_some_and(|war| war.rival == rival)
            && g.is_at_war(pid, rival)
            && (Self::faith_war_converting(g, pid, rival) > 0
                || self.faith_war_small_world_keeps(g, pid, rival).is_some())
            && !Self::our_cities_falling(g, pid)
            && !self.faith_strike_war_spares_the_stages(g, pid, rival)
    }

    /// Whether the gene's war with `rival` has done its work: no spreader of
    /// the faith at our cities for [`FAITH_WAR_CLEAR_TURNS`] standard turns
    /// and no city of ours converting (or, under
    /// `faith-strike-spares-the-stages`, a heretic strike's war once the
    /// Culture seat's band economy has begun), the host's minimum war term
    /// over, and the rival neither the plan's target nor an urgent victory
    /// threat.
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
        self.faith_war_is_run()
            && war.rival == rival
            && g.is_at_war(pid, rival)
            && ((g.turn.saturating_sub(war.spreader_seen)
                >= g.standard_duration(FAITH_WAR_CLEAR_TURNS).max(1)
                && Self::faith_war_converting(g, pid, rival) == 0)
                || self.faith_strike_war_spares_the_stages(g, pid, rival))
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
        if converting == 0 {
            if let Some((reading, since)) = self.faith_war_small_world_keeps(g, pid, rival) {
                think!(self.journal(), Diplomacy, Decision,
                       "{what} {}", g.players[rival].civ;
                       "faith-war-on-a-small-world: {} living majors and its race reads {}%; a spreader of its faith stood at our cities {since} turn(s) ago; at war our soldiers condemn its spreaders",
                       reading.majors, reading.race);
                return;
            }
        }
        think!(self.journal(), Diplomacy, Decision,
               "{what} {}", g.players[rival].civ;
               "{}: {converting} of our cities are still being converted to its faith; at war our soldiers condemn its spreaders",
               self.faith_war_tag());
    }
}

#[cfg(test)]
mod tests;
