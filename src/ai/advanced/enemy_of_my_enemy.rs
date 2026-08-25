//! Enemy of my enemy: a threat that hurts a rival more than us is left
//! standing, and the city-states and majors on the far side of a rival are
//! the ones worth courting.
//!
//! ★★★ EVERY CAMP CLEAR READ THE CAMP AS OURS TO CLEAR. A barbarian camp's
//! Scout reports the nearest major settlement or walker it sees and the
//! camp raises its party against that report (`Game::barbarian_scout_phase`),
//! so a camp whose nearest major city is a rival's raids the rival, not us —
//! and five deliberate clears took it down anyway: the adjacent clear
//! (`clear_adjacent_empty_barbarian_camp`), the camp errand
//! (`camp_bounty_target`), the home-defence threat list
//! (`home_defense_objective_inner`), the near-home chase (`nearest_enemy`)
//! and the presence alarm that admits the barbarian seat to the enemy list
//! at all (`barbarian_presence_at_home_inner`). Each measured the camp's
//! distance from OUR cities only. Likewise the envoy scorer's place term
//! (`flip-nearby-city-states`) and the alliance partner score
//! (`propose_strategic_alliance`) read a city-state's or a major's place
//! against our own cities: the one ACROSS a rival — the front that rival
//! cannot cover while facing us — scored nothing for being there. The
//! operator's rule (2026-08-25): *enemy of my enemy is my friend — create
//! and don't remove threats that asymmetrically impact neighbouring civs;
//! if a barbarian camp is more of a problem for my neighbours, leave it
//! that way; aligning city-states and neighbours on the far side of enemy
//! civs is generally a great strategy.*
//!
//! What the gene does, all of it inert while the flag is off:
//!
//! 1. **The neighbours' camps stand.** A camp is a neighbour's problem when
//!    a living major we are not allied with has a city strictly nearer it
//!    than any of ours and the camp is outside our own worked ring
//!    ([`ENEMY_OF_MY_ENEMY_HOME_RING`] — inside it the raiders pillage us
//!    whoever they were raised against). All five clears above skip such a
//!    camp (`BasicAi::camp_is_a_neighbours_problem`); a raider that does
//!    walk into our home ring is still answered, because the raider clause
//!    of every one of those gates is untouched.
//! 2. **Envoys across the rival.** One more term in `advanced_envoys`: a
//!    city-state on the far side of a rival — within
//!    [`ENEMY_OF_MY_ENEMY_CITY_STATE_RADIUS`] of the rival's cities, nearer
//!    the rival than us, and farther from us than the rival is — is worth
//!    [`ENEMY_OF_MY_ENEMY_CITY_STATE_BASE`] plus its proximity to the rival,
//!    plus [`ENEMY_OF_MY_ENEMY_RIVAL_CLIENT`] when the rival is its suzerain
//!    (a client it loses and a front it gains: a suzerain's clients fight
//!    its wars) or [`ENEMY_OF_MY_ENEMY_OPEN_CITY_STATE`] when nobody holds
//!    it; doubled for an enemy (at war, or the plan's target), amortised
//!    over the envoys the suzerainty still needs, zero for one we hold
//!    securely.
//! 3. **Alliances across the rival.** `propose_strategic_alliance` keeps its
//!    cadence and its kind, but a candidate partner with a city on a rival's
//!    far side scores [`ENEMY_OF_MY_ENEMY_PARTNER`] more (doubled for an
//!    enemy, plus [`ENEMY_OF_MY_ENEMY_PARTNER_AT_WAR`] when the partner is
//!    already fighting that rival).
//! 4. **A rival's joint war against its far side is refused.** An incoming
//!    `joint_war_target` from a rival, aimed at a major on that rival's far
//!    side, is priced [`ENEMY_OF_MY_ENEMY_JOINT_WAR_REFUSAL`] lower — unless
//!    the target is our own plan's target or already our enemy, when the
//!    stock valuation stands.
//!
//! "Rival" is read the same way everywhere ([`AdvancedAi::enemy_of_my_enemy_rivals`]):
//! a met, living major that is neither our friend nor our ally, weighted 2
//! as an enemy (at war with us, or the plan's target) and 1 as a neighbour
//! (a city within `CAMPAIGN_REACH` of one of ours); a major beyond reach and
//! at peace is nobody's concern.
//!
//! Counters: `eoe:camps_left` (an adjacent clear declined),
//! `eoe:envoys` (an envoy sent where this term was positive),
//! `eoe:partners` (an alliance proposed to a partner this term favoured).

use super::{AdvancedAi, GrandStrategy};
use crate::game::{DiplomaticDeal, Game};
use crate::Pos;

/// A camp this close to one of our cities is our problem whatever its
/// scout reports: the third ring is the worked ring, and a raider raised
/// beside it pillages us on the way to anyone.
pub(crate) const ENEMY_OF_MY_ENEMY_HOME_RING: i32 = 3;
/// A city-state this close to a rival's city is on that rival's frontier.
pub(crate) const ENEMY_OF_MY_ENEMY_CITY_STATE_RADIUS: i32 = 9;
/// Envoy score for a city-state on a rival's far side, before proximity.
pub(crate) const ENEMY_OF_MY_ENEMY_CITY_STATE_BASE: i64 = 50;
/// Envoy score per tile of proximity to the rival inside the radius.
pub(crate) const ENEMY_OF_MY_ENEMY_CITY_STATE_PER_TILE: i64 = 8;
/// Envoy score for unseating the rival as suzerain of its far-side client.
pub(crate) const ENEMY_OF_MY_ENEMY_RIVAL_CLIENT: i64 = 160;
/// Envoy score for a far-side city-state nobody holds.
pub(crate) const ENEMY_OF_MY_ENEMY_OPEN_CITY_STATE: i64 = 40;
/// Alliance partner score for a major with a city on a rival's far side.
pub(crate) const ENEMY_OF_MY_ENEMY_PARTNER: f64 = 90.0;
/// More still when that partner is already at war with the rival.
pub(crate) const ENEMY_OF_MY_ENEMY_PARTNER_AT_WAR: f64 = 60.0;
/// What a rival's invitation to war on its own far side is worth to us.
pub(crate) const ENEMY_OF_MY_ENEMY_JOINT_WAR_REFUSAL: f64 = -600.0;
/// A rival we are at war with, or the plan's target, weighs this much.
pub(crate) const ENEMY_OF_MY_ENEMY_ENEMY_WEIGHT: i64 = 2;
/// A rival merely within campaign reach of us weighs this much.
pub(crate) const ENEMY_OF_MY_ENEMY_NEIGHBOUR_WEIGHT: i64 = 1;

impl AdvancedAi {
    /// The gene's flag, read from the base controller that owns the camp
    /// clears. See `BasicAi::enemy_of_my_enemy`.
    pub fn enemy_of_my_enemy(&self) -> bool {
        self.base.enemy_of_my_enemy
    }

    /// The empire the standing plan is against, for the sites that carry no
    /// plan of their own: the Conquest plan's target, else the appointed
    /// war's.
    pub(crate) fn enemy_of_my_enemy_target(&self) -> Option<usize> {
        self.plan
            .as_ref()
            .filter(|plan| plan.strategy == GrandStrategy::Conquest)
            .and_then(|plan| plan.target_player)
            .or_else(|| self.war_plan.as_ref().map(|war| war.target_player))
    }

    /// The rivals whose far side this gene reads, each with its weight:
    /// [`ENEMY_OF_MY_ENEMY_ENEMY_WEIGHT`] for a major we are at war with or
    /// the `target` the caller's plan names, [`ENEMY_OF_MY_ENEMY_NEIGHBOUR_WEIGHT`]
    /// for a major with a city within `CAMPAIGN_REACH` of one of ours. A
    /// friend or ally is no rival, an unmet or dead major is nobody, and a
    /// major beyond reach and at peace is left out. Empty with the gene off.
    pub(crate) fn enemy_of_my_enemy_rivals(
        &self,
        g: &Game,
        pid: usize,
        target: Option<usize>,
    ) -> Vec<(usize, i64)> {
        if !self.base.enemy_of_my_enemy {
            return Vec::new();
        }
        g.players
            .iter()
            .filter(|p| {
                p.id != pid
                    && p.alive
                    && !p.is_minor
                    && !p.is_barbarian
                    && g.has_met(pid, p.id)
                    && !g.are_friends(pid, p.id)
                    && !g.are_allied(pid, p.id)
            })
            .filter_map(|p| {
                if g.is_at_war(pid, p.id) || target == Some(p.id) {
                    Some((p.id, ENEMY_OF_MY_ENEMY_ENEMY_WEIGHT))
                } else if self.rival_is_in_campaign_reach(g, pid, p.id) {
                    Some((p.id, ENEMY_OF_MY_ENEMY_NEIGHBOUR_WEIGHT))
                } else {
                    None
                }
            })
            .collect()
    }

    /// The distance from `seat` to the nearest city of `rival` when the seat
    /// lies on the rival's far side: within `reach` of the rival's cities,
    /// nearer the rival than us, and farther from us than the rival itself
    /// is. `None` on our side of the rival, beyond the reach, or while
    /// either empire has no city.
    pub(crate) fn far_side_of_rival(
        g: &Game,
        pid: usize,
        rival: usize,
        seat: Pos,
        reach: i32,
    ) -> Option<i32> {
        let ours: Vec<Pos> = g
            .player_city_ids(pid)
            .into_iter()
            .filter_map(|cid| g.cities.get(&cid).map(|city| city.pos))
            .collect();
        let theirs: Vec<Pos> = g
            .player_city_ids(rival)
            .into_iter()
            .filter_map(|cid| g.cities.get(&cid).map(|city| city.pos))
            .collect();
        let to_us = ours.iter().map(|pos| g.wdist(*pos, seat)).min()?;
        let to_rival = theirs.iter().map(|pos| g.wdist(*pos, seat)).min()?;
        let us_to_rival = ours
            .iter()
            .flat_map(|our| theirs.iter().map(move |their| (our, their)))
            .map(|(our, their)| g.wdist(*our, *their))
            .min()?;
        (to_rival <= reach && to_rival < to_us && to_us > us_to_rival).then_some(to_rival)
    }

    /// Whether `other` holds a city on the far side of `rival`, within
    /// campaign reach of the rival.
    fn has_a_city_across(g: &Game, pid: usize, rival: usize, other: usize) -> bool {
        g.player_city_ids(other).into_iter().any(|cid| {
            g.cities.get(&cid).is_some_and(|city| {
                Self::far_side_of_rival(
                    g,
                    pid,
                    rival,
                    city.pos,
                    super::city_campaign::CAMPAIGN_REACH,
                )
                .is_some()
            })
        })
    }

    /// The envoy term: what a city-state's place ACROSS a rival is worth —
    /// the base, its proximity to the rival, and the rival as its suzerain
    /// to unseat or nobody — at the rival's weight, taken over the best
    /// rival, amortised over the envoys the suzerainty still needs. Zero
    /// with the gene off, on our side of every rival, or for a city-state
    /// we hold securely.
    pub(crate) fn enemy_of_my_enemy_city_state_bonus(
        &self,
        g: &Game,
        pid: usize,
        minor: usize,
        needed: i64,
    ) -> i64 {
        if !self.base.enemy_of_my_enemy {
            return 0;
        }
        let Some(seat) = g
            .player_city_ids(minor)
            .into_iter()
            .next()
            .and_then(|cid| g.cities.get(&cid))
            .map(|city| city.pos)
        else {
            return 0;
        };
        let holder = g.suzerain_of(minor);
        if holder == Some(pid) {
            let mine = g.envoys_at(pid, minor);
            let rival = g
                .players
                .iter()
                .filter(|p| !p.is_minor && !p.is_barbarian && p.id != pid)
                .map(|p| g.envoys_at(p.id, minor))
                .max()
                .unwrap_or(0);
            if mine > rival + 1 {
                return 0;
            }
        }
        let best = self
            .enemy_of_my_enemy_rivals(g, pid, self.enemy_of_my_enemy_target())
            .into_iter()
            .filter_map(|(rival, weight)| {
                let near = Self::far_side_of_rival(
                    g,
                    pid,
                    rival,
                    seat,
                    ENEMY_OF_MY_ENEMY_CITY_STATE_RADIUS,
                )?;
                let proximity = i64::from(ENEMY_OF_MY_ENEMY_CITY_STATE_RADIUS + 1 - near)
                    * ENEMY_OF_MY_ENEMY_CITY_STATE_PER_TILE;
                let side = match holder {
                    Some(leader) if leader == rival => ENEMY_OF_MY_ENEMY_RIVAL_CLIENT,
                    None => ENEMY_OF_MY_ENEMY_OPEN_CITY_STATE,
                    _ => 0,
                };
                Some((ENEMY_OF_MY_ENEMY_CITY_STATE_BASE + proximity + side) * weight)
            })
            .max()
            .unwrap_or(0);
        best / needed.max(1)
    }

    /// The alliance partner term: [`ENEMY_OF_MY_ENEMY_PARTNER`] at the
    /// rival's weight for a partner with a city on a rival's far side, plus
    /// [`ENEMY_OF_MY_ENEMY_PARTNER_AT_WAR`] when the partner already fights
    /// that rival; the best rival counts. Zero with the gene off or for a
    /// partner on our side of every rival.
    pub(crate) fn enemy_of_my_enemy_partner_bonus(
        &self,
        g: &Game,
        pid: usize,
        partner: usize,
        target: Option<usize>,
    ) -> f64 {
        self.enemy_of_my_enemy_rivals(g, pid, target)
            .into_iter()
            .filter(|(rival, _)| *rival != partner)
            .filter(|(rival, _)| Self::has_a_city_across(g, pid, *rival, partner))
            .map(|(rival, weight)| {
                ENEMY_OF_MY_ENEMY_PARTNER * weight as f64
                    + if g.is_at_war(partner, rival) {
                        ENEMY_OF_MY_ENEMY_PARTNER_AT_WAR
                    } else {
                        0.0
                    }
            })
            .fold(0.0_f64, f64::max)
    }

    /// The incoming-deal term: [`ENEMY_OF_MY_ENEMY_JOINT_WAR_REFUSAL`] when a
    /// rival invites us to a joint war against a major on that rival's own
    /// far side — the enemy of our enemy — unless that major is our plan's
    /// target or already our enemy. Zero otherwise, and with the gene off.
    pub(crate) fn enemy_of_my_enemy_joint_war_penalty(
        &self,
        g: &Game,
        pid: usize,
        deal: &DiplomaticDeal,
        target: Option<usize>,
    ) -> f64 {
        let Some(against) = deal.joint_war_target else {
            return 0.0;
        };
        if target == Some(against) || g.is_at_war(pid, against) {
            return 0.0;
        }
        let proposer = deal.from;
        let is_rival = self
            .enemy_of_my_enemy_rivals(g, pid, target)
            .iter()
            .any(|(rival, _)| *rival == proposer);
        if is_rival && Self::has_a_city_across(g, pid, proposer, against) {
            ENEMY_OF_MY_ENEMY_JOINT_WAR_REFUSAL
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::genes::GENES;
    use super::super::{AdvancedAi, GrandStrategy, StrategicPlan};
    use super::*;
    use crate::ai::BasicAi;
    use crate::game::Game;

    fn opt_in_off_in_both_controllers(tag: &str, read: fn(&AdvancedAi) -> bool) {
        assert!(!read(&AdvancedAi::new()), "{tag} must be off in new()");
        assert!(
            !read(&AdvancedAi::legacy()),
            "{tag} must be off in legacy()"
        );
        let gene = GENES
            .iter()
            .find(|gene| gene.tag == tag)
            .expect("the gene is published for gene_screen");
        assert!(gene.opt_in() && gene.screenable() && !gene.live());
        let mut ai = AdvancedAi::new();
        (gene.enable)(&mut ai);
        assert!(read(&ai));
        (gene.disable)(&mut ai);
        assert!(!read(&ai));
    }

    #[test]
    fn enemy_of_my_enemy_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("enemy-of-my-enemy", |ai| ai.enemy_of_my_enemy());
        assert!(!BasicAi::new().enemy_of_my_enemy);
    }

    /// Open land at exactly `distance` from `anchor` satisfying `keep`.
    fn open_ground_where(
        game: &Game,
        anchor: Pos,
        distance: i32,
        keep: impl Fn(Pos) -> bool,
    ) -> Pos {
        game.map
            .tiles
            .keys()
            .copied()
            .filter(|position| {
                game.wdist(*position, anchor) == distance
                    && game.rules.is_passable(&game.map.tiles[position])
                    && !game.rules.is_water(&game.map.tiles[position])
                    && game.map.tiles[position].owner_city.is_none()
                    && game.city_at(*position).is_none()
                    && game.units_at(*position).is_empty()
                    && keep(*position)
            })
            .min()
            .expect("open ground at the distance")
    }

    /// Two majors far apart on flat land, a capital each, met, plus the two
    /// city-states with their seats removed for the test to place. Returns
    /// the game, our capital, the rival's capital and the two minors.
    fn far_side_board(seed: u64) -> (Game, Pos, Pos, Vec<usize>) {
        let mut game = Game::new_full(2, 30, 20, seed, 300, 2, true);
        game.map.clear_rivers();
        for tile in game.map.tiles.values_mut() {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
        }
        for uid in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(uid);
        }
        for camp in game.barb_camps.keys().copied().collect::<Vec<_>>() {
            game.barb_camps.remove(&camp);
        }
        game.barb_naval_camps.clear();
        game.barb_camp_guards.clear();
        game.barb_scout_homes.clear();
        game.barb_scout_targets.clear();
        game.barb_camp_targets.clear();
        for minor in game
            .players
            .iter()
            .filter(|p| p.is_minor && !p.is_barbarian)
            .map(|p| p.id)
            .collect::<Vec<_>>()
        {
            for cid in game.player_city_ids(minor) {
                game.cities.remove(&cid);
            }
        }
        let mut land: Vec<Pos> = game.map.tiles.keys().copied().collect();
        land.sort();
        let (home, theirs) = land
            .iter()
            .flat_map(|home| land.iter().map(move |other| (*home, *other)))
            .find(|(home, other)| {
                game.wdist(*home, *other) == 12
                    && game.wdisk(*home, 4).len() >= 24
                    && game.wdisk(*other, 8).len() >= 60
            })
            .expect("two interior sites twelve apart");
        game.found_city_for(0, home, None);
        game.found_city_for(1, theirs, None);
        game.record_contact(0, 1);
        let minors: Vec<usize> = game
            .players
            .iter()
            .filter(|p| p.is_minor && !p.is_barbarian)
            .map(|p| p.id)
            .collect();
        assert_eq!(minors.len(), 2);
        for minor in &minors {
            game.players[0].met.insert(*minor);
            game.players[1].met.insert(*minor);
            game.players[*minor].met.insert(0);
            game.players[*minor].met.insert(1);
        }
        game.turn = 60;
        game.current = 0;
        (game, home, theirs, minors)
    }

    fn eoe_ai() -> AdvancedAi {
        let mut ai = AdvancedAi::new();
        ai.enable_enemy_of_my_enemy();
        ai
    }

    /// A camp nearer the rival's capital than ours is the rival's problem;
    /// one nearer ours, or inside our worked ring, is ours; an ally's camp
    /// is not left; the stock controller leaves none.
    #[test]
    fn a_camp_nearer_the_rivals_city_is_the_rivals_problem() {
        let (mut game, home, theirs, _) = far_side_board(26_082_501);
        let across = open_ground_where(&game, theirs, 2, |pos| game.wdist(pos, home) > 6);
        let ours = open_ground_where(&game, home, 2, |pos| game.wdist(pos, theirs) > 6);
        let ring = open_ground_where(&game, home, 3, |pos| game.wdist(pos, theirs) > 4);
        for camp in [across, ours, ring] {
            game.barb_camps.insert(camp, game.turn + 1_000);
        }
        let ai = eoe_ai();
        assert!(ai.base.camp_is_a_neighbours_problem(&game, 0, across));
        assert!(!ai.base.camp_is_a_neighbours_problem(&game, 0, ours));
        assert!(
            !ai.base.camp_is_a_neighbours_problem(&game, 0, ring),
            "a camp inside our worked ring is ours whoever it was raised against"
        );
        // Symmetric from the rival's seat.
        assert!(ai.base.camp_is_a_neighbours_problem(&game, 1, ours));
        assert!(!ai.base.camp_is_a_neighbours_problem(&game, 1, across));
        // The stock controller leaves nothing standing.
        let stock = AdvancedAi::new();
        assert!(!stock.base.camp_is_a_neighbours_problem(&game, 0, across));
        // An ally's camp is our problem too.
        let mut allied = game.clone();
        allied.players[0].alliances.insert(
            1,
            crate::game::AllianceState {
                kind: "military".to_string(),
                points: 0.0,
                level: 1,
                ends: allied.turn + 30,
            },
        );
        assert!(allied.are_allied(0, 1));
        assert!(!ai.base.camp_is_a_neighbours_problem(&allied, 0, across));
    }

    /// The adjacent clear, the errand-free chase, the presence alarm and the
    /// home-defence threat list all leave the rival's camp; with the gene off
    /// the same unit walks in.
    #[test]
    fn the_rivals_camp_is_left_standing_by_every_clear() {
        let (mut game, home, theirs, _) = far_side_board(26_082_502);
        // The rival's border city eight tiles from our capital; the camp
        // five from us and three from it: inside every one of our alarm
        // rings, and still the rival's problem first.
        let border = open_ground_where(&game, home, 8, |pos| game.wdist(pos, theirs) <= 6);
        game.found_city_for(1, border, None);
        let camp = open_ground_where(&game, home, 5, |pos| game.wdist(pos, border) == 3);
        assert!(game.wdist(camp, home) <= crate::ai::HOME_THREAT_RADIUS);
        game.barb_camps.insert(camp, game.turn + 1_000);
        game.map.tiles.get_mut(&camp).unwrap().improvement = Some(crate::name!("barbarian_camp"));
        let beside = game
            .nbrs(camp)
            .into_iter()
            .filter(|pos| {
                game.rules.is_passable(&game.map.tiles[pos])
                    && !game.rules.is_water(&game.map.tiles[pos])
                    && game.city_at(*pos).is_none()
            })
            .min()
            .expect("open ground beside the camp");
        let warrior = game.spawn_test_unit("warrior", 0, beside);
        // A garrison at home, so the field warrior is the responder.
        game.spawn_test_unit("warrior", 0, home);
        assert!(game.player_can_see(0, camp));
        let barb = game.barb_pid.expect("the board has a barbarian seat");

        let mut off = game.clone();
        let stock = AdvancedAi::new();
        assert!(
            stock
                .base
                .clear_adjacent_empty_barbarian_camp(&mut off, 0, warrior),
            "the stock adjacent clear walks into the camp"
        );
        assert!(!off.barb_camps.contains_key(&camp));

        let ai = eoe_ai();
        assert!(!ai
            .base
            .clear_adjacent_empty_barbarian_camp(&mut game, 0, warrior));
        assert!(
            game.barb_camps.contains_key(&camp),
            "the rival's camp stands"
        );
        assert_eq!(game.players[0].counters.get("eoe:camps_left"), Some(&1));
        assert!(
            !ai.base.barbarian_presence_at_home_for_controller(
                &game,
                0,
                crate::ai::HOME_CAMP_RADIUS
            ),
            "a camp that is the rival's problem raises no alarm at home"
        );
        assert!(
            stock.base.barbarian_presence_at_home_for_controller(
                &game,
                0,
                crate::ai::HOME_CAMP_RADIUS
            ),
            "the stock alarm reads the same camp as home ground"
        );
        assert_eq!(ai.base.nearest_enemy(&game, 0, warrior, &[barb]), None);
        assert_eq!(
            stock.base.nearest_enemy(&game, 0, warrior, &[barb]),
            Some(camp)
        );
        assert_eq!(
            ai.base
                .barbarian_home_defense_objective(&game, 0, warrior, &[barb]),
            None
        );
        assert_eq!(
            stock
                .base
                .barbarian_home_defense_objective(&game, 0, warrior, &[barb]),
            Some(camp)
        );
    }

    /// A city-state seated beyond the rival is worth envoys — most when the
    /// rival holds it — and one seated on our side of the rival is worth
    /// nothing to this term. Rivals at war weigh double.
    #[test]
    fn a_city_state_across_the_rival_is_worth_envoys() {
        let (mut game, home, theirs, minors) = far_side_board(26_082_503);
        let beyond = open_ground_where(&game, theirs, 4, |pos| game.wdist(pos, home) > 12);
        let near_us = open_ground_where(&game, home, 4, |pos| game.wdist(pos, theirs) > 12);
        game.found_city_for(minors[0], beyond, None);
        game.found_city_for(minors[1], near_us, None);
        let ai = eoe_ai();
        let stock = AdvancedAi::new();
        assert_eq!(
            stock.enemy_of_my_enemy_city_state_bonus(&game, 0, minors[0], 1),
            0
        );
        let open = ai.enemy_of_my_enemy_city_state_bonus(&game, 0, minors[0], 1);
        assert_eq!(
            open,
            ENEMY_OF_MY_ENEMY_CITY_STATE_BASE
                + i64::from(ENEMY_OF_MY_ENEMY_CITY_STATE_RADIUS + 1 - 4)
                    * ENEMY_OF_MY_ENEMY_CITY_STATE_PER_TILE
                + ENEMY_OF_MY_ENEMY_OPEN_CITY_STATE
        );
        assert_eq!(
            ai.enemy_of_my_enemy_city_state_bonus(&game, 0, minors[1], 1),
            0
        );
        assert_eq!(
            ai.enemy_of_my_enemy_city_state_bonus(&game, 0, minors[0], 3),
            open / 3,
            "amortised over the envoys still needed"
        );
        // The rival as suzerain is the client worth unseating.
        game.current = 1;
        for _ in 0..3 {
            game.players[1].envoys_free += 1;
            game.apply(1, &crate::game::Action::SendEnvoy { player: minors[0] })
                .expect("the rival can send an envoy");
        }
        game.current = 0;
        assert_eq!(game.suzerain_of(minors[0]), Some(1));
        let held = ai.enemy_of_my_enemy_city_state_bonus(&game, 0, minors[0], 1);
        assert_eq!(
            held - open,
            ENEMY_OF_MY_ENEMY_RIVAL_CLIENT - ENEMY_OF_MY_ENEMY_OPEN_CITY_STATE
        );
        // An enemy at war weighs double.
        game.at_war.insert((0, 1));
        assert!(game.is_at_war(0, 1));
        assert_eq!(
            ai.enemy_of_my_enemy_city_state_bonus(&game, 0, minors[0], 1),
            held * ENEMY_OF_MY_ENEMY_ENEMY_WEIGHT
        );
        // A friend is no rival at all.
        let mut friends = game.clone();
        friends.at_war.remove(&(0, 1));
        assert!(!friends.is_at_war(0, 1));
        friends.players[0]
            .friends_until
            .insert(1, friends.turn + 30);
        friends.players[1]
            .friends_until
            .insert(0, friends.turn + 30);
        assert!(friends.are_friends(0, 1));
        assert_eq!(
            ai.enemy_of_my_enemy_city_state_bonus(&friends, 0, minors[0], 1),
            0
        );
    }

    /// A third major seated beyond the rival is the partner this term
    /// favours; one beside us is not; a rival's invitation to war on that
    /// far-side major is refused, its invitation against our own target is
    /// not.
    #[test]
    fn the_major_across_the_rival_is_the_partner_and_not_the_joint_war_target() {
        let mut game = Game::new_full(3, 30, 20, 26_082_504, 300, 0, false);
        game.map.clear_rivers();
        for tile in game.map.tiles.values_mut() {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
        }
        for uid in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(uid);
        }
        let mut land: Vec<Pos> = game.map.tiles.keys().copied().collect();
        land.sort();
        let (home, theirs) = land
            .iter()
            .flat_map(|home| land.iter().map(move |other| (*home, *other)))
            .find(|(home, other)| {
                game.wdist(*home, *other) == 10
                    && game.wdisk(*home, 4).len() >= 24
                    && game.wdisk(*other, 8).len() >= 60
            })
            .expect("two interior sites ten apart");
        game.found_city_for(0, home, None);
        game.found_city_for(1, theirs, None);
        let beyond = open_ground_where(&game, theirs, 4, |pos| game.wdist(pos, home) > 12);
        game.found_city_for(2, beyond, None);
        for a in 0..3 {
            for b in a + 1..3 {
                game.record_contact(a, b);
            }
        }
        game.turn = 60;
        game.current = 0;
        let ai = eoe_ai();
        let stock = AdvancedAi::new();
        assert_eq!(
            stock.enemy_of_my_enemy_partner_bonus(&game, 0, 2, None),
            0.0
        );
        assert_eq!(
            ai.enemy_of_my_enemy_partner_bonus(&game, 0, 2, None),
            ENEMY_OF_MY_ENEMY_PARTNER * ENEMY_OF_MY_ENEMY_NEIGHBOUR_WEIGHT as f64
        );
        assert_eq!(
            ai.enemy_of_my_enemy_partner_bonus(&game, 0, 1, None),
            0.0,
            "the rival itself sits on our side of nobody"
        );
        assert_eq!(
            ai.enemy_of_my_enemy_partner_bonus(&game, 0, 2, Some(1)),
            ENEMY_OF_MY_ENEMY_PARTNER * ENEMY_OF_MY_ENEMY_ENEMY_WEIGHT as f64
        );
        game.at_war.insert((1, 2));
        assert!(game.is_at_war(1, 2));
        assert_eq!(
            ai.enemy_of_my_enemy_partner_bonus(&game, 0, 2, None),
            ENEMY_OF_MY_ENEMY_PARTNER * ENEMY_OF_MY_ENEMY_NEIGHBOUR_WEIGHT as f64
                + ENEMY_OF_MY_ENEMY_PARTNER_AT_WAR
        );

        let invitation = DiplomaticDeal {
            id: 1,
            from: 1,
            to: 0,
            give_gold: 0.0,
            request_gold: 0.0,
            open_borders: false,
            friendship: false,
            peace: false,
            alliance: None,
            defensive_pact: false,
            joint_war_target: Some(2),
            promise: None,
            demand: false,
            expires: game.turn + 1,
        };
        assert_eq!(
            ai.enemy_of_my_enemy_joint_war_penalty(&game, 0, &invitation, None),
            ENEMY_OF_MY_ENEMY_JOINT_WAR_REFUSAL
        );
        assert_eq!(
            stock.enemy_of_my_enemy_joint_war_penalty(&game, 0, &invitation, None),
            0.0
        );
        assert_eq!(
            ai.enemy_of_my_enemy_joint_war_penalty(&game, 0, &invitation, Some(2)),
            0.0,
            "our own target is fair game whoever proposes it"
        );
        let plan = StrategicPlan {
            strategy: GrandStrategy::Expansion,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        };
        assert!(
            ai.incoming_deal_value(&game, 0, &invitation, &plan)
                < stock.incoming_deal_value(&game, 0, &invitation, &plan) - 500.0
        );
    }
}
