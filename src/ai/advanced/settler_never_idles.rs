//! `settler-never-idles`: a Settler always has somewhere to go, and a hold
//! is a bounded, named exception rather than the default.
//!
//! Operator, 2026-08-27: *"our settlers often just sit around in the capital
//! or other cities. this is a huge mistake. investigate how this was allowed
//! to happen and fix the behavior so this can't happen."*
//!
//! ## How it was allowed to happen
//!
//! `advanced_settler_step` grew, one live anecdote at a time, more than a
//! dozen branches that leave a Settler exactly where it stands: a Loyalty
//! forecast that refuses a site, a fog guess (`beyond_loyalty_reach`) that
//! refuses every frontier site while any plot within nine tiles is
//! unexplored, a safe-step guard that refuses every neighbour, a guard that
//! has not arrived, a raider's reach it will not enter. Each hold was right
//! for the run that motivated it, and none was ever charged for the turns
//! it costs, because every one of the holds that matter is gated on a
//! host-only gene (`frontier-loyalty`, `live-formationless-settler-shadow`)
//! or on `loyalty-rate-alarm`, and the screen cannot price host-only genes
//! (`docs/AI_GAPS.md`, "the settler-idling lane cannot be screened at all").
//! Worst of all, when every preferred site is refused the code *held*
//! (`return false`) instead of asking a wider question — and every refused
//! site was retired for thirty standard turns, so the refusals compounded
//! into a permanent "no target".
//!
//! `settler_idle_census` (`src/ai/advanced/settler_idle_census.rs`) put
//! numbers on it, 6-player 60×38 Online, four maps each:
//!
//! | genome on a native board | settler-turns idle | idle on an own city tile | settlers idle ≥10 turns in a city | never moved |
//! |---|---|---|---|---|
//! | deployment (`enable_engine_repairs`) | 24.9% | 6.0% of all settler-turns | 4.0% | 11 of 151 |
//! | live seat (`enable_live_bridge`) | **87.0%** | **32.7%** of all settler-turns | **27.5%** | 76 of 389 |
//!
//! On the live seat's genome 62.5% of the idle turns were "no target" —
//! the search returned nothing and the code held; the five worst Settlers
//! stood in the city that built them for 149–185 turns and never moved.
//!
//! ## What the gene does
//!
//! 1. **Exhaustion never holds.** When the preferred search (every filter,
//!    the Loyalty verdict included) returns nothing, the Settler asks two
//!    wider questions before it stands still: the advanced ranking with the
//!    fog guesses and the thirty-turn retirements set aside and only a
//!    *concrete* revolt inside [`STRANDED_SITE_MIN_HOLD_TURNS`] refused
//!    (`settler_exhaustion_target`, tier 2); then any legal reachable site
//!    at all, nearest first (tier 3). Failing both it founds where it stands
//!    if the engine allows, and otherwise says so in the journal — a Settler
//!    that holds is never silent.
//! 2. **A watchdog bounds every other hold.** A Settler that has stood on
//!    the same tile for [`SETTLER_IDLE_PATIENCE`] turns stops trusting the
//!    branch that held it and marches on one rule only: never end the turn
//!    on a tile a visible hostile can reach next turn (`civilian_safe_at`
//!    over the barbarian reach, plus the hex-distance reach of every visible
//!    at-war major unit). That is the rule `civilian-out-of-reach` already
//!    plays and the screen priced at +29/+20/+31 wins per 10,000 seats; a
//!    softer risk score, a guard that has not come, a forecast about fogged
//!    ground — none of those may hold a Settler past its patience.
//! 3. **The guard wait returns the turn.** Under the gene the live seat's
//!    "waits for its guard" no longer reports the turn as spent, so the
//!    watchdog can see it.
//!
//! Off, every touched path is byte-identical to before.

use super::civilian_safety::{BarbarianReach, REACH_SCAN_RADIUS};
use super::{AdvancedAi, SETTLEMENT_GLOBAL_PREFILTER_LIMIT};
use crate::ai::BasicAi;
use crate::game::{Action, Game};
use crate::think;
use crate::Pos;

/// Turns a Settler may stand on one tile before the watchdog marches it on
/// the exact-reach rule alone. Two: one turn is the ordinary weather of a
/// guard arriving or a raider passing; the census's idle-in-city streaks
/// run to 185.
pub(super) const SETTLER_IDLE_PATIENCE: u32 = 2;
/// A site the engine's own Loyalty calculation says would revolt inside
/// this many turns is doomed however stranded the Settler is; anything
/// slower is a city for a while, which beats a Settler for ever.
pub(super) const STRANDED_SITE_MIN_HOLD_TURNS: f64 = 12.0;
/// How far a stranded Settler looks for any legal site before Shipbuilding.
const STRANDED_SITE_RADIUS: i32 = 14;
/// How many forecast refusals the exhaustion search pays before it stops
/// asking the forecast; each is a speculative founding on a cloned board.
const STRANDED_FORECAST_RETRIES: usize = 4;
/// Turns a Settler found stranded on a tile waits before the exhaustion
/// search is asked again from that tile. The board that stranded it changes
/// slowly — a razed rival city, a border that recedes — and the search is
/// the dearest thing a Settler does; a Settler that moves is asked at once.
pub(super) const STRANDED_RECHECK_TURNS: u32 = 5;

impl AdvancedAi {
    /// Advance this Settler's idle streak, once per turn, and return it.
    /// Zero the turn it appears and every turn it stands somewhere new.
    pub(super) fn note_settler_idle(&mut self, g: &Game, uid: u32) -> u32 {
        let pos = g.units[&uid].pos;
        let turn = g.turn;
        self.settler_idle_streak
            .retain(|other, _| g.units.contains_key(other));
        let entry = self
            .settler_idle_streak
            .entry(uid)
            .or_insert((pos, turn, 0));
        if entry.1 != turn {
            let streak = if entry.0 == pos { entry.2 + 1 } else { 0 };
            *entry = (pos, turn, streak);
        }
        entry.2
    }

    /// How long this Settler has stood on its tile, as last noted.
    pub(super) fn settler_idle_streak(&self, uid: u32) -> u32 {
        self.settler_idle_streak
            .get(&uid)
            .map_or(0, |(_, _, streak)| *streak)
    }

    /// Whether this Settler was found stranded on its current tile inside
    /// the last [`STRANDED_RECHECK_TURNS`] turns, so the exhaustion search
    /// need not be paid again yet.
    pub(super) fn settler_stranded_recently(&self, g: &Game, uid: u32) -> bool {
        self.settler_stranded_at
            .get(&uid)
            .is_some_and(|(pos, turn)| {
                g.units.get(&uid).is_some_and(|unit| unit.pos == *pos)
                    && g.turn < turn.saturating_add(STRANDED_RECHECK_TURNS)
            })
    }

    /// The arrival verdict for a site the exhaustion search chose: the fog
    /// guesses had their say when the walk began and may not refuse the
    /// founding at its end; only the engine's own Loyalty calculation, at
    /// the same [`STRANDED_SITE_MIN_HOLD_TURNS`] floor the choice used, may.
    /// Without this a relaxed target was refused on arrival by the strict
    /// verdict, retired, chosen again by the next exhaustion search, walked
    /// to again — 1,062 idle turns of that loop in eight live-genome games.
    pub(super) fn relaxed_arrival_verdict(g: &Game, pid: usize, site: Pos) -> Option<String> {
        Self::settle_site_forecast_revolt(g, pid, site)
            .filter(|(_, turns)| *turns < STRANDED_SITE_MIN_HOLD_TURNS)
            .map(|(per_turn, turns)| {
                format!(
                    "the city would lose {:.1} Loyalty a turn and revolt in about {:.0} turns",
                    -per_turn,
                    turns.ceil()
                )
            })
    }

    /// The wider questions asked when the preferred search returns nothing.
    ///
    /// Tier 2: the advanced ranking over a stranded radius, with this
    /// Settler's retired sites, the empire's threat deferrals and the fog
    /// guesses set aside; a candidate is refused only when the engine's own
    /// Loyalty calculation of a city founded there revolts inside
    /// [`STRANDED_SITE_MIN_HOLD_TURNS`]. Tier 3: any legal reachable site,
    /// nearest first. `None` when the map holds nothing this Settler can
    /// reach.
    pub(super) fn settler_exhaustion_target(&self, g: &Game, pid: usize, uid: u32) -> Option<Pos> {
        let from = g.units[&uid].pos;
        let radius = if g.players[pid].techs.contains(&crate::name!("shipbuilding")) {
            g.map.width + g.map.height
        } else {
            STRANDED_SITE_RADIUS
        };
        let mut ranked: Vec<(Pos, f64)> = self
            .settle_sites_with_limit(
                g,
                pid,
                from,
                radius,
                Some(SETTLEMENT_GLOBAL_PREFILTER_LIMIT),
            )
            .into_iter()
            .filter(|(pos, _)| !g.blocked_city_sites.contains(pos))
            .collect();
        for _ in 0..=STRANDED_FORECAST_RETRIES {
            let Some((site, value)) = BasicAi::first_reachable_settle_site(g, uid, &ranked) else {
                break;
            };
            match Self::settle_site_forecast_revolt(g, pid, site) {
                Some((per_turn, turns)) if turns < STRANDED_SITE_MIN_HOLD_TURNS => {
                    think!(self.journal(), Expansion, Detail,
                           "Settler skips a doomed site at {site:?}";
                           "stranded, it would still take a site the forecast set aside — but this \
                            one loses {:.1} Loyalty a turn and revolts in about {turns:.0}", -per_turn;
                           site);
                    ranked.retain(|(pos, _)| *pos != site);
                }
                _ => {
                    think!(self.journal(), Expansion, Detail,
                           "Settler takes a site the preferred search refused";
                           "nothing passed every filter, so {site:?} (worth {value:.1}) is taken \
                            rather than standing still"; site);
                    return Some(site);
                }
            }
        }
        // Tier 3: a city anywhere beats a Settler for ever. Nearest first,
        // the position the tie-break — deliberately no site value here: the
        // value is a growth forecast over a radius-2 disk per plot, and a
        // stranded Settler asks this every recheck for every legal plot in
        // the radius.
        let mut legal: Vec<(Pos, f64)> = g
            .wdisk(from, radius)
            .into_iter()
            .filter(|pos| {
                self.base.valid_settle_site(g, pid, *pos) && !g.blocked_city_sites.contains(pos)
            })
            .map(|pos| (pos, -(g.wdist(from, pos) as f64)))
            .collect();
        legal.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
        let site = BasicAi::first_reachable_settle_site(g, uid, &legal).map(|(pos, _)| pos)?;
        think!(self.journal(), Expansion, Detail,
               "Settler takes the nearest legal site at {site:?}";
               "no ranked site is reachable; a city {} tiles away beats a Settler standing still",
               g.wdist(from, site); site);
        Some(site)
    }

    /// Nothing is reachable: found here if the engine allows and the city
    /// would hold, else say so. Never silent.
    pub(super) fn settler_stranded(&mut self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let here = g.units[&uid].pos;
        self.settler_targets.remove(&uid);
        self.settler_stalls.remove(&uid);
        self.settler_closest.remove(&uid);
        if g.can_found_city(uid)
            && !Self::settle_site_forecast_revolt(g, pid, here)
                .is_some_and(|(_, turns)| turns < STRANDED_SITE_MIN_HOLD_TURNS)
        {
            let worth = self.settle_value(g, pid, here);
            let founded = g.apply(pid, &Action::FoundCity { unit: uid }).is_ok();
            if founded {
                think!(self.journal(), Expansion, Decision,
                       "Founding where the stranded settler stands at {here:?}";
                       "no other legal site is reachable; the site is worth {worth:.1}"; here);
                return true;
            }
        }
        think!(self.journal(), Expansion, Detail, "Settler is stranded at {here:?}";
               "no legal site is reachable and a city cannot be founded here; it holds until \
                the board changes"; here);
        self.settler_stranded_at.insert(uid, (here, g.turn));
        false
    }

    /// The watchdog's one rule: a tile is safe to end the turn on when no
    /// visible hostile can reach it next turn — the barbarian reach flood,
    /// and the movement allowance of every visible at-war major unit.
    fn watchdog_tile_is_safe(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        tile: Pos,
        reach: &BarbarianReach,
    ) -> bool {
        if !self.civilian_safe_at(g, pid, uid, tile, reach) {
            return false;
        }
        if g.city_at(tile)
            .is_some_and(|cid| g.cities[&cid].owner == pid)
        {
            return true;
        }
        let visible = self.battlefront_visibility(g, pid);
        !g.units.values().any(|unit| {
            if unit.owner == pid
                || g.players[unit.owner].is_barbarian
                || !g.is_at_war(pid, unit.owner)
                || !g.sees(&visible, unit.pos)
                || !g.unit_visible_to(unit.id, pid)
            {
                return false;
            }
            let spec = &g.rules.units[unit.kind];
            spec.class == "military"
                && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                && g.wdist(unit.pos, tile) <= spec.moves.ceil() as i32
        })
    }

    /// A Settler past its patience marches: toward its target if it holds a
    /// legal one, else toward what the exhaustion search finds, onto the
    /// first progressing tile the exact-reach rule allows. Founds when it
    /// stands on its target. Returns whether it acted.
    pub(super) fn settler_watchdog_step(&mut self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let current = g.units[&uid].pos;
        if g.units[&uid].moves_left <= 0.0 {
            return false;
        }
        let cached = self.settler_targets.get(&uid).copied().filter(|target| {
            (*target == current && g.can_found_city(uid))
                || (*target != current
                    && self.base.valid_settle_site(g, pid, *target)
                    && !g.blocked_city_sites.contains(target)
                    && g.route_step(uid, *target, 0).is_some())
        });
        let target = match cached {
            Some(target) => target,
            None => match self.settler_exhaustion_target(g, pid, uid) {
                Some(site) => {
                    self.settler_targets.insert(uid, site);
                    self.settler_relaxed_targets.insert(uid, site);
                    self.settler_stalls.remove(&uid);
                    self.settler_closest.remove(&uid);
                    site
                }
                None => return self.settler_stranded(g, pid, uid),
            },
        };
        if target == current {
            let worth = self.settle_value(g, pid, current);
            self.settler_targets.remove(&uid);
            let founded = g.apply(pid, &Action::FoundCity { unit: uid }).is_ok();
            if founded {
                think!(self.journal(), Expansion, Decision,
                       "Founding a city at {current:?} after the watchdog's march";
                       "the site is worth {worth:.1}"; current);
            }
            return founded;
        }
        // A linked Settler has no Move action of its own; the formation
        // that was to carry it is the thing that has not moved.
        if g.units[&uid].linked_to.is_some() {
            let _ = g.apply(pid, &Action::UnlinkUnits { unit: uid });
        }
        let reach = self.barbarian_reach(g, pid, current, REACH_SCAN_RADIUS);
        let distance = g.wdist(current, target);
        let mut steps: Vec<Pos> = Vec::new();
        if let Some(next) = g.route_step(uid, target, 0) {
            steps.push(next);
        }
        let mut progressing: Vec<Pos> = g
            .nbrs(current)
            .into_iter()
            .filter(|next| !steps.contains(next) && g.wdist(*next, target) < distance)
            .collect();
        progressing.sort_by_key(|next| (g.wdist(*next, target), *next));
        steps.extend(progressing);
        let streak = self.settler_idle_streak(uid);
        for next in steps {
            if !g.can_move(uid, next) || !self.watchdog_tile_is_safe(g, pid, uid, next, &reach) {
                continue;
            }
            if self.base.path_move(g, pid, uid, next) {
                think!(self.journal(), Expansion, Detail,
                       "Settler stops waiting and marches to {next:?}";
                       "it stood still {streak} turns; {target:?} is {distance} tiles away and \
                        nothing visible can reach {next:?} next turn"; next);
                self.settler_stalls.remove(&uid);
                return true;
            }
        }
        think!(self.journal(), Expansion, Detail,
               "Settler holds at {current:?}: every step toward {target:?} is in a hostile's reach";
               "it has stood still {streak} turns; the watchdog will not walk it into a capture";
               current);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::super::AdvancedAi;
    use super::*;
    use crate::ai::Ai;
    use crate::game::Game;
    use crate::reasoning::Journal;

    /// The gene ships off in both stock controllers; the toggles are twins.
    #[test]
    fn the_gene_is_off_in_both_controllers_and_the_toggles_are_twins() {
        let fresh = AdvancedAi::new();
        let legacy = AdvancedAi::legacy();
        assert!(!fresh.settler_never_idles);
        assert!(!legacy.settler_never_idles);
        let mut opted = AdvancedAi::new();
        opted.enable_settler_never_idles();
        assert!(opted.settler_never_idles);
        opted.disable_settler_never_idles();
        assert!(!opted.settler_never_idles);
    }

    /// A Settler that never moves counts up; one that moves resets; a
    /// second look in the same turn does not count twice.
    #[test]
    fn the_idle_streak_counts_turns_on_one_tile_once_each() {
        let mut g = Game::new_full(1, 12, 8, 91_301, 60, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .expect("a starting settler");
        let mut ai = AdvancedAi::new();
        ai.enable_settler_never_idles();
        assert_eq!(ai.note_settler_idle(&g, settler), 0);
        assert_eq!(
            ai.note_settler_idle(&g, settler),
            0,
            "same turn, same answer"
        );
        g.turn += 1;
        assert_eq!(ai.note_settler_idle(&g, settler), 1);
        g.turn += 1;
        assert_eq!(ai.note_settler_idle(&g, settler), 2);
        let here = g.units[&settler].pos;
        let step = g
            .nbrs(here)
            .into_iter()
            .find(|pos| g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile)))
            .expect("a land neighbour");
        g.units.get_mut(&settler).expect("settler").pos = step;
        g.turn += 1;
        assert_eq!(
            ai.note_settler_idle(&g, settler),
            0,
            "moving resets the streak"
        );
    }

    /// The whole point: on a board where every site fails the preferred
    /// filters, the shipped code holds and the gene walks.
    ///
    /// The board: one city, one Settler standing in it, every plot within
    /// three tiles of the city legal-but-excluded by spacing, the rest of the
    /// map retired for this Settler as `settler_dead_sites` — exactly the
    /// state thirty-turn retirements leave a live seat in after a handful of
    /// forecast refusals.
    #[test]
    fn a_settler_whose_every_site_was_retired_walks_under_the_gene_and_holds_without_it() {
        for gene in [false, true] {
            let mut g = Game::new_full(1, 16, 10, 91_302, 120, 0, false);
            let founding = g
                .player_unit_ids(0)
                .into_iter()
                .find(|uid| g.units[uid].kind == "settler")
                .expect("a starting settler");
            g.apply(0, &Action::FoundCity { unit: founding })
                .expect("the starting settler founds");
            let home = g.cities.values().next().expect("a city").pos;
            let settler = g.spawn_test_unit("settler", 0, home);
            g.units.get_mut(&settler).expect("settler").moves_left = 2.0;
            let mut ai = AdvancedAi::new();
            ai.enable_engine_repairs();
            ai.enable_loyalty_rate_alarm();
            // The ledger pins the gene on; the off arm withholds it by name.
            if gene {
                ai.enable_settler_never_idles();
            } else {
                ai.disable_settler_never_idles();
            }
            ai.attach_journal(Journal::recording());
            // Retire every plot for this Settler: the state the live seat
            // reaches after its forecast refusals compound.
            let all: Vec<Pos> = g.map.tiles.keys().copied().collect();
            let retired = ai.settler_dead_sites.entry(settler).or_default();
            for pos in all {
                retired.insert(pos, g.turn + 1000);
            }
            let before = g.units[&settler].pos;
            let acted = ai.advanced_settler_step(&mut g, 0, settler);
            let after = g.units[&settler].pos;
            if gene {
                assert!(acted, "the gene finds a site and steps");
                assert_ne!(after, before, "the settler left the city");
                assert!(
                    ai.settler_targets.contains_key(&settler),
                    "the settler carries a target"
                );
            } else {
                assert!(!acted, "the shipped code holds");
                assert_eq!(after, before);
            }
        }
    }

    /// The watchdog respects the one rule it keeps: a tile a visible raider
    /// can reach next turn is not taken, however long the Settler has waited.
    #[test]
    fn the_watchdog_will_not_step_into_a_raiders_reach() {
        let mut g = Game::new_full(2, 16, 10, 91_303, 120, 0, false);
        let founding = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .expect("a starting settler");
        g.apply(0, &Action::FoundCity { unit: founding })
            .expect("the starting settler founds");
        let home = g
            .cities
            .values()
            .find(|city| city.owner == 0)
            .expect("a city")
            .pos;
        let settler = g.spawn_test_unit("settler", 0, home);
        g.units.get_mut(&settler).expect("settler").moves_left = 2.0;
        let Some(barbarian) = g.barb_pid else {
            return; // no barbarian seat on this fixture
        };
        // A warrior on every land tile two steps out: every neighbour of the
        // city is inside its reach.
        let ring: Vec<Pos> = g
            .wdisk(home, 2)
            .into_iter()
            .filter(|pos| g.wdist(*pos, home) == 2)
            .filter(|pos| g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile)))
            .collect();
        for pos in &ring {
            g.spawn_test_unit("warrior", barbarian, *pos);
        }
        let mut ai = AdvancedAi::new();
        ai.enable_engine_repairs();
        ai.enable_settler_never_idles();
        ai.attach_journal(Journal::recording());
        let target = g
            .map
            .tiles
            .keys()
            .copied()
            .find(|pos| g.wdist(*pos, home) == 6 && ai.base.valid_settle_site(&g, 0, *pos))
            .expect("a legal site six tiles out");
        ai.settler_targets.insert(settler, target);
        ai.settler_idle_streak
            .insert(settler, (home, g.turn, SETTLER_IDLE_PATIENCE));
        let reach = ai.barbarian_reach(&g, 0, home, REACH_SCAN_RADIUS);
        let every_neighbour_covered = g
            .nbrs(home)
            .into_iter()
            .filter(|pos| g.can_move(settler, *pos))
            .all(|pos| reach.covers(&g, pos));
        if !every_neighbour_covered {
            return; // the ring did not close on this fixture; nothing to prove
        }
        assert!(!ai.settler_watchdog_step(&mut g, 0, settler));
        assert_eq!(g.units[&settler].pos, home, "it stayed in the city");
    }

    /// A site the exhaustion search chose is founded on arrival even where
    /// the strict verdict's fog guess would refuse it; the same board with
    /// the site chosen the ordinary way is still refused. The engine's own
    /// forecast is the one thing both rules keep.
    #[test]
    fn an_exhaustion_site_is_founded_on_arrival_where_the_fog_guess_would_refuse_it() {
        for relaxed in [true, false] {
            let mut g = Game::new_full(1, 20, 12, 91_305, 120, 0, false);
            let founding = g
                .player_unit_ids(0)
                .into_iter()
                .find(|uid| g.units[uid].kind == "settler")
                .expect("a starting settler");
            g.apply(0, &Action::FoundCity { unit: founding })
                .expect("the starting settler founds");
            let home = g.cities.values().next().expect("a city").pos;
            // Explore only the home halo so a far site is a fogged frontier.
            let halo: Vec<Pos> = g.wdisk(home, 5);
            g.players[0].explored.clear();
            g.players[0].explored.extend(halo);
            let far: Vec<Pos> = g
                .map
                .tiles
                .keys()
                .copied()
                .filter(|pos| g.wdist(*pos, home) >= 9)
                .collect();
            let mut site = None;
            for pos in far {
                let probe = g.spawn_test_unit("settler", 0, pos);
                let legal = g.can_found_city(probe) && AdvancedAi::beyond_loyalty_reach(&g, 0, pos);
                g.remove_unit(probe);
                if legal {
                    site = Some(pos);
                    break;
                }
            }
            let site = site.expect("fixture needs a legal fogged frontier site");
            let settler = g.spawn_test_unit("settler", 0, site);
            g.units.get_mut(&settler).expect("settler").moves_left = 2.0;
            let mut ai = AdvancedAi::new();
            ai.enable_live_bridge();
            ai.enable_loyalty_rate_alarm();
            ai.enable_settler_never_idles();
            assert!(ai.frontier_loyalty, "the live seat carries the fog guess");
            ai.settler_targets.insert(settler, site);
            if relaxed {
                ai.settler_relaxed_targets.insert(settler, site);
            }
            let cities_before = g.player_city_ids(0).len();
            let acted = ai.advanced_settler_step(&mut g, 0, settler);
            let founded = g.player_city_ids(0).len() > cities_before;
            if relaxed {
                assert!(acted && founded, "the relaxed arrival founds the colony");
            } else {
                assert!(
                    !founded,
                    "the strict arrival still refuses the fogged frontier"
                );
            }
        }
    }

    /// A Settler with no legal site anywhere but the one underfoot founds
    /// there; one with none at all says it is stranded rather than holding
    /// silently.
    #[test]
    fn a_stranded_settler_founds_where_it_stands_or_says_so() {
        let mut g = Game::new_full(1, 12, 8, 91_304, 60, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .expect("a starting settler");
        let here = g.units[&settler].pos;
        // Block every plot but the one underfoot.
        for pos in g.map.tiles.keys().copied().collect::<Vec<_>>() {
            if pos != here {
                std::sync::Arc::make_mut(&mut g.blocked_city_sites).insert(pos);
            }
        }
        let mut ai = AdvancedAi::new();
        ai.enable_settler_never_idles();
        let journal = Journal::recording();
        ai.attach_journal(journal.handle());
        assert_eq!(
            ai.settler_exhaustion_target(&g, 0, settler),
            Some(here),
            "the tile underfoot is the one legal site"
        );
        assert!(g.can_found_city(settler));
        assert!(ai.settler_stranded(&mut g, 0, settler));
        assert_eq!(
            g.player_city_ids(0).len(),
            1,
            "the capital stands where it stood"
        );
        // A second Settler in that city cannot found (spacing) and has no
        // site anywhere: it reports itself stranded rather than holding
        // silently.
        let second = g.spawn_test_unit("settler", 0, here);
        assert!(ai.settler_exhaustion_target(&g, 0, second).is_none());
        assert!(!ai.settler_stranded(&mut g, 0, second));
        let delta = journal.since(0);
        assert!(
            delta
                .thoughts
                .iter()
                .any(|thought| thought.headline.starts_with("Settler is stranded")),
            "the hold is named"
        );
    }
}
