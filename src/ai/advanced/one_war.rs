//! One war at a time: one campaign front, a home guard, and peace on every
//! other front — and on the campaign front too, once the tide has turned
//! against us for long enough and nothing in reach is worth the next turn.
//!
//! ★★★★ THE WAR DESK COUNTS ITS WARS ONLY WHEN IT OPENS ONE. Every offensive
//! opening already refuses a second front — the elective declaration
//! (`major_wars > 0`), the appointment (`may_form_war_plan`), the air surge
//! (`air_surge_fronts`) and the raid — but nothing decides what to do once a
//! second war *arrives*: a neighbour's declaration, a Joint War accepted at
//! +300, an appointed attack that launches into a war that began after the
//! appointment. From then on the peace desk treats every enemy the same way
//! — outmatched, Recovery, or fatigued — the plan re-aims at whichever rival
//! prices lowest this turn, and the force planner hands every group the
//! union of all enemies, so an empire fighting two neighbours prosecutes both
//! at the same lukewarm pace until one of the generic clauses fires. The
//! operator's rule (2026-08-24): *fight one war at a time; keep some defence
//! at home and concentrate the rest on a single war; fight while there is
//! still something to take and pillage; sue for peace when the tide is no
//! longer in our favour, consistently.*
//!
//! What the gene does, all of it inert while the flag is off:
//!
//! 1. **One front.** Each turn, among the majors we are at war with, one is
//!    the *campaign front*: an urgent actionable military denial first (unless
//!    the operator ordered a target), then the front already chosen while it
//!    is still at war with us. A Domination front that no longer holds any
//!    required original capital hands the army to another active war that
//!    does. Otherwise use the appointed war's target, the plan's, then the
//!    enemy whose cities are nearest our soldiers. Every other major at war
//!    with us is a *second front*: offered peace every turn, its white peace
//!    accepted (`incoming_deal_value` +320). A Joint War offer while any war
//!    burns is refused outright.
//! 2. **No second declaration.** The appointed attack and the air surge hold
//!    while a major war is on against anyone else — the appointment gate
//!    runs at appointment, this one at the declaration. The one exception is
//!    a rival about to win (`urgent_victory_threat`): losing the game is the
//!    larger cost.
//! 3. **Concentrate.** `assess` keeps the plan's target on the front while
//!    the front is at war, and the force planner's objective enemies are the
//!    front alone — plus whoever is within relief range of a threatened city
//!    of ours, so the column still turns for a city about to fall. The bounded
//!    barbarian response (`barbarian_garrison_step`,
//!    `barbarian_response_objective`) stays outside the major-war planner.
//! 4. **Fight while there is something to take.** On the front the fatigue
//!    clause (war age ≥ 24, no capture in 12) stands down while a prize is in
//!    reach — a front city our soldiers stand at whose health is falling or
//!    already below `ONE_WAR_CITY_BROKEN_FRACTION`, or an unpillaged tile a
//!    soldier reaches within `ONE_WAR_PILLAGE_REACH_TURNS` — and the tide is
//!    not against us. The outmatched clauses (0.62 offer, 0.85 accept) keep
//!    their shape.
//! 5. **Sue when the tide turns, consistently.** The exchange on the front is
//!    read off the engine's own war ledger (`Game::wars`: units and cities
//!    lost by each side) at every observation. The net over the last
//!    `ONE_WAR_TIDE_WINDOW` standard turns is the tide; when it runs against
//!    us a clock starts, and after `ONE_WAR_TIDE_PATIENCE` standard turns of
//!    it with nothing in reach — or at once on a rout, `ONE_WAR_ROUT_NET` —
//!    peace is offered every turn and a white peace accepted. A capture or a
//!    favourable window stops the clock.
//!
//! The deployment genome pins `one-war-at-a-time` on after its +1.00 pp
//! displayed pooled Diff; the registry row stays a reversible `Kind::OptIn`
//! so `gene_screen --genes one-war-at-a-time` can still price it.

use std::collections::{BTreeMap, VecDeque};

use super::{AdvancedAi, GrandStrategy, VictoryTarget};
use crate::game::{DiplomaticDeal, Game};
use crate::think;
use crate::Pos;

/// The tide is read over this many standard turns of observations.
pub(crate) const ONE_WAR_TIDE_WINDOW: u32 = 8;
/// The tide must run against us for this many standard turns before peace
/// is sued for on the campaign front — "consistently", not one bad turn.
pub(crate) const ONE_WAR_TIDE_PATIENCE: u32 = 6;
/// A window net this far below zero is a rout: sue at once, prizes or not.
pub(crate) const ONE_WAR_ROUT_NET: i32 = -4;
/// What a city changing hands weighs against a unit in the exchange.
pub(crate) const ONE_WAR_CITY_WEIGHT: i32 = 3;
/// Our soldiers within this many tiles of an enemy city are besieging it,
/// for the purpose of "something to take".
pub(crate) const ONE_WAR_SIEGE_REACH: i32 = 3;
/// A pillage prize counts when a soldier reaches it within this many turns.
pub(crate) const ONE_WAR_PILLAGE_REACH_TURNS: i32 = 2;
/// A city whose defence (city + wall health) is below this fraction of full
/// while our soldiers stand at it is a city to take, even if the last
/// observation saw no drop.
pub(crate) const ONE_WAR_CITY_BROKEN_FRACTION: f64 = 0.5;
/// A breached city this low can be taken before an army redeploys to a new
/// rival. The capture body may be a few tiles behind the guns.
pub(crate) const ONE_WAR_FINISH_HP: i32 = 60;
pub(crate) const ONE_WAR_FINISH_REACH: i32 = 4;
/// `diplomatic-contender-kept`: the Diplomatic Victory points at which a
/// crushed rival's war is kept. Twenty win; a Congress awards two or more.
pub(crate) const DIPLOMATIC_CONTENDER_DVP: i64 = 15;
/// The points at which the leader among them opens a second front.
pub(crate) const DIPLOMATIC_CONTENDER_LEADER_DVP: i64 = 16;
/// How close a land taker stands to the unwalled objective for
/// `one_war_foothold_at_hand`: the reach `capture_opportunity_city` seizes
/// a foothold from.
pub(crate) const ONE_WAR_FOOTHOLD_REACH: i32 = 3;
/// A front we outgun this many times over is not traded away for a
/// counter-campaign against a rival whose clock is not yet urgent.
pub(crate) const ONE_WAR_CRUSHED_RATIO: f64 = 4.0;
/// A front we outgun this many times over is still being won: a bad window
/// of losses there is the price of a siege, not a rout or a turned tide.
pub(crate) const ONE_WAR_WINNING_RATIO: f64 = 2.0;
/// The power margin over a second rival at which a Domination seat opens
/// that war beside the one it is fighting. See `one_war_second_front`.
pub(crate) const ONE_WAR_SECOND_FRONT_RATIO: f64 = 1.5;

/// `capital-prey-opens-a-front`: the most military a rival may hold, as a
/// share of ours, for its original capital to open a front beside the war.
pub(crate) const CAPITAL_PREY_POWER: f64 = 0.15;
/// `capital-prey-opens-a-front`: the most wall hit points the prey capital
/// may stand behind: Ancient Walls, which the army opens in a few turns.
pub(crate) const CAPITAL_PREY_WALLS: i32 = 100;
/// `capital-prey-scales-the-walls`: the most wall hit points a prey capital
/// at peace may stand behind however far its army has collapsed: Medieval
/// Walls, which `guns-enter-together` and the air breach open.
pub(crate) const CAPITAL_PREY_MAX_WALLS: i32 = 300;
/// `prey-reads-a-steady-power`: the turns of rival military readings the prey
/// gates take the largest of.
pub(crate) const PREY_POWER_MEMORY_TURNS: u32 = 3;
/// `second-front-keeps-its-war`: the standard turns a second front the plan
/// named keeps its war against the one-war peace.
pub(crate) const SECOND_FRONT_MEMORY_TURNS: u32 = 10;
/// `recovery-peace-waits`: the turns a Recovery plan stands before its
/// "not the war the recovery plan is fighting" peace is offered.
pub(crate) const RECOVERY_PEACE_PATIENCE: u32 = 3;
/// `favor-spares-the-surprise-war`: the Diplomatic Victory points at which a
/// rival makes Favor worth more than the five turns a Formal War costs.
pub(crate) const FAVOR_SURPRISE_DVP: i64 = 9;
/// `favor-spares-the-surprise-war`: a target's culture finish this many turns
/// out or nearer still takes the surprise war.
pub(crate) const FAVOR_SURPRISE_CLOCK_TURNS: f64 = 8.0;
/// `declaration-needs-the-edge`: our military over the target's steady reading
/// a plain staged declaration needs.
pub(crate) const DECLARATION_EDGE_RATIO: f64 = 1.5;
/// `declaration-needs-the-edge-2`: our military over the target's peak
/// reading of the last [`DECLARATION_PEAK_TURNS`] turns at which a staged
/// declaration passes whichever empire out-produces the other.
pub(crate) const DECLARATION_PEAK_EDGE_RATIO: f64 = 2.0;
/// `declaration-needs-the-edge-2`: the turns of rival military readings the
/// peak reading takes the largest of.
pub(crate) const DECLARATION_PEAK_TURNS: u32 = 30;
/// `declaration-needs-production-parity`: our Production a turn over the
/// target's under which an offensive declaration on a major holds. Of the 20
/// declarations on majors in live Emperor G185-G198, the 16 made under 0.8
/// times the target's Production took one city within 40 turns; the 4 from
/// 0.8 to 1.2 took three.
pub(crate) const DECLARATION_PRODUCTION_PARITY: f64 = 0.8;
/// `declaration-needs-production-parity`: the most military per city a
/// Domination war lends the delegated governor while no city of ours is
/// threatened -- the genome's own floor (`mil_per_city`, 1.0), in place of
/// the two a city that sent 54% of wartime production to the army (32% at
/// peace) in the same games.
pub(crate) const UNTHREATENED_WAR_ARMY_PER_CITY: f64 = 1.0;

/// `declaration-needs-the-edge-2`: what a staged declaration is weighed on.
/// See `AdvancedAi::declaration_edge_2`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DeclarationEdge {
    /// Our military.
    pub ours: f64,
    /// The target's peak military of the last [`DECLARATION_PEAK_TURNS`].
    pub peak: f64,
    /// The target's steady military (`steady_rival_power`).
    pub steady: f64,
    /// Our Production a turn (`BasicAi::seat_production_per_turn`).
    pub our_production: f64,
    /// The target's Production a turn.
    pub their_production: f64,
}

impl DeclarationEdge {
    /// Our military over the target's peak reading.
    pub fn peak_ratio(&self) -> f64 {
        self.ours / self.peak.max(1.0)
    }

    /// Our military over the target's steady reading.
    pub fn steady_ratio(&self) -> f64 {
        self.ours / self.steady.max(1.0)
    }

    /// Twice its peak, or version 1's edge against a target that makes less
    /// Production than we do.
    pub fn passes(&self) -> bool {
        self.peak_ratio() >= DECLARATION_PEAK_EDGE_RATIO
            || (self.their_production < self.our_production
                && self.steady_ratio() >= DECLARATION_EDGE_RATIO)
    }
}
/// `diplomatic-contender-eliminated`: the Diplomatic Victory points at which
/// a rival we are fighting is to be eliminated rather than passed by.
pub(crate) const ELIMINATION_CONTENDER_DVP: i64 = 14;
/// `diplomatic-contender-eliminated`: our military over that rival's steady
/// reading at which the elimination front holds.
pub(crate) const ELIMINATION_POWER_RATIO: f64 = 2.0;
/// `overwhelming-power-declares`: our military over the target's steady
/// reading at which a Domination seat declares without a staged siege.
pub(crate) const OVERWHELMING_POWER_RATIO: f64 = 4.0;
/// `overwhelming-power-declares`: the field army's median distance to the
/// objective at or under which the overwhelming waiver applies.
pub(crate) const OVERWHELMING_MARCH_TILES: i32 = 10;

/// `liberation-funds-the-congress`: the Diplomatic Victory points at which a
/// rival makes a captured city-state city worth its liberation Favor; the
/// Congress denial floor.
pub(crate) const LIBERATION_DVP_FLOOR: i64 = 12;
/// `liberation-funds-the-congress`: the disposition value of that liberation,
/// above any small city's keep value.
pub(crate) const LIBERATION_CONGRESS_VALUE: f64 = 400.0;

/// `bleeding-capital-loyalty`: the Loyalty runway, in turns, under which a
/// captured original capital is bleeding.
pub(crate) const BLEEDING_CAPITAL_RUNWAY: f64 = 6.0;

/// `capital-prey-opens-a-front-2`: the most military a prey may hold, as a
/// share of ours, for its original capital to reach past the declaration
/// range, out to [`CAPITAL_PREY_DEEP_REACH`].
pub(crate) const CAPITAL_PREY_DEEP_POWER: f64 = 0.10;
/// `capital-prey-opens-a-front-2`: the deep reach, half again the 18-tile
/// declaration range.
pub(crate) const CAPITAL_PREY_DEEP_REACH: i32 = 27;
/// The standard turns between two journal lines for the same capital-prey
/// near miss failing the same gates.
pub(crate) const CAPITAL_PREY_NOTE_TURNS: u32 = 10;
/// `capital-prey-opens-a-front`: the turns a soft capital's siege is
/// expected to take once the army stands on its ring.
pub(crate) const CAPITAL_PREY_SIEGE_TURNS: f64 = 4.0;
/// `capital-prey-opens-a-front`: a culture finish projected sooner than the
/// prey's capture plus this many turns is a true match point, and the
/// counter keeps the army.
pub(crate) const CAPITAL_PREY_MATCH_MARGIN: f64 = 5.0;

/// The power ratio that keeps a second front already named: the opening
/// ratio, less a margin, so the pick does not flicker on the line. Live King
/// civvis-20261004T025448Z (game 45) had 302-340 power against 1.5 times
/// Nubia's 288-335; the pick switched between Nubia and the Spanish front
/// nearly every turn, and the army left Barcelona's ring for a war that never
/// opened (diagnosed by -60).
pub(crate) const ONE_WAR_SECOND_FRONT_HOLD_RATIO: f64 = 1.3;

/// The least power, against the rival's, at which a counter-war on a rival's
/// RELIGIOUS clock is worth opening or freeing the army for. Urgency waives
/// the ordinary war ratio, because a staged army at the border is the only
/// answer to a terminal clock: a Spaceport or a culture city taken stops it
/// whatever the empires' totals. A faith is not stopped that way. A
/// faithless seat cannot win its converted cities back, and the war does not
/// keep a third civilization unconverted, so at half the founder's strength
/// it only loses the army. Live King civvis-20261004T055130Z (game 48) offered
/// Kongo peace at turn 77, at 302 power against 182, "freeing the
/// Domination army to counter a rival victory threat", and declared on
/// Persia, whose faith already held five of our six cities, at turn 80 at
/// 316 against 619. By 109 the army stood at 225 with nothing taken. The
/// only other urgent declaration in forty live games opened at 0.99.
pub(crate) const COUNTER_WAR_POWER_FLOOR: f64 = 0.7;

/// `counter-war-needs-parity`: the least power, against the rival's, at
/// which a counter-war on any other clock is opened or takes the front.
/// Seven urgent counter declarations below parity on 2026-10-04/05 (0.24 to
/// 0.74) took 0.14 cities between them in the next forty turns, and all
/// seven games were lost; G83 declared on Germany at 0.70 for a science
/// counter and lost on Religion, G92 moved the army off a staged Qusqu onto
/// Sydney at 0.91 and stood 12-18 tiles out for 25 turns.
pub(crate) const COUNTER_WAR_PARITY: f64 = 1.0;

/// Standard turns a front siege still in Stage counts as live for
/// `front_siege_live`. A siege past Stage counts while it is read and its
/// city has fallen to a new low of health within this many standard turns.
pub(crate) const FRONT_SIEGE_LIVE_TURNS: u32 = 10;

/// `front-finishes-its-capital`: standard turns a siege stage on the front's
/// capital or last city holds the army against an urgent counter-war.
pub(crate) const FRONT_CAPITAL_FINISH_TURNS: u32 = 8;

/// `one-war-swaps-a-stalled-front`: standard turns without a new low of
/// health in any front city after which the front counts as stalled.
pub(crate) const FRONT_STALL_TURNS: u32 = 20;

/// `one-war-swaps-a-stalled-front`: our power over a second enemy at which
/// its required capital is worth moving the front to.
pub(crate) const STALLED_FRONT_SWAP_RATIO: f64 = 3.0;

/// `culture-counter-declares`: our power over a culture rival at match point
/// at which the declaration does not wait for a staged siege. See
/// `culture_counter_due`.
pub(crate) const CULTURE_COUNTER_RATIO: f64 = 1.5;
/// `counter-war-needs-the-emperor-edge`: our military over a rival's steady
/// power at which a culture or faith counter war opens without a staged
/// siege. Of the 53 such declarations of October 5-6, the 27 under 2.5
/// times lost a city of ours within 40 turns 3 times and saw our military
/// fall by a fifth within 30 turns 5 times; the 26 at 2.5 times or more lost
/// none and fell twice. Both Emperor counters under 2 times backfired (G188
/// Mapuche 1.6, G192 Greece 1.6); both at 3 times or more held (G185 France
/// 3.3, G190 Khmer 4.1). Counter wars took a city in 3 of 27 under the bar
/// and 3 of 26 over it, so the bar costs little conquest.
pub(crate) const COUNTER_WAR_EMPEROR_EDGE: f64 = 2.5;
/// Standard turns the front may refuse the peace that would free the army
/// before the second front opens beside it.
pub(crate) const ONE_WAR_SECOND_FRONT_PATIENCE: u32 = 3;
/// Standard turns a newly chosen front is kept against a merely
/// outranking (not urgent) clock elsewhere. See `one_war_peace`.
pub(crate) const ONE_WAR_FRESH_FRONT_TURNS: u32 = 10;
/// A second-front unit this close to a threatened city of ours keeps that
/// enemy in the force planner's sights: the relief column's own radius.
pub(crate) const ONE_WAR_RELIEF_REACH: i32 = 8;
/// A city's full health; the engine's ceiling (`Game::do_end_turn` heals to
/// 200), walls on top of it.
pub(crate) const ONE_WAR_CITY_FULL_HP: i32 = 200;

/// The campaign front as the gene sees it, carried across turns.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OneWarFront {
    /// The major we are concentrating on.
    pub(crate) target: usize,
    /// The turn this front was chosen.
    pub(crate) since: u32,
    /// The war ledger's loss counts at the last observation:
    /// (our units, their units, our cities, their cities).
    pub(crate) ledger: (u32, u32, u32, u32),
    /// Net exchange per observation over the tide window, newest last.
    pub(crate) window: VecDeque<(u32, i32)>,
    /// The turn the tide turned against us, if it has and has not turned
    /// back since.
    pub(crate) tide_against_since: Option<u32>,
    /// City and wall health of the front's cities at the last observation,
    /// by city tile. The live seat renumbers its cities every turn, so an id
    /// key compared each city with whichever city held its id the turn
    /// before.
    pub(crate) city_health: BTreeMap<Pos, (i32, i32)>,
    /// Cities of the front whose health fell at the last observation.
    pub(crate) sieges_advancing: usize,
    /// The first turn of the current run of observations in which the gene
    /// wanted this front closed for a purpose elsewhere (a secured capital, a
    /// displaced one, or a victory threat). See `one_war_second_front`.
    pub(crate) closure_wanted_since: Option<u32>,
}

impl OneWarFront {
    fn new(target: usize, turn: u32) -> Self {
        Self {
            target,
            since: turn,
            ledger: (0, 0, 0, 0),
            window: VecDeque::new(),
            tide_against_since: None,
            city_health: BTreeMap::new(),
            sieges_advancing: 0,
            closure_wanted_since: None,
        }
    }

    /// The net exchange over the window: positive is ours.
    pub(crate) fn window_net(&self) -> i32 {
        self.window.iter().map(|(_, net)| *net).sum()
    }
}

/// Why the gene wants peace with a rival this turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OneWarPeace {
    /// Not the campaign front: one war at a time.
    SecondFront,
    /// The required capital is secure and another capital remains to pursue.
    CapitalSecured,
    /// Another rival now holds this front's original capital.
    CapitalElsewhere,
    /// Another rival's religious or culture finish outranks optional conquest.
    VictoryThreat,
    /// The campaign front, and the tide has run against us for long enough
    /// with nothing left in reach worth the next turn.
    TideTurned,
    /// The campaign front, and the last window was a rout.
    Rout,
}

impl OneWarPeace {
    pub(crate) fn reason(self) -> &'static str {
        match self {
            OneWarPeace::SecondFront => "one war at a time, and this is not the one",
            OneWarPeace::CapitalSecured => {
                "the required capital is secure and another capital remains to pursue"
            }
            OneWarPeace::CapitalElsewhere => {
                "this rival's original capital is now held by another opponent"
            }
            OneWarPeace::VictoryThreat => {
                "freeing the Domination army to counter a rival victory threat"
            }
            OneWarPeace::TideTurned => {
                "the tide has run against us for long enough and nothing in reach is worth the next turn"
            }
            OneWarPeace::Rout => "the last window was a rout",
        }
    }
}

impl AdvancedAi {
    /// A congress vote can jump Diplomatic Victory pressure once, then stay
    /// flat for the whole congress interval. Below the ordinary denial bar,
    /// only the projected slope can call that jump urgent. It should still
    /// prepare a counter, but cannot pin a Domination army after we safely
    /// capture that rival's capital or a third party takes it.
    fn one_war_projected_diplomacy_below_bar(&self, g: &Game, rival: usize) -> bool {
        let pressure = self.rival_victory_pressure(g, rival);
        pressure.strategy == GrandStrategy::Diplomacy && pressure.progress < super::STOCK_DENIAL_BAR
    }

    /// A stable captured capital, or one captured by a third party, completes
    /// this front's Domination purpose. Prefer another known capital owner
    /// over the former owner's ordinary towns. The ordinary city chooser
    /// still enforces occupation safety and can prepare through that next
    /// rival's frontier before taking its capital.
    pub(super) fn domination_followup_target(
        &self,
        g: &Game,
        pid: usize,
        completed_rival: Option<usize>,
    ) -> Option<usize> {
        if !self.one_war_at_a_time
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || self.forced_target_player.is_some()
        {
            return None;
        }
        let displaced_owner = completed_rival.and_then(|other| {
            g.cities
                .values()
                .find(|city| {
                    city.is_capital
                        && city.original_owner == other
                        && city.owner != other
                        && city.owner != pid
                        && !g.same_team(pid, city.owner)
                        && g.players
                            .get(city.owner)
                            .is_some_and(|player| !player.is_minor && !player.is_barbarian)
                        && self.campaign_target_legal(g, pid, city.owner)
                })
                .map(|city| city.owner)
        });
        let secured = g.cities.values().any(|city| {
            city.owner == pid
                && city.is_capital
                && city.original_owner != pid
                && completed_rival.is_none_or(|other| city.original_owner == other)
                && !g.players[city.original_owner].is_minor
                && !g.players[city.original_owner].is_barbarian
                && city.loyalty >= 75.0
                && g.city_loyalty_per_turn(city) >= 0.0
        });
        if completed_rival.is_some_and(|other| {
            g.emergency_war_pair(pid, other)
                || (self.urgent_victory_threat(g, other)
                    && !((displaced_owner.is_some() || secured)
                        && self.one_war_projected_diplomacy_below_bar(g, other)))
                || g.cities.values().any(|city| {
                    city.owner == other
                        && city.is_capital
                        && !g.players[city.original_owner].is_minor
                        && !g.players[city.original_owner].is_barbarian
                })
        }) {
            return None;
        }
        // We do not need to finish this rival's ordinary towns if a third
        // party took its original capital. That third party now owns the
        // Domination objective even when we captured no capital ourselves.
        if let Some(owner) = displaced_owner {
            return Some(owner);
        }
        // A rival can consolidate another major's original capital before
        // we take one. In that case a former owner with only ordinary towns
        // is not a useful peacetime target either.
        let foreign_consolidation = completed_rival.is_none()
            && g.cities.values().any(|city| {
                city.is_capital
                    && city.owner != pid
                    && city.owner != city.original_owner
                    && !g.same_team(pid, city.owner)
                    && g.players
                        .get(city.owner)
                        .is_some_and(|owner| !owner.is_minor && !owner.is_barbarian)
                    && g.players
                        .get(city.original_owner)
                        .is_some_and(|founder| !founder.is_minor && !founder.is_barbarian)
            });
        if !secured && !foreign_consolidation {
            return None;
        }
        g.cities
            .values()
            .filter(|city| {
                city.is_capital
                    && city.owner != pid
                    && Some(city.owner) != completed_rival
                    && !g.same_team(pid, city.owner)
                    && !g.players[city.original_owner].is_minor
                    && !g.players[city.original_owner].is_barbarian
                    && (!g.same_team(pid, city.original_owner) || city.owner != city.original_owner)
                    && self.campaign_target_legal(g, pid, city.owner)
            })
            .min_by(|left, right| {
                self.campaign_city_value(g, pid, left, GrandStrategy::Conquest)
                    .total_cmp(&self.campaign_city_value(g, pid, right, GrandStrategy::Conquest))
                    .then(left.id.cmp(&right.id))
            })
            .map(|city| city.owner)
    }

    /// The living majors whose cities still form a war front. A defeated
    /// rival can remain alive and at war in a live host after losing its last
    /// city; that stale war must not hold the next capital's declaration.
    pub(crate) fn one_war_enemies(&self, g: &Game, pid: usize) -> Vec<usize> {
        g.players
            .iter()
            .filter(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && !g.player_city_ids(other.id).is_empty()
                    && g.is_at_war(pid, other.id)
            })
            .map(|other| other.id)
            .collect()
    }

    /// The campaign front's target while the gene is on and a major war is
    /// being fought; `None` otherwise.
    pub(crate) fn one_war_front(&self) -> Option<usize> {
        if !self.one_war_at_a_time {
            return None;
        }
        self.one_war.as_ref().map(|front| front.target)
    }

    /// Choose the front among the enemies: the appointed war's target, then
    /// the plan's, then the one whose nearest city is nearest to our army,
    /// then the lowest id. The choice sticks while its target stays at war
    /// with us, unless an urgent military denial needs a different active
    /// front or its known original capital has changed hands and another
    /// active front holds a required capital. An explicit operator target
    /// keeps its existing precedence.
    fn one_war_choose_front(&self, g: &Game, pid: usize, enemies: &[usize]) -> Option<usize> {
        let current = self
            .one_war
            .as_ref()
            .map(|front| front.target)
            .filter(|target| enemies.contains(target));
        let capital_handoff = current
            .and_then(|front| self.domination_followup_target(g, pid, Some(front)))
            .filter(|target| enemies.contains(target));
        // See `domination_finish_front`: the war that ends the game first.
        if let Some((owner, _)) = self
            .domination_finish_front(g, pid)
            .filter(|(owner, _)| enemies.contains(owner))
        {
            return Some(owner);
        }
        // See `diplomatic_contender_to_eliminate`: ahead of every other clock.
        if let Some(contender) = self
            .diplomatic_contender_to_eliminate(g, pid)
            .filter(|rival| enemies.contains(rival))
        {
            return Some(contender);
        }
        // See `capital_moves_on_next` (`capital-taken-moves-on`): once the war
        // on the next capital's owner is open, it is the front.
        if let Some(next) = self
            .capital_moves_on_next(g, pid)
            .filter(|next| enemies.contains(next) && current != Some(*next))
        {
            return Some(next);
        }
        // `diplomatic-contender-kept-2`: the crushed Diplomatic Victory
        // contender with the most points among the wars already running
        // takes the front ahead of every other clock. See
        // `diplomatic_contender_front`.
        if self.diplomatic_contender_kept_2 && self.forced_target_player.is_none() {
            if let Some(contender) = self.diplomatic_contender_front(g, pid, enemies) {
                return Some(contender);
            }
        }
        // The declaration gate already admits urgent victory denial. Once
        // that war exists, concentrate on it instead of immediately offering
        // the winning rival peace as a second front. Keep the ordinary rout
        // and sustained losing-tide safeguards on whichever front is chosen.
        if self.forced_target_player.is_none() {
            if let Some((rival, GrandStrategy::Conquest)) = self.actionable_victory_denial(g, pid) {
                // `counter-war-needs-parity`: an urgent rival under the floor
                // does not take the army off the front either.
                let hopeless =
                    self.counter_war_needs_parity && self.counter_war_hopeless(g, pid, rival);
                if enemies.contains(&rival)
                    && self.urgent_victory_threat(g, rival)
                    && !hopeless
                    // See `religious_threat_spares_the_front`.
                    && !self.religious_threat_spares_the_front(g, pid, rival)
                {
                    // A congress jump can make a subthreshold Diplomatic
                    // score look urgent even after this rival lost its
                    // original capital. Follow the active war for that
                    // capital while retaining the projected warning.
                    // See `front_siege_to_finish`.
                    let finishing = current.is_some_and(|front| front != rival)
                        && (self.front_siege_to_finish(g)
                            // See `front_capital_to_finish`.
                            || self.front_capital_to_finish(g, rival));
                    if !(current == Some(rival)
                        && capital_handoff.is_some()
                        && self.one_war_projected_diplomacy_below_bar(g, rival))
                        && !finishing
                    {
                        return Some(rival);
                    }
                }
            }
        }
        // When the front's original capital moves to another rival, the old
        // war no longer advances Domination. If its new owner is already at
        // war with us, move the army there and offer the old front peace.
        if let Some(next) = capital_handoff {
            return Some(next);
        }
        // See `stalled_front_swap`.
        if let Some(next) =
            current.and_then(|front| self.stalled_front_swap(g, pid, front, enemies))
        {
            return Some(next);
        }
        if let Some(current) = current {
            return Some(current);
        }
        if let Some(appointed) = self
            .war_plan
            .as_ref()
            .map(|war| war.target_player)
            .filter(|target| enemies.contains(target))
        {
            return Some(appointed);
        }
        if let Some(planned) = self
            .plan
            .as_ref()
            .and_then(|plan| plan.target_player)
            .filter(|target| enemies.contains(target))
        {
            return Some(planned);
        }
        let soldiers: Vec<Pos> = self.one_war_soldiers(g, pid);
        enemies.iter().copied().min_by_key(|enemy| {
            let nearest = g
                .player_city_ids(*enemy)
                .into_iter()
                .map(|cid| g.cities[&cid].pos)
                .map(|city| {
                    soldiers
                        .iter()
                        .map(|pos| g.wdist(*pos, city))
                        .min()
                        .unwrap_or(i32::MAX)
                })
                .min()
                .unwrap_or(i32::MAX);
            (nearest, *enemy)
        })
    }

    /// Positions of our land soldiers fit to fight.
    fn one_war_soldiers(&self, g: &Game, pid: usize) -> Vec<Pos> {
        g.player_unit_ids(pid)
            .into_iter()
            .filter_map(|uid| {
                let unit = &g.units[&uid];
                let spec = &g.rules.units[unit.kind];
                (spec.class == "military"
                    && spec.domain.as_deref() != Some("air")
                    && spec.domain.as_deref() != Some("sea")
                    && !g.is_embarked(unit))
                .then_some(unit.pos)
            })
            .collect()
    }

    /// Cumulative losses against `other`: our units, their units, our cities,
    /// their cities. Native unit counts come from confirmed combat events;
    /// each newly observed front seeds its baseline before reading changes.
    pub(super) fn one_war_ledger(
        &self,
        g: &Game,
        pid: usize,
        other: usize,
    ) -> (u32, u32, u32, u32) {
        let key = (pid.min(other), pid.max(other));
        let mut ledger = g
            .wars
            .get(&key)
            .map(|war| {
                let ours = war.losses.get(&pid);
                let theirs = war.losses.get(&other);
                (
                    ours.map_or(0, |l| l.units),
                    theirs.map_or(0, |l| l.units),
                    ours.map_or(0, |l| l.cities),
                    theirs.map_or(0, |l| l.cities),
                )
            })
            .unwrap_or((0, 0, 0, 0));
        if let Some(losses) = &self.host_war_unit_losses {
            ledger.0 = losses.get(&(pid, other)).copied().unwrap_or(0);
            ledger.1 = losses.get(&(other, pid)).copied().unwrap_or(0);
        }
        ledger
    }

    /// The observation pass: pick or keep the front, read the exchange since
    /// the last observation off the war ledger, and run the tide clock.
    /// Called once per acting turn from `observe_campaign`; exact no-op with
    /// the gene off.
    pub(crate) fn one_war_observe(&mut self, g: &Game, pid: usize) {
        if !self.one_war_at_a_time {
            self.one_war = None;
            self.one_war_second = None;
            return;
        }
        let enemies = self.one_war_enemies(g, pid);
        let Some(target) = self.one_war_choose_front(g, pid, &enemies) else {
            self.one_war = None;
            self.one_war_second = None;
            return;
        };
        let mut front = match self.one_war.take() {
            Some(front) if front.target == target => front,
            _ => {
                self.front_city_low.clear();
                let mut fresh = OneWarFront::new(target, g.turn);
                fresh.ledger = self.one_war_ledger(g, pid, target);
                fresh
            }
        };
        let ledger = self.one_war_ledger(g, pid, target);
        let (our_units, their_units, our_cities, their_cities) = (
            ledger.0.saturating_sub(front.ledger.0) as i32,
            ledger.1.saturating_sub(front.ledger.1) as i32,
            ledger.2.saturating_sub(front.ledger.2) as i32,
            ledger.3.saturating_sub(front.ledger.3) as i32,
        );
        front.ledger = ledger;
        let net = their_units - our_units + ONE_WAR_CITY_WEIGHT * (their_cities - our_cities);
        front.window.push_back((g.turn, net));
        let window = g.standard_duration(ONE_WAR_TIDE_WINDOW).max(1);
        while front
            .window
            .front()
            .is_some_and(|(turn, _)| g.turn.saturating_sub(*turn) >= window)
        {
            front.window.pop_front();
        }
        // The sieges: a front city whose health fell since the last
        // observation is a city being taken.
        let mut health_now = BTreeMap::new();
        let mut advancing = 0;
        for cid in g.player_city_ids(target) {
            let city = &g.cities[&cid];
            let health = (city.hp, city.wall_hp);
            if front
                .city_health
                .get(&city.pos)
                .is_some_and(|before| health.0 < before.0 || health.1 < before.1)
            {
                advancing += 1;
            }
            health_now.insert(city.pos, health);
            // See `FRONT_SIEGE_LIVE_TURNS`: the lowest health each front city
            // has shown, and when.
            let total = city.hp.max(0) + city.wall_hp.max(0);
            let low = self
                .front_city_low
                .entry(city.pos)
                .or_insert((total, g.turn));
            if total < low.0 {
                *low = (total, g.turn);
            }
        }
        front.city_health = health_now;
        front.sieges_advancing = advancing;
        // The tide clock: starts when the window runs against us, and only
        // a favourable window or a capture turns it back.
        let window_net = front.window_net();
        if their_cities > 0 || window_net > 0 {
            front.tide_against_since = None;
        } else if window_net < 0 {
            front.tide_against_since.get_or_insert(g.turn);
        }
        self.one_war = Some(front);
        // The closure clock: how long the gene has wanted this front closed
        // for a purpose elsewhere. Read once the front is stored, since
        // `one_war_peace` reads it.
        let wants_closure = matches!(
            self.one_war_peace(g, pid, target),
            Some(
                OneWarPeace::CapitalSecured
                    | OneWarPeace::CapitalElsewhere
                    | OneWarPeace::VictoryThreat
            )
        );
        if let Some(front) = self.one_war.as_mut() {
            if wants_closure {
                front.closure_wanted_since.get_or_insert(g.turn);
            } else {
                front.closure_wanted_since = None;
            }
        }
        self.one_war_second = self.one_war_second_front(g, pid);
        // See `second_front_recently_named`.
        if let Some(rival) = self.one_war_second {
            self.second_front_named.insert(rival, g.turn);
        }
        let memory = g.standard_duration(SECOND_FRONT_MEMORY_TURNS);
        self.second_front_named
            .retain(|_, turn| g.turn.saturating_sub(*turn) <= memory);
    }

    /// `second-front-keeps-its-war`: whether the plan named `rival` its second
    /// front within the last [`SECOND_FRONT_MEMORY_TURNS`] standard turns. A
    /// rival stops being named the moment the war on it opens (the second
    /// front is chosen among rivals at peace), so after the declaration only
    /// the counter's own readings keep the war, and they flicker: live King
    /// civvis-20261005T111622Z (game 124) declared on Norway at turn 105 when
    /// 6 of our 11 cities followed its Orthodoxy, and offered it "one war at a
    /// time" peace from 105 at 5 of 11. Seven of the 142 declarations of
    /// October 4-5 drew that peace within three turns.
    pub(crate) fn second_front_recently_named(&self, g: &Game, rival: usize) -> bool {
        self.second_front_keeps_its_war
            && self.second_front_named.get(&rival).is_some_and(|turn| {
                g.turn.saturating_sub(*turn) <= g.standard_duration(SECOND_FRONT_MEMORY_TURNS)
            })
    }

    /// The power ratio over `rival` a second front needs: the hold ratio for
    /// the one named at the last observation, the opening ratio otherwise.
    pub(crate) fn second_front_ratio(&self, rival: usize) -> f64 {
        if self.one_war_second == Some(rival) {
            ONE_WAR_SECOND_FRONT_HOLD_RATIO
        } else {
            ONE_WAR_SECOND_FRONT_RATIO
        }
    }

    /// Whether the front still offers something worth the next turn: a city
    /// our soldiers are at whose health is falling or already broken, or
    /// unpillaged tiles a soldier reaches within `ONE_WAR_PILLAGE_REACH_TURNS`.
    pub(crate) fn one_war_prizes_in_reach(&self, g: &Game, pid: usize) -> bool {
        let Some(front) = self.one_war.as_ref().filter(|_| self.one_war_at_a_time) else {
            return false;
        };
        let target = front.target;
        let strikers: Vec<(Pos, i32)> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter_map(|uid| {
                let unit = &g.units[&uid];
                let spec = &g.rules.units[unit.kind];
                if spec.class != "military"
                    || spec.domain.as_deref() == Some("air")
                    || spec.domain.as_deref() == Some("sea")
                    || g.is_embarked(unit)
                    || unit.hp < 50
                {
                    return None;
                }
                let reach =
                    (g.unit_max_moves(uid).floor() as i32).max(1) * ONE_WAR_PILLAGE_REACH_TURNS;
                Some((unit.pos, reach))
            })
            .collect();
        if strikers.is_empty() {
            return false;
        }
        for cid in g.player_city_ids(target) {
            let city = &g.cities[&cid];
            let at_it = strikers
                .iter()
                .any(|(pos, _)| g.wdist(*pos, city.pos) <= ONE_WAR_SIEGE_REACH);
            if !at_it {
                continue;
            }
            let falling = front
                .city_health
                .get(&city.pos)
                .is_some_and(|(hp, wall)| (city.hp, city.wall_hp) < (*hp, *wall))
                || front.sieges_advancing > 0;
            let full = ONE_WAR_CITY_FULL_HP + g.city_max_wall_hp(city).max(0);
            let broken = ((city.hp.max(0) + city.wall_hp.max(0)) as f64)
                < full as f64 * ONE_WAR_CITY_BROKEN_FRACTION;
            if falling || broken {
                return true;
            }
        }
        let explored = &g.players[pid].explored;
        for cid in g.player_city_ids(target) {
            let city = &g.cities[&cid];
            for pos in city.owned_tiles.iter().copied() {
                if !explored.contains(&pos) || !g.pillageable_after_declaring(pid, pos) {
                    continue;
                }
                if g.map
                    .get(pos)
                    .is_some_and(|tile| tile.owner_city == Some(cid))
                    && strikers
                        .iter()
                        .any(|(spos, reach)| g.wdist(*spos, pos) <= *reach)
                {
                    return true;
                }
            }
        }
        false
    }

    /// Do not end a Domination front with a breached city and a healthy
    /// capture unit close enough to finish it. A distant or merely damaged
    /// city must not delay an urgent counter-campaign.
    fn one_war_capture_at_hand(&self, g: &Game, pid: usize, other: usize) -> bool {
        g.player_city_ids(other).into_iter().any(|cid| {
            let city = &g.cities[&cid];
            city.wall_hp <= 0
                && city.hp <= ONE_WAR_FINISH_HP
                && g.player_unit_ids(pid).into_iter().any(|uid| {
                    let unit = &g.units[&uid];
                    let spec = &g.rules.units[unit.kind];
                    spec.class == "military"
                        && spec.domain.as_deref() != Some("air")
                        && spec.domain.as_deref() != Some("sea")
                        && spec.is_melee_capable()
                        && !g.is_embarked(unit)
                        && unit.hp >= 50
                        && g.wdist(unit.pos, city.pos) <= ONE_WAR_FINISH_REACH
                })
        })
    }

    /// `peace-waits-for-the-foothold`: the plan's objective is an unwalled
    /// city of `other` with one of our land takers in reach, on the terms
    /// that let the campaign seize it as an open foothold
    /// (`capture_opportunity_city`). An untouched city is not at
    /// [`ONE_WAR_FINISH_HP`], so `one_war_capture_at_hand` passes it by. The
    /// capture ledger bounds the wait: an objective nobody takes is stood
    /// down and the plan moves on. Live King civvis-20261004T212049Z (game
    /// 80) seized the open foothold Umgungundlovu (population 2, no walls) at
    /// turn 140, offered the Zulu peace at 141 at 312 power against 177 to
    /// counter Portugal, and declared on Portugal the same turn at 312
    /// against 311. Coimbra's 400 walls stood for the next 23 turns.
    fn one_war_foothold_at_hand(&self, g: &Game, pid: usize, other: usize) -> bool {
        if !self.peace_waits_for_the_foothold {
            return false;
        }
        let Some(city) = self
            .plan
            .as_ref()
            .and_then(|plan| plan.target_city)
            .and_then(|cid| g.cities.get(&cid))
            .filter(|city| city.owner == other && city.wall_hp <= 0)
        else {
            return false;
        };
        let fresh = city.hp >= super::CITY_MAX_HP;
        let (least_hp, least_strength) = if fresh { (70, 25.0) } else { (60, 30.0) };
        g.player_unit_ids(pid).into_iter().any(|uid| {
            let unit = &g.units[&uid];
            let spec = &g.rules.units[unit.kind];
            spec.class == "military"
                && !spec.has_ranged_attack()
                && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                && !g.is_embarked(unit)
                && unit.hp >= least_hp
                && g.unit_strength(unit, false) >= least_strength
                && g.wdist(unit.pos, city.pos) <= ONE_WAR_FOOTHOLD_REACH
        })
    }

    /// `peace-waits-for-unseen-prey`: a living major at war with a
    /// Domination seat, with no city the seat can see, and outgunned
    /// [`ONE_WAR_WINNING_RATIO`] times over. Its cities are in the fog, not
    /// gone, and `one_war_enemies` drops a rival without a known city, so
    /// the front's guards against peace lapse with the last city we saw.
    /// Live King civvis-20261004T213648Z (game 81) declared on Babylon at
    /// turn 78 at 396 power against 65 and took Mashkan-shapir, the only
    /// Babylonian city it knew, at 81. The campaign then offered "has taken
    /// its 1 city" peace, and the Recovery plan "this is not the war the
    /// recovery plan is fighting" at 381 against 114. Babylon still held six
    /// cities at turn 207.
    pub(crate) fn unseen_prey(&self, g: &Game, pid: usize, other: usize) -> bool {
        self.peace_waits_for_unseen_prey
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && g.is_at_war(pid, other)
            && g.players[other].alive
            && !g.players[other].is_minor
            && !g.players[other].is_barbarian
            && g.player_city_ids(other).is_empty()
            && g.military_power(pid) >= ONE_WAR_WINNING_RATIO * g.military_power(other).max(1.0)
    }

    /// `last-capital-war-kept`: the war on the one rival that still holds an
    /// original capital a Domination seat needs, every other one already
    /// ours, is offered no peace while we are at least its equal. That war
    /// is the game. Live King civvis-20261005T003728Z (game 89) held Uruk
    /// and Mashhad by turn 225, Tyre the last capital it lacked, and offered
    /// Phoenicia "the war has stalled" peace at turns 222-223 at 1,534-1,636
    /// power against 965-995; a peace taken there costs ten turns of treaty.
    pub(crate) fn last_capital_war_kept(&self, g: &Game, pid: usize, other: usize) -> bool {
        if !self.last_capital_war_kept
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !g.is_at_war(pid, other)
            || g.military_power(pid) < g.military_power(other)
        {
            return false;
        }
        let mut holders = g
            .players
            .iter()
            .filter(|player| {
                player.id != pid
                    && !player.is_minor
                    && !player.is_barbarian
                    && !g.same_team(pid, player.id)
            })
            .filter_map(|player| {
                g.cities
                    .values()
                    .find(|city| city.is_capital && city.original_owner == player.id)
                    .map(|city| city.owner)
            })
            .filter(|owner| *owner != pid);
        holders.next() == Some(other) && holders.all(|owner| owner == other)
    }

    /// `diplomatic-contender-kept`: a living major at
    /// [`DIPLOMATIC_CONTENDER_DVP`] Diplomatic Victory points or more that a
    /// Domination seat outguns [`ONE_WAR_CRUSHED_RATIO`] times over. Only its
    /// elimination takes those points off the board: a captured capital or
    /// town removes none. Such a war is kept as a second front
    /// (`second_front_war_kept`). Live King civvis-20261005T003728Z (game 89)
    /// stood at peace with Sumeria from its third capture to the end, at 17
    /// points against Sumeria's 3 military and four cities, while Persia, the
    /// front, won the Diplomatic Victory at turn 242.
    pub(crate) fn diplomatic_contender(&self, g: &Game, pid: usize, other: usize) -> bool {
        self.diplomatic_contender_kept && self.diplomatic_contender_reading(g, pid, other)
    }

    /// `diplomatic-contender-kept-2`: among `enemies`, the one reading as a
    /// diplomatic contender (`diplomatic_contender_reading`) with the most
    /// Diplomatic Victory points. Only its elimination takes those points
    /// off the board, so it takes the front ahead of every other clock. Live
    /// King civvis-20261005T021048Z (game 93) fought Portugal (15 points, 7
    /// cities, 172 military), the Zulu (14) and Indonesia (13, 988 military)
    /// at once from turn 220, at 1,970 power, and aimed the campaign at
    /// Indonesia every turn.
    pub(crate) fn diplomatic_contender_front(
        &self,
        g: &Game,
        pid: usize,
        enemies: &[usize],
    ) -> Option<usize> {
        enemies
            .iter()
            .copied()
            .filter(|enemy| self.diplomatic_contender_reading(g, pid, *enemy))
            .max_by_key(|enemy| (g.players[*enemy].dvp, std::cmp::Reverse(*enemy)))
    }

    /// A living major at [`DIPLOMATIC_CONTENDER_DVP`] Diplomatic Victory
    /// points or more that a Domination seat outguns
    /// [`ONE_WAR_CRUSHED_RATIO`] times over.
    fn diplomatic_contender_reading(&self, g: &Game, pid: usize, other: usize) -> bool {
        self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && g.players.get(other).is_some_and(|player| {
                player.alive
                    && !player.is_minor
                    && !player.is_barbarian
                    && player.dvp >= DIPLOMATIC_CONTENDER_DVP
            })
            && self.one_war_front_crushed(g, pid, other)
    }

    /// The Diplomatic Victory leader, when it is a `diplomatic_contender` at
    /// [`DIPLOMATIC_CONTENDER_LEADER_DVP`] or more: one Congress from the
    /// win. `one_war_second_front` opens its war at once.
    fn diplomatic_contender_leader(&self, g: &Game, pid: usize) -> Option<usize> {
        g.players
            .iter()
            .filter(|player| {
                player.id != pid && player.alive && !player.is_minor && !player.is_barbarian
            })
            .max_by_key(|player| (player.dvp, std::cmp::Reverse(player.id)))
            .filter(|leader| leader.dvp >= DIPLOMATIC_CONTENDER_LEADER_DVP)
            .map(|leader| leader.id)
            .filter(|leader| self.diplomatic_contender(g, pid, *leader))
    }

    /// `recovery-keeps-a-winning-war`: the Recovery plan offers "this is not
    /// the war the recovery plan is fighting" peace to every war but its
    /// target's. When that target is no war of ours, there is no other war
    /// it fights, and a war we outgun [`ONE_WAR_SECOND_FRONT_RATIO`] times
    /// over is kept. Forty such offers on 2026-10-04/05, sixteen to the only
    /// war we had, four of them at 1.5 times its power or more: live King
    /// civvis-20261005T014503Z (game 92) offered the Inca that peace at turn
    /// 85 at 321 power against 202 with Qusqu, their capital, the campaign's
    /// objective, and they took it at 86.
    /// `recovery-peace-waits`: whether the Recovery plan has stood for
    /// [`RECOVERY_PEACE_PATIENCE`] turns (always, with the gene off). A
    /// threatened city flips the grand strategy to Recovery for a turn or
    /// two and back: across October 4-5 the seat entered Recovery 4.9 times a
    /// game, the median spell lasted 2 turns and 248 of 331 spells 3 turns or
    /// fewer, and its clause offered peace to every war but the plan's 315
    /// times, accepted 22 times, 7 of them while we held 1.5 times the
    /// rival's power (live King civvis-20261004T201619Z: Gaul at turn 209,
    /// 982 against 701; T140744Z: Norway at 224, 1,119 against 604).
    pub(crate) fn recovery_peace_ready(&self, g: &Game) -> bool {
        !self.recovery_peace_waits
            || self
                .recovery_since
                .is_some_and(|since| g.turn.saturating_sub(since) >= RECOVERY_PEACE_PATIENCE)
    }

    /// `favor-spares-the-surprise-war`: whether a surprise war on `target`
    /// gives way to the denouncement and its Formal War five turns on, because
    /// a living rival stands at [`FAVOR_SURPRISE_DVP`] or more Diplomatic
    /// Victory points -- the Congress ballots that hold it back are bought
    /// with Favor -- unless `target`'s culture finish reads within
    /// [`FAVOR_SURPRISE_CLOCK_TURNS`] or its faith is at match point. A
    /// surprise war sets the target's grievances against us at 300 and the
    /// Favor they cost runs every turn. Live King civvis-20261005T114715Z
    /// (game 126): Favor rose 4-6 a turn from 162 to 186; the culture
    /// counter's surprise war on Sweden at 187 turned it to -3 to -5 a turn
    /// (about 110 Favor in 14 turns, 184 -> 0 by 212) while Sweden climbed
    /// from 11 to 17 Diplomatic Victory points, and the session at 221 cast
    /// one free vote; a Holy War on Byzantium at 240, with a casus belli,
    /// drained nothing (read by -60). Of the 15 Diplomatic losses of October
    /// 4-5, 8 met their last session at 0 Favor. 51 of the 147 declarations
    /// of those days were surprise wars, 36 of them neither urgent nor a
    /// culture embargo.
    pub(crate) fn favor_spares_surprise(&self, g: &Game, pid: usize, target: usize) -> bool {
        if !self.favor_spares_the_surprise_war {
            return false;
        }
        let contender = g.players.iter().any(|p| {
            p.id != pid && p.alive && !p.is_minor && !p.is_barbarian && p.dvp >= FAVOR_SURPRISE_DVP
        });
        let clock_out = self
            .observed_culture_finish(g, target)
            .is_some_and(|turns| turns <= FAVOR_SURPRISE_CLOCK_TURNS)
            || self.faith_at_match_point(g, target);
        contender && !clock_out
    }

    pub(crate) fn recovery_keeps_the_war(
        &self,
        g: &Game,
        pid: usize,
        other: usize,
        plan: &super::StrategicPlan,
    ) -> bool {
        self.recovery_keeps_a_winning_war
            && plan
                .target_player
                .is_none_or(|target| target == other || !g.is_at_war(pid, target))
            && g.military_power(pid)
                >= ONE_WAR_SECOND_FRONT_RATIO * g.military_power(other).max(1.0)
    }

    /// A front we outgun [`ONE_WAR_CRUSHED_RATIO`] times over. Peace there
    /// hands a beaten rival the turns to rebuild: on King
    /// `civvis-20260929T020236Z` the seat offered Norway peace at 812
    /// military against 36 to counter Mali's 61% culture reading; Norway
    /// accepted at turn 188, rebuilt to 1,308 military and more than tripled
    /// its visiting tourists, and won on culture at 206. Only an urgent
    /// clock is worth freeing the army from such a front.
    fn one_war_front_crushed(&self, g: &Game, pid: usize, other: usize) -> bool {
        g.military_power(pid) >= ONE_WAR_CRUSHED_RATIO * g.military_power(other).max(1.0)
    }

    /// A front we outgun [`ONE_WAR_WINNING_RATIO`] times over, or one of
    /// whose cities our siege train is reducing or taking, is being won
    /// whatever the recent exchange says. Live King civvis-20261001T033711Z:
    /// the first Reduce of the session, on unwalled Samarobriva, ended the
    /// same turn with "the last window was a rout" at 477 power against 169.
    /// civvis-20261001T022028Z offered Arabia the same peace at 1,461
    /// against 66, with Cairo its last city.
    fn one_war_still_winning(&self, g: &Game, pid: usize, other: usize) -> bool {
        g.military_power(pid) >= ONE_WAR_WINNING_RATIO * g.military_power(other).max(1.0)
            || self.siege_reducing_a_city_of(g, other)
    }

    /// Whether our siege train is reducing or taking one of `other`'s cities.
    fn siege_reducing_a_city_of(&self, g: &Game, other: usize) -> bool {
        self.sieges.iter().any(|(cid, siege)| {
            matches!(
                siege.stage,
                super::siege_train::SiegeStage::Reduce | super::siege_train::SiegeStage::Take
            ) && g.cities.get(cid).is_some_and(|city| city.owner == other)
        })
    }

    /// A Domination seat crushing a rival [`ONE_WAR_CRUSHED_RATIO`] times
    /// over, or reducing one of its cities, is not fatigued by a stalled
    /// front: peace hands the rival the turns to rebuild, and the next war
    /// restarts staging from nothing. Live King civvis-20261001T030914Z
    /// offered Ethiopia "the war has stalled" peace at turn 92 at 518 power
    /// against 66. civvis-20261001T050754Z offered Sweden the same at 519
    /// against 225 with Uppsala breached and near 10% health for eight turns.
    pub(crate) fn domination_front_crushed(&self, g: &Game, pid: usize, other: usize) -> bool {
        self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && (self.one_war_front_crushed(g, pid, other)
                || self.siege_reducing_a_city_of(g, other)
                || self.domination_capital_prey(g, pid, other)
                || self.holds_bleeding_capital_of(g, pid, other))
    }

    /// Whether a faithless Domination seat should declare on `rival` without
    /// a staged siege: `rival`'s faith is taking our cities
    /// (`domination_faithless_conversion_counter`) and we have
    /// [`ONE_WAR_SECOND_FRONT_RATIO`] times its power. At war our units may
    /// condemn its Missionaries and Apostles in our own land, which needs no
    /// army at its cities. Live King civvis-20261003T155014Z (game 41): with
    /// the Cree as the second front, a replay held the war from turn 89 to
    /// 113 for "0 staged on its ring" while the army besieged America, at
    /// 1.9 to 2.3 times their power; the Cree took our cities and won at 126.
    pub(crate) fn faith_counter_due(&self, g: &Game, pid: usize, rival: usize) -> bool {
        self.faith_counter(g, pid, rival)
            && g.military_power(pid)
                >= self.second_front_ratio(rival) * g.military_power(rival).max(1.0)
            // See `faith_counter_has_the_edge`.
            && self.faith_counter_has_the_edge(g, pid, rival)
            // See `faith_at_match_point`.
            && (!self.faith_counter_waits_for_match_point || self.faith_at_match_point(g, rival))
            // See `counter_war_has_the_emperor_edge`.
            && self.counter_war_has_the_emperor_edge(g, pid, rival)
    }

    /// `faith-counter-waits-for-match-point`: whether `rival`'s faith holds
    /// every living major but one, the religion lane's match point. The
    /// faithless counter's early warning is half the majors, and on a
    /// four-major map the founder and our own converted majority make that
    /// half from the first missionary: 96 of the 128 declarations of October
    /// 4-5 were on a rival whose faith our cities followed, 91 of them
    /// measured against their first objective. Those wars made contact (3
    /// land units within two tiles) in 33 and took the city in 9, against 20
    /// and 8 of the other 39; 36 of the 91 aimed past nine tiles of our
    /// nearest city. Nor did they hold the faith back: 20 turns on, our
    /// cities on it had fallen in 36, risen in 23 and held in 37, and about
    /// twelve Religious defeats came on October 4 with the counter live. Under
    /// the gene the declaration skips its staged siege only at match point;
    /// before it the counter still names the campaign target, and the
    /// ordinary readiness gates decide the war.
    pub(crate) fn faith_at_match_point(&self, g: &Game, rival: usize) -> bool {
        let living = g
            .players
            .iter()
            .filter(|p| p.alive && !p.is_minor && !p.is_barbarian)
            .count() as i32;
        let pressure = self.rival_victory_pressure(g, rival);
        let progress = if pressure.strategy == GrandStrategy::Religion {
            pressure.progress
        } else {
            self.lane_progress_table(g, rival)[2]
        };
        living > 1 && progress >= 100 * (living - 1) / living
    }

    /// `culture-counter-declares`: whether a Domination seat declares on
    /// `rival`, whose culture clock is urgent, without a staged siege, at
    /// [`CULTURE_COUNTER_RATIO`] times its power. The war itself works on the
    /// clock: it ends the open borders and trade route that carry their
    /// tourism to us, and opens their Theater Squares to our raiders, while
    /// the army stages at war. Live King civvis-20261004T122037Z (game 56)
    /// aimed at France from turn 163 and read "the army has not finished
    /// staging" every turn to 181, at 2.0 to 3.9 times France's power; France
    /// won on Culture at 181 with 102 foreign tourists against our 52
    /// domestic, at peace with us all game.
    pub(crate) fn culture_counter_due(&self, g: &Game, pid: usize, rival: usize) -> bool {
        self.culture_counter_declares
            && self.urgent_victory_threat(g, rival)
            && self.culture_lane_threat(g, rival)
            && g.military_power(pid) >= CULTURE_COUNTER_RATIO * g.military_power(rival).max(1.0)
            // See `counter_war_has_the_emperor_edge`.
            && self.counter_war_has_the_emperor_edge(g, pid, rival)
    }

    /// `counter-war-needs-the-emperor-edge`: whether a counter war that opens
    /// without a staged siege may open on `rival`: always with the gene off,
    /// and under it at [`COUNTER_WAR_EMPEROR_EDGE`] times the rival's steady
    /// power (`steady_rival_power`). Emperor G188
    /// (civvis-20261006T031849Z) declared the culture counter on the
    /// Mapuche at turn 170 at 1,062 against 662; our military stood at 732
    /// thirty turns on and 320 by 225, the Mapuche's at 1,659. G192
    /// (civvis-20261006T041656Z) declared it on Greece at 160 at 1,075
    /// against 693; ours was 454 ten turns on, Greece rebuilt to 1,732 by
    /// 190 and held four of our ten cities by 200.
    pub(crate) fn counter_war_has_the_emperor_edge(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> bool {
        !self.counter_war_needs_the_emperor_edge
            || g.military_power(pid)
                >= COUNTER_WAR_EMPEROR_EDGE * self.steady_rival_power(g, rival).max(1.0)
    }

    /// `culture-counter-declares`: a culture rival at match point, at peace
    /// with us, whose cities we have not found. The ordinary declaration
    /// needs a target city in reach. This war needs none: it ends the open
    /// borders and trade route that carry their tourism to us. Live King
    /// civvis-20261004T153748Z (game 64) never located a Maya city on a
    /// four-player Pangaea in 175 turns. Maya's visitors went from 16 to 82
    /// between turns 140 and 174 against our 45 to 89 domestic, and it won
    /// on Culture at 175 at peace with us. In game 61 Norway's Tourism rose
    /// from 266 to 404 within six turns of a peace.
    ///
    /// `counter-war-needs-the-emperor-edge`: never under the gene. Emperor
    /// G191 (civvis-20261006T040519Z) embargoed India at turn 160, "their
    /// culture race reads 80% and no city of theirs is located", at 448
    /// military against 331; by the Culture loss at 188 India's stood at 541
    /// and ours at 405, and no siege could follow a war with no objective.
    /// Over October 5-6 the five embargo wars took no city.
    pub(crate) fn culture_embargo_target(&self, g: &Game, pid: usize) -> Option<usize> {
        if !self.culture_counter_declares || self.counter_war_needs_the_emperor_edge {
            return None;
        }
        g.players
            .iter()
            .filter(|rival| {
                rival.id != pid && rival.alive && !rival.is_minor && !rival.is_barbarian
            })
            .map(|rival| rival.id)
            .filter(|rival| !g.is_at_war(pid, *rival) && g.player_city_ids(*rival).is_empty())
            .filter(|rival| self.culture_counter_due(g, pid, *rival))
            .max_by_key(|rival| {
                (
                    self.rival_culture_progress(g, *rival),
                    std::cmp::Reverse(*rival),
                )
            })
    }

    /// Declare the war `culture_embargo_target` names, when the treasury can
    /// carry it. Whether a declaration was made.
    pub(crate) fn culture_embargo_war(&mut self, g: &mut Game, pid: usize) -> bool {
        let Some(rival) = self.culture_embargo_target(g, pid) else {
            return false;
        };
        if g.turn < self.peace_until || !self.war_is_affordable(g, pid) {
            return false;
        }
        let Some(action) = self.preferred_war_opening(g, pid, rival) else {
            return false;
        };
        let progress = self.rival_culture_progress(g, rival);
        think!(self.journal(), Military, Strategy,
            "Declaring war on {}", g.players[rival].civ;
            "their culture race reads {progress}% and no city of theirs is located; \
             the war ends the open borders and trade route that carry their tourism to us");
        self.base.war_eve_liquidation(g, pid, &action);
        g.apply(pid, &action).is_ok()
    }

    /// `one-war-swaps-a-stalled-front`: another enemy to move the front to,
    /// when the current `front` has stalled. Stalled means: chosen at least
    /// [`FRONT_STALL_TURNS`] standard turns ago, with no front city at a new
    /// low of health in that time. The enemy must hold an original capital
    /// Domination needs, within declaration range, and we must hold
    /// [`STALLED_FRONT_SWAP_RATIO`] times its power. The front choice
    /// otherwise sticks while its war lasts. Live King
    /// civvis-20261004T160213Z (game 65) fought Germany from turn 37 to 129
    /// without taking Hamburg. The Maori were at war with us all that time
    /// at 122 to 199 power against our 384 to 695, their capital reachable by
    /// land (a Cuirassier stood beside it at turn 121). The seat offered them
    /// peace as "not the one". That capital was the last one taken, at
    /// turn 255.
    pub(crate) fn stalled_front_swap(
        &self,
        g: &Game,
        pid: usize,
        front: usize,
        enemies: &[usize],
    ) -> Option<usize> {
        if !self.one_war_swaps_a_stalled_front
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
        {
            return None;
        }
        let state = self
            .one_war
            .as_ref()
            .filter(|state| state.target == front)?;
        let window = g.standard_duration(FRONT_STALL_TURNS);
        if g.turn.saturating_sub(state.since) < window {
            return None;
        }
        let progressing = g.player_city_ids(front).iter().any(|cid| {
            self.front_city_low
                .get(&g.cities[cid].pos)
                .is_some_and(|(_, set)| g.turn.saturating_sub(*set) < window)
        });
        if progressing {
            return None;
        }
        let power = g.military_power(pid);
        enemies
            .iter()
            .copied()
            .filter(|rival| *rival != front)
            .filter(|rival| power >= STALLED_FRONT_SWAP_RATIO * g.military_power(*rival).max(1.0))
            .filter_map(|rival| {
                let (_, capital) = self.domination_capital_target_for(g, pid, Some(rival))?;
                Self::city_within_declaration_range(g, pid, g.cities[&capital].pos)
                    .then(|| (g.military_power(rival), rival))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
            .map(|(_, rival)| rival)
    }

    /// `rival`'s culture lane alone, as a percent of the bar: its foreign
    /// tourists against the largest domestic count among the other living
    /// majors, the comparison Firaxis makes. `rival_victory_pressure` keeps
    /// only a rival's highest lane, so a score lead or Diplomatic Victory
    /// points can stand in front of a culture race a war can still slow.
    pub(crate) fn rival_culture_progress(&self, g: &Game, rival: usize) -> i32 {
        let bar = g
            .players
            .iter()
            .filter(|p| p.alive && !p.is_minor && !p.is_barbarian && p.id != rival)
            .map(|p| g.domestic_tourists(p.id))
            .max()
            .unwrap_or(1)
            .max(1);
        // See `engine_culture_pressure`.
        self.engine_culture_pressure(
            g,
            rival,
            (100 * g.foreign_tourists(rival) / bar).clamp(0, 100) as i32,
        )
    }

    /// `culture-counter-declares`: whether a Domination seat counts `rival`'s
    /// culture race on its own lane, at the culture threat bar, whatever the
    /// rival's highest lane reads. Live King civvis-20261004T140744Z (game
    /// 61) was at war with Norway, the culture leader, from turn 40. At 224,
    /// with Norway's visitors at 124 against the largest staycation of 161
    /// (77%), Norway's score lead (86) and Diplomatic Victory points (80)
    /// stood in front of its culture lane, so it was no counter target. The
    /// Recovery plan offered it "this is not the war the recovery plan is
    /// fighting". Its Tourism rose from 266 to 404 within six turns of the
    /// peace, and it won on Culture at 234.
    pub(crate) fn culture_lane_threat(&self, g: &Game, rival: usize) -> bool {
        self.culture_counter_declares
            && self.deny_leaders
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && Self::victory_strategy_enabled(g, GrandStrategy::Culture)
            && self.rival_culture_progress(g, rival) >= self.culture_threat_pressure()
    }

    /// Whether a counter-war on `rival`'s religious clock falls under
    /// [`COUNTER_WAR_POWER_FLOOR`]. The clock is religious when the rival's
    /// highest lane reads Religion, or when its founded faith already holds
    /// our majority, whatever its highest lane reads: a score lead can stand
    /// in front of the religion lane. Live King civvis-20261004T205431Z
    /// (game 79) held four of seven cities under Scythia's Zoroastrianism at
    /// turn 72, Scythia led on score, and the seat declared on Scythia at 73
    /// at 344 power against 560 (0.61).
    pub(crate) fn counter_war_hopeless(&self, g: &Game, pid: usize, rival: usize) -> bool {
        let religious = self.rival_victory_pressure(g, rival).strategy == GrandStrategy::Religion
            || g.players[rival]
                .religion
                .as_deref()
                .is_some_and(|faith| g.civ_follows_religion(pid, faith));
        // `counter-war-needs-parity`: any other clock needs our equal power.
        let floor = if religious {
            COUNTER_WAR_POWER_FLOOR
        } else if self.counter_war_needs_parity {
            COUNTER_WAR_PARITY
        } else {
            return false;
        };
        g.military_power(pid) < floor * g.military_power(rival)
    }

    /// Whether a siege on one of the front's cities is live: not Hold, read
    /// this turn or the last, past Stage or entered within
    /// [`FRONT_SIEGE_LIVE_TURNS`] standard turns, and its city at a new low
    /// of health within that window. A siege that only stands is not one the
    /// army must finish first: live King civvis-20261004T111442Z (game 53)
    /// besieged Madrid from turn 44 to 152, sixty-four turns of them in Invest
    /// or Reduce, and never took it; Madrid's lowest health came at turn 62.
    pub(crate) fn front_siege_live(&self, g: &Game) -> bool {
        let Some(front) = self.one_war_front() else {
            return false;
        };
        let window = g.standard_duration(FRONT_SIEGE_LIVE_TURNS);
        self.sieges.iter().any(|(cid, siege)| {
            g.cities.get(cid).is_some_and(|city| {
                city.owner == front
                    && self
                        .front_city_low
                        .get(&city.pos)
                        .is_some_and(|(_, set)| g.turn.saturating_sub(*set) <= window)
            }) && siege.stage != super::siege_train::SiegeStage::Hold
                && g.turn.saturating_sub(siege.assessed) <= 1
                && (siege.stage != super::siege_train::SiegeStage::Stage
                    || g.turn.saturating_sub(siege.entered) <= window)
        })
    }

    /// `front-finishes-its-siege`: an urgent counter-war already running
    /// waits for the front's siege of an unwalled or breached city past
    /// Stage, read this turn or the last, while that city has set a new low
    /// of health within [`FRONT_SIEGE_LIVE_TURNS`]. The urgent clause in
    /// `one_war_choose_front` moved the army at once. Live King
    /// civvis-20261004T212049Z (game 80) had unwalled Viseu in Invest at turn
    /// 81, damage ready in 3.2 turns, 338 strength against a bill of 66; at
    /// 82 an urgent counter turned the front to the Zulu, already at war, and
    /// to walled Kwahlomendlini sixteen tiles away. Viseu was never taken,
    /// and neither was Kwahlomendlini.
    pub(crate) fn front_siege_to_finish(&self, g: &Game) -> bool {
        let Some(front) = self
            .one_war_front()
            .filter(|_| self.front_finishes_its_siege)
        else {
            return false;
        };
        let window = g.standard_duration(FRONT_SIEGE_LIVE_TURNS);
        self.sieges.iter().any(|(cid, siege)| {
            g.cities.get(cid).is_some_and(|city| {
                city.owner == front
                    && city.wall_hp <= 0
                    && self
                        .front_city_low
                        .get(&city.pos)
                        .is_some_and(|(_, set)| g.turn.saturating_sub(*set) <= window)
            }) && matches!(
                siege.stage,
                super::siege_train::SiegeStage::Invest
                    | super::siege_train::SiegeStage::Reduce
                    | super::siege_train::SiegeStage::Take
            ) && g.turn.saturating_sub(siege.assessed) <= 1
        })
    }

    /// `front-finishes-its-capital`: an urgent counter on `threat` — a war
    /// already running (`one_war_choose_front`) or a second front opened at
    /// peace (`one_war_second_front`) — waits for the front's siege of a
    /// city behind at most [`CAPITAL_PREY_WALLS`] of wall: the front rival's
    /// original capital or last city in any stage, any other city past Stage.
    /// The siege is not Hold, read this turn or the last, its stage entered
    /// within [`FRONT_CAPITAL_FINISH_TURNS`] standard turns — unless
    /// `threat`'s culture finish is projected inside that window.
    /// `front_siege_to_finish` holds only an unwalled city past Stage. Live
    /// King civvis-20261005T061801Z (game 105) had Pharsalos in Invest at
    /// turn 187, walls 28/400, damage ready in 4.1 turns at 1,275 strength
    /// against a bill of 122; at 188 Canada, at peace and the eventual
    /// culture winner, took the plan as an urgent second front, the
    /// declaration held for staging and then range, and Pharsalos stood at
    /// 400/400 again by 209. Live
    /// King civvis-20261005T060002Z (game 104) staged Canberra, Australia's
    /// original capital and last visible city, from turn 85 to 90 with 14
    /// units, walls 100, 318 strength against a bill of 48 and damage ready
    /// in 4.2 turns; an urgent counter took the army to Norway at 91, and
    /// Canberra had no siege row again for sixty turns while Australia held
    /// 18 to 40 military (diagnosed by -60).
    pub(crate) fn front_capital_to_finish(&self, g: &Game, threat: usize) -> bool {
        let Some(front) = self
            .one_war_front()
            .filter(|_| self.front_finishes_its_capital)
        else {
            return false;
        };
        let window = g.standard_duration(FRONT_CAPITAL_FINISH_TURNS);
        if self
            .observed_culture_finish(g, threat)
            .is_some_and(|turns| turns < f64::from(window))
        {
            return false;
        }
        let last_city = g.player_city_ids(front).len() == 1;
        self.sieges.iter().any(|(cid, siege)| {
            let past_stage = matches!(
                siege.stage,
                super::siege_train::SiegeStage::Invest
                    | super::siege_train::SiegeStage::Reduce
                    | super::siege_train::SiegeStage::Take
            );
            g.cities.get(cid).is_some_and(|city| {
                city.owner == front
                    && city.wall_hp <= CAPITAL_PREY_WALLS
                    && ((city.is_capital && city.original_owner == front)
                        || last_city
                        || past_stage)
            }) && siege.stage != super::siege_train::SiegeStage::Hold
                && g.turn.saturating_sub(siege.assessed) <= 1
                && g.turn.saturating_sub(siege.entered) <= window
        })
    }

    /// `religious-threat-spares-the-front`: a rival whose clock is religious
    /// does not take the army off its front while our cities keep the faith
    /// we founded. A Religious Victory needs our majority too, and a war on
    /// the rival's cities does not defend it; the counter is our own faith
    /// (`conversion_majority_alarm` and the inquisitors), and the rival stays
    /// a counter target for the declaration and the peace desk. Live King
    /// civvis-20261005T060002Z (game 104) read Norway's Orthodoxy, holding
    /// Greece, Australia and Norway, as urgent at turn 91 while all our
    /// cities kept Buddhism, and moved the army off Canberra (diagnosed by
    /// -60).
    pub(crate) fn religious_threat_spares_the_front(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> bool {
        self.religious_threat_spares_the_front
            && self.rival_victory_pressure(g, rival).strategy == GrandStrategy::Religion
            && g.players[pid]
                .religion
                .as_deref()
                .is_some_and(|ours| g.civ_follows_religion(pid, ours))
    }

    /// Whether the second front `rival` is declared on while the army stays
    /// on the front: a faith counter that is not urgent, beside a live front
    /// siege. The war on the faith condemns its spreaders in our own land,
    /// and that needs no army at its cities; the plan hands over once the
    /// front's siege ends. Live King civvis-20261004T040138Z (game 47) had
    /// Quebec City in Invest at turn 90, at 536 power against Canada's 80;
    /// at 92 the Ethiopian faith took the second front, the plan's target
    /// moved twelve tiles to Popayán, and the Quebec army turned around.
    /// Popayán stayed "0 of 10 staged", Quebec was never taken, and by 126
    /// the empire held nothing new at 822 against 139 (diagnosed by -60).
    pub(crate) fn second_front_waits_for_the_front(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> bool {
        !self.urgent_victory_threat(g, rival)
            && self.faith_counter(g, pid, rival)
            && self.front_siege_live(g)
    }

    /// `second-front-waits-for-its-war`: whether the second front `rival`,
    /// still at peace with us, leaves the plan's target on the war already
    /// running. It waits while the front is at war with us and still holds a
    /// city within the declaration range of ours; the diplomacy desk declares
    /// on it meanwhile (`waiting_second`), and once at war it takes the plan as
    /// before. An urgent clock and the Diplomatic Victory leader keep their
    /// claim on the plan: those are the fronts a running war must not delay.
    /// The Board writes Siege rows only for the plan's target and the
    /// campaign's cities while their owner is at war with us, so a plan aimed
    /// at a rival still at peace left the running war without a Siege row and
    /// the second front without a staged army. Live King
    /// civvis-20261005T162932Z (game 143): war on America from turn 88; from
    /// 99 the campaign read "a second front" on Egypt (a faith counter), the
    /// Washington row vanished, and the war on America produced nothing for 51
    /// turns until Egypt was declared on at 150. October 5: 308 such war-turns
    /// in 20 of 58 runs (census by -60's fork).
    pub(crate) fn second_front_waits_for_its_war(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> bool {
        if !self.second_front_waits_for_its_war
            || g.is_at_war(pid, rival)
            || self.urgent_victory_threat(g, rival)
            || self.diplomatic_contender_leader(g, pid) == Some(rival)
        {
            return false;
        }
        let Some(front) = self.one_war_front().filter(|front| *front != rival) else {
            return false;
        };
        g.is_at_war(pid, front)
            && g.player_city_ids(front)
                .into_iter()
                .any(|cid| Self::city_within_declaration_range(g, pid, g.cities[&cid].pos))
    }

    /// A faithless Domination seat whose cities `rival`'s faith is taking:
    /// `domination_faithless_conversion_counter`. See `faith_counter_due`.
    pub(crate) fn faith_counter(&self, g: &Game, pid: usize, rival: usize) -> bool {
        self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && g.players[pid].religion.is_none()
            && self.domination_faithless_conversion_counter(
                g,
                pid,
                rival,
                self.rival_victory_pressure(g, rival),
            )
    }

    /// Whether a war on `other`, beside the front, is one the Domination
    /// counter wants kept: `other`'s clock is urgent, its land is the only
    /// road to a target (`war_holds_the_road`), or it is a counter target we
    /// outgun [`ONE_WAR_SECOND_FRONT_RATIO`] times over. See `one_war_peace`.
    pub(crate) fn second_front_war_kept(&self, g: &Game, pid: usize, other: usize) -> bool {
        self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && g.is_at_war(pid, other)
            && (self.urgent_victory_threat(g, other)
                // See `war_holds_the_road`.
                || self.war_holds_the_road(g, pid, other)
                || (self.domination_counter_target(g, pid, other)
                    && g.military_power(pid)
                        >= ONE_WAR_SECOND_FRONT_RATIO * g.military_power(other).max(1.0))
                // See `second_front_kept_when_winning`.
                || (self.second_front_kept_when_winning
                    && (self.one_war_front_crushed(g, pid, other)
                        || (self.one_war_still_winning(g, pid, other)
                            && g.cities
                                .values()
                                .any(|city| city.owner == pid && city.original_owner == other))))
                // See `second_front_kept_when_winning_2`.
                || (self.second_front_kept_when_winning_2
                    && self.one_war_still_winning(g, pid, other)
                    && g.cities.values().any(|city| {
                        city.is_capital && city.original_owner == other && city.owner == other
                    }))
                // See `diplomatic_contender`.
                || self.diplomatic_contender(g, pid, other)
                // See `capital_prey_kept`.
                || self.capital_prey_kept(g, pid, other)
                // See `second_front_recently_named`.
                || self.second_front_recently_named(g, other))
    }

    /// `stalled-peace-spares-the-counter`: whether the fatigue clause's "the
    /// war has stalled" peace spares the war on `other`, a rival whose victory
    /// clock the Domination army answers (`domination_counter_target`) or one
    /// close to winning (`urgent_victory_threat`). The Recovery clause already
    /// spares a counter target; the fatigue clause did not. Live King
    /// civvis-20261005T033442Z (game 96) offered the Khmer this peace at turn
    /// 207 at 942 power against 555, with their visitors at 98 against the
    /// largest staycation of 140 (70%; the culture bar is 50). They took it,
    /// their Tourism rose from 240 to 357 through the open borders and trade
    /// route the war had closed, the seat declared on them again at 219 to
    /// close them, and they won on Culture at 223. Across 30 recent losses the
    /// eventual winner was offered this peace within 25 turns of the end in
    /// four (G95 the Zulu at 208-222, lost on Diplomacy at 222).
    ///
    /// Nor a war whose land is the only road to the front
    /// (`war_holds_the_road`, as the Recovery clause already reads it). Live
    /// King civvis-20261005T045443Z (game 101) offered Korea this peace at
    /// turn 184 at 1,255 power against 707 while the campaign stood on the
    /// Maya behind Korea's land; Korea took it, its borders closed, and the
    /// Siege of Wak Kab'nal read "0 of 8 units staged" to the end.
    pub(crate) fn stalled_peace_spares(&self, g: &Game, pid: usize, other: usize) -> bool {
        self.stalled_peace_spares_the_counter
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && (self.domination_counter_target(g, pid, other)
                || self.urgent_victory_threat(g, other)
                || self.war_holds_the_road(g, pid, other))
    }

    /// `capital-prey-opens-a-front`: the weakest rival beside the front whose
    /// own original capital is Domination progress the army can take now —
    /// military at most [`CAPITAL_PREY_POWER`] of ours, the capital known and
    /// behind at most [`CAPITAL_PREY_WALLS`] of wall, within the declaration
    /// range and reachable by land — while the front holds no live siege to
    /// finish first. A prey already at war is read the same way, less the
    /// walls and the legality (the war is what opens them), and its war is
    /// kept (`capital_prey_kept`). The second item names the rivals that fail
    /// exactly one gate, for the journal.
    ///
    /// Domination needs every original capital, and a non-capital city of the
    /// burning war is economy, not progress. Across games 92-103 the seat held
    /// one rival original capital (Ottawa, game 102). Live King
    /// civvis-20261005T053701Z (game 103) was at war with the Maori and then
    /// Vietnam while the Inca held 12 military against our ~600 at turn 100,
    /// Qusqu behind 100 walls nine tiles from Guayaquil; the seat offered the
    /// Inca "one war at a time" peace seven times and first staged Qusqu at
    /// turn 159, behind 300 walls (diagnosed by -60).
    pub(crate) fn capital_prey_beside_the_front(
        &self,
        g: &Game,
        pid: usize,
        front: Option<usize>,
    ) -> (Option<usize>, Vec<(usize, &'static str)>) {
        let armed = self.capital_prey_opens_a_front || self.capital_prey_opens_a_front_2;
        if !armed
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || (front.is_none() && !self.capital_prey_opens_a_front_2)
            || (front.is_some() && self.front_siege_live(g))
        {
            return (None, Vec::new());
        }
        let ours = g.military_power(pid);
        let mut best: Option<(f64, usize)> = None;
        let mut near_misses = Vec::new();
        for rival in g.players.iter().filter(|other| {
            other.id != pid
                && Some(other.id) != front
                && other.alive
                && !other.is_minor
                && !other.is_barbarian
                && g.has_met(pid, other.id)
        }) {
            let Some(capital) = g
                .player_city_ids(rival.id)
                .into_iter()
                .map(|cid| &g.cities[&cid])
                .find(|city| city.is_capital && city.original_owner == rival.id)
            else {
                continue;
            };
            let power = self.steady_rival_power(g, rival.id);
            let at_war = g.is_at_war(pid, rival.id);
            let weak = power <= CAPITAL_PREY_POWER * ours;
            let soft = at_war || capital.wall_hp <= self.capital_prey_walls(power, ours);
            // A prey already at war still needs a road: an overseas or far
            // capital would take the plan's target from the front for a march
            // the army cannot make (see `declarable_in_reach`).
            let reach = (Self::city_within_declaration_range(g, pid, capital.pos)
                || self.capital_prey_reaches_far(g, pid, rival.id, capital.pos))
                && self.rival_reachable_by_land(g, pid, rival.id)
                && (at_war || self.campaign_target_legal(g, pid, rival.id));
            let gates = match (weak, soft, reach) {
                (true, true, true) => {
                    if best.is_none_or(|(old, _)| power < old) {
                        best = Some((power, rival.id));
                    }
                    continue;
                }
                (false, true, true) => "power",
                (true, false, true) => "walls",
                (true, true, false) => "reach",
                (false, false, true) => "power and walls",
                (false, true, false) => "power and reach",
                (true, false, false) => "walls and reach",
                (false, false, false) => continue,
            };
            near_misses.push((rival.id, gates));
        }
        // A true match point outranks the prey: an urgent rival whose culture
        // finish is projected sooner than the prey's capture plus
        // [`CAPITAL_PREY_MATCH_MARGIN`]. No other lane carries a finish clock,
        // and the urgent flag alone misread Norway's religion at 75% while
        // our own cities held Buddhism (game 104).
        if let Some((_, prey)) = best {
            let eta = self.capital_prey_eta(g, pid, prey);
            let match_point = self
                .actionable_victory_denial(g, pid)
                .is_some_and(|(rival, _)| {
                    rival != prey
                        && self.urgent_victory_threat(g, rival)
                        && self
                            .observed_culture_finish(g, rival)
                            .is_some_and(|finish| finish < eta + CAPITAL_PREY_MATCH_MARGIN)
                });
            if match_point {
                return (None, near_misses);
            }
        }
        (best.map(|(_, rival)| rival), near_misses)
    }

    /// The most wall a prey capital at peace may stand behind:
    /// [`CAPITAL_PREY_WALLS`], and under `capital-prey-scales-the-walls` that
    /// bar raised in proportion as the prey's share of our power falls under
    /// [`CAPITAL_PREY_POWER`] -- 150 at a tenth, 200 at 7.5% -- up to
    /// [`CAPITAL_PREY_MAX_WALLS`]. Live King civvis-20261005T101841Z (game
    /// 120) logged "Capital prey near miss: Australia | fails the walls gate"
    /// at turn 94; at turn 100 Australia held 31 military against our 435
    /// (7%), Canberra known behind 200 walls. `declaration-waits-for-the-breaker`
    /// still holds the war until a breaker stands on the ring (diagnosed by
    /// -60).
    fn capital_prey_walls(&self, power: f64, ours: f64) -> i32 {
        if !self.capital_prey_scales_the_walls || ours <= 0.0 {
            return CAPITAL_PREY_WALLS;
        }
        let share = (power / ours).max(1e-9);
        let scaled = f64::from(CAPITAL_PREY_WALLS) * CAPITAL_PREY_POWER / share;
        (scaled.min(f64::from(CAPITAL_PREY_MAX_WALLS)).round() as i32).max(CAPITAL_PREY_WALLS)
    }

    /// The turns until `prey`'s original capital is expected to fall: our
    /// nearest land soldier's march at two tiles a turn, plus
    /// [`CAPITAL_PREY_SIEGE_TURNS`].
    fn capital_prey_eta(&self, g: &Game, pid: usize, prey: usize) -> f64 {
        let Some(capital) = g
            .cities
            .values()
            .find(|city| city.owner == prey && city.is_capital && city.original_owner == prey)
            .map(|city| city.pos)
        else {
            return f64::INFINITY;
        };
        let march = self
            .one_war_soldiers(g, pid)
            .into_iter()
            .map(|pos| g.wdist(pos, capital))
            .min()
            .map_or(f64::INFINITY, |tiles| f64::from(tiles) / 2.0);
        march + CAPITAL_PREY_SIEGE_TURNS
    }

    /// `prey-reads-a-steady-power`: record each living rival's military this
    /// turn, keeping [`PREY_POWER_MEMORY_TURNS`] turns of readings.
    pub(crate) fn record_rival_power(&mut self, g: &Game, pid: usize) {
        let rivals: Vec<usize> = g
            .players
            .iter()
            .filter(|p| p.id != pid && p.alive && !p.is_minor && !p.is_barbarian)
            .map(|p| p.id)
            .collect();
        // `declaration-needs-the-edge-2`: its own, longer memory, so the
        // prey gates' three-turn reading is unchanged.
        if self.declaration_needs_the_edge_2 {
            for &rival in &rivals {
                let seen = self.rival_power_peak_seen.entry(rival).or_default();
                seen.retain(|(turn, _)| {
                    *turn != g.turn && g.turn.saturating_sub(*turn) < DECLARATION_PEAK_TURNS
                });
                seen.push((g.turn, g.military_power(rival)));
            }
        } else {
            self.rival_power_peak_seen.clear();
        }
        if !self.prey_reads_a_steady_power {
            self.rival_power_seen.clear();
            return;
        }
        for rival in rivals {
            let seen = self.rival_power_seen.entry(rival).or_default();
            seen.retain(|(turn, _)| {
                *turn != g.turn && g.turn.saturating_sub(*turn) < PREY_POWER_MEMORY_TURNS
            });
            seen.push((g.turn, g.military_power(rival)));
        }
    }

    /// The military the prey gates read for `rival`: the host's reading, and
    /// under `prey-reads-a-steady-power` the largest of it and the readings
    /// of the last [`PREY_POWER_MEMORY_TURNS`] turns. The public reading
    /// can collapse for one turn and come back: live King
    /// civvis-20261005T103704Z (game 121) read America at 155, 147, 152, 144
    /// and 138 over turns 90-94 at peace, 4 at 95, and 151 again by 102; the
    /// seat declared on it at 95 "179 power against their 4". Over October
    /// 4-5 a rival's reading fell by three quarters in one turn and recovered
    /// to 60% within ten 21 times (diagnosed with -60).
    /// `domination-finish-holds-the-front`: the rival at war with us holding a
    /// city whose capture completes Domination (`capture_completes_domination`
    /// -- every other original capital already ours), with that city. While
    /// it stands, its owner holds the front and the plan's target under any
    /// grand strategy, ahead of the elimination front, the diplomatic
    /// contender, the urgent clause and the capital hop; no second front is
    /// opened (`one_war_second_front`); and the city outranks a committed
    /// objective at full health (`assess`). Live King civvis-20261005T141932Z
    /// (game 135) held Aduatuca and Wak Kab'nal from turn 188 with Madrid,
    /// held by Gaul whom we fought at twice its power, the last capital it
    /// needed; the campaign stayed "committed" to three other Gallic cities to
    /// 211, a Recovery flip at 224 dropped the finishing front, and the second
    /// front opened on the Maya at 19 points -- six cities no war could
    /// eliminate in time -- who won on Diplomacy at 232.
    pub(crate) fn domination_finish_front(&self, g: &Game, pid: usize) -> Option<(usize, u32)> {
        if !self.domination_finish_holds_the_front
            || self.forced_target_player.is_some()
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
        {
            return None;
        }
        g.cities
            .values()
            .filter(|city| {
                city.owner != pid
                    && g.is_at_war(pid, city.owner)
                    && Self::capture_completes_domination(g, pid, city.id)
            })
            .map(|city| (city.owner, city.id))
            .min()
    }

    /// `declaration-needs-the-edge`: whether a plain staged declaration on
    /// `target` has the edge -- our military at [`DECLARATION_EDGE_RATIO`]
    /// times its steady reading (`steady_rival_power`). Always with the gene
    /// off. The declaration's urgent clock and its counters (religion,
    /// culture, air, overwhelming power) keep their own waivers. Of the 186
    /// live declarations of October 4-5, the 25 made under 1.5 times the
    /// target's power took a city of theirs within 20 turns twice (0 of 15
    /// after turn 100); one was declared at 0.24 times (game 74, t185, on
    /// Mali behind 400 walls). Live King civvis-20261005T141932Z (game 135)
    /// opened a surprise war on the Maya at turn 48 at 196 power against 193
    /// because the army stood staged within reach.
    pub(crate) fn declaration_has_the_edge(&self, g: &Game, pid: usize, target: usize) -> bool {
        if self.declaration_needs_the_edge_2 {
            return self.declaration_edge_2(g, pid, target).passes();
        }
        !self.declaration_needs_the_edge
            || g.military_power(pid)
                >= DECLARATION_EDGE_RATIO * self.steady_rival_power(g, target).max(1.0)
    }

    /// `declaration-needs-the-edge-2`: the readings the version-2 edge
    /// decides on. A staged declaration passes at
    /// [`DECLARATION_PEAK_EDGE_RATIO`] times the target's peak power of the
    /// last [`DECLARATION_PEAK_TURNS`] turns, or, when the target makes less
    /// Production than we do, at version 1's [`DECLARATION_EDGE_RATIO`] times
    /// its steady power. Of the 180 live declarations of October 4-5 with
    /// ten turns seen after them, 45 were routs: our power fell under theirs
    /// or we sued for peace for a rout. The rival did not levy or buy its way
    /// back (3 of the 45 spent 50 Gold on the jump turn); it out-built us,
    /// its power x1.23 by ten turns on and x1.43 by twenty against our x0.87.
    /// Rival Production over ours read AUC 0.80 for the rout (medians 0.85
    /// converted, 1.47 routed) and our power over its 30-turn peak 0.79
    /// (2.46 against 1.25). The rule blocks 61 of the 119 staged
    /// declarations, 29 of their 30 routs and 2 of their 26 captures, and the
    /// staged wars it keeps take a city within 20 turns in 41% (22% before).
    /// Live King civvis-20261005T185206Z (game 153) declared on Spain at
    /// turns 100 and 120 at 611 and 712 against 305 and 282; Spain made 189
    /// and 231 Production to our 85 and 131, and both wars ended in a rout.
    pub(crate) fn declaration_edge_2(&self, g: &Game, pid: usize, target: usize) -> DeclarationEdge {
        DeclarationEdge {
            ours: g.military_power(pid),
            peak: self.peak_rival_power(g, target),
            steady: self.steady_rival_power(g, target),
            our_production: crate::ai::BasicAi::seat_production_per_turn(g, pid),
            their_production: crate::ai::BasicAi::seat_production_per_turn(g, target),
        }
    }

    /// `declaration-needs-production-parity`: `rival`'s Production a turn,
    /// when the board can read it. A native board holds every city, so its
    /// cities are the whole reading. A board that carries public figures for
    /// `rival` -- the live mirror, or a fogged native view -- holds only the
    /// cities in view, and its total is the host's public Production, carried
    /// as the production term of `observed_yield_adjustments`; the mirror
    /// leaves that term at zero when the host does not report it, and the
    /// fogged view never fills it. `None` then: there is no reading.
    pub(crate) fn rival_production_reading(g: &Game, rival: usize) -> Option<f64> {
        let partial = g.observed_public_empire_stats.contains_key(&rival)
            || g.observed_yield_adjustments.contains_key(&rival);
        let reported = g
            .observed_yield_adjustments
            .get(&rival)
            .is_some_and(|adjustment| adjustment.production != 0.0);
        (!partial || reported).then(|| crate::ai::BasicAi::seat_production_per_turn(g, rival))
    }

    /// `declaration-needs-production-parity`: whether an offensive
    /// declaration on the major `target` may open -- our Production a turn
    /// (`BasicAi::seat_production_per_turn`, the host's figure on the live
    /// seat) at [`DECLARATION_PRODUCTION_PARITY`] times the target's or
    /// more. Always with the gene off, and when the board has no reading of
    /// the target's Production (`rival_production_reading`). Says so when it
    /// holds.
    pub(crate) fn declaration_has_production_parity(
        &self,
        g: &Game,
        pid: usize,
        target: usize,
    ) -> bool {
        if !self.declaration_needs_production_parity {
            return true;
        }
        let Some(theirs) = Self::rival_production_reading(g, target) else {
            return true;
        };
        let ours = crate::ai::BasicAi::seat_production_per_turn(g, pid);
        if ours >= DECLARATION_PRODUCTION_PARITY * theirs {
            return true;
        }
        think!(self.journal(), Military, Detail,
               "Holding off war with {}", g.players[target].civ;
               "our production {ours:.0} vs their {theirs:.0}: an offensive war needs {:.1} times \
                their Production; live Emperor declarations under it took one city in sixteen",
               DECLARATION_PRODUCTION_PARITY);
        false
    }

    /// `declaration-needs-the-edge-2`: the largest of `rival`'s steady
    /// reading and its readings of the last [`DECLARATION_PEAK_TURNS`] turns.
    pub(crate) fn peak_rival_power(&self, g: &Game, rival: usize) -> f64 {
        self.rival_power_peak_seen
            .get(&rival)
            .into_iter()
            .flatten()
            .filter(|(turn, _)| g.turn.saturating_sub(*turn) < DECLARATION_PEAK_TURNS)
            .map(|(_, power)| *power)
            .fold(self.steady_rival_power(g, rival), f64::max)
    }

    /// `faith-counter-needs-the-edge`: whether the religion counter may
    /// declare on `rival` without a staged siege: always with the gene off,
    /// and under it at [`DECLARATION_EDGE_RATIO`] times its steady power.
    /// Live King civvis-20261005T184245Z (game 152) countered Indonesia's
    /// faith at turn 82 at 358 against 350, was routed by 91 (258 against
    /// 338), declared again at 101 at 449 against 388, took nothing, and lost
    /// to Indonesia's religion at 157. October 4-5's three counter wars under
    /// 1.5 times took no city.
    pub(crate) fn faith_counter_has_the_edge(&self, g: &Game, pid: usize, rival: usize) -> bool {
        !self.faith_counter_needs_the_edge
            || g.military_power(pid)
                >= DECLARATION_EDGE_RATIO * self.steady_rival_power(g, rival).max(1.0)
    }

    /// `urgent-denial-needs-the-edge`: whether a rival's urgent victory clock
    /// may waive the staged war's ratio and the version-2 edge: always with
    /// the gene off, and under it at [`DECLARATION_EDGE_RATIO`] times the
    /// rival's steady power. Live King civvis-20261006T001108Z (game 175)
    /// declared on Nubia at turn 168 at 1034 against 738 under the urgent
    /// waiver; our power fell to 547 within seven turns and Nubia won
    /// diplomatically at 242. October 4-5's staged declarations under 1.5
    /// times routed 13 of 17.
    pub(crate) fn urgent_denial_has_the_edge(&self, g: &Game, pid: usize, rival: usize) -> bool {
        !self.urgent_denial_needs_the_edge
            || g.military_power(pid)
                >= DECLARATION_EDGE_RATIO * self.steady_rival_power(g, rival).max(1.0)
    }

    /// `diplomatic-contender-eliminated`: the rival a Domination seat must
    /// eliminate: at war with us and holding cities, at
    /// [`ELIMINATION_CONTENDER_DVP`] Diplomatic Victory points or more, and
    /// under our military [`ELIMINATION_POWER_RATIO`] times over its steady
    /// reading; the one with the most points. Only elimination takes those
    /// points off the board -- a captured capital or town removes none -- so
    /// the front holds on it until it holds no city, ahead of a second front
    /// and of the next capital (`domination_followup_target`). Fourteen, not
    /// [`DIPLOMATIC_CONTENDER_DVP`]: a leader the rivals' B has knocked to 14
    /// is exactly the one they stop ganging on (0 of 15 after-B sessions at
    /// 14 or less) and that takes +5 at the next session. Live King
    /// civvis-20261005T134450Z (game 133) took Constantinople at t209, then
    /// moved the front to Korea's capital at t218 while Byzantium sat at 14
    /// points with 6 cities at 6-8 times less military than ours; Byzantium
    /// took +5 to 19 at t221 and won on Diplomacy at 241.
    pub(crate) fn diplomatic_contender_to_eliminate(&self, g: &Game, pid: usize) -> Option<usize> {
        // See `elimination_yields_to_a_shorter_clock` (`capital-taken-moves-on`).
        self.diplomatic_contender_base(g, pid)
            .filter(|rival| !self.elimination_yields_to_a_shorter_clock(g, pid, *rival))
    }

    /// `diplomatic_contender_to_eliminate` before `capital-taken-moves-on`'s
    /// yield to a shorter clock.
    pub(crate) fn diplomatic_contender_base(&self, g: &Game, pid: usize) -> Option<usize> {
        if !self.diplomatic_contender_eliminated
            || self.forced_target_player.is_some()
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
        {
            return None;
        }
        let ours = g.military_power(pid);
        self.one_war_enemies(g, pid)
            .into_iter()
            .filter(|rival| {
                g.players[*rival].dvp >= ELIMINATION_CONTENDER_DVP
                    && ours >= ELIMINATION_POWER_RATIO * self.steady_rival_power(g, *rival).max(1.0)
            })
            .max_by_key(|rival| (g.players[*rival].dvp, std::cmp::Reverse(*rival)))
    }

    /// `contender-at-peace-is-the-target`: with no war running, the rival a
    /// Domination seat goes to war on next: at peace with us and holding
    /// cities, at [`ELIMINATION_CONTENDER_DVP`] Diplomatic Victory points or
    /// more, under our military [`ELIMINATION_POWER_RATIO`] times over its
    /// steady reading, legal to target, and with a city inside the
    /// declaration range; the one with the most points. Ahead of the denial
    /// counter's pick in `assess`. `diplomatic_contender_to_eliminate` holds a
    /// front on a contender we already fight, and the second-front leader
    /// rule needs a war already running. Live King civvis-20261005T150407Z
    /// (game 138) finished Georgia at turn 200 and aimed at Ethiopia, then at
    /// 201 -- Ethiopia on 14 points at 2,171 power against 549 -- the counter
    /// named Mali's space race and the war went to Mali. Ethiopia took +5 to
    /// 19 at the 202 session, was never at war with us, and won on Diplomacy
    /// at 235.
    pub(crate) fn diplomatic_contender_at_peace(&self, g: &Game, pid: usize) -> Option<usize> {
        if !self.contender_at_peace_is_the_target
            || self.forced_target_player.is_some()
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !self.one_war_enemies(g, pid).is_empty()
        {
            return None;
        }
        let ours = g.military_power(pid);
        g.players
            .iter()
            .filter(|rival| {
                rival.id != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && rival.dvp >= ELIMINATION_CONTENDER_DVP
                    && !g.is_at_war(pid, rival.id)
                    && self.campaign_target_legal(g, pid, rival.id)
                    && ours
                        >= ELIMINATION_POWER_RATIO * self.steady_rival_power(g, rival.id).max(1.0)
                    && g.player_city_ids(rival.id)
                        .iter()
                        .any(|city| Self::city_within_declaration_range(g, pid, g.cities[city].pos))
            })
            .max_by_key(|rival| (rival.dvp, std::cmp::Reverse(rival.id)))
            .map(|rival| rival.id)
    }

    /// `diplomatic-contender-eliminated`: the contender's city nearest the
    /// field army's median unit (our nearest city when there is no army).
    /// Elimination needs every city, so the capital has no precedence.
    pub(crate) fn elimination_objective_city(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> Option<u32> {
        let army = self.campaign_field_army(g, pid);
        let ours = g.player_city_ids(pid);
        g.cities
            .values()
            .filter(|city| city.owner == rival)
            .map(|city| {
                let mut distances: Vec<i32> = army
                    .iter()
                    .map(|uid| g.wdist(g.units[uid].pos, city.pos))
                    .collect();
                distances.sort_unstable();
                let reach = distances
                    .get(distances.len() / 2)
                    .copied()
                    .unwrap_or_else(|| {
                        ours.iter()
                            .map(|mine| g.wdist(g.cities[mine].pos, city.pos))
                            .min()
                            .unwrap_or(i32::MAX)
                    });
                (reach, city.id)
            })
            .min()
            .map(|(_, city)| city)
    }

    /// `overwhelming-power-declares`: whether a Domination seat at
    /// [`OVERWHELMING_POWER_RATIO`] times `target`'s steady power
    /// (`steady_rival_power`) declares without a staged siege. Before a war
    /// the Objective Board writes no Siege row, so nothing orders the army
    /// onto the objective's ring, and the staged checks wait on units that
    /// only drift there; once the war opens the row exists and the army
    /// converges. Live King civvis-20261005T130519Z (game 131) held its war
    /// on the Inca every turn from 105 to 125 -- "the Siege row for
    /// Antawaylla asks 232-309 strength and 33-66 is staged on its ring in
    /// 1-2 bodies" -- at 906 power against 70, then 1316 against 159; game
    /// 130 waited on Maastricht at turn 101 with no Siege row on the board.
    /// Another major war, the Defend row, the named objective, the
    /// declaration range and the war opening (casus belli first) all stand.
    ///
    /// Power alone converts nothing: of the 186 live declarations of October
    /// 4-5, those at 5 or more times the target's power took a city of
    /// theirs within 20 turns 7 times in 11 when the field army's median unit
    /// stood within 10 tiles of the objective, and 0 times in 9 when it stood
    /// further out (all declarations: 37% at 6 tiles or less, 3% at 16 or
    /// more). So the waiver also asks that median to be at most
    /// [`OVERWHELMING_MARCH_TILES`].
    pub(crate) fn overwhelming_power_declares(
        &self,
        g: &Game,
        pid: usize,
        target: usize,
        objective: Pos,
    ) -> bool {
        if !self.overwhelming_power_declares
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || g.military_power(pid)
                < OVERWHELMING_POWER_RATIO * self.steady_rival_power(g, target).max(1.0)
        {
            return false;
        }
        let mut distances: Vec<i32> = self
            .campaign_field_army(g, pid)
            .iter()
            .map(|uid| g.wdist(g.units[uid].pos, objective))
            .collect();
        distances.sort_unstable();
        distances
            .get(distances.len() / 2)
            .is_some_and(|median| *median <= OVERWHELMING_MARCH_TILES)
    }

    pub(crate) fn steady_rival_power(&self, g: &Game, rival: usize) -> f64 {
        let now = g.military_power(rival);
        if !self.prey_reads_a_steady_power {
            return now;
        }
        self.rival_power_seen
            .get(&rival)
            .into_iter()
            .flatten()
            .filter(|(turn, _)| g.turn.saturating_sub(*turn) < PREY_POWER_MEMORY_TURNS)
            .map(|(_, power)| *power)
            .fold(now, f64::max)
    }

    /// `rout-spares-the-counter`: whether a bad window offers `other` no
    /// peace because it is the rival we are countering -- its clock urgent,
    /// the actionable denial's rival, or a culture lane at the threat bar --
    /// and we still hold at least its power. The war is the counter: the
    /// peace is undone at the next counter turn, with the siege's progress
    /// and the grievances already paid. 51 rout offers of October 4-5 were
    /// made from the stronger army and 15 were followed by our own
    /// declaration on the same rival within twenty turns, 7 of them while
    /// countering; `rout-spares-a-stronger-army` spares only 1.5 times over.
    /// Live King civvis-20261005T083500Z (game 113) offered Spain "the last
    /// window was a rout" at turn 216 at 560 power against 483, then named it
    /// the counter's second front at 218.
    pub(crate) fn rout_spares_the_counter(&self, g: &Game, pid: usize, other: usize) -> bool {
        self.rout_spares_the_counter
            && g.military_power(pid) >= g.military_power(other).max(1.0)
            && (self.urgent_victory_threat(g, other)
                || self
                    .actionable_victory_denial(g, pid)
                    .is_some_and(|(rival, _)| rival == other)
                || self.culture_lane_threat(g, other))
    }

    /// `peace-asks-a-city`: whether a white peace offer to `other` also asks
    /// it to cede a town — not routed, from [`super::PEACE_CITY_ASK_RATIO`]
    /// times its power.
    pub(crate) fn peace_asks_city_from_strength(&self, g: &Game, pid: usize, other: usize) -> bool {
        self.peace_asks_a_city
            && !self.peace_routed.contains(&other)
            && g.military_power(pid)
                >= super::PEACE_CITY_ASK_RATIO * g.military_power(other).max(1.0)
    }

    /// `capital-prey-opens-a-front`: whether the war on `other` is a capital
    /// prey's, kept beside the front whatever the front's siege: at war, its
    /// military at most [`CAPITAL_PREY_POWER`] of ours, and its own original
    /// capital still in its hands.
    pub(crate) fn capital_prey_kept(&self, g: &Game, pid: usize, other: usize) -> bool {
        self.capital_prey_opens_a_front
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && g.is_at_war(pid, other)
            && self.steady_rival_power(g, other) <= CAPITAL_PREY_POWER * g.military_power(pid)
            && g.cities
                .values()
                .any(|city| city.owner == other && city.is_capital && city.original_owner == other)
    }

    /// Whether a Domination seat holds `rival`'s original capital while the
    /// city's Loyalty is falling. A peace then hands the capital back: the
    /// war is what finds and takes the cities whose pressure is draining it.
    /// Live King civvis-20261003T135713Z (game 38) took Ondini, the Zulu
    /// capital, at turn 57 at 33 Loyalty and -17 a turn, offered the Zulu
    /// peace twice the same turn (the opening's "taken every city it knows
    /// of" with Kwadukuza still in the fog, and the campaign's "has taken its
    /// 1 city"), and the city was the Zulu's again by turn 71.
    /// `liberation-funds-the-congress`: whether liberating captured `city` is
    /// worth its Favor: a Domination seat, the city founded by a city-state,
    /// and a living rival on at least [`LIBERATION_DVP_FLOOR`] Diplomatic
    /// Victory points. Liberating a city-state's city is worth 100 Favor
    /// (FAVOR_FOR_LIBERATE_CITY_STATE), about seven bought Congress votes,
    /// and a city-state's city is no Domination progress. Domination drains
    /// the bank it would need: each foreign original capital we hold costs 5
    /// Favor a turn (FAVOR_PER_OWNED_ORIGINAL_CAPITAL). Live King
    /// civvis-20261005T081917Z (game 112) kept Singapore at turn 191 with
    /// Kongo on 15 points; Favor fell 195 -> 91 under two held capitals, and
    /// at turn 201, the last two voters, Kongo's 8 A votes beat the 7 that
    /// 91 Favor buys and it won on Diplomacy, one capital short of our
    /// Domination.
    ///
    /// A town founded by an eliminated major is liberated too: that revives
    /// the civilization, worth 200 Favor (FAVOR_FOR_REVIVE_PLAYER), and leaves
    /// the Domination count as it was -- every original capital is needed
    /// whoever holds it. Its original capital itself is never handed back.
    /// Three October games had such a town in a third party's hands, among
    /// them game 114 (civvis-20261005T084952Z), whose Mamuell Mapu, founded by
    /// the Mapuche we eliminated, Vietnam held from turn 169 while Vietnam
    /// won on Diplomacy at 240 against our empty bank.
    pub(crate) fn liberation_funds_the_congress(&self, g: &Game, pid: usize, city: u32) -> bool {
        self.liberation_funds_the_congress
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && g.cities.get(&city).is_some_and(|city| {
                city.original_owner != pid
                    && g.players.get(city.original_owner).is_some_and(|founder| {
                        founder.is_minor
                            || (!founder.alive && !founder.is_barbarian && !city.is_capital)
                    })
            })
            && g.players.iter().any(|rival| {
                rival.id != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && rival.dvp >= LIBERATION_DVP_FLOOR
            })
    }

    /// `bleeding-capital-loyalty`: a captured original capital of ours whose
    /// Loyalty runway (`BasicAi::loyalty_emergency`) is under
    /// [`BLEEDING_CAPITAL_RUNWAY`] turns. A capture starts at 50 Loyalty;
    /// live King civvis-20261005T081917Z (game 112) took Xanadu at turn 119
    /// and lost it to the Free Cities at 124 at -6.5 to -14.5 a turn, a
    /// Governor appointed but not established and no Loyalty card slotted;
    /// civvis-20261005T061801Z lost Athens in two turns at -22. Limitanei
    /// (+2 with the garrison) and Victor's three-turn establishment
    /// (`BasicAi::victor_first_for_a_short_runway`) buy the turns to take
    /// the city exerting the pressure.
    pub(crate) fn bleeding_capital(&self, g: &Game, pid: usize) -> Option<u32> {
        g.cities
            .values()
            .filter(|city| city.owner == pid && city.is_capital && city.original_owner != pid)
            .find(|city| {
                self.base
                    .loyalty_emergency(g, city.id)
                    .is_some_and(|turns| turns < BLEEDING_CAPITAL_RUNWAY)
            })
            .map(|city| city.id)
    }

    pub(crate) fn holds_bleeding_capital_of(&self, g: &Game, pid: usize, rival: usize) -> bool {
        self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && g.cities.values().any(|city| {
                city.owner == pid
                    && city.is_capital
                    && city.original_owner == rival
                    && g.city_loyalty_per_turn(city) < 0.0
            })
    }

    /// A rival still holding its own original capital, which Domination
    /// needs, that we outgun [`ONE_WAR_WINNING_RATIO`] times over: a war to
    /// keep, and no ally to bind ourselves to. Live King
    /// civvis-20261003T040354Z offered Kongo "the war has stalled" peace at
    /// turn 136 at 613 power against 224, and proposed it a Research Alliance
    /// the same turn. Kongo held Kabasa, the last original capital the seat
    /// would need, and won on Science at turn 216 while the army finished
    /// Rome's towns.
    ///
    /// A living rival whose original capital we have not seen still holds
    /// it unless a known city says otherwise: game 29 (civvis-20261003T090618Z)
    /// proposed Canada a Research Alliance at turn 157 at 1,130 power against
    /// 431, with Ottawa in the fog, and lost to Canada's Culture at 169.
    pub(crate) fn domination_capital_prey(&self, g: &Game, pid: usize, other: usize) -> bool {
        self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && g.players
                .get(other)
                .is_some_and(|player| player.alive && !player.is_minor && !player.is_barbarian)
            && !g.player_city_ids(other).is_empty()
            && g.military_power(pid) >= ONE_WAR_WINNING_RATIO * g.military_power(other).max(1.0)
            && !g
                .cities
                .values()
                .any(|city| city.is_capital && city.original_owner == other && city.owner != other)
    }

    /// Whether the gene wants peace with `other` this turn, and why.
    pub(crate) fn one_war_peace(&self, g: &Game, pid: usize, other: usize) -> Option<OneWarPeace> {
        let front = self.one_war.as_ref().filter(|_| self.one_war_at_a_time)?;
        if !g.is_at_war(pid, other) || g.players[other].is_minor || g.players[other].is_barbarian {
            return None;
        }
        if front.target != other {
            // A war the Domination counter wants is not a second front to
            // close: `one_war_second_front` opened it on purpose. Live King
            // civvis-20261003T135713Z (game 38) declared on Scythia at turn
            // 208 for its culture surge, beside the war on Korea, and offered
            // it "one war at a time" peace the same turn.
            if self.second_front_war_kept(g, pid, other) {
                return None;
            }
            return Some(OneWarPeace::SecondFront);
        }
        if let Some(next) = self.domination_followup_target(g, pid, Some(other)) {
            let displaced = g
                .cities
                .values()
                .any(|city| city.is_capital && city.original_owner == other && city.owner == next);
            return Some(if displaced {
                OneWarPeace::CapitalElsewhere
            } else {
                OneWarPeace::CapitalSecured
            });
        }
        // Keep the current front until peace is actually accepted. A public
        // victory clock is a reason to offer peace, never proof that the old
        // enemy has stopped attacking or permission to erase its threat field.
        // A front that is itself a threat, or a capital Domination needs at
        // our mercy, is not traded for another rival's clock: game 29 offered
        // Canada this peace every turn from 146 at 866-1,125 power against
        // 218-362, to answer Georgia's faith; Canada accepted at 157 and won
        // on Culture at 169. An urgent rival opens the second front beside
        // it instead (`one_war_second_front`).
        if self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && self.forced_target_player.is_none()
            && !g.emergency_war_pair(pid, other)
            && !self.urgent_victory_threat(g, other)
            && !self.domination_capital_prey(g, pid, other)
            && !self.holds_bleeding_capital_of(g, pid, other)
            && !self.domination_counter_target(g, pid, other)
            && self.nearest_finish_culture_clock(g, other).is_none()
            // The conquest opening's own war answers to `conquest_peace`, not
            // to another rival's clock. Live King 2026-10-03T103619Z declared
            // on the Maori at turn 41 with 83% of the strike force at the
            // rally and had Ngaruawahia at 182 of 200 when this clause
            // offered peace at turn 55 ("freeing the Domination army ... 167
            // power against their 147"); the Maori accepted and the opening
            // was released with the city standing.
            && !self.conquest_opening_war(other)
            && self
                .actionable_victory_denial(g, pid)
                .is_some_and(|(rival, counter)| {
                    // A war just opened is not traded for a clock that only
                    // now outranks it: game 36 (civvis-20261003T123922Z)
                    // declared on the Ottomans at turn 103 and offered them
                    // this peace at 105 and 106.
                    let fresh_front = g.turn.saturating_sub(front.since)
                        < g.standard_duration(ONE_WAR_FRESH_FRONT_TURNS);
                    rival != other
                        && counter == GrandStrategy::Conquest
                        && self.domination_counter_target(g, pid, rival)
                        // See `COUNTER_WAR_POWER_FLOOR`.
                        && !self.counter_war_hopeless(g, pid, rival)
                        && (self.urgent_victory_threat(g, rival)
                            || (!fresh_front && !self.one_war_front_crushed(g, pid, other)))
                })
            && !self.one_war_capture_at_hand(g, pid, other)
            && !self.one_war_foothold_at_hand(g, pid, other)
        {
            return Some(OneWarPeace::VictoryThreat);
        }
        if self.one_war_still_winning(g, pid, other) {
            return None;
        }
        // `rout-spares-a-stronger-army`: a bad window at this margin is a
        // tactical loss, not a lost war. Live King civvis-20261005T003728Z
        // (game 89) offered Persia "the last window was a rout" peace at turn
        // 90 at 362 power against 229 and was denouncing Persia for the next
        // war at 92; T131543Z offered Russia the same at 615 against 410 and
        // declared on it again at 126, T232618Z India at 605 against 329 and
        // again at 159. If the losses go on, the margin falls and peace opens.
        if self.rout_spares_a_stronger_army
            && g.military_power(pid)
                >= ONE_WAR_SECOND_FRONT_RATIO * g.military_power(other).max(1.0)
        {
            return None;
        }
        // See `rout_spares_the_counter`.
        if self.rout_spares_the_counter(g, pid, other) {
            return None;
        }
        if front.window_net() <= ONE_WAR_ROUT_NET {
            return Some(OneWarPeace::Rout);
        }
        let against_for = front
            .tide_against_since
            .map(|since| g.turn.saturating_sub(since))?;
        if against_for >= g.standard_duration(ONE_WAR_TIDE_PATIENCE).max(1)
            && !self.one_war_prizes_in_reach(g, pid)
        {
            return Some(OneWarPeace::TideTurned);
        }
        None
    }

    /// Whether the gene keeps pressing the war on `other` against the
    /// fatigue clause. A breached city with a nearby capture body remains a
    /// finishable prize even if the recent exchange has turned against us.
    pub(crate) fn one_war_presses(&self, g: &Game, pid: usize, other: usize) -> bool {
        let Some(front) = self.one_war.as_ref().filter(|_| self.one_war_at_a_time) else {
            return false;
        };
        front.target == other
            && g.is_at_war(pid, other)
            && self
                .domination_followup_target(g, pid, Some(other))
                .is_none()
            && (self.one_war_capture_at_hand(g, pid, other)
                || (front.tide_against_since.is_none() && self.one_war_prizes_in_reach(g, pid)))
    }

    /// The rival a Domination seat must go to war with beside the war it is
    /// already fighting: the owner of the next original capital once the
    /// current front's capital is secured (`domination_followup_target`),
    /// or a rival whose victory clock is urgent, when we outgun it
    /// [`ONE_WAR_SECOND_FRONT_RATIO`] times over. The one-war gate kept the
    /// plan, the declaration and the front on the burning war. Live King
    /// civvis-20261003T040354Z held Rome's original capital from turn 199
    /// and spent turns 199-216 on Rome's towns at 2,114 power against
    /// Kongo's 1,347. Kongo, holding Kabasa (the last capital Domination
    /// needed) and past its Exoplanet launch at 198, won on Science at 216.
    pub(crate) fn one_war_second_front(&self, g: &Game, pid: usize) -> Option<usize> {
        // See `domination_finish_front`: no new war while the war that ends
        // the game stands.
        if self.domination_finish_front(g, pid).is_some() {
            return None;
        }
        // See `declarable_in_reach`.
        self.one_war_second_front_named(g, pid).filter(|rival| {
            !self.front_needs_a_declarable_rival
                || g.is_at_war(pid, *rival)
                || self.declarable_in_reach(g, pid, *rival)
        })
    }

    /// `front-needs-a-declarable-rival`: whether the declaration could reach
    /// `rival` — a city of theirs within the declaration range, or the far
    /// reach a denial earns (`denial_reaches_far`). A second front the
    /// declaration then holds off ("no city of theirs is within 18 tiles")
    /// takes the plan's target from the burning war and leaves the army with
    /// no Siege row at all. Live King civvis-20261005T060002Z (game 104) read
    /// "Campaign aimed at Greece" from turn 147 to 182+, Greece at peace and
    /// out of range, while Australia (military 18-57, its capital Canberra
    /// standing) and Norway were at war with us; the same aim at a held-off
    /// rival ran 27 turns in game 103 (Vietnam, the eventual culture winner)
    /// and 6-11 turns in five more games (census by -60).
    pub(crate) fn declarable_in_reach(&self, g: &Game, pid: usize, rival: usize) -> bool {
        g.player_city_ids(rival).into_iter().any(|cid| {
            let pos = g.cities[&cid].pos;
            Self::city_within_declaration_range(g, pid, pos)
                || self.denial_reaches_far(g, pid, rival, pos)
                || self.capital_prey_reaches_far(g, pid, rival, pos)
        })
    }

    /// `capital-prey-opens-a-front-2`: whether `objective` is `rival`'s own
    /// original capital, `rival` holds at most [`CAPITAL_PREY_DEEP_POWER`] of
    /// our military, and a city of ours stands within
    /// [`CAPITAL_PREY_DEEP_REACH`] of it. A dead rival's capital is worth a
    /// longer march than a parity war, and it is permanent Domination
    /// progress. Live King civvis-20261005T080337Z (game 111) left Spain, 49
    /// military against our 1,080 at turn 125, with Madrid behind 100 walls
    /// 21 tiles from our nearest city, while it held off a war on Persia at
    /// parity and declared none through turn 200 (diagnosed by -60).
    pub(crate) fn capital_prey_reaches_far(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
        objective: Pos,
    ) -> bool {
        self.capital_prey_opens_a_front_2
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && self.steady_rival_power(g, rival) <= CAPITAL_PREY_DEEP_POWER * g.military_power(pid)
            && g.city_at(objective).is_some_and(|cid| {
                let city = &g.cities[&cid];
                city.owner == rival && city.is_capital && city.original_owner == rival
            })
            && g.player_city_ids(pid)
                .into_iter()
                .any(|cid| g.wdist(g.cities[&cid].pos, objective) <= CAPITAL_PREY_DEEP_REACH)
    }

    /// `capital-prey-opens-a-front`: the prey, every turn, and each near miss
    /// when its failing gates change or once per [`CAPITAL_PREY_NOTE_TURNS`]
    /// standard turns, at war or at peace.
    pub(crate) fn journal_capital_prey(&mut self, g: &Game, pid: usize) {
        if !(self.capital_prey_opens_a_front || self.capital_prey_opens_a_front_2) {
            return;
        }
        let front = self.one_war_front();
        let (prey, near) = self.capital_prey_beside_the_front(g, pid, front);
        if let Some(prey) = prey {
            think!(self.journal(), Military, Detail,
                   "Capital prey beside the front: {}", g.players[prey].civ;
                   "its original capital is within reach behind light walls and its military is \
                    at most {:.0}% of ours", CAPITAL_PREY_POWER * 100.0);
        }
        let every = g.standard_duration(CAPITAL_PREY_NOTE_TURNS);
        for (rival, gates) in near {
            let due = self
                .capital_prey_noted
                .get(&rival)
                .is_none_or(|(noted, turn)| {
                    *noted != gates || g.turn.saturating_sub(*turn) >= every
                });
            if due {
                self.capital_prey_noted.insert(rival, (gates, g.turn));
                think!(self.journal(), Military, Detail,
                       "Capital prey near miss: {}", g.players[rival].civ;
                       "fails the {gates} gate");
            }
        }
    }

    fn one_war_second_front_named(&self, g: &Game, pid: usize) -> Option<usize> {
        if !self.one_war_at_a_time
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || self.forced_target_player.is_some()
        {
            return None;
        }
        let front_state = self.one_war.as_ref()?;
        let front = front_state.target;
        // See `capital_prey_beside_the_front`.
        if let Some(prey) = self.capital_prey_beside_the_front(g, pid, Some(front)).0 {
            return Some(prey);
        }
        // See `road_blocker_front` (`blocker-becomes-the-target`): the weak
        // major whose closed borders shut the front's road opens it.
        if let Some(blocker) = self
            .road_blocker_front(g, pid)
            .filter(|blocker| *blocker != front)
        {
            return Some(blocker);
        }
        // See `capital_moves_on_second_front` (`capital-taken-moves-on`): the
        // next capital's owner once the beaten rival refuses its peace.
        if let Some(next) = self.capital_moves_on_second_front(g, pid) {
            return Some(next);
        }
        // See `diplomatic_contender`: the Diplomatic Victory leader we crush
        // opens the second front at once, the front's refusal or not.
        if let Some(leader) = self.diplomatic_contender_leader(g, pid).filter(|leader| {
            *leader != front
                && !g.is_at_war(pid, *leader)
                && self.campaign_target_legal(g, pid, *leader)
        }) {
            return Some(leader);
        }
        // An offered peace is not a refused one: the front gets a few turns
        // to accept before a second war opens beside it.
        let refused = front_state.closure_wanted_since.is_some_and(|since| {
            g.turn.saturating_sub(since)
                >= g.standard_duration(ONE_WAR_SECOND_FRONT_PATIENCE).max(1)
        });
        // A capital-prey front, or the conquest opening's own war, is never
        // offered the peace that would free the army (`one_war_peace`), so
        // there is no refusal to wait for: an urgent rival opens beside it at
        // once.
        let prey_front =
            self.domination_capital_prey(g, pid, front) || self.conquest_opening_war(front);
        if !refused && !prey_front {
            return None;
        }
        let outguns = |rival: usize| {
            g.military_power(pid)
                >= self.second_front_ratio(rival) * g.military_power(rival).max(1.0)
        };
        let usable = |rival: usize| {
            rival != front
                && !g.is_at_war(pid, rival)
                && self.campaign_target_legal(g, pid, rival)
                && outguns(rival)
        };
        // A faith taking our cities need not be urgent. Its front is never
        // traded while it is capital prey, so waiting for urgency waits for
        // the loss: live King civvis-20261003T155014Z (game 41) held the Cree
        // as the faithless-conversion counter from turn 91, at 2.2 times their
        // power by 110, while the army besieged a prey America. The Cree read
        // urgent only at 116, at 1.07 times, and won on Religion at 126. The
        // war itself counters a faith (`faith_counter_due`); any other counter
        // still needs its siege staged, so it waits for urgency rather than
        // pull the army off a live siege for a war that cannot open.
        self.actionable_victory_denial(g, pid)
            .filter(|(rival, counter)| {
                *counter == GrandStrategy::Conquest
                    && ((self.urgent_victory_threat(g, *rival)
                        // See `front_capital_to_finish`.
                        && !self.front_capital_to_finish(g, *rival))
                        || self.faith_counter(g, pid, *rival))
            })
            .map(|(rival, _)| rival)
            .filter(|rival| usable(*rival))
            .or_else(|| {
                self.domination_followup_target(g, pid, Some(front))
                    .filter(|rival| refused && usable(*rival))
            })
    }

    /// Whether a declaration on `target` is held: a major war is already
    /// being fought against someone else and `target` is not about to win.
    pub(crate) fn one_war_holds_declaration(&self, g: &Game, pid: usize, target: usize) -> bool {
        if !self.one_war_at_a_time
            || g.is_at_war(pid, target)
            || self.one_war_second_front(g, pid) == Some(target)
        {
            return false;
        }
        let other_war = self
            .one_war_enemies(g, pid)
            .into_iter()
            .any(|enemy| enemy != target);
        other_war && !self.urgent_victory_threat(g, target)
    }

    /// A Joint War offer while any major war burns is a second front by
    /// treaty: refused outright, whatever the target is worth to the plan.
    pub(crate) fn one_war_refuses_joint_war(
        &self,
        g: &Game,
        pid: usize,
        deal: &DiplomaticDeal,
    ) -> bool {
        self.one_war_at_a_time
            && deal.joint_war_target.is_some()
            && !self.one_war_enemies(g, pid).is_empty()
    }

    /// The enemies the force planner aims a group at: the front alone, plus
    /// any enemy with a unit within relief range of a threatened city of
    /// ours, so the column still turns for a city about to fall. The full
    /// set when the gene is off or no front is chosen.
    pub(crate) fn one_war_objective_enemies(
        &self,
        g: &Game,
        threatened_city: Option<u32>,
        enemies: &[usize],
    ) -> Vec<usize> {
        let Some(front) = self.one_war_front() else {
            return enemies.to_vec();
        };
        if !enemies.contains(&front) {
            return enemies.to_vec();
        }
        let threatened = threatened_city
            .and_then(|cid| g.cities.get(&cid))
            .map(|city| city.pos);
        enemies
            .iter()
            .copied()
            .filter(|enemy| {
                *enemy == front
                    || threatened.is_some_and(|city| {
                        g.units.values().any(|unit| {
                            unit.owner == *enemy
                                && g.rules.units[unit.kind].class == "military"
                                && g.wdist(unit.pos, city) <= ONE_WAR_RELIEF_REACH
                        })
                    })
            })
            .collect()
    }
}

#[cfg(test)]
mod capital_handoff_tests;

#[cfg(test)]
mod culture_counter_tests;
#[cfg(test)]
mod urgent_denial_tests;
