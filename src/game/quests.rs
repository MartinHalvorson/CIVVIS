//! City-state Envoy quests.
//!
//! Every city-state a civilization has met asks it for one thing at a time,
//! and pays an Envoy when it is done. The eight quests are the shipped
//! `Quests.xml` rows; the reward is `LOC_QUEST_REWARD_ENVOYS`, which is one
//! Envoy.
//!
//! Two design points are worth stating because they are easy to get wrong:
//!
//! * A quest is per *pair*, not per city-state. Two civilizations that have
//!   both met Geneva are each asked for something, and each is paid
//!   separately; one finishing does not cancel the other's.
//! * The roll is a hash of the pair and the era, not a draw from the
//!   simulation RNG. Asking a city-state what it wants is a query the UI makes
//!   freely, and a query that consumes RNG would make merely opening a panel
//!   change the game.

use crate::name::Name;
use super::*;

/// One outstanding request, as stored on the civilization that owes it.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct CityStateQuest {
    /// Which of the eight shipped quests this is.
    pub kind: String,
    /// The thing named by the quest: a unit, a district family, a technology,
    /// a civic, or a Great Person class. Empty for the three quests that name
    /// nothing (religious conversion, trade route, barbarian outpost).
    #[serde(default)]
    pub target: String,
    /// World era the quest was issued in. A new era retires it.
    #[serde(default)]
    pub era: usize,
    /// The outpost a `clear_barbarian_camp` quest names.
    #[serde(default)]
    pub pos: Option<Pos>,
    /// The value of the counter the quest watches, as it stood when the quest
    /// was issued. Clearing a camp *before* being asked does not pay.
    #[serde(default)]
    pub mark: i64,
}

/// The shipped `Quests.xml` rows, in table order.
pub const QUEST_KINDS: [&str; 8] = [
    "convert_capital_to_religion",
    "send_trade_route",
    "clear_barbarian_camp",
    "train_unit_type",
    "zone_district_type",
    "trigger_tech_boost",
    "trigger_civic_boost",
    "recruit_great_person_class",
];

/// Civilization VI looks for an outpost within five tiles of the city-state.
const CAMP_SEARCH_RADIUS: i32 = 5;

impl Game {
    /// Has `pid` met `minor`? The same test the `met_city_states` Inspiration
    /// uses: one of the city-state's cities has been seen.
    pub(crate) fn has_met_city_state(&self, pid: usize, minor: usize) -> bool {
        self.players
            .get(minor)
            .is_some_and(|state| state.alive && state.is_minor && !state.is_barbarian)
            && self
                .cities
                .values()
                .any(|city| city.owner == minor && self.players[pid].explored.contains(&city.pos))
    }

    /// The quest `minor` is currently asking `pid` for, if any.
    pub fn city_state_quest(&self, pid: usize, minor: usize) -> Option<&CityStateQuest> {
        self.players.get(pid)?.quests.get(&minor)
    }

    /// Stable per-pair, per-era, per-attempt ordering key. Rolling a quest must
    /// not touch the simulation RNG: the client asks what a city-state wants
    /// every time it draws the panel.
    fn quest_key(pid: usize, minor: usize, era: usize, attempt: u64, name: &str) -> u64 {
        let mut hash = 0xcbf29ce484222325_u64
            ^ (pid as u64)
            ^ ((minor as u64) << 16)
            ^ ((era as u64) << 32)
            ^ (attempt << 48);
        for byte in name.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    /// Military units `pid` could train today, by the gates the production
    /// legality check reads. A quest that names a unit the civilization cannot
    /// reach is not a quest, it is a dead entry.
    fn quest_unit_candidates(&self, pid: usize) -> Vec<Name> {
        let civ = self.players[pid].civ.as_str();
        self.rules
            .units
            .iter()
            .filter(|(_, spec)| spec.buildable && spec.class == "military")
            .filter(|(_, spec)| match spec.unique_to.as_deref() {
                Some(owner) => owner == civ,
                None => true,
            })
            .filter(|(_, spec)| {
                spec.tech
                    .as_deref()
                    .is_none_or(|tech| self.players[pid].techs.contains(&Name::new(tech)))
                    && spec
                        .civic
                        .as_deref()
                        .is_none_or(|civic| self.players[pid].civics.contains(&Name::new(civic)))
            })
            // A unit whose successor is already available is not what the
            // city-state would ask for, and the queue would modernize it away.
            .filter(|(unit, _)| !self.unit_is_obsolete(pid, unit))
            .map(|(unit, _)| unit.clone())
            .collect()
    }

    /// Specialty district families `pid` has unlocked but not yet built.
    fn quest_district_candidates(&self, pid: usize) -> Vec<Name> {
        let built: BTreeSet<Name> = self
            .cities
            .values()
            .filter(|city| city.owner == pid)
            .flat_map(|city| city.districts.keys())
            .map(|district| self.district_family(*district))
            .collect();
        self.rules
            .districts
            .iter()
            .filter(|(name, spec)| spec.specialty && spec.buildable && self.district_family(**name) == name.as_str())
            .filter(|(_, spec)| match spec.unique_to.as_deref() {
                Some(owner) => owner == self.players[pid].civ,
                None => true,
            })
            .filter(|(_, spec)| {
                spec.tech
                    .as_deref()
                    .is_none_or(|tech| self.players[pid].techs.contains(&Name::new(tech)))
                    && spec
                        .civic
                        .as_deref()
                        .is_none_or(|civic| self.players[pid].civics.contains(&Name::new(civic)))
            })
            .filter(|(name, _)| !built.contains(*name))
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Technologies with an unearned Eureka. A quest can only ask for a boost
    /// that is still available to trigger.
    fn quest_tech_candidates(&self, pid: usize) -> Vec<Name> {
        let player = &self.players[pid];
        self.rules
            .techs
            .iter()
            .filter(|(_, spec)| spec.boost.is_some())
            .filter(|(name, _)| !player.techs.contains(*name) && !player.boosted_techs.contains(*name))
            .map(|(name, _)| name.clone())
            .collect()
    }

    fn quest_civic_candidates(&self, pid: usize) -> Vec<Name> {
        let player = &self.players[pid];
        self.rules
            .civics
            .iter()
            .filter(|(_, spec)| spec.boost.is_some())
            .filter(|(name, _)| !player.civics.contains(*name) && !player.boosted_civics.contains(*name))
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Great Person classes the game still has someone left to recruit in.
    fn quest_great_person_candidates(&self, pid: usize) -> Vec<String> {
        let mut kinds: Vec<String> = self
            .rules
            .great_people
            .values()
            .map(|spec| spec.kind.clone())
            .collect();
        kinds.sort_unstable();
        kinds.dedup();
        kinds.retain(|kind| self.current_great_person(kind).is_some());
        // A civilization that already has a religion can never claim another
        // Prophet, so asking it for one is a quest it cannot finish.
        kinds.retain(|kind| kind != "prophet" || self.players[pid].religion.is_none());
        kinds
    }

    /// The barbarian outposts within five tiles of the city-state.
    fn camps_near_city_state(&self, minor: usize) -> Vec<Pos> {
        let mut found = Vec::new();
        for city in self.player_city_ids(minor) {
            let centre = self.cities[&city].pos;
            for pos in self.wdisk(centre, CAMP_SEARCH_RADIUS) {
                if self
                    .map
                    .get(pos)
                    .is_some_and(|tile| tile.improvement.as_deref() == Some("barbarian_camp"))
                {
                    found.push(pos);
                }
            }
        }
        found.sort_unstable();
        found.dedup();
        found
    }

    /// Which of the eight quests `minor` could ask `pid` for right now, with
    /// the thing each one would name. A quest already satisfied is excluded:
    /// the game does not hand out an Envoy for something already done.
    fn available_quests(&self, pid: usize, minor: usize) -> Vec<CityStateQuest> {
        let era = self.world_era;
        let camps = self.players[pid].counters.get("camps").copied().unwrap_or(0);
        let new = |kind: &str, target: String, pos: Option<Pos>, mark: i64| CityStateQuest {
            kind: kind.to_string(),
            target,
            era,
            pos,
            mark,
        };
        let pick = |candidates: Vec<String>| -> Option<String> {
            candidates
                .into_iter()
                .min_by_key(|name| Self::quest_key(pid, minor, era, 0, name))
        };
        // The interned half of the same draw. Quest targets are stored as text,
        // so a chosen name is spelled out once here rather than at each caller.
        let pick_name = |candidates: Vec<Name>| -> Option<String> {
            candidates
                .into_iter()
                .min_by_key(|name| Self::quest_key(pid, minor, era, 0, name))
                .map(|name| name.to_string())
        };
        let mut out = Vec::new();
        // A quest already satisfied is not offered: the game does not pay an
        // Envoy for something that was done before it was asked for.
        let mut offer = |quest: CityStateQuest| out.push(quest);

        // Religious conversion needs a religion to convert to.
        if self.players[pid].religion.is_some() {
            let quest = new("convert_capital_to_religion", String::new(), None, 0);
            if !self.quest_done(pid, minor, &quest) {
                offer(quest);
            }
        }
        if self.trade_capacity(pid) > 0 {
            let quest = new("send_trade_route", String::new(), None, 0);
            if !self.quest_done(pid, minor, &quest) {
                offer(quest);
            }
        }
        // The outpost is named when the quest is issued, so clearing a
        // different camp elsewhere does not finish this one.
        if let Some(camp) = self
            .camps_near_city_state(minor)
            .into_iter()
            .min_by_key(|pos| Self::quest_key(pid, minor, era, 0, &format!("{pos:?}")))
        {
            offer(new(
                "clear_barbarian_camp",
                String::new(),
                Some(camp),
                camps,
            ));
        }
        if let Some(unit) = pick_name(self.quest_unit_candidates(pid)) {
            let quest = new("train_unit_type", unit, None, 0);
            if !self.quest_done(pid, minor, &quest) {
                offer(quest);
            }
        }
        if let Some(district) = pick_name(self.quest_district_candidates(pid)) {
            offer(new("zone_district_type", district, None, 0));
        }
        if let Some(tech) = pick_name(self.quest_tech_candidates(pid)) {
            offer(new("trigger_tech_boost", tech, None, 0));
        }
        if let Some(civic) = pick_name(self.quest_civic_candidates(pid)) {
            offer(new("trigger_civic_boost", civic, None, 0));
        }
        if let Some(kind) = pick(self.quest_great_person_candidates(pid)) {
            let mark = self.players[pid]
                .gp_claimed
                .get(&kind)
                .copied()
                .unwrap_or(0);
            offer(new("recruit_great_person_class", kind, None, mark));
        }
        out
    }

    /// Has `pid` done what `quest` asks?
    fn quest_done(&self, pid: usize, minor: usize, quest: &CityStateQuest) -> bool {
        let player = &self.players[pid];
        match quest.kind.as_str() {
            "convert_capital_to_religion" => player.religion.as_deref().is_some_and(|religion| {
                self.player_city_ids(minor)
                    .iter()
                    .any(|city| self.city_religion(&self.cities[city]) == Some(religion))
            }),
            "send_trade_route" => {
                let theirs: BTreeSet<u32> = self.player_city_ids(minor).into_iter().collect();
                self.routes.iter().any(|route| {
                    route.owner == pid && route.ends > self.turn && theirs.contains(&route.dest)
                })
            }
            // The named outpost is gone, and this civilization is the one that
            // cleared a camp since being asked. Somebody else destroying it
            // does not earn this civilization an Envoy.
            "clear_barbarian_camp" => {
                let gone = quest.pos.is_none_or(|pos| {
                    self.map
                        .get(pos)
                        .is_none_or(|tile| tile.improvement.as_deref() != Some("barbarian_camp"))
                });
                gone && player.counters.get("camps").copied().unwrap_or(0) > quest.mark
            }
            "train_unit_type" => self
                .units
                .values()
                .any(|unit| unit.owner == pid && unit.kind == quest.target),
            "zone_district_type" => self
                .cities
                .values()
                .filter(|city| city.owner == pid)
                .flat_map(|city| city.districts.keys())
                .any(|district| self.district_family(*district) == quest.target),
            "trigger_tech_boost" => player.boosted_techs.contains(&Name::new(&quest.target)),
            "trigger_civic_boost" => player.boosted_civics.contains(&Name::new(&quest.target)),
            "recruit_great_person_class" => {
                player.gp_claimed.get(&quest.target).copied().unwrap_or(0) > quest.mark
            }
            _ => false,
        }
    }

    /// Issue the quest `minor` asks `pid` for, replacing any current one.
    fn roll_quest(&mut self, pid: usize, minor: usize) {
        let era = self.world_era;
        let available = self.available_quests(pid, minor);
        let chosen = available
            .into_iter()
            .min_by_key(|quest| Self::quest_key(pid, minor, era, 1, &quest.kind));
        match chosen {
            Some(quest) => {
                self.players[pid].quests.insert(minor, quest);
            }
            // Nothing this city-state can usefully ask for right now. It asks
            // again next turn rather than holding a quest nobody can finish.
            None => {
                self.players[pid].quests.remove(&minor);
            }
        }
    }

    /// Pay out finished quests, retire quests a new era has aged out, and hand
    /// a quest to every met city-state that is not currently asking for one.
    pub(crate) fn check_city_state_quests(&mut self, pid: usize) {
        if self.players[pid].is_minor || self.players[pid].is_barbarian || !self.players[pid].alive {
            return;
        }
        let minors: Vec<usize> = self
            .players
            .iter()
            .filter(|state| state.alive && state.is_minor && !state.is_barbarian)
            .map(|state| state.id)
            .collect();
        for minor in minors {
            if !self.has_met_city_state(pid, minor) {
                // A city-state that has been destroyed, or that this
                // civilization has not met, holds no request.
                self.players[pid].quests.remove(&minor);
                continue;
            }
            let current = self.players[pid].quests.get(&minor).cloned();
            let Some(quest) = current else {
                self.roll_quest(pid, minor);
                continue;
            };
            if self.quest_done(pid, minor, &quest) {
                self.players[pid].envoys_free += 1;
                self.players[pid].quests.remove(&minor);
                let state = self.players[minor].civ.clone();
                self.note_important(
                    pid,
                    "CityState",
                    format!(
                        "completed {state}'s {} and earned an Envoy",
                        Self::quest_name(&quest.kind)
                    ),
                    self.player_city_ids(minor)
                        .first()
                        .map(|city| self.cities[city].pos),
                );
                self.roll_quest(pid, minor);
            } else if quest.era != self.world_era {
                // A new era retires the old request and asks for something
                // era-appropriate instead.
                self.roll_quest(pid, minor);
            }
        }
    }

    /// The shipped display name of a quest.
    pub fn quest_name(kind: &str) -> &'static str {
        match kind {
            "convert_capital_to_religion" => "Religious Conversion",
            "send_trade_route" => "Send Trade Route",
            "clear_barbarian_camp" => "Destroy Barbarian Outpost within 5 tiles",
            "train_unit_type" => "Train Unit",
            "zone_district_type" => "Construct District",
            "trigger_tech_boost" => "Trigger Eureka",
            "trigger_civic_boost" => "Trigger Inspiration",
            "recruit_great_person_class" => "Recruit Great Person",
            _ => "Quest",
        }
    }

    /// The shipped description, with the named target filled in.
    pub fn quest_description(&self, quest: &CityStateQuest) -> String {
        match quest.kind.as_str() {
            "convert_capital_to_religion" => "Convert the city-state to your religion.".to_string(),
            "send_trade_route" => "Send a Trade Route to the city-state.".to_string(),
            "clear_barbarian_camp" => {
                "Destroy one Barbarian Outpost within 5 tiles of the city.".to_string()
            }
            "train_unit_type" => format!("Train a {} military unit.", pretty(&quest.target)),
            "zone_district_type" => format!("Construct a {} district.", pretty(&quest.target)),
            "trigger_tech_boost" => format!(
                "Trigger the Eureka moment for the {} technology.",
                pretty(&quest.target)
            ),
            "trigger_civic_boost" => format!(
                "Trigger the Inspiration moment for the {} civic.",
                pretty(&quest.target)
            ),
            "recruit_great_person_class" => {
                format!("Recruit one Great {}.", pretty(&quest.target))
            }
            _ => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A game with two civilizations that have founded their capitals, plus a
    /// city-state seated with a city of its own.
    fn game_with_city_state(seed: u64) -> (Game, usize, u32) {
        let mut game = Game::new_full(2, 28, 18, seed, 300, 0, false);
        for pid in 0..2 {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            let pos = game.units[&settler].pos;
            game.found_city_for(pid, pos, None);
            game.remove_unit(settler);
        }
        let minor = game.players.len();
        game.players.push(Player::new(minor, "Hattusa", true));
        // Seat the city-state well away from either capital.
        let taken: BTreeSet<Pos> = game.cities.values().map(|city| city.pos).collect();
        let site = game
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| !taken.contains(pos))
            .filter(|pos| {
                game.map
                    .get(*pos)
                    .is_some_and(|tile| !game.rules.is_water(tile))
            })
            .filter(|pos| taken.iter().all(|other| game.wdist(*pos, *other) > 6))
            .min()
            .expect("the map has room for a city-state");
        let city = game.found_city_for(minor, site, Some("Hattusa".into()));
        (game, minor, city)
    }

    fn meet(game: &mut Game, pid: usize, city: u32) {
        let pos = game.cities[&city].pos;
        game.players[pid].explored.insert(pos);
    }

    #[test]
    fn only_a_city_state_a_civilization_has_met_asks_it_for_anything() {
        let (mut game, minor, city) = game_with_city_state(31_001);
        game.check_city_state_quests(0);
        assert!(
            game.city_state_quest(0, minor).is_none(),
            "an unmet city-state asked for something"
        );
        meet(&mut game, 0, city);
        game.check_city_state_quests(0);
        assert!(
            game.city_state_quest(0, minor).is_some(),
            "a met city-state asked for nothing"
        );
        // The other civilization has not met it, so it is owed nothing.
        assert!(game.city_state_quest(1, minor).is_none());
    }

    #[test]
    fn a_quest_is_per_pair_and_pays_one_envoy_to_the_civilization_that_finishes_it() {
        let (mut game, minor, city) = game_with_city_state(31_002);
        meet(&mut game, 0, city);
        meet(&mut game, 1, city);
        game.check_city_state_quests(0);
        game.check_city_state_quests(1);
        assert!(game.city_state_quest(0, minor).is_some());
        assert!(game.city_state_quest(1, minor).is_some());

        // Force a quest with a completion the test can drive directly.
        let quest = CityStateQuest {
            kind: "trigger_tech_boost".into(),
            target: "writing".into(),
            era: game.world_era,
            pos: None,
            mark: 0,
        };
        game.players[0].quests.insert(minor, quest.clone());
        game.players[1].quests.insert(minor, quest);
        let before = game.players[0].envoys_free;
        let rival_before = game.players[1].envoys_free;

        game.players[0].boosted_techs.insert(crate::name!("writing"));
        game.check_city_state_quests(0);
        assert_eq!(
            game.players[0].envoys_free,
            before + 1,
            "finishing a quest paid something other than one Envoy"
        );
        // The rival's identical quest is untouched.
        game.check_city_state_quests(1);
        assert_eq!(game.players[1].envoys_free, rival_before);
        assert_eq!(
            game.city_state_quest(1, minor).map(|q| q.kind.as_str()),
            Some("trigger_tech_boost")
        );
        // And a new quest was issued rather than the old one paying twice.
        game.check_city_state_quests(0);
        assert_eq!(game.players[0].envoys_free, before + 1);
    }

    #[test]
    fn a_quest_is_never_issued_already_finished() {
        let (mut game, minor, city) = game_with_city_state(31_003);
        meet(&mut game, 0, city);
        // Every Eureka already earned, so no tech-boost quest can be offered.
        let techs: Vec<Name> = game
            .rules
            .techs
            .iter()
            .filter(|(_, spec)| spec.boost.is_some())
            .map(|(name, _)| name.clone())
            .collect();
        for tech in techs {
            game.players[0].boosted_techs.insert(tech);
        }
        for _ in 0..8 {
            game.check_city_state_quests(0);
            let Some(quest) = game.city_state_quest(0, minor).cloned() else {
                continue;
            };
            assert_ne!(
                quest.kind, "trigger_tech_boost",
                "asked for a Eureka the civilization had already triggered"
            );
            // Whatever was asked for, it must not already be satisfied --
            // that would pay an Envoy for nothing.
            assert!(
                !game.quest_done(0, minor, &quest),
                "issued an already-finished quest: {quest:?}"
            );
            game.players[0].quests.remove(&minor);
        }
    }

    #[test]
    fn a_new_era_retires_the_outstanding_quest() {
        let (mut game, minor, city) = game_with_city_state(31_004);
        meet(&mut game, 0, city);
        game.check_city_state_quests(0);
        let first = game.city_state_quest(0, minor).cloned().unwrap();
        assert_eq!(first.era, game.world_era);
        game.world_era += 1;
        game.check_city_state_quests(0);
        let second = game.city_state_quest(0, minor).cloned().unwrap();
        assert_eq!(second.era, game.world_era, "the quest kept the old era");
    }

    #[test]
    fn clearing_some_other_camp_does_not_finish_the_outpost_quest() {
        let (mut game, minor, city) = game_with_city_state(31_005);
        meet(&mut game, 0, city);
        let centre = game.cities[&city].pos;
        let camp = game
            .wdisk(centre, 3)
            .into_iter()
            .find(|pos| {
                *pos != centre
                    && game
                        .map
                        .get(*pos)
                        .is_some_and(|tile| !game.rules.is_water(tile))
            })
            .unwrap();
        game.map.tiles.get_mut(&camp).unwrap().improvement = Some(crate::name!("barbarian_camp"));
        let quest = CityStateQuest {
            kind: "clear_barbarian_camp".into(),
            target: String::new(),
            era: game.world_era,
            pos: Some(camp),
            mark: 0,
        };
        game.players[0].quests.insert(minor, quest.clone());

        // A camp cleared somewhere else moves the counter but leaves the named
        // outpost standing.
        *game.players[0]
            .counters
            .entry("camps".to_string())
            .or_insert(0) += 1;
        assert!(!game.quest_done(0, minor, &quest));

        // Razing the named outpost without having cleared one does not pay
        // either -- somebody else destroying it is not this civilization's work.
        game.map.tiles.get_mut(&camp).unwrap().improvement = None;
        let untouched = CityStateQuest { mark: 1, ..quest.clone() };
        assert!(!game.quest_done(0, minor, &untouched));

        // Both together is the completion.
        assert!(game.quest_done(0, minor, &quest));
    }

    #[test]
    fn rolling_a_quest_never_consumes_the_simulation_rng() {
        let (mut game, minor, city) = game_with_city_state(31_006);
        meet(&mut game, 0, city);
        let before = game.rng.clone();
        game.check_city_state_quests(0);
        let first = game.city_state_quest(0, minor).cloned().unwrap();
        assert_eq!(
            game.rng,
            before,
            "asking a city-state what it wants moved the simulation RNG"
        );
        // And the same state rolls the same quest.
        game.players[0].quests.remove(&minor);
        game.check_city_state_quests(0);
        assert_eq!(game.city_state_quest(0, minor).cloned().unwrap(), first);
    }

    #[test]
    fn every_shipped_quest_kind_can_be_recognised_and_described() {
        let (game, _, _) = game_with_city_state(31_007);
        for kind in QUEST_KINDS {
            let quest = CityStateQuest {
                kind: kind.to_string(),
                target: "writing".into(),
                era: 0,
                pos: None,
                mark: 0,
            };
            assert_ne!(Game::quest_name(kind), "Quest", "{kind} has no name");
            assert!(
                !game.quest_description(&quest).is_empty(),
                "{kind} has no description"
            );
        }
    }

    /// A rollout, not a fixture: city-states must actually reach civilizations
    /// and ask them for things over a real game. A quest system that only
    /// works when a test hands it a met city-state is a quest system nobody
    /// ever sees.
    #[test]
    fn city_states_issue_and_pay_quests_over_a_real_rollout() {
        use crate::ai::{AdvancedAi, Ai};
        let mut game = Game::new_full(4, 44, 28, 4_242, 120, 6, false);
        let mut ais = AdvancedAi::fleet(&game);
        let mut outstanding = 0usize;
        let mut completed = 0usize;
        // A quest leaving a civilization's book while its target is satisfied
        // is a completion; a quest replaced by a new era is not.
        let mut held: Vec<BTreeMap<usize, CityStateQuest>> =
            vec![BTreeMap::new(); game.players.len()];
        while game.winner.is_none() && game.turn <= 90 {
            let pid = game.current;
            ais[pid].take_turn(&mut game, pid);
            if game.winner.is_none() && game.current == pid {
                let _ = game.apply(pid, &Action::EndTurn);
            }
            for player in 0..4 {
                let now = game.players[player].quests.clone();
                for (minor, was) in &held[player] {
                    let replaced = now.get(minor) != Some(was);
                    if replaced && was.era == game.world_era {
                        completed += 1;
                    }
                }
                outstanding = outstanding.max(now.len());
                held[player] = now;
            }
        }
        assert!(
            outstanding > 0,
            "no city-state asked anybody for anything in ninety turns"
        );
        assert!(
            completed > 0,
            "ninety turns of four empires and six city-states finished no quest"
        );
    }
}
