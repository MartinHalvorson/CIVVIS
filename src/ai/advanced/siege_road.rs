//! `siege-target-needs-a-road`: a siege the army cannot walk to is stood
//! down at once, and its owner leaves the campaign while the road stays shut.
//!
//! `stage-march-keeps-to-land` holds a land unit on land when no dry road
//! within `siege_train::STAGE_DRY_LIMIT` reaches the staging ring, and logs
//! it as "holds on land short of the water". Over the 10-04/05 control runs
//! that line mostly meant a closed border, not the sea: in the windows it
//! dominates, 324 unit-turns stood behind a road open only through closed
//! borders against 1 cut off by water. 19 siege windows whose force stood
//! mostly border-blocked (306 siege-turns) converted 0 times, and in 18 the
//! target was dropped or still stuck 15 turns later; the "nobody went"
//! stand-down waited for the breaker exemption to lapse first. Live King
//! civvis-20261005T200350Z (game 158): the Siege of Apu mustered 7 to 18
//! units at 0% ready from turn 129 to 160; its land road ran through Poland,
//! at peace with closed borders, once Poland took Wolin from our
//! suzerainty, and 30 of those turns read "Holding the capture of Apu for a
//! wall-breaker on its way". Beijing repeated it from 161 to 171.
//!
//! Under the gene the Stage step records each march's outcome; when half or
//! more of a turn's train holds for want of a road [`ROAD_HOLD_TURNS`] turns
//! running, the capture is stood down through `capture_stood_down` whatever
//! the breaker and muster waits say, and the hold line names the border
//! that shuts the road.

use super::siege_train::{StageMarch, STAGE_DRY_LIMIT, STAGING_FAR};
use super::AdvancedAi;
use crate::game::Game;
use crate::think;
use crate::Pos;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Consecutive turns half the train must hold for want of a road before the
/// capture is stood down. Three is `commitments::STALL_TURNS` and
/// `COMMITMENT_PATIENCE`, the controller's own "not getting there".
pub(super) const ROAD_HOLD_TURNS: u32 = 3;

/// One siege's march outcomes for the turn, and its run of road-bound turns.
#[derive(Clone, Debug, Default)]
pub(super) struct RoadTally {
    /// The turn `held` and `marched` describe.
    turn: u32,
    /// Units the Stage step held on land this turn, and the owner whose
    /// closed borders shut the road, when one does.
    held: BTreeMap<u32, Option<usize>>,
    /// Units the Stage step marched (or walked a dry road) this turn.
    marched: BTreeSet<u32>,
    /// The last turn the ledger read this tally.
    counted: u32,
    /// Consecutive read turns half the train or more was held.
    pub(super) streak: u32,
}

/// A capture stood down for want of a road.
#[derive(Clone, Copy, Debug)]
pub(super) struct RoadClosed {
    /// The seat whose road it is.
    seat: usize,
    /// The city's owner when the stand-down fired.
    owner: usize,
    /// The owner whose closed borders shut the road, when one does; `None`
    /// for water or impassable ground.
    blocker: Option<usize>,
}

impl AdvancedAi {
    /// The Stage step's march outcome for `uid` toward `city`. Records the
    /// outcome and, for a hold, returns the name of the civilization whose
    /// closed borders shut the land road, when one does. Off, records
    /// nothing and returns `None`.
    pub(super) fn note_stage_march(
        &mut self,
        g: &Game,
        uid: u32,
        city: u32,
        march: &StageMarch,
    ) -> Option<String> {
        if !self.siege_target_needs_a_road {
            return None;
        }
        let tally = self.siege_road_tally.entry(city).or_default();
        if tally.turn != g.turn {
            tally.turn = g.turn;
            tally.held.clear();
            tally.marched.clear();
        }
        match march {
            StageMarch::Hold { .. } => {
                let blocker = g
                    .cities
                    .get(&city)
                    .and_then(|target| border_blocker(g, uid, target.pos));
                let tally = self.siege_road_tally.entry(city).or_default();
                tally.marched.remove(&uid);
                tally.held.insert(uid, blocker);
                blocker.map(|owner| g.players[owner].civ.clone())
            }
            _ => {
                tally.held.remove(&uid);
                tally.marched.insert(uid);
                None
            }
        }
    }

    /// `siege-target-needs-a-road`: the end-of-turn reading. Called by
    /// `reconcile_commitments` after the capture ledger is read; stands the
    /// open capture down when its train has been held for want of a road
    /// [`ROAD_HOLD_TURNS`] turns running.
    pub(super) fn reconcile_siege_roads(&mut self, g: &mut Game, pid: usize) {
        if !self.siege_target_needs_a_road {
            return;
        }
        use super::commitments::{Kind, Owner, Target, CAPTURE_STAND_DOWN_TURNS};
        let Some(Target::City(cid)) = self
            .commitments
            .open_for(Kind::Capture, Owner::Empire)
            .map(|c| c.target)
        else {
            return;
        };
        let Some(city) = g.cities.get(&cid) else {
            return;
        };
        let (name, pos, owner) = (city.name.clone(), city.pos, city.owner);
        // The train within the staging ring never reaches the Stage march,
        // so it is counted here: a road-bound share is of the whole train.
        let near = g
            .units
            .values()
            .filter(|unit| {
                let spec = &g.rules.units[unit.kind];
                unit.owner == pid
                    && spec.class == "military"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && g.wdist(unit.pos, pos) <= STAGING_FAR
            })
            .count();
        let tally = self.siege_road_tally.entry(cid).or_default();
        if tally.counted == g.turn {
            return;
        }
        tally.counted = g.turn;
        let fresh = tally.turn == g.turn;
        let held = if fresh { tally.held.len() } else { 0 };
        let marched = if fresh { tally.marched.len() } else { 0 };
        let train = held + marched + near;
        if held > 0 && 2 * held >= train {
            tally.streak += 1;
        } else {
            tally.streak = 0;
        }
        if tally.streak < ROAD_HOLD_TURNS || self.capture_stood_down.contains_key(&cid) {
            return;
        }
        let streak = tally.streak;
        let mut blockers: BTreeMap<usize, usize> = BTreeMap::new();
        for owner in tally.held.values().flatten() {
            *blockers.entry(*owner).or_insert(0) += 1;
        }
        let blocker = blockers
            .into_iter()
            .max_by_key(|(owner, count)| (*count, std::cmp::Reverse(*owner)))
            .map(|(owner, _)| owner);
        self.siege_road_tally.remove(&cid);
        self.capture_stood_down
            .insert(cid, g.turn + CAPTURE_STAND_DOWN_TURNS);
        self.siege_road_closed.insert(
            cid,
            RoadClosed {
                seat: pid,
                owner,
                blocker,
            },
        );
        *g.players[pid]
            .counters
            .entry("commit:capture:road_stand_downs".to_string())
            .or_insert(0) += 1;
        let shut = blocker.map_or_else(
            || "water or impassable ground".to_string(),
            |owner| format!("{}'s closed borders", g.players[owner].civ),
        );
        think!(self.journal(), Military, Strategy, "Standing down a siege the army cannot reach";
               "{name}: {held} of the {train} units of its train held short of {shut} for {streak} turns; no land road opens within {} steps",
               STAGE_DRY_LIMIT; pos);
    }

    /// `siege-target-needs-a-road`: whether a road stand-down of `city` has
    /// lifted early because the border that shut it opened (a war on its
    /// owner, open borders, or the owner gone).
    pub(super) fn siege_road_reopened(&self, g: &Game, city: u32) -> bool {
        self.siege_target_needs_a_road
            && self.siege_road_closed.get(&city).is_some_and(|closed| {
                closed.blocker.is_some_and(|blocker| {
                    g.is_at_war(closed.seat, blocker)
                        || g.has_open_borders(closed.seat, blocker)
                        || !g.players.get(blocker).is_some_and(|player| player.alive)
                })
            })
    }

    /// `siege-target-needs-a-road`: whether `rival` is out of the campaign
    /// ranking because a siege of one of its cities was stood down for want
    /// of a road and still no land road from our cities reaches any of its
    /// cities.
    pub(super) fn siege_road_cuts_off(&self, g: &Game, pid: usize, rival: usize) -> bool {
        self.siege_target_needs_a_road
            && self.siege_road_closed.iter().any(|(city, closed)| {
                closed.owner == rival
                    && closed.seat == pid
                    && self.capture_stood_down_holds(g, *city)
            })
            && !self.rival_reachable_by_land(g, pid, rival)
    }

    /// `blocker-becomes-the-target`: whether the second front `rival`, still
    /// at peace, is displaced from the plan by a war the plan keeps: the
    /// plan's target is another rival already at war with us, so neither the
    /// plan nor `second_front_waits_*`'s waiting declaration would ever open
    /// the second front. The declaration desk then declares on it as on a
    /// waiting one. The elimination front (`diplomatic_contender_to_eliminate`)
    /// ranks ahead of the second front in `assess`, and a second front that
    /// does not wait (an urgent clock, the road blocker) was left with no
    /// declaration at all. Live King civvis-20261005T215912Z (game 167) read
    /// "Eliminating Maori" from turn 200 while the Ottomans, the second front
    /// from 201 (an urgent science clock, then the road blocker of five
    /// stood-down Maori sieges), stood at peace at 516-1,234 power against our
    /// 2,870-4,711 until they won on Science at 249; the gene's road blocker
    /// was named every turn and never declared on. Off with the gene.
    pub(crate) fn second_front_displaced(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
        plan: &super::StrategicPlan,
    ) -> bool {
        self.blocker_becomes_the_target
            && !g.is_at_war(pid, rival)
            && plan
                .target_player
                .is_some_and(|target| target != rival && g.is_at_war(pid, target))
    }

    /// `blocker-becomes-the-target`: the major whose closed borders shut the
    /// road of a siege stood down for want of one, while that stand-down
    /// holds: alive, met, at peace with us and a legal campaign target,
    /// holding an original capital we need (a Domination objective in its own
    /// right), and passing the version-2 declaration edge
    /// (`declaration_edge_2`) whether or not that gene is armed. A war on it
    /// opens the road (`siege_road_reopened`) and is Domination progress.
    /// `None` with the gene off, under another victory target, or when no
    /// such blocker stands; the lowest player id of several.
    pub(crate) fn road_blocker_front(&self, g: &Game, pid: usize) -> Option<usize> {
        if !self.blocker_becomes_the_target
            || self.active_victory_target(g) != Some(super::VictoryTarget::Domination)
        {
            return None;
        }
        self.siege_road_closed
            .iter()
            .filter(|(city, closed)| closed.seat == pid && self.capture_stood_down_holds(g, **city))
            .filter_map(|(_, closed)| closed.blocker)
            .filter(|blocker| {
                *blocker != pid
                    && g.players.get(*blocker).is_some_and(|player| {
                        player.alive && !player.is_minor && !player.is_barbarian
                    })
                    && !g.is_at_war(pid, *blocker)
                    && self.campaign_target_legal(g, pid, *blocker)
                    && g.cities.values().any(|city| {
                        city.owner == *blocker
                            && city.is_capital
                            && city.original_owner != pid
                            && !g.same_team(pid, city.original_owner)
                    })
                    && self.declaration_edge_2(g, pid, *blocker).passes()
            })
            .min()
    }
}

/// The owner whose closed borders shut `uid`'s land road to the staging ring
/// of a city at `target`: the first closed territory on the shortest road
/// over dry, passable ground with every border open. `None` when no such
/// road exists within [`STAGE_DRY_LIMIT`] (water or impassable ground), or
/// when the road needs no closed border.
fn border_blocker(g: &Game, uid: u32, target: Pos) -> Option<usize> {
    let unit = g.units.get(&uid)?;
    let seat = unit.owner;
    let open = |owner: usize| {
        owner == seat || g.is_at_war(seat, owner) || g.has_open_borders(seat, owner)
    };
    let mut seen = BTreeSet::from([unit.pos]);
    let mut queue = VecDeque::from([(unit.pos, None::<usize>, 0usize)]);
    while let Some((current, blocker, steps)) = queue.pop_front() {
        if steps >= STAGE_DRY_LIMIT {
            continue;
        }
        for next in g.nbrs(current) {
            if seen.contains(&next) {
                continue;
            }
            let Some(tile) = g.map.get(next) else {
                continue;
            };
            if g.rules.is_water(tile) || !g.rules.is_passable(tile) {
                continue;
            }
            seen.insert(next);
            let closed = tile
                .owner_city
                .and_then(|cid| g.cities.get(&cid))
                .map(|city| city.owner)
                .filter(|owner| !open(*owner));
            let blocker = blocker.or(closed);
            if g.wdist(next, target) <= STAGING_FAR {
                return blocker;
            }
            queue.push_back((next, blocker, steps + 1));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::commitments::{Kind, Owner, Target};
    use super::super::siege_train::StageMarch;
    use super::super::{AdvancedAi, GrandStrategy, StrategicPlan, VictoryTarget};
    use super::ROAD_HOLD_TURNS;
    use crate::game::Game;

    /// A land strip: us at the west end, a civilization with closed borders
    /// across the middle, the target at the east end (at war with us). The
    /// strip is closed at both ends so the wrap cannot go round the middle.
    fn strip() -> (Game, u32, u32) {
        let mut g = Game::new_full(3, 40, 12, 931_036, 300, 0, false);
        for unit in g.units.keys().copied().collect::<Vec<_>>() {
            g.remove_unit(unit);
        }
        for tile in g.map.tiles.values_mut() {
            tile.terrain = if (4..=8).contains(&tile.pos.1) && (2..=36).contains(&tile.pos.0) {
                crate::name!("grassland")
            } else {
                crate::name!("ocean")
            };
            tile.feature = None;
            tile.hills = false;
        }
        g.found_city_for(0, (4, 6), None);
        let screen = g.found_city_for(1, (18, 6), None);
        let target = g.found_city_for(2, (32, 6), None);
        for tile in g.map.tiles.values_mut() {
            if (14..=22).contains(&tile.pos.0) && (4..=8).contains(&tile.pos.1) {
                tile.owner_city = Some(screen);
            }
        }
        g.players[1].borders_enforced = Some(true);
        g.at_war.insert((0, 2));
        g.at_war.insert((2, 0));
        let soldier = g.spawn_test_unit("warrior", 0, (8, 6));
        (g, target, soldier)
    }

    fn ai(gene: bool, g: &Game, target: u32) -> AdvancedAi {
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        ai.enable_capture_go_or_stand_down_2();
        if gene {
            ai.enable_siege_target_needs_a_road();
        }
        ai.plan = Some(StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(2),
            target_city: Some(target),
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: g.turn,
            rush: false,
        });
        ai
    }

    /// Run `turns` turns in which the Stage step reads `march` for the
    /// soldier, reconciling the ledger after each.
    fn run(ai: &mut AdvancedAi, g: &mut Game, target: u32, soldier: u32, march: StageMarch, turns: u32) {
        for _ in 0..turns {
            let _ = ai.note_stage_march(g, soldier, target, &march);
            ai.reconcile_commitments(g, 0);
            g.turn += 1;
        }
    }

    /// A target whose only land road runs through closed borders is stood
    /// down within `ROAD_HOLD_TURNS` under the gene, names the border, and
    /// drops its owner from the campaign; the gene off, the same holds
    /// stand nothing down.
    #[test]
    fn a_border_blocked_siege_is_stood_down_within_three_turns() {
        let (game, target, soldier) = strip();
        assert!(
            game.route_step_dry(soldier, game.cities[&target].pos, 5, 128).is_none(),
            "fixture: no open land road"
        );
        let hold = StageMarch::Hold { wet: 30 };

        let mut on = ai(true, &game, target);
        let mut g = game.clone();
        assert_eq!(
            on.note_stage_march(&g, soldier, target, &hold).as_deref(),
            Some(g.players[1].civ.as_str()),
            "the hold names the closed border"
        );
        assert!(on.campaign_target_legal(&g, 0, 2), "at war: a legal campaign target");
        run(&mut on, &mut g, target, soldier, hold, ROAD_HOLD_TURNS - 1);
        assert!(!on.capture_stood_down.contains_key(&target), "two holds are not yet the rule");
        run(&mut on, &mut g, target, soldier, hold, 1);
        assert!(on.capture_stood_down.contains_key(&target), "gene on: stood down");
        assert!(on.capture_stood_down_holds(&g, target));
        assert!(on.siege_road_cuts_off(&g, 0, 2), "the campaign drops the rival");
        assert!(!on.campaign_target_legal(&g, 0, 2));
        assert_eq!(
            g.players[0].counters.get("commit:capture:road_stand_downs"),
            Some(&1)
        );
        // A war on the screen opens the road: the stand-down lifts.
        g.at_war.insert((0, 1));
        g.at_war.insert((1, 0));
        assert!(on.siege_road_reopened(&g, target));
        assert!(!on.capture_stood_down_holds(&g, target));
        assert!(!on.siege_road_cuts_off(&g, 0, 2));

        let mut off = ai(false, &game, target);
        let mut g = game.clone();
        assert_eq!(off.note_stage_march(&g, soldier, target, &hold), None);
        run(&mut off, &mut g, target, soldier, hold, ROAD_HOLD_TURNS);
        assert!(
            !off.capture_stood_down.contains_key(&target),
            "gene off: three holds stand nothing down"
        );
        assert!(off.siege_road_closed.is_empty());
    }

    /// A reachable target is unaffected: a train that marches, or one held
    /// only a minority of the time, keeps its capture.
    #[test]
    fn a_reachable_siege_is_not_stood_down() {
        let (mut game, target, soldier) = strip();
        game.players[1].borders_enforced = Some(false);
        assert!(game.route_step_dry(soldier, game.cities[&target].pos, 5, 128).is_some());
        let mut on = ai(true, &game, target);
        let mut g = game.clone();
        run(&mut on, &mut g, target, soldier, StageMarch::Ordinary, ROAD_HOLD_TURNS + 2);
        assert!(!on.capture_stood_down.contains_key(&target));
        assert_eq!(
            on.commitments()
                .open_for(Kind::Capture, Owner::Empire)
                .map(|c| c.target),
            Some(Target::City(target))
        );

        // Held, but by a minority of the train: two marchers stand beside it.
        let (game, target, soldier) = strip();
        let mut g = game.clone();
        let mates: Vec<u32> = [(9, 6), (9, 5)]
            .into_iter()
            .map(|pos| g.spawn_test_unit("warrior", 0, pos))
            .collect();
        let mut on = ai(true, &g, target);
        for _ in 0..ROAD_HOLD_TURNS + 2 {
            let _ = on.note_stage_march(&g, soldier, target, &StageMarch::Hold { wet: 30 });
            for mate in &mates {
                let _ = on.note_stage_march(&g, *mate, target, &StageMarch::Ordinary);
            }
            on.reconcile_commitments(&mut g, 0);
            g.turn += 1;
        }
        assert!(
            !on.capture_stood_down.contains_key(&target),
            "one of three held is not the train"
        );
    }

    fn front(target: usize, turn: u32) -> crate::ai::advanced::one_war::OneWarFront {
        crate::ai::advanced::one_war::OneWarFront {
            target,
            since: turn,
            ledger: (0, 0, 0, 0),
            window: std::collections::VecDeque::new(),
            tide_against_since: None,
            city_health: std::collections::BTreeMap::new(),
            sieges_advancing: 0,
            closure_wanted_since: None,
        }
    }

    /// `blocker-becomes-the-target`: the screen whose closed borders shut the
    /// stood-down siege's road -- weak, met, at peace and holding its own
    /// original capital -- is named as the war that opens the road, and the
    /// one-war gate admits it as the second front beside the cut-off war.
    /// With the gene off nothing is named, and a war on the screen (the road
    /// open) or a screen at strength names nothing either.
    #[test]
    fn a_weak_blocker_holding_a_capital_becomes_the_second_front() {
        let (mut game, target, soldier) = strip();
        game.record_contact(0, 1);
        game.record_contact(0, 2);
        let screen_capital = game.player_city_ids(1)[0];
        assert!(game.cities[&screen_capital].is_capital, "fixture: the screen's capital");
        for pos in [(6, 5), (6, 7)] {
            game.spawn_test_unit("warrior", 0, pos);
        }
        let hold = StageMarch::Hold { wet: 30 };
        for gene in [false, true] {
            let mut ai = ai(true, &game, target);
            if gene {
                ai.enable_blocker_becomes_the_target();
            }
            ai.disable_capital_prey_opens_a_front();
            ai.disable_capital_prey_opens_a_front_2();
            ai.enable_one_war_at_a_time();
            ai.one_war = Some(front(2, game.turn));
            let mut g = game.clone();
            run(&mut ai, &mut g, target, soldier, hold, ROAD_HOLD_TURNS);
            assert!(ai.capture_stood_down_holds(&g, target), "the road stand-down holds");
            assert!(ai.declaration_edge_2(&g, 0, 1).passes(), "fixture: the screen is weak");
            if gene {
                assert_eq!(ai.road_blocker_front(&g, 0), Some(1), "gene on: the screen is named");
                assert_eq!(ai.one_war_second_front(&g, 0), Some(1), "and opens the second front");
                // A war on the screen opens the road: nothing is named any more.
                let mut opened = g.clone();
                opened.at_war.insert((0, 1));
                opened.at_war.insert((1, 0));
                assert_eq!(ai.road_blocker_front(&opened, 0), None, "at war: the road is open");
            } else {
                assert_eq!(ai.road_blocker_front(&g, 0), None, "gene off: nothing is named");
                assert_ne!(ai.one_war_second_front(&g, 0), Some(1), "gene off: no second front on the screen");
            }
        }
    }

    /// `blocker-becomes-the-target`: a screen at strength -- out-producing
    /// nothing and short of the version-2 edge -- is not named, nor is a
    /// screen without an original capital.
    #[test]
    fn a_blocker_short_of_the_edge_or_without_a_capital_is_not_named() {
        let (mut game, target, soldier) = strip();
        game.record_contact(0, 1);
        game.record_contact(0, 2);
        let hold = StageMarch::Hold { wet: 30 };

        // At strength: the screen fields more than half our power.
        let mut strong = game.clone();
        for pos in [(19, 5), (19, 7), (17, 5), (17, 7)] {
            strong.spawn_test_unit("warrior", 1, pos);
        }
        let mut ai_strong = ai(true, &strong, target);
        ai_strong.enable_blocker_becomes_the_target();
        run(&mut ai_strong, &mut strong, target, soldier, hold, ROAD_HOLD_TURNS);
        assert!(ai_strong.capture_stood_down_holds(&strong, target));
        assert!(!ai_strong.declaration_edge_2(&strong, 0, 1).passes(), "fixture: short of the edge");
        assert_eq!(ai_strong.road_blocker_front(&strong, 0), None);

        // Weak, but its capital is not an original capital of a rival we need:
        // the screen's city is ours by origin.
        let screen = game.player_city_ids(1)[0];
        game.cities.get_mut(&screen).unwrap().original_owner = 0;
        let mut ai_nocap = ai(true, &game, target);
        ai_nocap.enable_blocker_becomes_the_target();
        let mut g = game.clone();
        run(&mut ai_nocap, &mut g, target, soldier, hold, ROAD_HOLD_TURNS);
        assert!(ai_nocap.capture_stood_down_holds(&g, target));
        assert_eq!(ai_nocap.road_blocker_front(&g, 0), None);
    }

    /// Live King civvis-20261005T215912Z (game 167)'s shape: at war with the
    /// target (player 2), a Diplomatic Victory contender the elimination
    /// front keeps the plan on; its siege's road shut by the screen's
    /// (player 1's) closed borders and stood down; the screen weak, holding
    /// its original capital, and at 90% of a Science Victory. The road
    /// blocker is named and the plan stays on the elimination front either
    /// way; under the gene the diplomacy desk declares on the screen, off it
    /// nothing does.
    #[test]
    fn a_blocker_the_elimination_front_displaces_is_still_declared_on() {
        let (mut game, target, soldier) = strip();
        game.turn = 200;
        game.record_contact(0, 1);
        game.record_contact(0, 2);
        game.found_city_for(0, (8, 4), None);
        game.players[0].gold = 3000.0;
        game.players[2].dvp = 18;
        game.players[1].science_projects.extend([
            "launch_earth_satellite".to_string(),
            "launch_moon_landing".to_string(),
            "launch_mars_colony".to_string(),
            "exoplanet_expedition".to_string(),
        ]);
        let observed = std::sync::Arc::make_mut(&mut game.observed_public_empire_stats)
            .entry(1)
            .or_default();
        observed.science_victory_points = Some(14.0);
        observed.science_victory_points_needed = Some(25.0);
        observed.science_victory_points_per_turn = Some(6.0);
        for (seat, power) in [(0, 4000.0), (1, 600.0), (2, 500.0)] {
            std::sync::Arc::make_mut(&mut game.observed_military_power).insert(seat, power);
        }
        let hold = StageMarch::Hold { wet: 30 };
        for gene in [false, true] {
            let mut ai = ai(true, &game, target);
            if gene {
                ai.enable_blocker_becomes_the_target();
            }
            ai.enable_diplomatic_contender_eliminated();
            ai.enable_one_war_at_a_time();
            ai.disable_capital_prey_opens_a_front();
            ai.disable_capital_prey_opens_a_front_2();
            ai.coalition_before_war = false;
            ai.coalition_before_war_2 = false;
            ai.coalition_before_war_3 = false;
            ai.one_war = Some(front(2, game.turn));
            let mut g = game.clone();
            run(&mut ai, &mut g, target, soldier, hold, ROAD_HOLD_TURNS);
            assert!(ai.capture_stood_down_holds(&g, target), "the road stand-down holds");
            assert_eq!(ai.diplomatic_contender_to_eliminate(&g, 0), Some(2), "fixture: eliminating the target");
            assert_eq!(ai.rival_pressure(&g, 1).0, GrandStrategy::Science, "fixture: the screen's race");
            let mut plan = ai.plan.clone().expect("the fixture's plan");
            plan.assessed_turn = g.turn;
            for _ in 0..12 {
                if g.is_at_war(0, 1) {
                    break;
                }
                assert_eq!(plan.target_player, Some(2), "the plan stays on the elimination front");
                ai.advanced_diplomacy(&mut g, 0, &plan);
                g.turn += 1;
            }
            if gene {
                assert_eq!(ai.road_blocker_front(&g, 0), None, "at war: the road is open");
                assert!(g.is_at_war(0, 1), "gene on: the displaced blocker is declared on");
            } else {
                assert!(!g.is_at_war(0, 1), "gene off: nothing declares on the screen");
            }
        }
    }

    /// `blocker-becomes-the-target` is a registered, reversible opt-in.
    #[test]
    fn blocker_becomes_the_target_is_a_reversible_opt_in() {
        let mut ai = AdvancedAi::new();
        assert!(!ai.blocker_becomes_the_target);
        ai.enable_blocker_becomes_the_target();
        assert!(ai.blocker_becomes_the_target);
        ai.disable_blocker_becomes_the_target();
        assert!(!ai.blocker_becomes_the_target);
        assert!(super::super::GENES
            .iter()
            .any(|gene| gene.tag == "blocker-becomes-the-target" && gene.opt_in()));
    }
}
